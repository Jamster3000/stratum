//! Camera control and cursor helpers.
//!
//! Provides mouse-look handling via `camera_look` and cursor grabbing via
//! `cursor_grab`. `camera_look` accumulates mouse motion for the current
//! update and applies yaw/pitch to the player's transform. `cursor_grab`
//! toggles cursor lock/visibility in response to input.

use bevy::input::mouse::MouseMotion;
use bevy::prelude::*;
use bevy::window::{CursorGrabMode, PrimaryWindow};

use crate::player::Player;

// Centralized camera tuning constants — change these to adjust behavior used
// by both the live system and benchmarks.
const CAMERA_MAX_PITCH_DEG: f32 = 85.0;

/// Stores the player's look orientation (yaw and pitch) in radians.
///
/// - `yaw`: horizontal rotation around the Y axis.
/// - `pitch`: vertical rotation around the X axis and clamped to a safe range.
#[derive(Component, Default)]
pub struct PlayerLook {
    /// Horizontal angle (radians).
    pub yaw: f32,
    /// Vertical angle (radians).
    pub pitch: f32,
}

impl PlayerLook {
    /// Apply a raw mouse-delta to this `PlayerLook` (updates yaw/pitch and clamps pitch).
    ///
    /// Public so benchmarks/systems can call the same logic.
    pub fn apply_delta(
        &mut self, 
        delta: Vec2,
        settings: &crate::settings::Settings,
    ) {
        let max_pitch = CAMERA_MAX_PITCH_DEG.to_radians();
        let min_pitch = -max_pitch;

        self.yaw -= delta.x * (settings.controls.mouse_sensitivity / 10000.0 );
        self.pitch -= delta.y * (settings.controls.mouse_sensitivity / 10000.0);
        self.pitch = self.pitch.clamp(min_pitch, max_pitch);
    }
}

/// Apply mouse-look to players with a `PlayerLook` component.
///
/// # Arguments
/// * `windows` - query for the primary window (used to check cursor visibility)
/// * `motion_events` - mouse motion events for this update
/// * `query` - query for `(Transform, PlayerLook)` to update
#[allow(clippy::needless_pass_by_value)]
pub fn camera_look(
    windows: Query<&Window, With<PrimaryWindow>>,
    motion_events: Res<Events<MouseMotion>>, // use Events iterator for current update (Bevy 0.14)
    mut query: Query<(&mut Transform, &mut PlayerLook), With<Player>>,
    settings: Res<crate::settings::Settings>,
) {
    // accumulate mouse delta since this update's events
    let mut delta = Vec2::ZERO;
    for ev in motion_events.iter_current_update_events() {
         let mut axis = ev.delta;
         if settings.controls.invert_x { axis.x = -axis.x; }
         if settings.controls.invert_y { axis.y = -axis.y; }
         delta += axis;

    }

    if delta == Vec2::ZERO {
        return;
    }

    let Ok(window) = windows.get_single() else { return };

    if window.cursor.visible {
        return;
    }

    for (mut transform, mut look) in &mut query {
        // update using shared helper (keeps system and benchmarks consistent)
        look.apply_delta(delta, &*settings);

        // apply rotation: yaw around Y, pitch around X
        transform.rotation = Quat::from_euler(bevy::math::EulerRot::YXZ, look.yaw, look.pitch, 0.0);
    }
}

/// Toggle cursor grab and visibility.
///
/// # Arguments
/// * `wq` - mutable window query to change cursor state
/// * `mb` - mouse button input to detect left-click for grabbing
/// * `kb` - keyboard input to detect Escape to release cursor
#[allow(clippy::needless_pass_by_value)]
pub fn cursor_grab(
    mut wq: Query<&mut Window, With<PrimaryWindow>>,
    mb: Res<ButtonInput<MouseButton>>,
    kb: Res<ButtonInput<KeyCode>>,
    settings: Res<crate::settings::Settings>,
) {
    let mut w = wq.single_mut();
    if mb.just_pressed(MouseButton::Left) {
        w.cursor.grab_mode = CursorGrabMode::Locked;
        w.cursor.visible = false;
    }

    let pause_kc = settings
        .controls
        .keybinds
        .get("pause")
        .and_then(|s| crate::settings::Settings::keycode_from_str(s))
        .unwrap_or(KeyCode::Escape);

    if kb.just_pressed(pause_kc) {
        w.cursor.grab_mode = CursorGrabMode::None;
        w.cursor.visible = true;
    }
}
