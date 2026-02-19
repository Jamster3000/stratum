//! Player-related small systems.
//!
//! This module contains small per-player systems kept separate so the
//! main application file remains compact.
use bevy::prelude::*;

/// Follow the player camera with a small local fill light.
///
/// This system moves the `PlayerFillLight` transform to match the current
/// camera position each frame. It expects the `Player` camera entity to be
/// present and will silently no-op if it is not.
///
/// # Arguments
/// - `camera_query`: Query for the player's `GlobalTransform` (camera).
/// - `lights`: Query for transforms tagged with `PlayerFillLight` to update.
#[allow(clippy::needless_pass_by_value)]
pub fn update_player_fill_light(
    camera_query: Query<&GlobalTransform, With<stratum::player::Player>>,
    mut lights: Query<&mut Transform, With<crate::PlayerFillLight>>,
) {
    if let Ok(cam) = camera_query.get_single() {
        let pos = cam.translation();
        for mut t in &mut lights.iter_mut() {
            t.translation = pos;
        }
    }
}
