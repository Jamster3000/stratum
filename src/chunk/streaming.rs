//! Chunk streaming, generation, and LOD mesh scheduling.
//!
//! This module is responsible for streaming chunks around the player, queuing
//! background generation and mesh-building tasks, and managing LOD scheduling
//! and entity spawn/despawn. It uses the async compute pool for off-main-thread
//! chunk generation and mesh builds and applies completed meshes on the
//! main thread.
use super::{Chunk, ChunkEntity, CHUNK_SIZE, MAX_LODS};
use crate::atlas_builder::AtlasUVMap;
use crate::voxel_material::VoxelMaterial;
use crate::world::World;
use bevy::pbr::{ExtendedMaterial, StandardMaterial};
use bevy::prelude::*;
use bevy::tasks::{AsyncComputeTaskPool, Task};
use std::collections::HashMap;

// CHUNK_SIZE as a signed `i32` for arithmetic convenience in this module.
#[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
const CHUNK_SIZE_I32: i32 = CHUNK_SIZE as i32;

// Make complex map value easier to read via a type alias
type ChunkEntry = (Entity, Vec<Option<Handle<Mesh>>>, u8);

use crate::chunk::lod::{
    LOD_BUILD_BUDGET_PER_FRAME, PREWARM_DISTANCE_MARGIN, PREWARM_LEVELS, LodBuildResult, MAX_PENDING_GENERATION_TASKS, MAX_PENDING_LOD_TASKS,
};
use crate::chunk::MeshGenerationStats;
use crate::chunk::{LodStability, PendingLodBuilds};
use std::collections::HashSet as StdHashSet;
use bevy::tasks::Task as BevyTask;
use std::collections::HashMap as StdHashMap;

/// Result produced by a completed mesh build task for a freshly generated chunk.
pub struct MeshBuildResult {
    pub chunk_x: i32,
    pub chunk_z: i32,
    pub chunk: Chunk,
    pub mesh: Mesh,
    pub triangle_count: usize,
    pub lod: u8,
}

/// Pending mesh build tasks scheduled on the compute pool.
#[derive(Resource, Default)]
pub struct PendingMeshBuilds {
    pub tasks: Vec<BevyTask<MeshBuildResult>>,
    pub coords: StdHashSet<(i32, i32)>,
}

/// Temporary storage for finished mesh handles for coords that do not yet have
/// a spawned entity. This allows the system to avoid spawning a visible
/// entity until the desired LOD's mesh is available, preventing early
/// high-detail uploads.
#[derive(Resource, Default)]
pub struct PendingMeshHandles {
    pub map: StdHashMap<(i32, i32), Vec<Option<Handle<Mesh>>>>,
}

// How many mesh builds to start per frame to avoid sustained queue growth.
const MESH_SCHEDULE_BUDGET_PER_FRAME: usize = 8;
// How many finished meshes to apply per frame to avoid main-thread stalls.
const MESH_APPLY_BUDGET_PER_FRAME: usize = 2;


/// Lightweight diagnostics for streaming to allow periodic logging without
/// allocating or spamming logs every frame.
#[derive(Resource, Default)]
pub struct MeshStreamingDiagnostics {
    pub last_log_seconds: f64,
}

/// Tracks spawned chunk entities and per-LOD mesh handles so meshes can be
/// updated in-place without despawning. The map key is `(chunk_x, chunk_z)`.
///
/// Map value: `(entity, per_lod_handles, active_lod)`
/// - `entity`: spawned entity id for the chunk
/// - `per_lod_handles`: vector of Optional mesh handles indexed by LOD
/// - `active_lod`: current LOD index used for rendering
#[derive(Resource, Default)]
pub struct ChunkEntities {
    pub map: HashMap<(i32, i32), ChunkEntry>,
} 

