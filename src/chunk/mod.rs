//! Chunk data structures, terrain generation, and the mesh build pipeline.
//!
//! This module defines the `Chunk` container which stores block IDs and
//! provides methods for procedural terrain generation (`generate`) and mesh
//! construction (`build_mesh`). The implementation uses a per-axis greedy
//! mesher to merge adjacent exposed faces into
//! larger quads for efficient rendering.
//!
//! # Example
//! ```
//! use voxel_game::chunk::Chunk;
//! use voxel_game::atlas_builder::AtlasUVMap;
//!
//! let mut chunk = Chunk::new();
//! let atlas = AtlasUVMap::default();
//! let (_mesh, tris) = chunk.build_mesh(&Default::default(), &atlas, 0);
//! println!("built {} triangles", tris);
//! ```

use crate::atlas_builder::AtlasUVMap;
use crate::block::BlockRegistry;
use crate::block::{blocks, BlockId};
use crate::world::MAX_HEIGHT;
use bevy::prelude::*;
use bevy::render::render_asset::RenderAssetUsages;
use noise::{Fbm, MultiFractal, NoiseFn, Perlin, RidgedMulti, Simplex};

pub const CHUNK_SIZE: usize = 32;
pub const MAX_LODS: usize = 6;
pub const CHUNK_DIM: usize = 32; 
pub const CHUNK_LAYERS_Y: usize = 8;
pub const WORLD_HEIGHT_BLOCKS: usize = CHUNK_DIM * CHUNK_LAYERS_Y;

pub mod streaming;
pub mod mesh;
pub mod frustum;

pub mod stats;
pub use stats::MeshGenerationStats;

pub mod lod;
pub use lod::{compute_lod_from_dist, LodStability, PendingLodBuilds};

pub mod debug;
pub use debug::debug_chunk_report;

pub use streaming::*;

#[derive(Component)]
pub struct ChunkEntity {
    pub chunk_x: i32,
    pub chunk_z: i32,
}

#[derive(Clone)]
pub struct Chunk {
    pub blocks: Vec<BlockId>,
}

impl Chunk {
    /// Create a new, empty `Chunk` filled with `AIR` blocks.
    ///
    /// # Return
    /// * `Chunk` - a newly initialized chunk with all blocks set to `AIR`.
    #[must_use]
    pub fn new() -> Self {
        Chunk {
            blocks: vec![blocks::AIR; CHUNK_SIZE * MAX_HEIGHT * CHUNK_SIZE],
        }
    }
    /// Read a block ID at the given local chunk coordinates.
    ///
    /// # Arguments
    /// * `x` - local x in `[0, CHUNK_SIZE)`
    /// * `y` - local y in `[0, MAX_HEIGHT)`
    /// * `z` - local z in `[0, CHUNK_SIZE)`
    ///
    /// # Return
    /// * `BlockId` - the block id at the given coordinates, or `AIR` if out of bounds.
    #[must_use]
    pub fn get(&self, x: usize, y: usize, z: usize) -> BlockId {
        if x >= CHUNK_SIZE || y >= MAX_HEIGHT || z >= CHUNK_SIZE {
            blocks::AIR
        } else {
            self.blocks[x + y * CHUNK_SIZE + z * CHUNK_SIZE * MAX_HEIGHT]
        }
    }
    /// Set a block ID at the given local chunk coordinates.
    ///
    /// # Arguments
    /// * `x` - local x in `[0, CHUNK_SIZE)`
    /// * `y` - local y in `[0, MAX_HEIGHT)`
    /// * `z` - local z in `[0, CHUNK_SIZE)`
    /// * `block` - the `BlockId` to write at the specified coordinates
    pub fn set(&mut self, x: usize, y: usize, z: usize, block: BlockId) {
        if x < CHUNK_SIZE && y < MAX_HEIGHT && z < CHUNK_SIZE {
            self.blocks[x + y * CHUNK_SIZE + z * CHUNK_SIZE * MAX_HEIGHT] = block;
        }
    }

