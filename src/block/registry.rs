//! This module defines the `Block` and `BlockRegistry` types used by the
//! engine and the per-face `BlockTextures` configuration. Blocks should
//! supply explicit per-face textures (`top`, `bottom`, `side`) via the
//! `textures` field. The previous single-texture shortcut was removed to
//! encourage explicitness and correct atlas packing.
//!
//! Example:
//! ```rust
//! use voxel_game::block::registry::{Block, BlockTextures};
//!
//! // Create a default block and set explicit per-face textures
//! let mut b = Block::default();
//! b.name = "example".to_string();
//! b.textures = BlockTextures {
//!     top: "textures/blocks/top.png".to_string(),
//!     bottom: "textures/blocks/bottom.png".to_string(),
//!     side: "textures/blocks/side.png".to_string(),
//! };
//!
//! // `get_all_textures` returns unique sorted texture paths
//! let all = b.get_all_textures();
//! assert_eq!(all, vec![
//!     "textures/blocks/bottom.png".to_string(),
//!     "textures/blocks/side.png".to_string(),
//!     "textures/blocks/top.png".to_string(),
//! ]);
//! ```
//!
use bevy::prelude::Resource;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Internal struct holding per-face texture paths. This type is intentionally
/// kept private: prefer using `TextureConfig` which is the public API for
/// specifying textures (either a single texture or per-face textures).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockTextures {
    pub top: String, // Texture used for the top face of the block
    pub bottom: String, // Texture used for the bottom face of the block
    pub side: String, // Texture used for all 4 side faces of the block
}

impl Default for BlockTextures {
    fn default() -> Self {
        Self {
            top: "textures/blocks/default.png".to_string(),
            bottom: "textures/blocks/default.png".to_string(),
            side: "textures/blocks/default.png".to_string(),
        }
    }
}

/// Texture configuration for a block whether to apply 1 texture to all faces or 
// Texture configuration: blocks must specify per-face textures using
// `BlockTextures`. The previous single-texture shortcut has been removed
// in favor of explicit per-face configuration.
pub type TextureConfig = BlockTextures;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Block {
    pub name: String,
    pub id: u8,

    /// Per-face textures are required. Use `textures` to specify `top`,
    /// `bottom`, and `side` image paths.
    #[serde(default)]
    pub textures: BlockTextures,
    pub hardness: f32,
    pub breakable: bool,
    pub solid: bool,
    pub color_tint: (f32, f32, f32),
    pub transparent: bool,
    pub friction: f32,
    pub drop_item: String,
    pub drop_count: u32,
}

impl Block {
    /// Return the per-face textures for this block.
    #[must_use]
    pub fn get_texture_config(&self) -> TextureConfig {
        self.textures.clone()
    }

    /// Get all texture paths for this block (top, bottom, side).
    #[must_use]
    pub fn get_all_textures(&self) -> Vec<String> {
        let mut textures = vec![self.textures.top.clone(), self.textures.bottom.clone(), self.textures.side.clone()];
        textures.sort();
        textures.dedup();
        textures
    }
}

impl Default for Block {
    fn default() -> Self {
        Self {
            name: "stone".to_string(),
            id: 1,
            textures: BlockTextures::default(),
            hardness: 1.5,
            breakable: true,
            solid: true,
            color_tint: (1.0, 1.0, 1.0),
            transparent: false,
            friction: 0.6,
            drop_item: "stone".to_string(),
            drop_count: 1,
        }
    }
}

#[derive(Resource, Default, Clone)]
pub struct BlockRegistry {
    pub blocks: HashMap<String, Block>,
    pub blocks_by_id: HashMap<u8, String>,
}

impl BlockRegistry {
    pub fn register(&mut self, block: Block) {
        self.blocks_by_id.insert(block.id, block.name.clone());
        self.blocks.insert(block.name.clone(), block);
    }

    #[must_use]
    pub fn get(&self, name: &str) -> Option<&Block> {
        self.blocks.get(name)
    }

    #[must_use]
    pub fn get_by_id(&self, id: u8) -> Option<&Block> {
        self.blocks_by_id
            .get(&id)
            .and_then(|name| self.blocks.get(name))
    }

    /// Lookup numeric ID for a block `name`.
    #[must_use]
    pub fn id_for_name(&self, name: &str) -> Option<u8> {
        self.blocks.get(name).map(|b| b.id)
    }

    /// Resolve a biome `BlockRef` (either numeric id or name) into a block id.
    #[must_use]
    pub fn resolve_blockref(&self, r: &crate::biome::BlockRef) -> Option<u8> {
        match r {
            crate::biome::BlockRef::Id(id) => Some(*id),
            crate::biome::BlockRef::Name(name) => self.id_for_name(name),
        }
    }

    /// Collect the set of block IDs referenced by a `Biome` configuration.
    ///
    /// This returns block ids from `surface_block`, `soil_block`, `rock_block`,
    /// and any names listed in `block_layers`. It also attempts to resolve
    /// `Ore` entries (preferring `name`, falling back to numeric `id`). The
    /// returned vector preserves insertion order but will contain unique ids.
    #[must_use]
    pub fn ids_for_biome(&self, biome: &crate::biome::Biome) -> Vec<u8> {
        use std::collections::HashSet;

        let mut seen = HashSet::new();
        let mut ids = Vec::new();

        let push = |id: u8, seen: &mut HashSet<u8>, ids: &mut Vec<u8>| {
            if !seen.contains(&id) {
                seen.insert(id);
                ids.push(id);
            }
        };

        if let Some(ref s) = biome.surface_block 
            && let Some(id) = self.resolve_blockref(s) {
            push(id, &mut seen, &mut ids);
        }

        if let Some(ref s) = biome.soil_block 
            && let Some(id) = self.resolve_blockref(s) {
            push(id, &mut seen, &mut ids);
        }

        if let Some(ref s) = biome.rock_block 
            && let Some(id) = self.resolve_blockref(s) {
            push(id, &mut seen, &mut ids);
        }

        for name in &biome.block_layers {
            if let Some(id) = self.id_for_name(name) {
                push(id, &mut seen, &mut ids);
            }
        }

        // Resolve ores (prefer name, then numeric id)
        for ore in &biome.ores {
            if let Some(ref name) = ore.name 
                && let Some(id) = self.id_for_name(name) {
                push(id, &mut seen, &mut ids);
            }
        }

        ids
    }

    /// Sentinel id to use when a requested block name is missing.
    /// This id is reserved for a placeholder block that uses the default texture.
    #[must_use]
    pub fn missing_id(&self) -> u8 {
        u8::MAX
    }
}