/// Configuration parameters controlling streaming distances and culling.
///
/// # Fields
/// * `load_distance` - radius (in chunks) to actively load/generate around player
/// * `unload_distance` - distance beyond which chunks are unloaded
/// * `frustum_culling` - enable/disable chunk frustum culling (useful for debugging)
#[derive(Resource)]
pub struct ChunkStreamingConfig {
    pub load_distance: i32,
    pub unload_distance: i32,
    pub frustum_culling: bool,
} 

impl Default for ChunkStreamingConfig {
    fn default() -> Self {
        Self {
            load_distance: 5,
            unload_distance: 7,
            frustum_culling: true,
        }
    }
}

use crate::chunk::compute_lod_from_dist;

#[derive(Resource)]
pub struct StartupTimer {
    pub elapsed: f32,
    pub startup_complete: bool,
} 

#[derive(bevy::ecs::system::SystemParam)]
pub struct StreamChunksCtx<'w, 's> {
    pub player_query: Query<'w, 's, &'static GlobalTransform, With<Camera3d>>,
    pub commands: Commands<'w, 's>,
    pub meshes: ResMut<'w, Assets<Mesh>>,
    pub world: ResMut<'w, World>,
    pub block_registry: Res<'w, crate::block::BlockRegistry>,
    pub config: Res<'w, ChunkStreamingConfig>,
    pub loaded_chunks: Local<'s, std::collections::HashSet<(i32, i32)>>,
    pub pending: ResMut<'w, PendingChunks>,
    pub startup_timer: ResMut<'w, StartupTimer>,
    pub time: Res<'w, Time>,
    pub layer_map: Option<Res<'w, AtlasUVMap>>,
    pub pending_mesh: ResMut<'w, PendingMeshBuilds>,
    pub chunk_entities: ResMut<'w, ChunkEntities>,
    pub stats: ResMut<'w, MeshGenerationStats>,
    pub pending_lod: ResMut<'w, PendingLodBuilds>,
    pub lod_stability: ResMut<'w, LodStability>,
    pub material_handle: Option<Res<'w, VoxelMaterialHandle>>,
    pub mesh_diag: ResMut<'w, MeshStreamingDiagnostics>,
    pub pending_handles: ResMut<'w, PendingMeshHandles>,
}

/// Represents an in-flight chunk generation task scheduled on the compute
/// pool.
pub struct ChunkTask {
    pub coords: (i32, i32), // the x and z chunks that are being generated
    pub task: Task<(i32, i32, Chunk)>, // the background task producing the chunk
} 

/// A generated chunk that is ready for mesh building.
pub struct GeneratedChunk {
    pub coords: (i32, i32), // the x and z chunks that were generated
    pub chunk: Chunk,       // the generated chunk data
} 

/// Holds pending generation tasks and newly completed generated chunks.
#[derive(Resource, Default)]
pub struct PendingChunks {
    pub tasks: Vec<ChunkTask>,
    pub completed: Vec<GeneratedChunk>,
} 

/// Handle to the shared voxel material used for all chunk entities.
///
/// Stored as a resource so systems can wait for material readiness before
/// spawning chunk entities.
#[derive(Resource)]
pub struct VoxelMaterialHandle(pub Handle<ExtendedMaterial<StandardMaterial, VoxelMaterial>>);

