//! Player physics: gravity, jumping, and ground detection.
//!
//! Applies gravity each frame, handles jumping input, and performs ground
//! collision checks to maintain `on_ground` and correct vertical position.
//! Register `player_physics` as a system to run it each frame.

use crate::block::blocks;
use crate::player::Player;
use crate::world::World;
use bevy::prelude::*;

pub const GRAVITY: f32 = -32.0;
pub const JUMP_VELOCITY: f32 = 8.0;

/// Apply gravity, jumping and ground detection for the player each frame.
///
/// # Arguments
/// * `time` - time resource for delta timing
/// * `world` - world access for block queries (ground detection)
/// * `kb` - keyboard input to detect jump/fly toggles
/// * `q` - query for `(Transform, Player)` to update
/// Step the *core* player vertical-physics for one frame.
///
/// Extracted helper so systems and benchmarks exercise identical logic.
pub fn physics_step(tf: &mut Transform, player: &mut Player, world: &World, dt: f32, kb: &ButtonInput<KeyCode>, fly_key: KeyCode, jump_key: KeyCode) {
    // Flying: while the mapped fly key is held, disable gravity and allow vertical movement handled elsewhere
    if kb.pressed(fly_key) {
        player.flying = true;
        player.velocity.y = 0.0;
        // do not apply gravity or ground logic while flying
        return;
    }

    // Ensure flying flag is cleared when fly key released
    player.flying = false;

    player.velocity.y += GRAVITY * dt;
    if player.velocity.y < -50.0 {
        player.velocity.y = -50.0;
    }

    if kb.just_pressed(jump_key) && player.on_ground {
        player.velocity.y = JUMP_VELOCITY;
        player.on_ground = false;
    }

    let new_y = tf.translation.y + player.velocity.y * dt;
    let feet_y = new_y - 1.7;
    let pr = 0.3;
    let mut gnd = false;
    for dx in [-pr, pr] {
        for dz in [-pr, pr] {
            if world.get_block(
                (tf.translation.x + dx).floor() as i32,
                feet_y.floor() as i32,
                (tf.translation.z + dz).floor() as i32,
            ) != blocks::AIR
            {
                gnd = true;
            }
        }
    }

    if gnd && player.velocity.y < 0.0 {
        tf.translation.y = feet_y.floor() + 1.0 + 1.7;
        player.velocity.y = 0.0;
        player.on_ground = true;
    } else {
        tf.translation.y = new_y;
        if player.velocity.y < 0.0 {
            player.on_ground = false;
        }
    }
}

#[allow(clippy::cast_possible_truncation, clippy::needless_pass_by_value)]
pub fn player_physics(
    time: Res<Time>,
    world: Res<World>,
    kb: Res<ButtonInput<KeyCode>>,
    settings: Res<crate::settings::Settings>,
    mut q: Query<(&mut Transform, &mut Player), With<Camera3d>>,
) {
    let (mut tf, mut player) = q.single_mut();

    let fly_key = settings
        .controls
        .keybinds
        .get("fly")
        .and_then(|s| crate::settings::Settings::keycode_from_str(s))
        .unwrap_or(KeyCode::Tab);

    let jump_key = settings
        .controls
        .keybinds
        .get("jump")
        .and_then(|s| crate::settings::Settings::keycode_from_str(s))
        .unwrap_or(KeyCode::Space);

    physics_step(&mut tf, &mut player, &*world, time.delta_seconds(), &*kb, fly_key, jump_key);
}