    /// Procedurally generate terrain content for this chunk.
    ///
    /// Fills the chunk's internal block buffer using layered noise
    /// functions (base terrain FBM, ridged mountains, biome selector, cave
    /// noises, and surface detail). The function uses `chunk_x` and `chunk_z`
    /// to produce reproducible world-space results for each chunk coordinate.
    ///
    /// # Arguments
    /// * `chunk_x` - chunk coordinate (world X) used as noise seed offset
    /// * `chunk_z` - chunk coordinate (world Z) used as noise seed offset
    /// * `block_registry` - registry used to resolve block names to `BlockId`
    ///
    /// # Panics
    ///
    /// - If the compile-time `CHUNK_SIZE` constant cannot be converted to `i32`.
    /// - If a local index (`x`, `y`, or `z`) cannot be converted to `i32`.
    pub fn generate(&mut self, chunk_x: i32, chunk_z: i32, block_registry: &crate::block::BlockRegistry) {
        let seed: u32 = 12345; 

        // Base terrain noise (fractal brownian motion for smooth hills)
        let base_fbm: Fbm<Perlin> = Fbm::new(seed)
            .set_octaves(4)
            .set_frequency(0.01)
            .set_persistence(0.5);

        // Ridged noise for mountains
        let ridged: RidgedMulti<Perlin> = RidgedMulti::new(seed + 1)
            .set_octaves(3)
            .set_frequency(0.008);

        // Biome selector (low frequency)
        let biome_noise = Simplex::new(seed + 2);

        // 3D noise for caves
        let cave_noise = Simplex::new(seed + 3);
        let cave_noise_2 = Simplex::new(seed + 4); // Second layer for spaghetti caves

        // Detail noise for surface variation
        let detail_noise = Perlin::new(seed + 5);

        // Precompute CHUNK_SIZE as i32 for safe integer arithmetic.
        let chunk_size_i32 = i32::try_from(CHUNK_SIZE).expect("CHUNK_SIZE fits in i32");

        for x in 0..CHUNK_SIZE {
            for z in 0..CHUNK_SIZE {
                let wx = chunk_x * chunk_size_i32 + i32::try_from(x).expect("x fits in i32");
                let wz = chunk_z * chunk_size_i32 + i32::try_from(z).expect("z fits in i32");
                let wxf = f64::from(wx);
                let wzf = f64::from(wz);

                // Get biome blend factor using midpoint to avoid manual averaging
                let biome = f64::midpoint(biome_noise.get([wxf * 0.002, wzf * 0.002]), 1.0);

                // Base terrain height
                let base_height = base_fbm.get([wxf, wzf]) * 20.0 + 16.0;

                // Mountain contribution
                let mountain_height = ridged.get([wxf, wzf]).abs() * 40.0 * biome;

                // Surface detail
                let detail = detail_noise.get([wxf * 0.1, wzf * 0.1]) * 2.0;

                // Final height (floor then convert) and clamp into chunk bounds.
                // Check finiteness before converting; exact i64 bounds are not
                // needed here because we clamp to `CHUNK_SIZE - 1` below.
                let height_f = (base_height + mountain_height + detail).max(1.0);
                let hf = height_f.floor();
                assert!(hf.is_finite());

                #[allow(clippy::cast_possible_truncation)]
                let height_i64 = hf as i64;
                let mut height = usize::try_from(height_i64).unwrap_or(CHUNK_SIZE - 1);
                height = height.min(CHUNK_SIZE - 1);

                for y in 0..CHUNK_SIZE {
                    let wy = i32::try_from(y).expect("y fits in i32");
                    let wyf = f64::from(wy);

                    // Cave generation using two 3D noise functions
                    let cave_val_1 = cave_noise.get([wxf * 0.03, wyf * 0.03, wzf * 0.03]);
                    let cave_val_2 = cave_noise_2.get([wxf * 0.03, wyf * 0.03, wzf * 0.03]);

                    // Caves exist where both noise values are near zero
                    let cave_threshold = 0.1;
                    let is_cave = cave_val_1.abs() < cave_threshold && cave_val_2.abs() < cave_threshold;

                    // Don't carve caves too close to surface
                    let cave_allowed = y < height.saturating_sub(3);

                    if y < height && !(is_cave && cave_allowed) {
                        let depth_from_surface = height - y;
                        // Resolve ids from registry by name; fall back to registry.missing_id() when missing
                        let grass_id = block_registry.id_for_name("grass").unwrap_or(block_registry.missing_id());
                        let dirt_id = block_registry.id_for_name("dirt").unwrap_or(block_registry.missing_id());
                        let stone_id = block_registry.id_for_name("stone").unwrap_or(block_registry.missing_id());

                        let block = if depth_from_surface == 1 {
                            grass_id
                        } else if depth_from_surface <= 4 {
                            dirt_id
                        } else {
                            stone_id
                        };
                        self.set(x, y, z, block);
                    }
                }
            }
        }
    }