/// Main streaming system executed each frame to manage chunk lifecycle.
///
/// # Panics
/// Panics if an internal conversion of a LOD `slot` to `u8` fails. This
/// should never happen because `MAX_LODS` is a small compile-time constant.
///
/// # Arguments
/// * `player_query` - query to find the player's camera transform for centering
///   streaming operations
/// * `commands` - commands to spawn/despawn chunk entities
/// * `meshes` - asset storage to create/update mesh handles
/// * `world` - mutable world chunk storage used by the game state
/// * `block_registry` - registry for resolving block metadata (used for
///   generation and future mesh metadata)
/// * `config` - streaming configuration resource (`ChunkStreamingConfig`)
/// * `loaded_chunks` - local set tracking currently-loaded chunk coords
/// * `pending` - resource tracking background generation tasks and completed
///   generated chunks
/// * `startup_timer` - resource used to temporarily reduce load distance
///   during early startup
/// * `time` - time resource used for hysteresis/time-based logic
/// * `layer_map` - optional atlas UV map resource (may not be ready at startup)
/// * `chunk_entities` - tracks spawned chunk entities and per-LOD handles
/// * `stats` - mesh generation statistics resource updated for each built chunk
/// * `pending_lod` - pending LOD build tasks resource used to schedule/detail builds
/// * `lod_stability` - hysteresis tracking to prevent LOD thrash
/// * `material_handle` - optional shared voxel material used to spawn entities
#[allow(clippy::implicit_hasher, clippy::needless_pass_by_value)]
pub fn stream_chunks(mut ctx: StreamChunksCtx<'_, '_>) {
    crate::debug::record_thread_global("stream_chunks_system");

    // Early returns for optional resources
    let Some(layer_map_res) = ctx.layer_map.as_ref() else { return; };
    let atlas_map = (*layer_map_res).clone();

    let Ok(player_transform) = ctx.player_query.get_single() else { return; };

    // Ensure material ready
    let Some(mat_handle_res) = ctx.material_handle.as_ref() else { return; };
    let _mat_handle = mat_handle_res.0.clone(); // owned handle for spawns (unused here) 

    // Update startup timer
    if !ctx.startup_timer.startup_complete {
        ctx.startup_timer.elapsed += ctx.time.delta_seconds();
        if ctx.startup_timer.elapsed > 2.0 {
            ctx.startup_timer.startup_complete = true;
        }
    }

    // During the initial startup phase prioritize generating the full
    // configured load distance but schedule nearest-first so the player
    // sees nearby terrain quickly while background work continues.
    let load_dist = ctx.config.load_distance;

    let player_pos = player_transform.translation();
    #[allow(clippy::cast_precision_loss, clippy::cast_possible_truncation)]
    let player_chunk_x = (player_pos.x / (CHUNK_SIZE_I32 as f32)).floor() as i32;
    #[allow(clippy::cast_precision_loss, clippy::cast_possible_truncation)]
    let player_chunk_z = (player_pos.z / (CHUNK_SIZE_I32 as f32)).floor() as i32;

    let pool = AsyncComputeTaskPool::get();

    // Keep `stream_chunks` short — delegate work to small helpers that operate
    // on the grouped `ctx` SystemParam. This removes the argument-count and
    // function-length clippy complaints without changing behavior.
    queue_generation(&mut ctx, player_chunk_x, player_chunk_z, load_dist, pool);

    let newly_completed = collect_completed_generation(&mut ctx);
    ctx.pending.completed.extend(newly_completed);

    build_and_apply_meshes(&mut ctx, player_chunk_x, player_chunk_z, &atlas_map);

    update_lods_and_schedule(&mut ctx, player_chunk_x, player_chunk_z, load_dist, &atlas_map);

    process_finished_lod_tasks(&mut ctx, player_chunk_x, player_chunk_z);

    process_finished_mesh_builds(&mut ctx, player_chunk_x, player_chunk_z);

    // Periodic lightweight diagnostics to understand pending-task buildup
    let now = ctx.time.elapsed_seconds_f64();
    if now - ctx.mesh_diag.last_log_seconds > 1.0 {
        ctx.mesh_diag.last_log_seconds = now;
        let pending_mesh_tasks = ctx.pending_mesh.tasks.len();
        let pending_mesh_coords = ctx.pending_mesh.coords.len();
        let pending_gen_tasks = ctx.pending.tasks.len();
        let completed_gen = ctx.pending.completed.len();
        let loaded = ctx.loaded_chunks.len();
        let spawned = ctx.chunk_entities.map.len();
        info!("StreamingDiag: pending_mesh_tasks={} coords={} pending_gen_tasks={} completed_gen={} loaded={} spawned={}",
            pending_mesh_tasks, pending_mesh_coords, pending_gen_tasks, completed_gen, loaded, spawned);
    }

    unload_and_cleanup(&mut ctx, player_chunk_x, player_chunk_z);
}

