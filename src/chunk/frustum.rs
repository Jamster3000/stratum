//! This file is for player frustum culling of chunk entities.
//! The main system is `cull_chunk_entities_system`, which queries the camera's
//! position and orientation, iterates over chunk entities, and sets their
//! `Visibility` based on whether they are within the camera's view cone.
use bevy::prelude::*;
use crate::chunk::CHUNK_SIZE;

/// Function to test if a chunk AABB is within the camera's view cone,
/// used for simple frustum culling of chunk entities.
/// 
/// # Arguments
/// - `camera_pos`: The world position of the camera.
/// - `camera_forward`: The forward direction vector of the camera (should be normalized).
/// - `chunk_min`: The minimum corner of the chunk's AABB.
/// - `chunk_max`: The maximum corner of the chunk's AABB.
/// - `fov_deg`: The camera's field of view in degrees (used to compute the cone angle).
/// - `max_distance`: The maximum distance at which the chunk should be considered visible.
///
/// # Returns
/// Boolean: `true` if the chunk is within the view cone and should be visible, `false` otherwise.
fn chunk_in_view_cone(
    camera_pos: Vec3,
    camera_forward: Vec3,
    chunk_min: Vec3,
    chunk_max: Vec3,
    fov_deg: f32,
    max_distance: f32,
) -> bool {
    // Bounding-sphere early-out
    let center = (chunk_min + chunk_max) * 0.5;
    let half = (chunk_max - chunk_min) * 0.5;
    let radius = (half.x * half.x + half.y * half.y + half.z * half.z).sqrt();
    let to_center = center - camera_pos;
    let center_dist = to_center.length();
    if center_dist > max_distance + radius {
        return false;
    }

    let forward = camera_forward.normalize();
    let cos_half = (fov_deg.to_radians() * 0.5).cos();

    // If center is inside cone, accept.
    if center_dist > 0.0 {
        let center_dir = to_center / center_dist;
        if forward.dot(center_dir) >= cos_half {
            return true;
        }
    }

    // Check AABB corners â€” if any corner is inside the cone
    let corners = [
        Vec3::new(chunk_min.x, chunk_min.y, chunk_min.z),
        Vec3::new(chunk_min.x, chunk_min.y, chunk_max.z),
        Vec3::new(chunk_min.x, chunk_max.y, chunk_min.z),
        Vec3::new(chunk_min.x, chunk_max.y, chunk_max.z),
        Vec3::new(chunk_max.x, chunk_min.y, chunk_min.z),
        Vec3::new(chunk_max.x, chunk_min.y, chunk_max.z),
        Vec3::new(chunk_max.x, chunk_max.y, chunk_min.z),
        Vec3::new(chunk_max.x, chunk_max.y, chunk_max.z),
    ];

    for corner in corners {
        let to_corner = corner - camera_pos;
        let d = to_corner.length();
        if d <= 1e-6 || d > max_distance + radius { continue; }
        let dir = to_corner / d;
        if forward.dot(dir) >= cos_half {
            return true;
        }
    }

    false
}