    /// Build a renderable `bevy::mesh::Mesh` from the chunk's blocks.
    ///
    /// The mesh generation pipeline uses the greedy mesher (in
    /// `src/chunk/mesh.rs`) to merge exposed faces and populate position,
    /// normal, color and UV attributes. `lod` controls merging aggressiveness
    /// (higher value -> more aggressive merging and fewer triangles).
    ///
    /// # Arguments
    /// * `_block_registry` - currently unused; retained for future use
    /// * `atlas_map` - texture atlas UV lookup used to compute face UVs
    /// * `lod` - level-of-detail hint controlling merge size
    ///
    /// # Return
    /// * `(Mesh, usize)` - the constructed mesh and the triangle count
    #[must_use]
    pub fn build_mesh(
        &self,
        _block_registry: &BlockRegistry,
        atlas_map: &AtlasUVMap,
        lod: u8,
        chunk_coords: (i32, i32),
        neighbors: Option<std::collections::HashMap<(i32, i32), Chunk>>,
    ) -> (Mesh, usize) {
        // Reserve capacities to avoid repeated reallocations (upper bounds)
        let est_quads = CHUNK_SIZE * CHUNK_SIZE; // very conservative upper bound
        let mut positions = Vec::with_capacity(est_quads * 6);
        let mut normals = Vec::with_capacity(est_quads * 6);
        let mut colors = Vec::with_capacity(est_quads * 6);
        let mut uvs = Vec::with_capacity(est_quads * 6);
        let mut uvs_b: Vec<[f32; 2]> = Vec::with_capacity(est_quads * 6);
        let mut indices = Vec::with_capacity(est_quads * 6);

        // Always use full resolution for mesh generation - LOD will be handled by face merging

        let mut out = crate::chunk::mesh::MeshOutput { positions: &mut positions, normals: &mut normals, colors: &mut colors, uvs: &mut uvs, uvs_b: &mut uvs_b, indices: &mut indices };
        let neigh_ref = neighbors.as_ref();
        self.greedy_mesh_axis(0, &mut out, atlas_map, lod, chunk_coords, neigh_ref);
        self.greedy_mesh_axis(1, &mut out, atlas_map, lod, chunk_coords, neigh_ref);
        self.greedy_mesh_axis(2, &mut out, atlas_map, lod, chunk_coords, neigh_ref);

        let mut mesh = Mesh::new(
            bevy::render::mesh::PrimitiveTopology::TriangleList,
            RenderAssetUsages::default(),
        );
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
        mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
        mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
        mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
        mesh.insert_attribute(Mesh::ATTRIBUTE_UV_1, uvs_b);
        mesh.insert_indices(bevy::render::mesh::Indices::U32(indices));

        let triangle_count = mesh.indices().map_or(0, |i| i.len() / 3);
        (mesh, triangle_count)
    }

}

impl Default for Chunk {
    fn default() -> Self {
        Self::new()
    }
}
