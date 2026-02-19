//! Chunk mesh generation statistics and helpers.
//!
//! This module provides `MeshGenerationStats` which tracks per-chunk triangle
//! counts and a global total. It's useful for debugging and
//! displaying performance metrics (in the debug overlay (F1)).
//!
//! # Example:
//! ```
//! use voxel_game::chunk::MeshGenerationStats;
//! let mut stats = MeshGenerationStats::default();
//! stats.update_chunk((0,0), 100);
//! assert_eq!(stats.total_triangles, 100);
//! ```

use bevy::prelude::*;
use std::collections::HashMap;

/// Tracks mesh generation statistics.
///
/// `per_chunk_triangles` maps chunk coords `(chunk_x, chunk_z)` to the
/// most-recent triangle count for that chunk. `total_triangles` stores the
/// aggregate sum across all tracked chunks.
#[derive(Resource, Default)]
pub struct MeshGenerationStats {
    pub per_chunk_triangles: HashMap<(i32, i32), usize>,
    pub total_triangles: usize,
}

impl MeshGenerationStats {
    /// Update the triangle count for a chunk and adjust the global total.
    ///
    /// # Arguments
    /// * `coord` - Chunk coordinates `(chunk_x, chunk_z)` used as the key.
    /// * `tri_count` - Triangle count produced for the chunk's latest mesh.
    pub fn update_chunk(&mut self, coord: (i32, i32), tri_count: usize) {
        let prev = self
            .per_chunk_triangles
            .insert(coord, tri_count)
            .unwrap_or(0);
        self.total_triangles = self.total_triangles + tri_count - prev;
    }

    /// Remove a chunk's stats (e.g., when unloading) and adjust total.
    ///
    /// # Arguments
    /// * `coord` - Chunk coordinates `(chunk_x, chunk_z)` to remove from tracking.
    ///
    pub fn remove_chunk(&mut self, coord: (i32, i32)) {
        if let Some(prev) = self.per_chunk_triangles.remove(&coord) {
            self.total_triangles = self.total_triangles.saturating_sub(prev);
        }
    }

    /// Return the top N chunks sorted by triangle count (descending).
    ///
    /// # Arguments
    /// * `n` - number of top entries to return.
    ///
    /// # Return
    /// A `Vec` of `(coord, tri_count)` pairs for the top `n` chunks, sorted
    /// descending by triangle count. If `n` is larger than the number of
    /// tracked chunks, all entries are returned.
    #[must_use]
    pub fn top_chunks(&self, n: usize) -> Vec<((i32, i32), usize)> {
        let mut entries: Vec<((i32, i32), usize)> = self
            .per_chunk_triangles
            .iter()
            .map(|(k, v)| (*k, *v))
            .collect();
        entries.sort_by(|a, b| b.1.cmp(&a.1));
        entries.into_iter().take(n).collect()
    }
}