fn queue_generation(ctx: &mut StreamChunksCtx<'_, '_>, p_x: i32, p_z: i32, load_dist: i32, pool: &bevy::tasks::AsyncComputeTaskPool) {
    // Build a prioritized list of coordinates sorted by manhattan distance
    // from the player chunk so closest chunks are generated first.
    let mut coords: Vec<(i32, i32, i32)> = Vec::new();
    for cx in (p_x - load_dist)..=(p_x + load_dist) {
        for cz in (p_z - load_dist)..=(p_z + load_dist) {
            coords.push((cx, cz, (p_x - cx).abs().max((p_z - cz).abs())));
        }
    }
    coords.sort_by_key(|&(_x, _z, d)| d);

    for (cx, cz, _d) in coords {
        // Cap concurrent generation tasks to avoid unbounded queuing
        if ctx.pending.tasks.len() >= MAX_PENDING_GENERATION_TASKS { break; }
        if ctx.loaded_chunks.contains(&(cx, cz)) { continue; }
        if ctx.pending.tasks.iter().any(|t| t.coords == (cx, cz)) { continue; }

        let cloned_registry = (*ctx.block_registry).clone();
        let task = pool.spawn(async move {
            // Record worker-thread execution for the chunk generation task
            crate::debug::record_thread_global("chunk_generation_task");
            let mut chunk = Chunk::new();
            chunk.generate(cx, cz, &cloned_registry);
            (cx, cz, chunk)
        });

        ctx.pending.tasks.push(ChunkTask { coords: (cx, cz), task });
    }
} 

fn collect_completed_generation(ctx: &mut StreamChunksCtx<'_, '_>) -> Vec<GeneratedChunk> {
    let mut newly_completed = Vec::new();
    ctx.pending.tasks.retain_mut(|gen_task| {
        if gen_task.task.is_finished() {
            if let Ok((cx, cz, chunk)) = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                futures::executor::block_on(&mut gen_task.task)
            })) {
                newly_completed.push(GeneratedChunk { coords: (cx, cz), chunk });
            }
            false
        } else {
            true
        }
    });
    newly_completed
} 

