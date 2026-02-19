//! Debug utilities, including a system (F3 deafult) to dump diagnostics,
//! entity counts, asset counts, and thread usage to a timestamped text file in './debug-dumps/'.
//!
//! This is a useful module for quickly capturing a snapshot of the game's internal state
//! and performance characteristics without needing to set up an external profiler or attach a debugger.
use bevy::diagnostic::{Diagnostic, DiagnosticsStore, FrameTimeDiagnosticsPlugin};
use bevy::pbr::StandardMaterial;
use bevy::prelude::*;
use bevy::render::mesh::Mesh;
use bevy::render::texture::Image;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::fmt::Write;
use std::time::{SystemTime, UNIX_EPOCH};
use chrono::{DateTime, Utc};
use std::sync::{Mutex, OnceLock};
use sysinfo::{SystemExt, ProcessExt, PidExt, Pid, System};


#[derive(Resource, Default)]
pub struct SystemThreadLog {
    map: HashMap<String, HashSet<String>>,
    last_updated: Option<SystemTime>,
}

/// Registry mapping asset handle debug strings to their source path strings.
#[derive(Resource, Default)]
pub struct AssetPathRegistry(pub HashMap<String, String>);

// Global, thread-safe collector for instrumenting background worker threads
static GLOBAL_THREAD_MAP: OnceLock<Mutex<HashMap<String, HashSet<String>>>> = OnceLock::new();

/// Record the current thread id for `system` from any thread (worker or main).
/// Safe to call from async/rayon worker tasks.
pub fn record_thread_global(system: &str) {
    let tid = format!("{:?}", std::thread::current().id());
    let map = GLOBAL_THREAD_MAP.get_or_init(|| Mutex::new(HashMap::new()));
    let mut guard = map.lock().expect("GLOBAL_THREAD_MAP lock");
    guard.entry(system.to_string()).or_default().insert(tid);
}

/// Return a snapshot of the global thread map (system -> sorted list of thread ids).
pub fn snapshot_global_thread_map() -> HashMap<String, Vec<String>> {
    let map = GLOBAL_THREAD_MAP.get_or_init(|| Mutex::new(HashMap::new()));
    let guard = map.lock().expect("GLOBAL_THREAD_MAP lock");
    guard.iter().map(|(k, s)| {
        let mut v: Vec<_> = s.iter().cloned().collect();
        v.sort();
        (k.clone(), v)
    }).collect()
}

impl SystemThreadLog {
    /// Record that a system is runninng on a current thread.
    /// This is intended to be called from an instrumented system that wants to log which thread it's running on.
    ///
    /// # Arguments
    /// * `system` - The name of the system being recorded (e.g., "chunk_mesh_system").
    ///
    /// # Example
    /// ```
    /// fn some_system(mut sys_log: Option<ResMut<SystemThreadLog>>) {
    ///     if let Some(mut log) = sys_log {
    ///         log.record("some_system");
    ///     }
    /// }
    /// ```
    pub fn record(&mut self, system: &str) {
        let tid = format!("{:?}", std::thread::current().id());
        self.map.entry(system.to_string()).or_default().insert(tid);
        self.last_updated = Some(SystemTime::now());
    }

    /// Generate a human-readable snapshot of the current system-to-thread mapping
    /// This can include a debug dump of which systems are running on which threads at the time of the snapshot.
    ///
    /// # Returns
    /// A formatted string showing each system and the threads it's running on.
    fn snapshot_text(&self) -> String {
        if self.map.is_empty() {
            return "no instrumented systems recorded\n".into();
        }
        let mut out = String::new();
        for (sys, ids) in &self.map {
            let mut ids: Vec<_> = ids.iter().cloned().collect();
            ids.sort();
            out.push_str(&format!("  {} -> threads: {}\n", sys, ids.join(", ")));
        }
        out
    }
}

pub struct DebugDumpPlugin;

impl Plugin for DebugDumpPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(SystemThreadLog::default()).add_systems(Update, debug_input_system);
    }
}

/// A internal helper function to convert kilobytes megabytes.
///
/// # Arguments
/// * `kb` - The size in kilobytes to convert to a human-readable megabyte string.
///
/// # Returns
/// A string representing the size in megabytes, formatted to two decimal places (e.g., "123.45 MB").
fn kb_to_mb(kb: u64) -> String {
    format!("{:.2} MB", (kb as f64) / 1024.0)
}

fn bytes_to_mb(bytes: usize) -> String {
    format!("{:.2} MB", (bytes as f64) / 1024.0 / 1024.0)
}

