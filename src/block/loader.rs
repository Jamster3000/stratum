//! Block loader and watcher for loading block definitions from RON files
//! and monitoring changes for hot reloading during runtime.
//! # Example
//! ```
//! use bevy::prelude::*;
//! use voxel_game::block::loader as block_loader;
//! use voxel_game::block::BlockRegistry;
//! 
//! fn main() {
//!     let mut app = App::new();
//! 
//!     // Load initial registry and insert as a resource
//!     let registry = block_loader::load_blocks_from_dir("data/blocks");
//!     app.insert_resource(registry);
//!
//!     // Create watcher (fallback to stub on error) and insert as resource
//!     let watcher = block_loader::setup_block_watcher("data/blocks")
//!         .unwrap_or_else(|_| block_loader::BlockWatcher::stub());
//!     app.insert_resource(watcher);
//! 
//!     // Add check system (runs every update and will reload when files change)
//!     app.add_system(block_loader::check_block_changes);
//! 
//!     app.run();
//! }
//! ```

use super::{Block, BlockRegistry};
use crate::ron_loader::{load_ron_files, setup_ron_watcher};
use bevy::prelude::{Res, ResMut, Commands, Resource};
use crate::atlas_builder::{AtlasBuilder, AtlasUVMap, AtlasTextureHandle};
use bevy::asset::AssetServer;
use std::path::Path;
use std::sync::Arc;
use crate::chunk::PendingChunks;

#[derive(Resource)]
pub struct BlockWatcher(pub crate::ron::RonWatcher);

/// Load all block definitions from RON files.
///
/// # Arguments
/// * `path` - The directory path where block RON files are located (e.g., "data/blocks").
///
/// # Returns
/// A `BlockRegistry` containing all loaded block definitions, indexed by both name and numeric ID
///
/// # Example
/// ```rust
/// use voxel_game::block::loader::load_blocks_from_dir;
/// use voxel_game::block::BlockRegistry;
///
/// let registry = load_blocks_from_dir("data/blocks");
/// if let Some(dirt_block) = registry.get("dirt") {
///     println!("Dirt block ID: {}", dirt_block.id);
/// }
/// ```
#[must_use]
pub fn load_blocks_from_dir(path: &str) -> BlockRegistry {
    let mut registry = BlockRegistry::default();
    let blocks: Vec<Block> = load_ron_files(path);
    for block in blocks {
        registry.register(block);
    }

    // Use default.png texture for any blocks that are missing textures (or don't currently have any existing textures).
    let missing_id = registry.missing_id();
    if !registry.blocks_by_id.contains_key(&missing_id) {
        let placeholder = Block {
            name: "__missing__".to_string(),
            id: missing_id,
            textures: crate::block::registry::BlockTextures::default(),
            ..Default::default()
        };
        registry.register(placeholder);
    }

    registry
}

/// Set up a file watcher to monitor changes in block RON files
/// This is most ideal for hot reloading without rerunning the game instance
///
/// # Arguments
/// * `path` - The directory path where block RON files are located (e.g., "data/blocks").
///
/// # Returns
/// A `BlockWatcher` that can be used as a Bevy resource to check for changes in block definitions during runtime
///
/// # Errors
/// Returns a `notify::Error` if the underlying file watcher could not be created or configured.
/// # Example
/// ```rust
/// use voxel_game::block::loader::{setup_block_watcher, check_block_changes};
/// use bevy::prelude::{App, ResMut};
/// let mut app = App::new();
/// app.insert_resource(setup_block_watcher("data/blocks"));
/// app.add_system(check_block_changes);
/// ```
pub fn setup_block_watcher(path: &str) -> Result<BlockWatcher, notify::Error> {
    setup_ron_watcher(path).map(BlockWatcher)
}

