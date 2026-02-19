//! This module contains the core block types and helpers.
//! It exposes block definitions (`Block`), the `BlockRegistry` which
//! stores all loaded blocks, texture configuration types, and the runtime
//! loader/watchers used for hot-reloading block data from RON files.
//!
//! Example:
//!
//! ```rust
//! use bevy::prelude::*;
//! use voxel_game::block::loader as block_loader;
//!
//! fn main() {
//!     let mut app = App::new();
//!     // Load and insert the block registry resource
//!     let registry = block_loader::load_blocks_from_dir("data/blocks");
//!     app.insert_resource(registry);
//!     // Watcher (fallback to stub on error)
//!     let watcher = block_loader::setup_block_watcher("data/blocks").unwrap_or_else(|_| block_loader::BlockWatcher::stub());
//!     app.insert_resource(watcher);
//!     app.add_system(block_loader::check_block_changes);
//!     app.run();
//! }
//! ```

pub mod interaction;
pub use interaction::*;

/// Type used throughout the engine to represent a compact block identifier.
///
/// This is intentionally a `u8` to keep chunk storage memory-efficient.
pub type BlockId = u8;

/// Small helpers and constants used by the chunk generator that refer to
/// special block ids (for example `AIR`). These are intentionally small
/// utilities so chunk code can reference `blocks::AIR` without depending
/// directly on the full registry.
pub mod blocks {
    use super::BlockId;

    /// The block id used to represent empty space (no block present).
    pub const AIR: BlockId = 0;
    pub const DEFAULT: BlockId = 1;

    /// Convenience lookup helpers for interop with `BlockRegistry`.
    pub mod lookup {
        use super::BlockId;
        use crate::block::registry::BlockRegistry;

        /// Return a numeric id for a block name if present in the registry.
        #[must_use]
        pub fn id_for(registry: &BlockRegistry, name: &str) -> Option<BlockId> {
            registry.get(name).map(|b| b.id)
        }

        /// Return a block name for a numeric id if present in the registry.
        #[must_use]
        pub fn name_for(registry: &BlockRegistry, id: BlockId) -> Option<String> {
            registry.blocks_by_id.get(&id).cloned()
        }
    }
}

/// Loader/watchers for block RON files.
pub mod loader;

/// Block registry and related data structures.
pub mod registry;

pub use registry::{Block, BlockRegistry, TextureConfig};
