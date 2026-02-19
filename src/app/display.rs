//! Display-related systems, such as syncing vsync
//! settings from the main `Settings` resource to the primary window's present mode.
use bevy::prelude::*;
use bevy::window::{PresentMode, PrimaryWindow};
use stratum::settings::Settings;

/// Sync `Settings.graphics.vsync` into the present mode of the primary window.
/// Allows the user to toggle vsync at runtime without restarting.
///
/// # Arguments
/// - `settings`: The current settings resource, from which the vsync preference is read.
/// - `windows`: Query for the primary window to update its present mode.
/// - `last`: A local cache of the last applied vsync state to avoid redundant updates.
///
/// # Example
/// ```
/// app.add_systems(Update, crate::app::sync_vsync_settings);
/// ```
pub fn sync_vsync_settings(
    settings: Res<Settings>,
    mut windows: Query<&mut Window, With<PrimaryWindow>>,
    mut last: Local<Option<bool>>,
) {
    let desired = settings.graphics.vsync;
    if last.map(|v| v) == Some(desired) { return; }

    for mut w in windows.iter_mut() {
        w.present_mode = if desired { PresentMode::Fifo } else { PresentMode::AutoNoVsync };
    }
    *last = Some(desired);
}