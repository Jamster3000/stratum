//! Atlas: types and runtime resources for texture-atlas handling.
//!
//! This module defines the core types produced by the atlas builder and
//! the resources used at runtime to sample textures from the generated
//! atlas image. Types here are intentionally minimal data containers
//! (UV bounds, per-face UVs, and the `AtlasInfo` descriptor) used by
//! the mesh builder and systems that update the atlas.

use std::collections::HashMap;
use std::sync::Arc;
use bevy::prelude::Resource;

/// Information about a generated texture atlas.
pub struct AtlasInfo {
    pub width: u32, //Width of atlas image in pixels
    pub height: u32,// Height of atlas image in pixels
    pub tex_size: u32, // Size of one tile in pixels (assumes square tiles)
    pub texture_positions: HashMap<String, (u32, u32, u32)>, // Map of texture name -> (x, y, index) in the atlas
}

impl AtlasInfo {
    /// Get UV bounds for a named texture within the atlas.
    ///
    ///  If the name is not present the method will
    /// fall back to a tile named "default" if available, otherwise the
    /// tile with the smallest index. If the atlas contains no tiles full
    /// rectangle UVs are returned.
    ///
    /// # Arguments
    /// * `tex_name` - The name of the texture tile to look up.
    ///
    /// # Return
    /// Returns an `UVBounds` describing the min/max U and V coordinates
    /// for the requested tile.
    #[must_use]
    #[allow(clippy::cast_precision_loss)] // atlas sizes are validated to fit f32 mantissa (see debug_assert)
    pub fn get_uv_bounds(&self, tex_name: &str) -> UVBounds {
        // Ensure atlas integer dimensions are within the exact integer range for f32
        // (24-bit mantissa). This keeps UV math exact for typical atlas sizes
        // and documents the assumption that prevents precision-loss concerns (something that clippy didn't/doesn't like).
        debug_assert!(self.width <= (1 << 24) as u32 && self.height <= (1 << 24) as u32,
            "atlas dimensions exceed exact f32 integer range; UV precision may be lost");

        if let Some((x, y, _)) = self.texture_positions.get(tex_name) {
            return UVBounds {
                min_u: *x as f32 / self.width as f32,
                max_u: (*x + self.tex_size) as f32 / self.width as f32,
                min_v: *y as f32 / self.height as f32,
                max_v: (*y + self.tex_size) as f32 / self.height as f32,
            };
        }

        if let Some((x, y, _)) = self.texture_positions.get("default") {
            return UVBounds {
                min_u: *x as f32 / self.width as f32,
                max_u: (*x + self.tex_size) as f32 / self.width as f32,
                min_v: *y as f32 / self.height as f32,
                max_v: (*y + self.tex_size) as f32 / self.height as f32,
            };
        }

        if !self.texture_positions.is_empty() 
            && let Some((_name, (x, y, _idx))) = self
                .texture_positions
                .iter()
                .min_by_key(|(_, (_, _, idx))| *idx){

            return UVBounds {
                    min_u: *x as f32 / self.width as f32,
                    max_u: (*x + self.tex_size) as f32 / self.width as f32,
                    min_v: *y as f32 / self.height as f32,
                    max_v: (*y + self.tex_size) as f32 / self.height as f32,
                };
        }

        UVBounds {
            min_u: 0.0,
            max_u: 1.0,
            min_v: 0.0,
            max_v: 1.0,
        }
    }

    /// Get the UV range (the size of a single tile in UV coordinates).
    ///
    /// # Return
    /// Returns the tile size in UV space as `f32` (`AtlasInfo::tex_size` / `AtlasInfo::width`).
    #[allow(clippy::cast_precision_loss)]
    #[must_use]
    pub fn get_uv_range(&self) -> f32 {
        debug_assert!(self.width <= (1 << 24) as u32,
            "atlas width exceeds exact f32 integer range; UV precision may be lost");
        self.tex_size as f32 / self.width as f32
    }
}

/// Axis-aligned UV bounds (min/max U and V) for a single texture tile.
#[derive(Clone, Copy, Debug, Default)]
pub struct UVBounds {
    pub min_u: f32, // Minimum U coordinate.
    pub max_u: f32, // Maximum U coordinate.
    pub min_v: f32, // Minimum V coordinate.
    pub max_v: f32, // Maximum V coordinate.
}

/// Per-face UV bounds for a block type.
///
/// Stores the `UVBounds` for the top, bottom and side faces so the
/// mesh builder can sample the correct UVs for each face.
#[derive(Clone, Copy, Debug, Default)]
pub struct BlockAtlasUVs {
    pub top: UVBounds, // UVs for the top face.
    pub bottom: UVBounds, // UVs for the bottom face.
    pub side: UVBounds, // UVs for the side faces.
}

/// Enumeration of block faces for UV lookup.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlockFace {
    Top, // Top face of the cube.
    Bottom, // Bottom face of the cube.
    Side, // Side faces of the cube.
}

/// Bevy resource storing atlas UV mappings for all registered blocks.
///
/// Contains a shared map from numeric block id to `BlockAtlasUVs`, the
/// `uv_range` used for tiling, and a default `BlockAtlasUVs` to fall
/// back to when a block id is missing.
#[derive(Resource, Clone, Debug, Default)]
pub struct AtlasUVMap {
    pub block_uvs: Arc<HashMap<u8, BlockAtlasUVs>>, // Shared map of block id -> per-face UV bounds
    pub uv_range: f32, // Size of one texture tile in UV space (useful for repeating/tiling).
    pub default_uvs: BlockAtlasUVs, // Default UV bounds used when a block id is missing from the map.
}

impl AtlasUVMap {
    /// Create a new `AtlasUVMap` resource.
    ///
    /// # Arguments
    /// * `block_uvs` - Shared mapping of block id -> per-face UVs.
    /// * `uv_range` - Size of one tile in UV coordinates.
    /// * `default_uvs` - UVs to use when a block id is missing.
    ///
    /// # Return
    /// Returns a constructed `AtlasUVMap` resource.
    #[must_use]
    pub fn new(
        block_uvs: Arc<HashMap<u8, BlockAtlasUVs>>,
        uv_range: f32,
        default_uvs: BlockAtlasUVs,
    ) -> Self {
        Self {
            block_uvs,
            uv_range,
            default_uvs,
        }
    }

    /// Get UV bounds for a given block id and face.
    ///
    /// # Arguments
    /// * `block_id` - Numeric block id used to lookup per-face UVs.
    /// * `face` - Which face of the block to query.
    ///
    /// # Return
    /// Returns the `UVBounds` for the requested face; if the block id
    /// is not present the configured `default_uvs` are returned.
    #[must_use]
    pub fn get_face_uvs(&self, block_id: u8, face: BlockFace) -> UVBounds {
        match self.block_uvs.get(&block_id) {
            Some(uvs) => match face {
                BlockFace::Top => uvs.top,
                BlockFace::Bottom => uvs.bottom,
                BlockFace::Side => uvs.side,
            },
            None => match face {
                BlockFace::Top => self.default_uvs.top,
                BlockFace::Bottom => self.default_uvs.bottom,
                BlockFace::Side => self.default_uvs.side,
            },
        }
    }
}

pub mod builder;
pub mod compat;

pub use builder::AtlasBuilder;
pub use builder::AtlasTextureHandle;
