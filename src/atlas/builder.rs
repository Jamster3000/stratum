//! Atlas builder: create atlas image and map block textures.
//!
//! This module provides utilities to build a texture atlas from a directory
//! of PNG files and to map block texture
//! configurations to atlas UV coordinates.
//! The builder is intentionally synchronous and designed to be invoked at
//! startup or during hot-reload of block textures.

use crate::block::BlockRegistry;
use bevy::prelude::Resource;
use bevy::prelude::Handle;
use bevy::render::texture::Image;
use image::{ImageBuffer, Rgba, RgbaImage};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

pub struct AtlasBuilder;

/// Handle to the atlas image stored in Bevy assets
#[derive(Resource, Clone, Debug)]
pub struct AtlasTextureHandle(pub Handle<Image>);

impl AtlasBuilder {
    /// Build `atlas.png` from all PNG files in the `textures/blocks` directory.
    ///
    /// # Errors
    ///
    /// Returns an `Err` when:
    /// - reading or writing files fails (I/O errors),
    /// - a texture file cannot be decoded or has an unsupported format,
    /// - the atlas packing/layout fails (insufficient space or invalid inputs),
    /// - serialization of the atlas metadata fails,
    /// - or when a provided `BlockRegistry` contains invalid/missing entries.
    ///
    /// # Arguments
    /// * `texture_dir` - Directory containing source PNG tiles to pack into the atlas.
    /// * `output_path` - Destination path for the generated atlas image (PNG).
    /// * `registry` - Optional `BlockRegistry` used to deterministically name tiles
    ///   when synthesizing metadata from an existing atlas image.
    ///
    /// # Return
    /// Returns `AtlasInfo` on success describing the generated atlas (dimensions,
    /// tile size and texture positions). Returns an error if no textures or
    /// metadata could be found and atlas generation fails.
    pub fn build_atlas_from_directory(
        texture_dir: &Path,
        output_path: &Path,
        registry: Option<&BlockRegistry>,
    ) -> Result<crate::atlas::AtlasInfo, Box<dyn std::error::Error>> {
        // Collect textures from directory (may be empty)
        let textures = Self::collect_textures(texture_dir)?;

        // If no textures, attempt to restore from metadata or autodetect/synthesize from existing atlas image
        if textures.is_empty() {
            if let Some(info) = Self::try_restore_from_metadata_or_autodetect(output_path, registry)? {
                return Ok(info);
            }
            return Err("No textures found for atlas and no metadata available".into());
        }

        // Build atlas image and metadata from collected textures
        let info = Self::build_from_textures(&textures, output_path)?;
        Ok(info)
    }

    // --- helper methods extracted to reduce function length ---

    fn collect_textures(texture_dir: &Path) -> Result<Vec<(String, RgbaImage)>, Box<dyn std::error::Error>> {
        let mut textures: Vec<(String, RgbaImage)> = Vec::new();
        if texture_dir.exists() {
            for entry in fs::read_dir(texture_dir)? {
                let entry = entry?;
                let path = entry.path();
                // case-insensitive, UTF-8-aware extension check
                if path
                    .extension()
                    .and_then(|e| e.to_str())
                    .is_some_and(|s| s.eq_ignore_ascii_case("png"))
                {
                    let filename = path
                        .file_stem()
                        .and_then(std::ffi::OsStr::to_str)
                        .unwrap_or("unknown")
                        .to_string();
                    if filename == "atlas" {
                        continue;
                    }
                    if let Ok(img) = image::open(&path) {
                        textures.push((filename, img.to_rgba8()));
                    }
                }
            }
        }
        Ok(textures)
    }

