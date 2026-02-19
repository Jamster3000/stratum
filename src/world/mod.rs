//! World storage and block access helpers.
//!
//! This module provides the `World` resource which manages loaded chunks
//! (a `HashMap<(chunk_x, chunk_z), Chunk>`). It contains helpers for
//! querying and setting blocks in world coordinates and will generate a
//! deterministic chunk when a write occurs to an unloaded chunk.
//!
//! # Example:
//!
//! ```
//! // Query a block at world coordinates
//! let id = world.get_block(10, 64, -5);
//! // Set a block (will generate the chunk if necessary)
//! world.set_block(10, 64, -5, block_id, &block_registry);
//! ```

use crate::block::{blocks, BlockId};
use crate::chunk::{Chunk, CHUNK_SIZE};
use bevy::prelude::*;
use std::collections::HashMap;

/// Maximum world build height (exclusive upper bound).
pub const MAX_HEIGHT: usize = 256;

/// The `World` resource holds loaded chunks keyed by `(chunk_x, chunk_z)`.
///
/// # Fields
/// * `chunks` - mapping from chunk coordinates to `Chunk` data
#[derive(Resource)]
pub struct World {
    pub chunks: HashMap<(i32, i32), Chunk>,
}

impl World {
    /// Create an empty `World` resource with no loaded chunks.
    ///
    /// # Return
    /// * `World` - a newly constructed world with an empty chunk map
    #[must_use]
    pub fn new() -> Self {
        World {
            chunks: HashMap::new(),
        }
    }

    /// Get the block ID at world coordinates (x, y, z).
    ///
    /// # Arguments
    /// * `x`, `y`, `z` - world coordinates for the requested block
    ///
    /// # Return
    /// * `BlockId` - block id at the given coordinates, or `AIR` if out of bounds
    ///
    /// # Panics
    ///
    /// This function uses `i32::try_from(...)` for internal constant conversions
    /// and will panic if those compile-time constants cannot be represented as
    /// `i32` on the target platform (practically impossible for the current
    /// constants, but documented for completeness).
    #[must_use]
    pub fn get_block(&self, x: i32, y: i32, z: i32) -> BlockId {
        let max_h = i32::try_from(MAX_HEIGHT).expect("MAX_HEIGHT fits in i32");
        if y < 0 || y >= max_h {
            return blocks::AIR;
        }

        let chunk_size_i32 = i32::try_from(CHUNK_SIZE).expect("CHUNK_SIZE fits in i32");
        let cx = x.div_euclid(chunk_size_i32);
        let cz = z.div_euclid(chunk_size_i32);
        let lx = usize::try_from(x.rem_euclid(chunk_size_i32)).expect("local x non-negative");
        let ly = usize::try_from(y).expect("local y non-negative");
        let lz = usize::try_from(z.rem_euclid(chunk_size_i32)).expect("local z non-negative");

        self.chunks
            .get(&(cx, cz))
            .map_or(blocks::AIR, |c| c.get(lx, ly, lz))
    }

    /// Set a block at world coordinates, generating the chunk if necessary.
    ///
    /// # Arguments
    /// * `x`, `y`, `z` - world coordinates where the block will be placed
    /// * `block` - the `BlockId` to place
    /// * `block_registry` - used when generating the chunk deterministically
    ///
    /// # Return
    /// * `Option<(i32, i32)>` - `(chunk_x, chunk_z)` of the chunk modified, or
    ///   `None` if the coordinates were out-of-bounds (e.g., y outside valid range)
    ///
    /// # Panics
    ///
    /// Uses `i32::try_from` / `usize::try_from` for constant and index
    /// conversions and will panic if those conversions fail (not expected
    /// for configured constants).
    pub fn set_block(&mut self, x: i32, y: i32, z: i32, block: BlockId, block_registry: &crate::block::BlockRegistry) -> Option<(i32, i32)> {
        let max_h = i32::try_from(MAX_HEIGHT).expect("MAX_HEIGHT fits in i32");
        if y < 0 || y >= max_h {
            return None;
        }

        let chunk_size_i32 = i32::try_from(CHUNK_SIZE).expect("CHUNK_SIZE fits in i32");
        let cx = x.div_euclid(chunk_size_i32);
        let cz = z.div_euclid(chunk_size_i32);
        let lx = usize::try_from(x.rem_euclid(chunk_size_i32)).expect("local x non-negative");
        let ly = usize::try_from(y).expect("local y non-negative");
        let lz = usize::try_from(z.rem_euclid(chunk_size_i32)).expect("local z non-negative");

        // If chunk not present, generate it deterministically and insert so changes succeed
        self.chunks.entry((cx, cz)).or_insert_with(|| {
            let mut c = Chunk::new();
            c.generate(cx, cz, block_registry);
            c
        });
        self.chunks.get_mut(&(cx, cz)).map(|c| {
            c.set(lx, ly, lz, block);
            (cx, cz)
        })
    }
}

impl Default for World {
    fn default() -> Self {
        Self::new()
    }
}