/// Checks for changes in block RON files and reloads the block registry if changes are detected.
///
/// # Arguments
/// * `watcher` - A `BlockWatcher` resource that monitors changes in block R
/// * `registry` - A mutable reference to the `BlockRegistry` resource that will be updated if changes are detected
///
/// # Example
/// ```rust
/// use bevy::prelude::{App, ResMut};
/// use voxel_game::block::loader;
///
/// let mut app = App::new();
/// let watcher = loader::setup_block_watcher("data/blocks").unwrap();
/// app.insert_resource(watcher);
/// app.add_system(loader::check_block_changes);
/// ```
///
/// # Panics
/// Will panic if the internal `BlockWatcher` mutex is poisoned when calling `lock().unwrap()`.
#[allow(clippy::needless_pass_by_value)]
pub fn check_block_changes(
    watcher: Res<BlockWatcher>,
    mut registry: ResMut<BlockRegistry>,
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut pending: ResMut<PendingChunks>,
    world: ResMut<crate::world::World>,
    mut asset_paths: ResMut<crate::debug::AssetPathRegistry>,
) {
    if *watcher.0.changed.lock().unwrap() {
        println!("Blocks changed, reloading...");

        // Clone old registry to detect texture-only changes
        let old_registry = registry.clone();

        // Load new registry from disk
        let new_registry = load_blocks_from_dir("data/blocks");

        // Determine if textures changed (compare texture config per-block name)
        let mut textures_changed = false;
        for (name, new_block) in &new_registry.blocks {
            let old_texts = old_registry.blocks.get(name).map(Block::get_all_textures);
            let new_texts = new_block.get_all_textures();
            if old_texts.as_ref() != Some(&new_texts) {
                // Either missing previously or textures changed
                textures_changed = true;
                break;
            }
        }

        // Replace registry resource with new data
        *registry = new_registry;
        *watcher.0.changed.lock().unwrap() = false;

        if textures_changed {
            println!("Block textures changed: rebuilding atlas and scheduling remesh...");

            // Rebuild atlas (synchronous) and update AtlasUVMap + atlas image handle resource
            let texture_dir = Path::new("assets/textures/blocks");
            let atlas_output = Path::new("assets/textures/blocks/atlas.png");
            match AtlasBuilder::build_atlas_from_directory(texture_dir, atlas_output, Some(&registry)) {
                Ok(atlas_info) => {
                    // Map blocks to atlas UVs
                    let block_uvs = AtlasBuilder::map_blocks_to_atlas(&registry, &atlas_info);
                    let uv_range = atlas_info.get_uv_range();
                    let default_bounds = atlas_info.get_uv_bounds("default");
                    let default_uvs = crate::atlas_builder::BlockAtlasUVs {
                        top: default_bounds,
                        bottom: default_bounds,
                        side: default_bounds,
                    };

                    // Insert updated AtlasUVMap resource
                    commands.insert_resource(AtlasUVMap::new(
                        Arc::new(block_uvs),
                        uv_range,
                        default_uvs,
                    ));

                    // Load atlas image into Bevy assets and insert handle resource
                    let handle: bevy::prelude::Handle<bevy::render::texture::Image> =
                        asset_server.load("textures/blocks/atlas.png");
                    // Register atlas path for debug mapping
                    asset_paths.0.insert(format!("{:?}", handle.clone()), "textures/blocks/atlas.png".to_string());
                    commands.insert_resource(AtlasTextureHandle(handle));

                    // Enqueue remesh for all loaded chunks: push existing chunk clones into pending.completed
                    for ((cx, cz), chunk) in &world.chunks {
                        pending.completed.push(crate::chunk::GeneratedChunk {
                            coords: (*cx, *cz),
                            chunk: chunk.clone(),
                        });
                    }
                }
                Err(e) => {
                    eprintln!("Failed to rebuild atlas: {e}");
                }
            }
        } else {
            println!("Blocks changed but no texture differences detected; registry reloaded only.");
        }
    }
}

impl BlockWatcher {
    /// Create a stub `BlockWatcher` that does not have an active OS watcher.
    #[must_use]
    pub fn stub() -> Self {
        BlockWatcher(crate::ron::RonWatcher::stub())
    }
}
