//! Systems related to chunk streaming and render distance management.
//! This module includes a system to sync the render distance from the main `Settings` resource.
use bevy::prelude::*;
use stratum::chunk::ChunkStreamingConfig;
use stratum::settings::Settings;

/// Sync `Settings.graphics.render_distance` into the running `ChunkStreamingConfig`.
/// This allows the user to update the render distance at runtime without restarting.
///
/// # Arguments
/// - `settings`: The current settings resource, from which the render distance is read.
/// - `cfg`: The chunk streaming configuration resource that is updated with the new render distance.
/// - `last`: A local cache of the last applied render distance to avoid redundant updates.
///
/// # Example
/// ```
/// app.add_systems(Update, crate::app::sync_streaming_settings);
/// ```
pub fn sync_streaming_settings(
    settings: Res<Settings>,
    mut cfg: ResMut<ChunkStreamingConfig>,
    mut last: Local<Option<u32>>,
) {
    let r = settings.graphics.render_distance;
    if last.map(|v| v) == Some(r) { return; }

    let load = r as i32;
    cfg.load_distance = load;
    cfg.unload_distance = load + 2;

    *last = Some(r);
}