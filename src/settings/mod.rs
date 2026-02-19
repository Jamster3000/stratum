//! Settings, types and defaults.
//!
//! Settings are stored as a RON file under `data/settings/` and are hot-reloadable
//! using the existing RON watcher utilities (see `ron::setup_ron_watcher`).
use bevy::prelude::{Resource, KeyCode};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphicsSettings {
    #[serde(default = "GraphicsSettings::default_vsync")]
    pub vsync: bool, // Enable vertical sync to cap FPS to the display refresh rate.
    #[serde(default = "GraphicsSettings::default_present_mode")]
    pub present_mode: String, // Window present mode (e.g., AutoNoVsync). Controls buffering/Latency.
    #[serde(default = "GraphicsSettings::default_render_distance")]
    pub render_distance: u32, // How many chunks away from the player are rendered.
    #[serde(default = "GraphicsSettings::default_shadows")]
    pub shadows: bool, // Enable/disable directional light shadows
    #[serde(default = "GraphicsSettings::default_ambient_tint_strength")]
    pub ambient_tint_strength: f32, // Multiplier for ambient shadow tint applied to voxel materials
}

impl GraphicsSettings {
    fn default_vsync() -> bool { true }
    fn default_present_mode() -> String { "AutoNoVsync".to_string() }
    fn default_render_distance() -> u32 { 8 }
    fn default_shadows() -> bool { true }
    fn default_ambient_tint_strength() -> f32 { 1.0 }
}

impl Default for GraphicsSettings {
    fn default() -> Self {
        Self {
            vsync: Self::default_vsync(),
            present_mode: Self::default_present_mode(),
            render_distance: Self::default_render_distance(),
            shadows: Self::default_shadows(),
            ambient_tint_strength: Self::default_ambient_tint_strength(),
        }
    }
}

/// Audio related settings for the game.
/// Currently there's no audio in the game so these settings
/// haven't been implemented.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioSettings {
    #[serde(default = "AudioSettings::default_master")]
    pub master_volume: f32, // Master output volume
    #[serde(default = "AudioSettings::default_music")]
    pub music_volume: f32, // Music volume multiplier
    #[serde(default = "AudioSettings::default_effects")]
    pub effects_volume: f32, // Sound effects volume multiplier
}

impl AudioSettings {
    fn default_master() -> f32 { 1.0 }
    fn default_music() -> f32 { 0.8 }
    fn default_effects() -> f32 { 0.8 }
}

impl Default for AudioSettings {
    fn default() -> Self {
        Self {
            master_volume: Self::default_master(),
            music_volume: Self::default_music(),
            effects_volume: Self::default_effects(),
        }
    }
}

/// Controls / input settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControlsSettings {
    #[serde(default)]
    pub invert_y: bool, // Invert mouse Y axis
    pub invert_x: bool, // Invert mouse X axis
    #[serde(default = "ControlsSettings::default_sensitivity")]
    pub mouse_sensitivity: f32, // Mouse sensitivity multiplier
    #[serde(default)]
    pub keybinds: HashMap<String, String>, // Map of action names to key identifiers (editable by user)
}

impl ControlsSettings {
    fn default_sensitivity() -> f32 { 1.0 }

    fn default_keybinds() -> HashMap<String, String> {
        use std::collections::HashMap;
        let mut m = HashMap::new();
        m.insert("forward".to_string(), "W".to_string());
        m.insert("back".to_string(), "S".to_string());
        m.insert("left".to_string(), "A".to_string());
        m.insert("right".to_string(), "D".to_string());
        m.insert("jump".to_string(), "Space".to_string());
        m.insert("sneak".to_string(), "LShift".to_string());
        m.insert("toggle_debug".to_string(), "F1".to_string());
        m.insert("toggle_grid".to_string(), "F2".to_string());
        m.insert("dump_debug".to_string(), "F3".to_string());
        m
    }
}
impl Default for ControlsSettings {
    fn default() -> Self {
        Self {
            invert_y: false,
            invert_x: false,
            mouse_sensitivity: Self::default_sensitivity(),
            keybinds: Self::default_keybinds(),
        }
    }
}

/// Performance tuning presets and runtime-related limits.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PerformancePreset { VeryLow, Low, Medium, High, VeryHigh }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceSettings {
    #[serde(default = "PerformanceSettings::default_preset")]
    pub preset: PerformancePreset, // Quick performance preset (very_low..very_high) adjusting multiple subsystems.
    #[serde(default = "PerformanceSettings::default_background_meshing")]
    pub background_meshing: bool, // Allow chunk meshing to run on background worker threads.
    #[serde(default = "PerformanceSettings::default_max_chunk_meshes_per_frame")]
    pub max_chunk_meshes_per_frame: u8, // Limit how many chunk meshes the main thread may build per frame.
}

