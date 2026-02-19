//! Rendering material and bindings. This will be used as the default and 
//! main used shader for rendering and shading, though there might be
//! additional shaders in the future to make things look different as optional for
//! players to change how their game looks.
//!
//! This module defines the `VoxelMaterial` used by the voxel renderer.
//! The material expects a 2D texture atlas containing block textures and
//! a small ambient tint value used to shade shadowed areas.

use bevy::asset::Asset;
use bevy::pbr::MaterialExtension;
use bevy::prelude::*;
use bevy::render::render_resource::{AsBindGroup, ShaderRef};

/// Material used for rendering.
///
/// The material exposes two bindings:
/// - a 2D texture atlas (`atlas_texture`) containing the block textures,
/// - a small uniform `ambient_tint` (rgba) used to tint shadowed areas.
///
/// The binding indices are intentionally fixed via the attributes so the
/// shader can rely on stable binding slots; do not change them without
/// updating `shaders/voxel_material.wgsl`.
#[derive(AsBindGroup, Asset, TypePath, Clone, Default)]
pub struct VoxelMaterial {
    /// Handle to the 2D texture atlas containing block textures.
    #[texture(100, dimension = "2d")]
    #[sampler(101)]
    pub atlas_texture: Handle<Image>,

    /// Ambient tint applied to shadowed fragments. RGB = tint color,
    /// A = opacity (0.0..1.0). Typical usage: `Vec4::new(r, g, b, a)`.
    #[uniform(102)]
    pub ambient_tint: Vec4,
}

impl MaterialExtension for VoxelMaterial {
    /// Return the fragment shader used by this material.
    fn fragment_shader() -> ShaderRef {
        "shaders/voxel_material.wgsl".into()
    }
}