fn build_and_apply_meshes(ctx: &mut StreamChunksCtx<'_, '_>, player_chunk_x: i32, player_chunk_z: i32, atlas_map: &AtlasUVMap) {
    if ctx.pending.completed.is_empty() {
        return;
    }

    let atlas_map_clone = atlas_map.clone();
    let block_registry_clone = ctx.block_registry.clone();

    // Drain completed generated chunks and schedule async mesh builds on the
    // compute pool so the main thread doesn't block waiting for mesh work.
    let mut gen_list: Vec<GeneratedChunk> = ctx.pending.completed.drain(..).collect();

    // Build a temporary world snapshot including loaded chunks
    let mut world_snapshot = ctx.world.chunks.clone();
    for g in &gen_list {
        world_snapshot.insert(g.coords, g.chunk.clone());
    }
    // Prioritize nearer chunks so LOD builds for visible/near areas complete
    // before distant areas — this prevents far-detail builds from starving
    // near-LOD builds.
    gen_list.sort_by_key(|g| {
        let (cx, cz) = g.coords;
        let dist = (player_chunk_x - cx).abs().max((player_chunk_z - cz).abs());
        dist
    });
    let pool = AsyncComputeTaskPool::get();
    let mut scheduled_this_frame = 0usize;
    for generated in gen_list {
        let (cx, cz) = generated.coords;
        // avoid scheduling duplicate mesh builds for the same coord
        if ctx.pending_mesh.coords.contains(&(cx, cz)) { continue; }

        let dist = (player_chunk_x - cx).abs().max((player_chunk_z - cz).abs());
        let lod = compute_lod_from_dist(dist);

        
        // Build neighbor snapshot for this generated chunk
        let mut neigh: std::collections::HashMap<(i32, i32), Chunk> = std::collections::HashMap::new();
        for (dx, dz) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
            let key = (cx + dx, cz + dz);
            if let Some(n) = world_snapshot.get(&key) {
                neigh.insert(key, n.clone());
            }
        }

        // Cap pending mesh builds to avoid overwhelming the compute pool
        // and limit how many we start this frame.
        if ctx.pending_mesh.tasks.len() >= MAX_PENDING_LOD_TASKS || scheduled_this_frame >= MESH_SCHEDULE_BUDGET_PER_FRAME {
            // requeue this generated chunk for later
            ctx.pending.completed.push(GeneratedChunk { coords: (cx, cz), chunk: generated.chunk });
            continue;
        }

        let chunk_clone = generated.chunk.clone();
        let atlas_clone = atlas_map_clone.clone();
        let registry_clone = block_registry_clone.clone();
        let neigh_clone = if neigh.is_empty() { None } else { Some(neigh) };

        let task = pool.spawn(async move {
            crate::debug::record_thread_global("mesh_build_task");
            let (mesh, tri_count) = chunk_clone.build_mesh(&registry_clone, &atlas_clone, lod, (cx, cz), neigh_clone);
            MeshBuildResult { chunk_x: cx, chunk_z: cz, chunk: chunk_clone, mesh, triangle_count: tri_count, lod }
        });

        ctx.pending_mesh.coords.insert((cx, cz));
        ctx.pending_mesh.tasks.push(task);
        scheduled_this_frame += 1;
    }
}