/// System to cull chunk entities based on the camera's position and orientation.
///
/// This system iterates over all chunk entities, computes their AABBs, and
/// uses the `chunk_in_view_cone` function to determine if they should be visible.
///
/// # Arguments
/// * `commands` - Commands to modify entity visibility.
/// * `camera_query` - Query to get the primary camera's global transform.
/// * `chunks` - Query to get chunk entities and their transforms.
/// * `settings` - Optional resource for chunk streaming configuration (used for max distance).
/// * `time` - Resource to get the current time for potential future use in visibility hysteresis.
#[allow(clippy::needless_pass_by_value)]
pub fn cull_chunk_entities_system(
    mut commands: Commands,
    camera_query: Query<&GlobalTransform, With<Camera3d>>,
    // Capture current Visibility so we only log changes
    chunks: Query<(Entity, &GlobalTransform, &crate::chunk::ChunkEntity, Option<&Visibility>)>,
    settings: Option<Res<crate::chunk::ChunkStreamingConfig>>,
    time: Res<Time>,
) {
    let Ok(cam_tf) = camera_query.get_single() else { return; };
    let cam_pos = cam_tf.translation();

    // If culling is disabled in the streaming config, make all chunks visible
    if let Some(cfg) = settings.as_ref() {
        if !cfg.frustum_culling {
            for (entity, _tf, _chunk_comp, _vis) in chunks.iter() {
                commands.entity(entity).insert(Visibility::Visible);
            }
            return;
        }
    }

    let max_distance = settings.as_ref().map_or(64.0, |s| (s.load_distance as f32) * (CHUNK_SIZE as f32) * 1.5);
    let fov = 100.0_f32; // wider default FOV to avoid edge popping

    let _now = time.elapsed_seconds_f64();

    for (entity, tf, chunk_comp, vis_opt) in chunks.iter() {
        let chunk_min = tf.translation();
        let chunk_max = chunk_min + Vec3::new(
            CHUNK_SIZE as f32,
            crate::world::MAX_HEIGHT as f32,
            CHUNK_SIZE as f32,
        );

        // If the camera is inside this chunk's AABB, always keep it visible
        let contains_cam = (cam_pos.x >= chunk_min.x && cam_pos.x <= chunk_max.x)
            && (cam_pos.y >= chunk_min.y && cam_pos.y <= chunk_max.y)
            && (cam_pos.z >= chunk_min.z && cam_pos.z <= chunk_max.z);
        if contains_cam {
            commands.entity(entity).insert(Visibility::Visible);
            continue;
        }

        // Use camera forward at call site to avoid type mismatches
        let forward = cam_tf_forward(&cam_tf).normalize();
        let in_view = chunk_in_view_cone(cam_pos, forward, chunk_min, chunk_max, fov, max_distance);

        let currently_visible = matches!(vis_opt, Some(v) if matches!(v, Visibility::Visible));

        // Conservative hysteresis: when a chunk is already visible, require it to be
        // *clearly* outside an *expanded* view cone before hiding.  Instead of
        // testing only the chunk center we also test the AABB corners so thin
        // slivers at the frustum edge are kept visible longer (avoids popping).
        let new_visible = if currently_visible {
            if in_view {
                true
            } else {
                // expanded hysteresis cone (degrees)
                let hysteresis_deg = 12.0_f32;
                let hide_angle = (fov * 0.5) + hysteresis_deg;
                let hide_cos = hide_angle.to_radians().cos();

                // quick camera-inside check
                let center = (chunk_min + chunk_max) * 0.5;
                let to_center = center - cam_pos;
                let center_dist = to_center.length();
                if center_dist <= 1e-6 {
                    true
                } else {
                    // if the chunk center is still within the expanded cone, keep visible
                    let center_dot = forward.dot(to_center / center_dist);
                    if center_dot >= hide_cos {
                        true
                    } else {
                        // otherwise check AABB corners (keep visible if *any* corner
                        // is inside the expanded cone)
                        let half = (chunk_max - chunk_min) * 0.5;
                        let radius = (half.x * half.x + half.y * half.y + half.z * half.z).sqrt();

                        let corners = [
                            Vec3::new(chunk_min.x, chunk_min.y, chunk_min.z),
                            Vec3::new(chunk_min.x, chunk_min.y, chunk_max.z),
                            Vec3::new(chunk_min.x, chunk_max.y, chunk_min.z),
                            Vec3::new(chunk_min.x, chunk_max.y, chunk_max.z),
                            Vec3::new(chunk_max.x, chunk_min.y, chunk_min.z),
                            Vec3::new(chunk_max.x, chunk_min.y, chunk_max.z),
                            Vec3::new(chunk_max.x, chunk_max.y, chunk_min.z),
                            Vec3::new(chunk_max.x, chunk_max.y, chunk_max.z),
                        ];

                        let mut any_in_margin = false;
                        for corner in corners {
                            let to_corner = corner - cam_pos;
                            let d = to_corner.length();
                            if d <= 1e-6 || d > (max_distance + radius) { continue; }
                            let dir = to_corner / d;
                            if forward.dot(dir) >= hide_cos {
                                any_in_margin = true;
                                break;
                            }
                        }

                        any_in_margin
                    }
                }
            }
        } else {
            // when currently hidden, be permissive and show immediately if nominal test passes
            in_view
        };

        if new_visible != currently_visible {
            if new_visible {
                commands.entity(entity).insert(Visibility::Visible);
            } else {
                commands.entity(entity).insert(Visibility::Hidden);
            }
        }
    }
}

fn cam_tf_forward(cam_tf: &GlobalTransform) -> Vec3 {
    cam_tf.forward().into()
}


#[cfg(test)]
mod tests {
    use super::*;
    use bevy::math::Vec3;

    #[test]
    fn chunk_in_front_is_visible() {
        let cam = Vec3::new(0.0, 1.6, 0.0);
        let fwd = Vec3::Z; // looking down +Z
        let chunk_min = Vec3::new(-8.0, 0.0, 8.0);
        let chunk_max = Vec3::new(8.0, 16.0, 24.0);
        assert!(chunk_in_view_cone(cam, fwd, chunk_min, chunk_max, 90.0, 100.0));
    }

    #[test]
    fn chunk_behind_is_not_visible() {
        let cam = Vec3::new(0.0, 1.6, 0.0);
        let fwd = Vec3::Z; // looking down +Z
        let chunk_min = Vec3::new(-8.0, 0.0, -24.0);
        let chunk_max = Vec3::new(8.0, 16.0, -8.0);
        assert!(!chunk_in_view_cone(cam, fwd, chunk_min, chunk_max, 90.0, 100.0));
    }

    #[test]
    fn far_away_chunk_is_not_visible() {
        let cam = Vec3::new(0.0, 1.6, 0.0);
        let fwd = Vec3::Z;
        // place chunk far beyond max_distance
        let chunk_min = Vec3::new(0.0, 0.0, 1000.0);
        let chunk_max = Vec3::new(16.0, 16.0, 1016.0);
        assert!(!chunk_in_view_cone(cam, fwd, chunk_min, chunk_max, 90.0, 200.0));
    }
}