    fn try_restore_from_metadata_or_autodetect(
        output_path: &Path,
        registry: Option<&BlockRegistry>,
    ) -> Result<Option<crate::atlas::AtlasInfo>, Box<dyn std::error::Error>> {
        let meta_path = output_path.with_extension("ron");
        if meta_path.exists() && output_path.exists() {
            let meta_content = std::fs::read_to_string(&meta_path)?;
            match ron::from_str::<AtlasMetadata>(&meta_content) {
                Ok(meta) => {
                    println!(
                        "No source textures found; loaded atlas metadata from {}",
                        meta_path.display()
                    );
                    if let Ok(existing_img) = image::open(output_path) {
                        let rgba = existing_img.to_rgba8();
                        if rgba.width() != meta.width || rgba.height() != meta.height {
                            eprintln!("Warning: atlas image size differs from metadata (image: {}x{}, meta: {}x{}). Using metadata values.", rgba.width(), rgba.height(), meta.width, meta.height);
                        }
                    }
                    return Ok(Some(crate::atlas::AtlasInfo {
                        width: meta.width,
                        height: meta.height,
                        tex_size: meta.tex_size,
                        texture_positions: meta.texture_positions,
                    }));
                }
                Err(e) => eprintln!("Failed to parse atlas metadata {}: {:?}", meta_path.display(), e),
            }
        }

        if output_path.exists() {
            // Autodetect tile grid and synthesize metadata
            return Ok(Some(Self::synthesize_metadata_from_image(output_path, registry)?));
        }

        Ok(None)
    }

    fn synthesize_metadata_from_image(
        output_path: &Path,
        registry: Option<&BlockRegistry>,
    ) -> Result<crate::atlas::AtlasInfo, Box<dyn std::error::Error>> {
        let existing_img = image::open(output_path)?;
        let rgba = existing_img.to_rgba8();
        let atlas_width = rgba.width();
        let atlas_height = rgba.height();

        let candidates = [8u32, 16, 32, 64, 128, 256];
        let mut viable: Vec<u32> = candidates
            .iter()
            .copied()
            .filter(|&t| t > 0 && atlas_width % t == 0 && atlas_height % t == 0)
            .collect();

        if viable.is_empty() {
            eprintln!("Failed to autodetect a tile size that divides atlas dimensions. Please regenerate atlas with textures or provide atlas.ron metadata.");
            return Err("No textures found for atlas and no metadata available".into());
        }

        viable.sort_unstable();
        let chosen = viable[0];
        let cols = atlas_width / chosen;
        let rows = atlas_height / chosen;

        // Names from registry (if available)
        let mut names: Vec<String> = Vec::new();
        if let Some(reg) = registry {
            use std::collections::HashSet;
            let mut set: HashSet<String> = HashSet::new();
            for block in reg.blocks.values() {
                let faces = block.get_texture_config();
                set.insert(
                    std::path::Path::new(&faces.top)
                        .file_stem()
                        .and_then(std::ffi::OsStr::to_str)
                        .unwrap_or("default")
                        .to_string(),
                );
                set.insert(
                    std::path::Path::new(&faces.bottom)
                        .file_stem()
                        .and_then(std::ffi::OsStr::to_str)
                        .unwrap_or("default")
                        .to_string(),
                );
                set.insert(
                    std::path::Path::new(&faces.side)
                        .file_stem()
                        .and_then(std::ffi::OsStr::to_str)
                        .unwrap_or("default")
                        .to_string(),
                );
            }
            names = set.into_iter().collect();
            names.sort();
        }

        let mut texture_positions: HashMap<String, (u32, u32, u32)> = HashMap::new();
        let mut idx: u32 = 0;
        for row in 0..rows {
            for col in 0..cols {
                let x = col * chosen;
                let y = row * chosen;
                let name = if (idx as usize) < names.len() {
                    names[idx as usize].clone()
                } else {
                    format!("tile_{idx}")
                };
                texture_positions.insert(name, (x, y, idx));
                idx += 1;
            }
        }

        // Save synthesized metadata
        let meta = AtlasMetadata {
            width: atlas_width,
            height: atlas_height,
            tex_size: chosen,
            texture_positions: texture_positions.clone(),
        };
        let meta_path = output_path.with_extension("ron");
        if let Ok(s) = ron::ser::to_string_pretty(&meta, ron::ser::PrettyConfig::default()) {
            std::fs::write(&meta_path, s)?;
        } else {
            eprintln!("Failed to serialize autogenerated atlas metadata");
        }

        Ok(crate::atlas::AtlasInfo {
            width: atlas_width,
            height: atlas_height,
            tex_size: chosen,
            texture_positions,
        })
    }

    // Private helper: integer ceil(sqrt(n)) implemented without floats.
    // Binary-search ceil(sqrt(n)) without overflow using `usize::midpoint`.
    fn ceil_sqrt(n: usize) -> usize {
        if n <= 1 { return n; }
        let mut low = 1usize;
        let mut high = n;
        while low + 1 < high {
            let mid = usize::midpoint(low, high);
            if mid.saturating_mul(mid) >= n { high = mid; } else { low = mid; }
        }
        high
    }