/// A Bevy system that listens for the (debug, default F3) key press
/// and generates a debug dump of diagnostics, entity counts, asset counts, and system thread usage.
///
/// # Arguments
/// * `keys` - Bevy resource for keyboard input, used to detect when the debug key is pressed.
/// * `diagnostics` - Bevy resource that stores performance diagnostics like FPS and frame time.
/// * `query_entities` - A Bevy query that counts the total number of entities in the world.
/// * `meshes`, `materials`, `images` - Bevy asset resources that count the number of loaded meshes, materials, and images.
/// * `sys_log` - An optional resource that tracks which systems are running on which threads, for inclusion in the debug dump.
fn debug_input_system(
    keys: Res<ButtonInput<KeyCode>>,
    diagnostics: Res<DiagnosticsStore>,
    query_entities: Query<Entity>,
    meshes: Res<Assets<Mesh>>,
    materials: Res<Assets<StandardMaterial>>,
    images: Res<Assets<Image>>,
    sys_log: Option<Res<SystemThreadLog>>,
    asset_paths: Option<Res<AssetPathRegistry>>,
) {
    if !keys.just_pressed(KeyCode::F3) {
        return;
    }

    // timestamp & filename
    let now = SystemTime::now();
    let ts_secs = now.duration_since(UNIX_EPOCH).unwrap().as_secs();
    let dt: DateTime<Utc> = DateTime::from(now);
    let human_ts = dt.format("%Y-%m-%d %H:%M:%S").to_string();
    let dir = "debug-dumps";
    let fname = format!("{}/debug-{}.txt", dir, ts_secs);

    // Bevy diagnostics (fps / frame_time)
    let fps = diagnostics
        .get(&FrameTimeDiagnosticsPlugin::FPS)
        .and_then(Diagnostic::smoothed)
        .unwrap_or(0.0);
    let frame_time = diagnostics
        .get(&FrameTimeDiagnosticsPlugin::FRAME_TIME)
        .and_then(Diagnostic::smoothed)
        .unwrap_or(0.0);

    // entity & asset counts
    let entity_count = query_entities.iter().count();
    let mesh_count = meshes.len();
    let material_count = materials.len();
    let image_count = images.len();

    // compute image memory stats (bytes)
    let mut total_image_bytes: usize = 0;
    let mut image_list: Vec<(String, usize)> = Vec::new();
    for (handle, image) in images.iter() {
        // image.data length is bytes stored for the texture
        let size = image.data.len();
        total_image_bytes += size;
        // Lookup human-readable path if registered, otherwise fall back to handle debug
        let key = format!("{:?}", handle);
        let name = asset_paths
            .as_ref()
            .and_then(|ap| ap.0.get(&key))
            .cloned()
            .unwrap_or(key);
        image_list.push((name, size));
    }
    // sort descending and keep top 10
    image_list.sort_by_key(|(_, s)| std::cmp::Reverse(*s));
    let top_images = image_list.iter().take(10).cloned().collect::<Vec<_>>();

    // CPU / cores
    let cores = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1);

    // process / system memory (sysinfo)
    let mut sys = System::new_all();
    sys.refresh_all();
    let pid = std::process::id();
    let proc = sys.process(Pid::from(pid as usize));
    let proc_mem_kb = proc.map(|p| p.memory()).unwrap_or(0);
    let proc_virt_kb = proc.map(|p| p.virtual_memory()).unwrap_or(0);
    let total_mem_kb = sys.total_memory();
    let used_mem_kb = sys.used_memory();

    // build text
    let mut out = String::new();
    writeln!(out, "Debug dump: {}", ts_secs).ok();
    writeln!(out, "Timestamp: {} (epoch secs: {})", human_ts, ts_secs).ok();
    writeln!(out, "FPS: {:.1}, frame_time: {:.4} ms", fps, frame_time * 1000.0).ok();
    writeln!(out, "Entities: {}", entity_count).ok();
    writeln!(out,
        "Assets: meshes={} materials={} images={} (image mem total={})",
        mesh_count, material_count, image_count, bytes_to_mb(total_image_bytes))
    .ok();
    writeln!(out, "CPU cores (available): {}", cores).ok();
    writeln!(out,
        "Process memory: {} (virtual {})",
        kb_to_mb(proc_mem_kb),
        kb_to_mb(proc_virt_kb)
    )
    .ok();
    writeln!(out,
        "System memory: total={} used={}",
        kb_to_mb(total_mem_kb),
        kb_to_mb(used_mem_kb)
    )
    .ok();

    if !top_images.is_empty() {
        writeln!(out, "Top images by memory:").ok();
        for (name, sz) in top_images {
            writeln!(out, "  {} -> {}", name, bytes_to_mb(sz)).ok();
        }
    }

    // Prepare a borrowed reference so we can inspect the optional sys_log
    // multiple times without moving it.
    let sys_log_ref = sys_log.as_ref();

    writeln!(out, "\nInstrumented system thread map (resource-backed):").ok();
    if let Some(log) = sys_log_ref {
        out.push_str(&log.snapshot_text());
    } else {
        out.push_str("  (no system thread log resource present)\n");
    }

    // Include global worker-thread entries recorded via `record_thread_global`.
    let global_map = snapshot_global_thread_map();
    writeln!(out, "\nGlobal thread map (background tasks / workers):").ok();
    if global_map.is_empty() {
        writeln!(out, "  (no global worker-thread entries recorded)").ok();
    } else {
        for (sys, threads) in global_map {
            writeln!(out, "  {} -> threads: {}", sys, threads.join(", ")).ok();
        }
    }

    // ensure directory & write
    if let Err(e) = fs::create_dir_all(dir) {
        error!("debug dump: failed to create dir '{}': {}", dir, e);
        return;
    }
    if let Err(e) = fs::write(&fname, out) {
        error!("debug dump: failed to write {}: {}", fname, e);
    } else {
        info!("wrote debug dump: {}", fname);
    }
}