impl PerformanceSettings {
    fn default_preset() -> PerformancePreset { PerformancePreset::Medium }
    fn default_background_meshing() -> bool { true }
    fn default_max_chunk_meshes_per_frame() -> u8 { 2 }
}

impl Default for PerformanceSettings {
    fn default() -> Self {
        Self {
            preset: Self::default_preset(),
            background_meshing: Self::default_background_meshing(),
            max_chunk_meshes_per_frame: Self::default_max_chunk_meshes_per_frame(),
        }
    }
}

/// Atmosphere settings to configure the bevy_atmosphere crate
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SkyboxCreationMode {
    FromProjectionFarWithFallback(f32), 
    Fixed(f32),
    FromProjectionFar,
}

impl Default for SkyboxCreationMode {
    fn default() -> Self { SkyboxCreationMode::FromProjectionFarWithFallback(1000.0) }
}

/// Atmosphere settings to configure the bevy_atmosphere crate
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AtmosphereSettings {
    #[serde(default = "AtmosphereSettings::default_enabled")]
    pub enabled: bool, // Enable the atmosphere (sky) renderer (required a restart of runtime)
    #[serde(default = "AtmosphereSettings::default_resolution")]
    pub resolution: u32, // Resolution of each skybox face (Auto update at runtime)
    #[serde(default = "AtmosphereSettings::default_dithering")]
    pub dithering: bool, // Enable dithering to reduce color banding in the sky (Auto update at runtime)
    #[serde(default)]
    pub skybox_creation_mode: SkyboxCreationMode,
}

impl AtmosphereSettings {
    fn default_enabled() -> bool { true }
    fn default_resolution() -> u32 { 512 }
    fn default_dithering() -> bool { true }
}

impl Default for AtmosphereSettings {
    fn default() -> Self {
        Self {
            enabled: Self::default_enabled(),
            resolution: Self::default_resolution(),
            dithering: Self::default_dithering(),
            skybox_creation_mode: SkyboxCreationMode::default(),
        }
    }
}

/// Top-level Settings
#[derive(Resource, Clone, Debug, Serialize, Deserialize)]
pub struct Settings {
    #[serde(default)]
    pub graphics: GraphicsSettings,
    #[serde(default)]
    pub audio: AudioSettings,
    #[serde(default)]
    pub controls: ControlsSettings,
    #[serde(default)]
    pub performance: PerformanceSettings,
    #[serde(default)]
    pub atmosphere: AtmosphereSettings,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            graphics: GraphicsSettings::default(),
            audio: AudioSettings::default(),
            controls: ControlsSettings::default(),
            performance: PerformanceSettings::default(),
            atmosphere: AtmosphereSettings::default(),
        }
    }
}

impl Settings {
    #[must_use]
    pub fn defaults() -> Self { Settings::default() }

    /// Add descriptions to each setting field so users understand
    /// what each setting does and gets an idea of what to expect when changing the setting.
    ///
    /// # Return
    /// A nested `HashMap` where the first level keys are section names (e.g. "graphics")
    /// and the second level maps setting field names to their descriptions.
    pub fn field_descriptions() -> std::collections::HashMap<&'static str, std::collections::HashMap<&'static str, &'static str>> {
        use std::collections::HashMap;
        let mut out: HashMap<&'static str, HashMap<&'static str, &'static str>> = HashMap::new();

        out.insert("graphics", {
            let mut m = HashMap::new();
            m.insert("vsync", "Enable vertical sync to cap FPS to the display refresh rate.");
            m.insert("present_mode", "Window present mode (e.g. AutoNoVsync). Controls buffering/latency.");
            m.insert("render_distance", "How many chunks away from the player are rendered (in chunk units).");
            m.insert("shadows", "Enable/disable directional light shadows (can be expensive).");
            m.insert("ambient_tint_strength", "Multiplier for ambient shadow tint applied to voxel materials (0 disables)." );
            m.insert("section", "Label used by the UI to group graphics settings.");
            m
        });

        out.insert("audio", {
            let mut m = HashMap::new();
            m.insert("master_volume", "Master output volume (0.0 = silent, 1.0 = full)." );
            m.insert("music_volume", "Music volume multiplier.");
            m.insert("effects_volume", "Sound effects volume multiplier.");
            m.insert("section", "Label used by the UI to group audio settings.");
            m
        });