    fn build_from_textures(
        textures: &[(String, RgbaImage)],
        output_path: &Path,
    ) -> Result<crate::atlas::AtlasInfo, Box<dyn std::error::Error>> {
        // Sort textures by name for consistent ordering
        let mut textures = textures.to_vec();
        textures.sort_by(|a, b| a.0.cmp(&b.0));

        let tex_size = textures[0].1.width();
        let num_textures = textures.len();

        // Compute integer ceil(sqrt(num_textures)) without floating casts to avoid
        // truncation warnings.
        let cols = u32::try_from(Self::ceil_sqrt(num_textures)).unwrap();
        let rows = u32::try_from(num_textures.div_ceil(cols as usize)).unwrap();

        let atlas_width = tex_size * cols;
        let atlas_height = tex_size * rows;

        let mut atlas: RgbaImage = ImageBuffer::new(atlas_width, atlas_height);
        for pixel in atlas.pixels_mut() {
            *pixel = Rgba([255, 0, 255, 255]);
        }

        let mut texture_positions: HashMap<String, (u32, u32, u32)> = HashMap::new();
        for (idx, (tex_name, tex_img)) in textures.iter().enumerate() {
            let idx_u32 = u32::try_from(idx).unwrap(); // safe: texture count is small and fits in u32 in practice
            let col = idx_u32 % cols;
            let row = idx_u32 / cols;
            let x = col * tex_size;
            let y = row * tex_size;
            image::imageops::overlay(&mut atlas, tex_img, i64::from(x), i64::from(y));
            texture_positions.insert(tex_name.clone(), (x, y, idx_u32));
        }

        if let Some(parent) = output_path.parent() {
            fs::create_dir_all(parent)?;
        }
        atlas.save(output_path)?;

        let meta = AtlasMetadata {
            width: atlas_width,
            height: atlas_height,
            tex_size,
            texture_positions: texture_positions.clone(),
        };
        let meta_path = output_path.with_extension("ron");
        if let Ok(s) = ron::ser::to_string_pretty(&meta, ron::ser::PrettyConfig::default()) {
            std::fs::write(&meta_path, s)?;
        } else {
            eprintln!("Failed to serialize atlas metadata");
        }

        Ok(crate::atlas::AtlasInfo {
            width: atlas_width,
            height: atlas_height,
            tex_size,
            texture_positions,
        })
    }

    /// Map blocks from registry to atlas UV coordinates
    pub fn map_blocks_to_atlas(
        registry: &BlockRegistry,
        atlas_info: &crate::atlas::AtlasInfo,
    ) -> HashMap<u8, crate::atlas::BlockAtlasUVs> {
        let mut block_uvs: HashMap<u8, crate::atlas::BlockAtlasUVs> = HashMap::new();

        for block in registry.blocks.values() {
            let faces = block.get_texture_config();

            let top_name = Path::new(&faces.top)
                .file_stem()
                .and_then(std::ffi::OsStr::to_str)
                .unwrap_or("default")
                .to_string();
            let bottom_name = Path::new(&faces.bottom)
                .file_stem()
                .and_then(std::ffi::OsStr::to_str)
                .unwrap_or("default")
                .to_string();
            let side_name = Path::new(&faces.side)
                .file_stem()
                .and_then(std::ffi::OsStr::to_str)
                .unwrap_or("default")
                .to_string();

            let uvs = crate::atlas::BlockAtlasUVs {
                top: atlas_info.get_uv_bounds(&top_name),
                bottom: atlas_info.get_uv_bounds(&bottom_name),
                side: atlas_info.get_uv_bounds(&side_name),
            };

            block_uvs.insert(block.id, uvs);
        }

        block_uvs
    }
}

use serde::{Deserialize, Serialize};

/// Internal metadata format written next to `atlas.png` to preserve texture positions.
///
/// This struct is serialized to `atlas.ron` next to the generated atlas image
/// so subsequent runs can restore texture-to-position mappings without the
/// original source files.
#[derive(Serialize, Deserialize, Debug)]
struct AtlasMetadata {
    pub width: u32, // Atlas image width in pixels
    pub height: u32, // Atlas image height in pixels
    pub tex_size: u32, // Side length of a single tile in pixels
    pub texture_positions: HashMap<String, (u32, u32, u32)>,  // Mapping of texture name -> (x, y, index) in pixel coordinates.
}
