//! Chunk Level-of-Detail (LOD) configuration and helpers.
//!
//! This module provides constants and resource types used to determine which
//! LOD a chunk should use based on its distance from the player, and to
//! manage LOD generation tasks and stability timers. Tuning these constants
//! controls memory, build concurrency and visual popping behavior.
use crate::chunk::MAX_LODS;
use bevy::prelude::*;

pub const PREWARM_LEVELS: u8 = 1; // How many LOD levels to prebuild for chunks near load bounary
pub const PREWARM_DISTANCE_MARGIN: i32 = 1; //Chunk distance beyond configured load distance
pub const LOD_BUILD_BUDGET_PER_FRAME: usize = 4; // MAX number of LOD builds per frame
pub const MAX_PENDING_GENERATION_TASKS: usize = 64; // Max number of pending chunk generation concurrently
pub const MAX_PENDING_LOD_TASKS: usize = 256; // Max pending LOD builds

/// Threshold distances (in chunk units) used to select LOD levels.
/// The array must have length `MAX_LODS`. For a given `dist`, the first
/// threshold value `d` where `dist <= d` determines the returned LOD index.
/// More aggressive LOD thresholds so coarser LODs apply earlier (reduces
/// triangles for distant chunks). Values are in chunk units.
pub const LOD_DISTANCES: [i32; MAX_LODS] = [1, 2, 4, 8, 16, 32];

/// Compute the LOD index for a chunk given its distance (in chunk units).
///
/// # Panics
///
/// - If `LOD_DISTANCES.len() >= 256` (index won't fit in `u8`).
/// - If `MAX_LODS == 0`.
///
/// # Return 
/// Returns `0` for the highest detail and larger numbers for progressively
/// coarser LODs. The function scans `LOD_DISTANCES` from smallest to largest
/// and returns the index of the first threshold `d` such that `dist <= d`.
/// If `dist` is larger than all thresholds the highest LOD index is returned.
///
/// # Examples
/// ```rust
/// use crate::chunk::lod::compute_lod_from_dist;
/// assert_eq!(compute_lod_from_dist(3), 1);
/// assert_eq!(compute_lod_from_dist(15), 3);
/// assert_eq!(compute_lod_from_dist(100), 5);
/// ```
#[must_use]
pub fn compute_lod_from_dist(dist: i32) -> u8 {
    for (i, &d) in LOD_DISTANCES.iter().enumerate() {
        if dist <= d {
            return u8::try_from(i).expect("LOD_DISTANCES length must be less than 256");
        }
    }
    u8::try_from(MAX_LODS - 1).expect("MAX_LODS must be greater than 0")
}

/// Tracks how long a candidate LOD has been stable for each loaded chunk.
/// The `map` stores (`candidate_lod`, `elapsed_seconds`) for each chunk coord.
#[derive(Resource, Default)]
pub struct LodStability {
    pub map: std::collections::HashMap<(i32, i32), (u8, f32)>, // (`candidate_lod`, `elapsed_seconds`)
}

/// Result produced by a completed LOD build task.
///
/// Naming the result fields makes call sites clearer than using a naked tuple.
#[derive(Debug)]
pub struct LodBuildResult {
    pub chunk_x: i32, // Chunk X coordinate
    pub chunk_z: i32, // Chunk Z coordinate
    pub lod: u8, // Built LOD index
    pub mesh: Mesh, //Generated mesh
    pub triangle_count: usize, //Triangle Count
}

/// Type alias for an in-flight LOD build task.
pub type LodTask = bevy::tasks::Task<LodBuildResult>;

/// Pending LOD build tasks and a set of in-flight coordinates.
///
/// - `tasks` stores asynchronous tasks that produce a `LodBuildResult` when
///   complete (see `LodBuildResult`).
/// - `coords` is a lookup set to avoid scheduling duplicate builds for the same
///   `(chunk_x, chunk_z, lod)` tuple.
#[derive(Resource, Default)]
pub struct PendingLodBuilds {
    pub tasks: Vec<LodTask>,
    pub coords: std::collections::HashSet<(i32, i32, u8)>,
}