        out.insert("controls", {
            let mut m = HashMap::new();
            m.insert("invert_y", "Invert the vertical look/mouse Y axis.");
            m.insert("invert_x", "Invert the horizontal look/mouse X axis.");
            m.insert("mouse_sensitivity", "Mouse look sensitivity multiplier.");
            m.insert("keybinds", "Map of action names to key identifiers (editable by user)." );
            m.insert("section", "Label used by the UI to group control settings.");
            m
        });

        out.insert("performance", {
            let mut m = HashMap::new();
            m.insert("preset", "Quick performance preset (very_low..very_high) adjusting multiple subsystems.");
            m.insert("background_meshing", "Allow chunk meshing to run on background worker threads.");
            m.insert("max_chunk_meshes_per_frame", "Limit how many chunk meshes the main thread may build per frame.");
            m.insert("section", "Label used by the UI to group performance settings.");
            m
        });

        out.insert("atmosphere", {
            let mut m = HashMap::new();
            m.insert("enabled", "Enable the atmosphere (sky) renderer.");
            m.insert("resolution", "Resolution of each skybox face (must be multiple of 8).");
            m.insert("dithering", "Enable dithering to reduce color banding in the sky.");
            m
        });

        out
    }

    /// Convert a string key identifier (e.g., from `controls.keybinds`) into a `KeyCode` that
    /// can be used with Bevy's input system.
    ///
    /// # Arguments
    /// * `name` - The string key identifier to convert (e.g., "W", "Space", "F1").
    ///
    /// # Returns
    /// An `Option<KeyCode>` corresponding to the provided string, or `None` if the string
    /// does not match any known key.
    pub fn keycode_from_str(name: &str) -> Option<KeyCode> {
        let s = name.to_ascii_uppercase();
        if s.len() == 1 {
            let c = s.chars().next().unwrap();
            if ('A'..='Z').contains(&c) {
                return Some(match c {
                    'A' => KeyCode::KeyA,
                    'B' => KeyCode::KeyB,
                    'C' => KeyCode::KeyC,
                    'D' => KeyCode::KeyD,
                    'E' => KeyCode::KeyE,
                    'F' => KeyCode::KeyF,
                    'G' => KeyCode::KeyG,
                    'H' => KeyCode::KeyH,
                    'I' => KeyCode::KeyI,
                    'J' => KeyCode::KeyJ,
                    'K' => KeyCode::KeyK,
                    'L' => KeyCode::KeyL,
                    'M' => KeyCode::KeyM,
                    'N' => KeyCode::KeyN,
                    'O' => KeyCode::KeyO,
                    'P' => KeyCode::KeyP,
                    'Q' => KeyCode::KeyQ,
                    'R' => KeyCode::KeyR,
                    'S' => KeyCode::KeyS,
                    'T' => KeyCode::KeyT,
                    'U' => KeyCode::KeyU,
                    'V' => KeyCode::KeyV,
                    'W' => KeyCode::KeyW,
                    'X' => KeyCode::KeyX,
                    'Y' => KeyCode::KeyY,
                    'Z' => KeyCode::KeyZ,
                    _ => return None,
                });
            }
            if ('0'..='9').contains(&c) {
                return Some(match c {
                    '0' => KeyCode::Digit0,
                    '1' => KeyCode::Digit1,
                    '2' => KeyCode::Digit2,
                    '3' => KeyCode::Digit3,
                    '4' => KeyCode::Digit4,
                    '5' => KeyCode::Digit5,
                    '6' => KeyCode::Digit6,
                    '7' => KeyCode::Digit7,
                    '8' => KeyCode::Digit8,
                    '9' => KeyCode::Digit9,
                    _ => return None,
                });
            }
        }

        Some(match s.as_str() {
            // Function keys
            "F1" => KeyCode::F1,
            "F2" => KeyCode::F2,
            "F3" => KeyCode::F3,
            "F4" => KeyCode::F4,
            "F5" => KeyCode::F5,
            "F6" => KeyCode::F6,
            "F7" => KeyCode::F7,
            "F8" => KeyCode::F8,
            "F9" => KeyCode::F9,
            "F10" => KeyCode::F10,
            "F11" => KeyCode::F11,
            "F12" => KeyCode::F12,
            "F13" => KeyCode::F13,
            "F14" => KeyCode::F14,
            "F15" => KeyCode::F15,
            "F16" => KeyCode::F16,
            "F17" => KeyCode::F17,
            "F18" => KeyCode::F18,
            "F19" => KeyCode::F19,
            "F20" => KeyCode::F20,
            "F21" => KeyCode::F21,
            "F22" => KeyCode::F22,
            "F23" => KeyCode::F23,
            "F24" => KeyCode::F24,

            // Arrows / navigation
            "LEFT" | "ARROWLEFT" => KeyCode::ArrowLeft,
            "RIGHT" | "ARROWRIGHT" => KeyCode::ArrowRight,
            "UP" | "ARROWUP" => KeyCode::ArrowUp,
            "DOWN" | "ARROWDOWN" => KeyCode::ArrowDown,
            "HOME" => KeyCode::Home,
            "END" => KeyCode::End,
            "PAGEUP" => KeyCode::PageUp,
            "PAGEDOWN" => KeyCode::PageDown,
            "INSERT" => KeyCode::Insert,
            "DELETE" | "DEL" => KeyCode::Delete,

            // Whitespace / control
            "ESC" | "ESCAPE" => KeyCode::Escape,
            "SPACE" => KeyCode::Space,
            "TAB" => KeyCode::Tab,
            "ENTER" | "RETURN" => KeyCode::Enter,
            "BACKSPACE" | "BACK" => KeyCode::Backspace,

            // Modifiers
            "LSHIFT" | "SHIFT" => KeyCode::ShiftLeft,
            "RSHIFT" => KeyCode::ShiftRight,
            "LCTRL" | "CTRL" | "CONTROL" => KeyCode::ControlLeft,
            "RCTRL" => KeyCode::ControlRight,
            "LALT" | "ALT" => KeyCode::AltLeft,
            "RALT" => KeyCode::AltRight,
            "LSUPER" | "SUPER" | "LWINDOWS" | "WINDOWS" => KeyCode::SuperLeft,
            "RSUPER" | "RWINDOWS" => KeyCode::SuperRight,

            // Numpad
            "NUMPAD0" | "KP_0" => KeyCode::Numpad0,
            "NUMPAD1" | "KP_1" => KeyCode::Numpad1,
            "NUMPAD2" | "KP_2" => KeyCode::Numpad2,
            "NUMPAD3" | "KP_3" => KeyCode::Numpad3,
            "NUMPAD4" | "KP_4" => KeyCode::Numpad4,
            "NUMPAD5" | "KP_5" => KeyCode::Numpad5,
            "NUMPAD6" | "KP_6" => KeyCode::Numpad6,
            "NUMPAD7" | "KP_7" => KeyCode::Numpad7,
            "NUMPAD8" | "KP_8" => KeyCode::Numpad8,
            "NUMPAD9" | "KP_9" => KeyCode::Numpad9,
            "NUMPADADD" | "KP_ADD" => KeyCode::NumpadAdd,
            "NUMPADSUBTRACT" | "KP_SUBTRACT" => KeyCode::NumpadSubtract,
            "NUMPADMULTIPLY" | "KP_MULTIPLY" => KeyCode::NumpadMultiply,
            "NUMPADDIVIDE" | "KP_DIVIDE" => KeyCode::NumpadDivide,
            "NUMPADDECIMAL" | "KP_DECIMAL" => KeyCode::NumpadDecimal,
            "NUMPADENTER" | "KP_ENTER" => KeyCode::NumpadEnter,

            // Punctuation / symbols
            "-" | "MINUS" => KeyCode::Minus,
            "=" | "EQUALS" | "PLUS" => KeyCode::Equal,
            "[" | "LBRACKET" | "LEFTBRACKET" => KeyCode::BracketLeft,
            "]" | "RBRACKET" | "RIGHTBRACKET" => KeyCode::BracketRight,
            "\\" | "BACKSLASH" => KeyCode::Backslash,
            ";" | "SEMICOLON" => KeyCode::Semicolon,
            "'" | "APOSTROPHE" | "QUOTE" => KeyCode::Quote,
            "`" | "Backquote" | "GRAVE" => KeyCode::Backquote,
            "," | "COMMA" => KeyCode::Comma,
            "." | "DOT" | "PERIOD" => KeyCode::Period,
            "/" | "SLASH" => KeyCode::Slash,

            // Special
            "CAPSLOCK" => KeyCode::CapsLock,
            "SCROLLLOCK" => KeyCode::ScrollLock,
            "PAUSE" | "BREAK" => KeyCode::Pause,
            "PRINTSCREEN" | "PRTSCR" => KeyCode::PrintScreen,
            "NUMLOCK" => KeyCode::NumLock,

            _ => return None,
        })
    }
}

pub mod loader;