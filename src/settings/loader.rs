//! Settings loading and hot-reloading.
//! This module provides utilities for loading settings from RON files and watching
//! for changes to enable hot-reloading of settings at runtime.
//!
//! Settings are loaded from RON files in the `data/settings` directory. If multiple
//! RON files are present, the first successfully parsed `Settings` will be used.
//! If no RON files are found or if no parsing succeeds, default settings will be used.
use crate::ron_loader::{load_ron_files, setup_ron_watcher};
use bevy::prelude::{Res, ResMut, Resource};
use crate::settings::Settings;

#[derive(Resource)]
pub struct SettingsWatcher(pub crate::ron::RonWatcher);

/// Load settings from `path` (directory). If multiple `.ron` files are present
/// the first parsed `Settings` will be used. If none exist the `Default` is used.
///
/// # Arguments
/// * `path` - The directory path where settings RON files are located (e.g., "data/settings").
///
/// # Returns
/// A `Settings` struct loaded from the first successfully parsed RON file in the specified directory
/// or default settings if no valid RON files are found.
///
/// # Example
/// ```
/// let settings = load_settings_from_dir("data/settings");
/// ```
#[must_use]
pub fn load_settings_from_dir(path: &str) -> Settings {
    let items: Vec<Settings> = load_ron_files(path);
    if let Some(first) = items.into_iter().next() {
        first
    } else {
        Settings::defaults()
    }
}

/// Create a watcher for the settings directory (hot-reload).
///
/// # Arguments
/// * `path` - The directory path where settings RON files are located (e.g"data/settings").
///
/// # Returns
/// A `SettingsWatcher` that can be used as a Bevy resource to check for changes in settings during runtime.
#[must_use]
pub fn setup_settings_watcher(path: &str) -> Result<SettingsWatcher, notify::Error> {
    setup_ron_watcher(path).map(SettingsWatcher)
}

/// Check for changes and reload settings resource when files change.
///
/// # Arguments
/// * `watcher` - The `SettingsWatcher` resource that monitors changes in settings RON files.
/// * `settings` - The mutable `Settings` resource that is updated when changes are detected
///
/// # Example
/// ```
/// app.add_systems(Update, crate::settings::loader::check_settings_changes);
/// ```
#[allow(clippy::needless_pass_by_value)]
pub fn check_settings_changes(watcher: Res<SettingsWatcher>, mut settings: ResMut<Settings>) {
    match watcher.0.changed.lock() {
        Ok(mut flag) => {
            if *flag {
                println!("Settings changed, reloading...");
                *settings = load_settings_from_dir("data/settings");
                *flag = false;
            }
        }
        Err(poisoned) => {
            eprintln!("warning: settings watcher mutex poisoned — recovering");
            let mut flag = poisoned.into_inner();
            if *flag {
                println!("Settings changed, reloading...");
                *settings = load_settings_from_dir("data/settings");
                *flag = false;
            }
        }
    }
}

impl SettingsWatcher {
    #[must_use]
    pub fn stub() -> Self {
        SettingsWatcher(crate::ron::RonWatcher::stub())
    }
}