fn update_lods_and_schedule(ctx: &mut StreamChunksCtx<'_, '_>, player_chunk_x: i32, player_chunk_z: i32, load_dist: i32, atlas_map: &AtlasUVMap) {
    let mut builds_scheduled = 0usize;
    for &(cx, cz) in &ctx.loaded_chunks {
        let dist = (player_chunk_x - cx).abs().max((player_chunk_z - cz).abs());
        let candidate_lod = compute_lod_from_dist(dist);

        let entry = ctx.lod_stability.map.entry((cx, cz)).or_insert((candidate_lod, 0.0));
        if entry.0 == candidate_lod { entry.1 += ctx.time.delta_seconds(); } else { entry.0 = candidate_lod; entry.1 = 0.0; }

        if let Some((entity, handles, active_lod)) = ctx.chunk_entities.map.get_mut(&(cx, cz)) {
                // Always allow LOD changes immediately; hysteresis removed.
                let allow_change = true;

            if allow_change && *active_lod != candidate_lod {
                let slot = candidate_lod as usize;

                if handles.len() > slot && let Some(h) = handles[slot].as_ref() {
                    ctx.commands.entity(*entity).insert(h.clone());
                    *active_lod = candidate_lod;
                    continue;
                }

                let coord = (cx, cz, candidate_lod);
                if !ctx.pending_lod.coords.contains(&coord)
                    && builds_scheduled < LOD_BUILD_BUDGET_PER_FRAME
                    && let Some(chunk) = ctx.world.chunks.get(&(cx, cz)) {
                        let chunk_clone = chunk.clone();
                        let atlas_clone = atlas_map.clone();
                        let registry_clone = ctx.block_registry.clone();

                        // Snapshot neighbors for this chunk to allow neighbor-aware meshing
                        let mut neigh: std::collections::HashMap<(i32, i32), Chunk> = std::collections::HashMap::new();
                        for (dx, dz) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
                            let key = (cx + dx, cz + dz);
                            if let Some(n) = ctx.world.chunks.get(&key) {
                                neigh.insert(key, n.clone());
                            }
                        }

                        let pool = AsyncComputeTaskPool::get();
                        let task = pool.spawn(async move {
                            // Record worker-thread execution for LOD build
                            crate::debug::record_thread_global("lod_build_task");
                            let (mesh, tri_count) = chunk_clone.build_mesh(&registry_clone, &atlas_clone, candidate_lod, (cx, cz), if neigh.is_empty() { None } else { Some(neigh) });
                            LodBuildResult { chunk_x: cx, chunk_z: cz, lod: candidate_lod, mesh, triangle_count: tri_count }
                        });
                        ctx.pending_lod.coords.insert(coord);
                        ctx.pending_lod.tasks.push(task);
                        builds_scheduled += 1;
                    }
            }

            if dist <= load_dist + PREWARM_DISTANCE_MARGIN {
                let mut target = candidate_lod;
                for _ in 0..PREWARM_LEVELS {
                    target = (target + 1).min(u8::try_from(MAX_LODS - 1).expect("MAX_LODS fits in u8"));
                    let coord = (cx, cz, target);
                    if !ctx.pending_lod.coords.contains(&coord)
                        && builds_scheduled < LOD_BUILD_BUDGET_PER_FRAME
                        && (handles.len() <= target as usize || handles[target as usize].is_none())
                        && let Some(chunk) = ctx.world.chunks.get(&(cx, cz)) {
                            let chunk_clone = chunk.clone();
                            let atlas_clone = atlas_map.clone();
                            let registry_clone = ctx.block_registry.clone();
                            let pool = AsyncComputeTaskPool::get();
                            // Snapshot neighbors for this chunk to allow neighbor-aware meshing
                            let mut neigh: std::collections::HashMap<(i32, i32), Chunk> = std::collections::HashMap::new();
                            for (dx, dz) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
                                let key = (cx + dx, cz + dz);
                                if let Some(n) = ctx.world.chunks.get(&key) {
                                    neigh.insert(key, n.clone());
                                }
                            }

                            let task = pool.spawn(async move {
                                // Record worker-thread execution for prewarm LOD build
                                crate::debug::record_thread_global("lod_prewarm_task");
                                let (mesh, tri_count) = chunk_clone.build_mesh(&registry_clone, &atlas_clone, target, (cx, cz), if neigh.is_empty() { None } else { Some(neigh) });
                                LodBuildResult { chunk_x: cx, chunk_z: cz, lod: target, mesh, triangle_count: tri_count }
                            });
                            ctx.pending_lod.coords.insert(coord);
                            ctx.pending_lod.tasks.push(task);
                            builds_scheduled += 1;
                        }
                }
            }
        }
    }
} 

fn process_finished_lod_tasks(ctx: &mut StreamChunksCtx<'_, '_>, player_chunk_x: i32, player_chunk_z: i32) {
    let mut i = 0usize;
    while i < ctx.pending_lod.tasks.len() {
        if ctx.pending_lod.tasks[i].is_finished() {
            if let Ok(LodBuildResult { chunk_x: cx, chunk_z: cz, lod, mesh, triangle_count: tri_count }) = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                futures::executor::block_on(&mut ctx.pending_lod.tasks[i])
            })) {
                ctx.pending_lod.coords.remove(&(cx, cz, lod));
                let slot = lod as usize;
                if let Some((entity, handles, active_lod)) = ctx.chunk_entities.map.get_mut(&(cx, cz)) {
                    if handles.len() < MAX_LODS { handles.resize(MAX_LODS, None); }

                    if tri_count == 0 {
                        // Built LOD produced no geometry — clear the slot and
                        // possibly despawn the entity if nothing remains.
                        handles[slot] = None;

                        // If no LOD handles remain for this chunk, remove the
                        // visible entity and free any remaining assets.
                        if !handles.iter().any(|h| h.is_some()) {
                            if let Some((entity, handles_to_drop, _)) = ctx.chunk_entities.map.remove(&(cx, cz)) {
                                ctx.commands.entity(entity).despawn();
                                for mh in handles_to_drop.into_iter().flatten() { ctx.meshes.remove(&mh); }
                            }
                        } else {
                            // If some other handle exists, ensure the entity uses
                            // a valid handle (prefer the first available).
                            if let Some((idx, slot_h)) = handles.iter().enumerate().find(|(_, hh)| hh.is_some()) {
                                if let Some(h) = slot_h.as_ref() {
                                    ctx.commands.entity(*entity).insert(h.clone());
                                    *active_lod = idx as u8;
                                }
                            }
                        }

                        ctx.stats.update_chunk((cx, cz), tri_count);
                    } else {
                        let handle = ctx.meshes.add(mesh);
                        handles[slot] = Some(handle.clone());
                        let dist = (player_chunk_x - cx).abs().max((player_chunk_z - cz).abs());
                        let desired_lod_now = compute_lod_from_dist(dist);
                        if desired_lod_now == lod { ctx.commands.entity(*entity).insert(handle.clone()); *active_lod = lod; }
                        ctx.stats.update_chunk((cx, cz), tri_count);
                    }
                }
            }
            std::mem::drop(ctx.pending_lod.tasks.swap_remove(i));
        } else { i += 1; }
    }
} 

