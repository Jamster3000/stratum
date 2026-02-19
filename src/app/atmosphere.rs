//! Atmosphere-related systems and utilities.
//! This module includes systems for syncing atmosphere settings from the main `Settings` resource.
use bevy::prelude::*;
use bevy_atmosphere::prelude::AtmosphereSettings as BevyAtmosphereSettings;
use stratum::settings::Settings;

/// Sync `Settings.atmosphere` fields into the running `BevyAtmosphereSettings`.
/// This allows the user to update atmosphere settings at runtime without restarting.
///
/// Though the way the atmosphere crate and bevy works, the `enable` setting requires the user
/// to restart the runtime.
///
/// # Arguments
/// - `settings`: The current settings resource, from which the atmosphere settings are read.
/// - `atm_settings`: The BevyAtmosphereSettings resource that is updated with the new settings.
/// - `last`: A local cache of the last applied settings to avoid redundant updates.
///
/// # Example
/// ```
/// app.add_systems(Update, crate::app::sync_atmosphere_settings);
/// ```
pub fn sync_atmosphere_settings(
    settings: Res<Settings>,
    mut last: Local<Option<(u32, bool)>>,
    mut atm_settings: ResMut<BevyAtmosphereSettings>,
) {
    let r = settings.atmosphere.resolution;
    let d = settings.atmosphere.dithering;
    let changed = match *last {
        Some((lr, ld)) => lr != r || ld != d,
        None => true,
    };

    if changed {
        atm_settings.resolution = r;
        atm_settings.dithering = d;
        *last = Some((r, d));
    }
}