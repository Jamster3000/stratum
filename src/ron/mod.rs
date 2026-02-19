//! Utilities for loading RON files and watching directories for changes.
//!
//! This module provides a small helper for reading RON files from disk
//! and a simple filesystem watcher resource
//! that sets a shared boolean when files change. The
//! watcher is used for hot-reloading RON-based configuration (blocks,
//! biomes, etc.) during development.

use bevy::prelude::Resource;
use notify::{Config, RecommendedWatcher, RecursiveMode, Watcher};
use serde::de::DeserializeOwned;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

#[derive(Resource)]
/// File-watcher resource for RON hot-reload.
pub struct RonWatcher {
    pub changed: Arc<Mutex<bool>>, // Shared boolean set to `true` when watched files change.
    _watcher: Option<notify::RecommendedWatcher>, //watcher handle kept to prevent immediate drop.
}

impl RonWatcher {
    /// Create a stub `RonWatcher` that does not have an active OS watcher.
    ///
    /// # Return
    /// Returns a `RonWatcher` with `changed` initialized to `false` and
    /// no underlying OS watcher. Useful as a fallback when watcher
    /// creation fails or when running on platforms without notify support.
    #[must_use]
    pub fn stub() -> Self {
        RonWatcher {
            changed: Arc::new(Mutex::new(false)),
            _watcher: None,
        }
    }
}

/// Load all `.ron` files from a directory and deserialize them into `T`.
///
/// # Arguments
/// * `path` - Directory path to scan for `.ron` files.
///
/// # Return
/// A `Vec<T>` containing all successfully deserialized items found in
/// the directory. Files that fail to parse are skipped and a warning is
/// printed to stderr.
#[must_use]
pub fn load_ron_files<T: DeserializeOwned>(path: &str) -> Vec<T> {
    let mut items = Vec::new();

    if let Ok(entries) = std::fs::read_dir(path) {
        for entry in entries.flatten() {
            if let Ok(metadata) = entry.metadata()
                && metadata.is_file()
                    && let Some(ext) = entry.path().extension()
                        && ext == "ron"
                            && let Ok(content) = std::fs::read_to_string(entry.path()) {
                                match ron::from_str::<T>(&content) {
                                    Ok(item) => {
                                        items.push(item);
                                    }
                                    Err(e) => {
                                        eprintln!("Failed to parse {}: {e:?}", entry.path().display());
                                    }
                                }
                            }
        }
    }

    items
}

/// Create a `RonWatcher` that watches a directory for modifications.
///
/// # Arguments
/// * `path` - Directory path to watch for `.ron` file changes.
///
/// # Return
/// Returns a `RonWatcher` on success. The returned watcher's `changed`
/// flag will be set to `true` when a file modification event under the
/// watched directory is observed.
///
/// # Errors
/// Returns a `notify::Error` if the underlying file-watcher cannot be
/// created or the watcher cannot be registered for the provided path.
///
/// # Panics
/// This function uses `Mutex::lock().unwrap()` when setting the shared
/// `changed` flag; that call can panic if the mutex is poisoned.
pub fn setup_ron_watcher(path: &str) -> Result<RonWatcher, notify::Error> {
    let changed = Arc::new(Mutex::new(false));
    let changed_clone = changed.clone();
    // Resolve watched path to a canonical form if possible so we can filter events
    let watched_path: PathBuf = std::fs::canonicalize(path).unwrap_or_else(|_| PathBuf::from(path));

    let mut watcher: RecommendedWatcher = Watcher::new(
        move |res: Result<notify::Event, notify::Error>| match res {
            Ok(event) => {
                if matches!(event.kind, notify::EventKind::Modify(_)) {
                    // Check event paths and only set changed if the path is under the watched directory
                    let mut relevant = false;
                    for p in &event.paths {
                        let p_canon = std::fs::canonicalize(p).unwrap_or_else(|_| p.clone());
                        if p_canon.starts_with(&watched_path) {
                            relevant = true;
                            break;
                        }
                    }
                    if relevant {
                        *changed_clone.lock().unwrap() = true;
                    }
                }
            }
            Err(e) => eprintln!("Watch error: {e:?}"),
        },
        Config::default(),
    )?;

    watcher.watch(Path::new(path), RecursiveMode::NonRecursive)?;
    Ok(RonWatcher { changed, _watcher: Some(watcher) })
}
