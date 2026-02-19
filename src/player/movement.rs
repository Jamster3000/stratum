//! Player movement system with collision detection.
//!
//! Handles WASD movement, flying, and collision checks against the world.

use crate::block::blocks;
use crate::player::Player;
use crate::world::World;
use bevy::prelude::*;

/// Floor a float to an `i32` with bounds checks to avoid direct truncating casts.
#[allow(clippy::cast_possible_truncation)]
fn floor_to_i32(v: f32) -> i32 {
    let f = f64::from(v).floor();
    assert!(f.is_finite() && f >= f64::from(i32::MIN) && f <= f64::from(i32::MAX));
    i32::try_from(f as i64).expect("floored value fits in i32")
}

/// Handle camera/player movement and collisions each frame.
///
/// # Arguments
/// * `keyboard_input` - current keyboard state for movement/flying input
/// * `world` - voxel world used for collision checks
/// * `time` - delta time resource used to scale movement
/// * `query` - query for `(Transform, Player)` to apply movement to
#[allow(clippy::needless_pass_by_value)]
pub fn camera_movement(
    keyboard_input: Res<ButtonInput<KeyCode>>,
    world: Res<World>,
    time: Res<Time>,
    settings: Res<crate::settings::Settings>,
    mut query: Query<(&mut Transform, &mut Player), With<Camera3d>>,
) {
    let (mut camera, mut player) = query.single_mut();
    let base_speed = 5.0;
    let fly_speed = 40.0;
    let player_height = 1.7;
    let player_radius = 0.35;
    let dt = time.delta_seconds();

    // Map movement keys from settings with defaults
    let map_key = |name: &str, default: KeyCode| {
        settings
            .controls
            .keybinds
            .get(name)
            .and_then(|s| crate::settings::Settings::keycode_from_str(s))
            .unwrap_or(default)
    };

    let forward_kc = map_key("forward", KeyCode::KeyW);
    let back_kc = map_key("back", KeyCode::KeyS);
    let left_kc = map_key("left", KeyCode::KeyA);
    let right_kc = map_key("right", KeyCode::KeyD);
    let fly_kc = map_key("fly", KeyCode::Tab);
    let jump_kc = map_key("jump", KeyCode::Space);

    let mut dir = Vec3::ZERO;

    let forward_raw = camera.forward();
    let fwd = Vec3::new(forward_raw.x, 0.0, forward_raw.z).normalize_or_zero();
    let right_raw = camera.right();
    let right = Vec3::new(right_raw.x, 0.0, right_raw.z).normalize_or_zero();

    if keyboard_input.pressed(forward_kc) {
        dir += fwd;
    }
    if keyboard_input.pressed(back_kc) {
        dir -= fwd;
    }
    if keyboard_input.pressed(left_kc) {
        dir -= right;
    }
    if keyboard_input.pressed(right_kc) {
        dir += right;
    }

    // Toggle flying while mapped fly key is held
    player.flying = keyboard_input.pressed(fly_kc);

    if player.flying {
        // Flying: direct movement with no collisions and vertical control (mapped jump)
        let mut movement = if dir.length_squared() > 0.0001 {
            dir.normalize() * fly_speed * dt
        } else {
            Vec3::ZERO
        };
        if keyboard_input.pressed(jump_kc) {
            movement.y += fly_speed * dt;
        }

        camera.translation += movement;
        // Reset vertical velocity so physics doesn't interfere when un-flying
        player.velocity.y = 0.0;
        player.on_ground = false;
        return;
    }

    // Grounded movement (existing collision checks)
    let speed = base_speed;
    if dir.length_squared() > 0.0001 {
        dir = dir.normalize() * speed * dt;
    }
    let new_pos = camera.translation + dir;

    // Check collision, but if jumping (velocity.y > 0), check from a higher position
    let y_offset = if player.velocity.y > 0.0 { 0.5 } else { 0.0 };
    let feet_y = floor_to_i32(camera.translation.y - player_height + 0.1 + y_offset);
    let head_y = floor_to_i32(camera.translation.y + y_offset);

    let mut can_move_x = true;
    let mut can_move_z = true;

    // Check X movement separately
    for y in feet_y..=head_y {
        for dz in [-player_radius, 0.0, player_radius] {
            if world.get_block(
                floor_to_i32(new_pos.x + player_radius),
                y,
                floor_to_i32(camera.translation.z + dz),
            ) != blocks::AIR
            || world.get_block(
                floor_to_i32(new_pos.x - player_radius),
                y,
                floor_to_i32(camera.translation.z + dz),
            ) != blocks::AIR
            {
                can_move_x = false;
            }
        }
    }

    // Check Z movement separately
    for y in feet_y..=head_y {
        for dx in [-player_radius, 0.0, player_radius] {
            if world.get_block(
                floor_to_i32(camera.translation.x + dx),
                y,
                floor_to_i32(new_pos.z + player_radius),
            ) != blocks::AIR
            || world.get_block(
                floor_to_i32(camera.translation.x + dx),
                y,
                floor_to_i32(new_pos.z - player_radius),
            ) != blocks::AIR
            {
                can_move_z = false;
            }
        }
    }

    // Apply movement if no collision
    if can_move_x {
        camera.translation.x = new_pos.x;
    }
    if can_move_z {
        camera.translation.z = new_pos.z;
    }
}
