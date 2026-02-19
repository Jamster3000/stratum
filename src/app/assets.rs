//! Asset-related systems for the application.
//!
//! This module contains small systems that operate on assets (images/atlases)
//! and ensure they are configured correctly for rendering. Keeping this
//! functionality isolated keeps `main.rs` focused on wiring the app together.

use bevy::prelude::*;
use bevy::render::texture::{ImageAddressMode, ImageFilterMode, ImageSampler, ImageSamplerDescriptor};
use stratum::atlas_builder::AtlasTextureHandle;
use crate::AtlasSamplerReady;

/// Ensure the atlas image uses nearest filtering and clamp-to-edge addressing.
///
/// This system runs after the atlas `Image` has been loaded into Bevy's
/// asset storage. It updates the `Image::sampler` descriptor so the texture
/// atlas behaves correctly (nearest filtering, clamp addressing) and then
/// marks the `AtlasSamplerReady` resource so the change is applied only once.
///
/// # Arguments
/// - `atlas_texture`: Optional resource containing the handle to the atlas image.
/// - `images`: Mutable access to Bevy's `Assets<Image>` to update the sampler.
/// - `ready`: Mutable `AtlasSamplerReady` resource indicating whether the sampler
///   has already been configured.
pub fn ensure_atlas_sampler(
    atlas_texture: Option<Res<AtlasTextureHandle>>,
    mut images: ResMut<Assets<Image>>,
    mut ready: ResMut<AtlasSamplerReady>,
) {
    if ready.0 {
        return;
    }
    let Some(atlas) = atlas_texture else { return; };

    if let Some(image) = images.get_mut(&atlas.0) {
        image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
            address_mode_u: ImageAddressMode::ClampToEdge,
            address_mode_v: ImageAddressMode::ClampToEdge,
            mag_filter: ImageFilterMode::Nearest,
            min_filter: ImageFilterMode::Nearest,
            ..Default::default()
        });
        ready.0 = true;
    }
}