fn unload_and_cleanup(ctx: &mut StreamChunksCtx<'_, '_>, player_chunk_x: i32, player_chunk_z: i32) {
    let mut to_remove = Vec::new();
    for &(cx, cz) in &ctx.loaded_chunks {
        let dist = (cx - player_chunk_x).abs().max((cz - player_chunk_z).abs());
        if dist > ctx.config.unload_distance { to_remove.push((cx, cz)); }
    }

    for (cx, cz) in to_remove {
        ctx.world.chunks.remove(&(cx, cz));
        ctx.loaded_chunks.remove(&(cx, cz));
        if let Some((entity, mesh_handles, _active)) = ctx.chunk_entities.map.remove(&(cx, cz)) {
            ctx.commands.entity(entity).despawn();
            for mh in mesh_handles.into_iter().flatten() { ctx.meshes.remove(&mh); }
        }
        ctx.stats.remove_chunk((cx, cz));
    }

}

fn process_finished_mesh_builds(ctx: &mut StreamChunksCtx<'_, '_>, player_chunk_x: i32, player_chunk_z: i32) {
    let mut i = 0usize;
    let mut applied = 0usize;
    while i < ctx.pending_mesh.tasks.len() {
        if ctx.pending_mesh.tasks[i].is_finished() {
            if applied >= MESH_APPLY_BUDGET_PER_FRAME {
                break; // defer remaining finished tasks to next frame
            }
            if let Ok(MeshBuildResult { chunk_x: cx, chunk_z: cz, chunk, mesh, triangle_count: tri_count, lod }) = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                futures::executor::block_on(&mut ctx.pending_mesh.tasks[i])
            })) {
                ctx.pending_mesh.coords.remove(&(cx, cz));
                
                let slot = lod as usize;
                // If the built mesh contains no triangles, treat the chunk as
                // "data-only": store the chunk + stats and avoid creating any
                // mesh assets or spawned entities. This prevents spawning empty
                // entities for fully-solid chunks.
                if tri_count == 0 {
                    // Update world data & stats so the chunk is considered
                    // generated/loaded (prevents re-generation).
                    ctx.world.chunks.insert((cx, cz), chunk);
                    ctx.stats.update_chunk((cx, cz), tri_count);
                    ctx.loaded_chunks.insert((cx, cz));

                    // If an entity already existed for this coord, remove it
                    // (its previous mesh is now obsolete / empty).
                    if let Some((entity, handles, _)) = ctx.chunk_entities.map.remove(&(cx, cz)) {
                        ctx.commands.entity(entity).despawn();
                        for mh in handles.into_iter().flatten() { ctx.meshes.remove(&mh); }
                    }

                    applied += 1;
                } else {
                    // Apply mesh on main thread: add/replace handles. If an entity
                    // already exists for this coord, update it in-place. If not,
                    // store the handle in `pending_handles` and only spawn an
                    // entity when the desired LOD handle becomes available. This
                    // prevents early high-detail uploads from being rendered.
                    let handle = ctx.meshes.add(mesh);
                    if let Some((entity, handles, active_lod)) = ctx.chunk_entities.map.get_mut(&(cx, cz)) {
                        if handles.len() < MAX_LODS { handles.resize(MAX_LODS, None); }
                        handles[slot] = Some(handle.clone());
                        let dist = (player_chunk_x - cx).abs().max((player_chunk_z - cz).abs());
                        let desired_lod_now = compute_lod_from_dist(dist);
                        if desired_lod_now == lod { ctx.commands.entity(*entity).insert(handle.clone()); *active_lod = lod; }
                        ctx.stats.update_chunk((cx, cz), tri_count);
                        ctx.world.chunks.insert((cx, cz), chunk);
                        ctx.loaded_chunks.insert((cx, cz));
                    } else {
                        // No entity yet: stash handle in pending_handles for coord.
                        let entry = ctx.pending_handles.map.entry((cx, cz)).or_insert_with(|| vec![None; MAX_LODS]);
                        if entry.len() < MAX_LODS { entry.resize(MAX_LODS, None); }
                        entry[slot] = Some(handle.clone());
                        // Also store the chunk data so future spawn can access it
                        ctx.world.chunks.insert((cx, cz), chunk);
                        // Update stats now (we'll account for triangles per-LOD later)
                        ctx.stats.update_chunk((cx, cz), tri_count);
                    }
                    applied += 1;
                }
            }
            std::mem::drop(ctx.pending_mesh.tasks.swap_remove(i));
        } else { i += 1; }
    }

    // Try to spawn a limited number of pending chunk entities whose desired
    // LOD handle is available. This avoids creating entities for the first
    // mesh that completes (which may be overly detailed compared to the
    // current desired LOD).
    let mut spawns_this_frame = 0usize;
    let mut to_remove_coords = Vec::new();
    for (&coord, handles_vec) in ctx.pending_handles.map.iter() {
        if spawns_this_frame >= MESH_APPLY_BUDGET_PER_FRAME { break; }
        let (cx, cz) = coord;
        let dist = (player_chunk_x - cx).abs().max((player_chunk_z - cz).abs());
        let desired_lod_now = compute_lod_from_dist(dist) as usize;
        if desired_lod_now < handles_vec.len() {
            if let Some(Some(mesh_handle)) = handles_vec.get(desired_lod_now) {
                // spawn entity using this handle and move other handles into map
                let mut handles: Vec<Option<Handle<Mesh>>> = vec![None; MAX_LODS];
                for (j, h) in handles_vec.iter().enumerate().take(MAX_LODS) {
                    if let Some(hh) = h { handles[j] = Some(hh.clone()); }
                }

                let entity = ctx.commands.spawn((
                    MaterialMeshBundle {
                        mesh: mesh_handle.clone(),
                        material: ctx.material_handle.as_ref().unwrap().0.clone(),
                        #[allow(clippy::cast_precision_loss)]
                        transform: Transform::from_xyz((cx * CHUNK_SIZE_I32) as f32, 0.0, (cz * CHUNK_SIZE_I32) as f32),
                        ..default()
                    },
                    ChunkEntity { chunk_x: cx, chunk_z: cz },
                )).id();

                ctx.chunk_entities.map.insert((cx, cz), (entity, handles, desired_lod_now as u8));
                ctx.loaded_chunks.insert((cx, cz));
                to_remove_coords.push(coord);
                spawns_this_frame += 1;
            }
        }
    }
    for c in to_remove_coords { ctx.pending_handles.map.remove(&c); }
}