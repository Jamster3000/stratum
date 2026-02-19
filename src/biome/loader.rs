//! Loader module for biomes.
//! This module provides functions to load biome data from RON files and set up file watchers for dynamic updates.
//!
//! ## Features
//! - Load biomes from RON files in a specified directory.
//! - Set up a file watcher to monitor changes in biome RON files.
//! - Check for changes and reload the `BiomeRegistry` when updates are detected.
//!
//! # Example
//! ```
//! // Load biomes from the "data/biomes" directory
//! let registry = load_biomes_from_dir("data/biomes");
//!
//! // Set up a watcher for the "data/biomes" directory
//! let watcher = setup_biome_watcher("data/biomes");
//!
//! // In the game update loop, check for changes and reload biomes if necessary
//! check_biome_changes(watcher, registry);
//! ```

use super::Biome;
use crate::ron_loader::{load_ron_files, setup_ron_watcher};
use bevy::prelude::{Res, ResMut, Resource};

#[derive(Resource)]
pub struct BiomeWatcher(pub crate::ron::RonWatcher);

/// Loads biome data from RON files in the specified directory and populates the `BiomeRegistry`.
///
/// This function reads all RON files in the given directory, deserializes them into Biome structs,
/// and inserts them into the `BiomeRegistry` for use in the game.
///
/// # Arguments
/// * `path` - The path to the directory containing biome RON files (e.g. "data/biomes").
///
/// # Returns
/// A `BiomeRegistry` populated with the biomes loaded from the specified directory.
///
/// # Example
/// ```
/// let registry = load_biomes_from_dir("data/biomes");
/// ```
#[must_use]
pub fn load_biomes_from_dir(path: &str) -> super::BiomeRegistry {
    let mut registry = super::BiomeRegistry::default();
    let biomes: Vec<Biome> = load_ron_files(path);
    for biome in biomes {
        registry.biomes.insert(biome.name.clone(), biome);
    }
    registry
}

/// Set's up a file watcher for the data/biomes directory.
///
/// This function initializes a watcher to monitor changes (e.g., file additions, modifications, or deletions).
///
/// Ideal for changing biomes whilst the game is running and dynamic updating.
///
/// # Errors
///
/// Returns `Err` if the watcher cannot be created — for example when the
/// path does not exist or is inaccessible, on permission / I/O failures,
/// or when the underlying filesystem-watcher backend fails to initialize.
/// The returned error is the underlying `notify::Error` from `setup_ron_watcher`.
///
/// # Arguments
/// * `path` - The path to the directory containing biome RON files (e.g., "data/biomes").
///
/// # Returns
/// A `Result` containing the initialized `BiomeWatcher` or an error if the watcher setup fails.
///
/// # Example
/// ```
/// setup_biome_watcher("data/biomes");
/// ```
pub fn setup_biome_watcher(path: &str) -> Result<BiomeWatcher, notify::Error> {
    setup_ron_watcher(path).map(BiomeWatcher)
}

/// Checks the file-watcher and reloads any changed biome RON files.
///
/// This function should be called regularly (e.g., in a game update loop) to monitor and respond to
/// changes in biome data files.
///
/// # Arguments
/// * `watcher` - A resource containing the `BiomeWatcher` that monitors file changes
/// * `registry` - A mutable resource containing the `BiomeRegistry` to be updated when changes are detected
///
/// # Example
/// ```
/// check_biome_changes(watcher, registry);
/// ```
#[allow(clippy::needless_pass_by_value)]
pub fn check_biome_changes(watcher: Res<BiomeWatcher>, mut registry: ResMut<super::BiomeRegistry>) {
    // Handle poisoned mutex instead of calling `unwrap()` so this function
    // does not panic if another thread panicked while holding the lock.
    match watcher.0.changed.lock() {
        Ok(mut flag) => {
            if *flag {
                println!("Biomes changed, reloading...");
                *registry = load_biomes_from_dir("data/biomes");
                *flag = false;
            }
        }
        Err(poisoned) => {
            // Recover the guard (best-effort) and continue; log so we can debug.
            eprintln!("warning: biome watcher mutex poisoned — recovering");
            let mut flag = poisoned.into_inner();
            if *flag {
                println!("Biomes changed, reloading...");
                *registry = load_biomes_from_dir("data/biomes");
                *flag = false;
            }
        }
    }
}

impl BiomeWatcher {
    /// Create a stub `BiomeWatcher` that does not have an active OS watcher.
    #[must_use]
    pub fn stub() -> Self {
        BiomeWatcher(crate::ron::RonWatcher::stub())
    }
}
