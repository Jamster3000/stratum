//! Daylight and skylight related systems.
//!
//! This module handles the day/night cycle, updates directional and ambient
//! lighting, and writes the ambient tint into the shared voxel material so
//! rendered chunks receive consistent lighting across the scene.
//!
//! The main exported system is `daylight_cycle` and a small helper `smoothstep`.
use bevy::prelude::*;
use bevy_atmosphere::prelude::*;
use bevy::pbr::ExtendedMaterial;
use bevy::pbr::StandardMaterial;
use stratum::voxel_material::VoxelMaterial;
use stratum::chunk::VoxelMaterialHandle;
use crate::CycleTimer;
use crate::GameTicks;
use stratum::debug::SystemThreadLog;
use stratum::settings::Settings;

// Small cached previous-daylight state to avoid noisy GPU/material updates
#[derive(Resource, Default)]
pub struct DaylightPrev {
    pub sun_illuminance: f32,
    pub sun_color: Vec3,
    pub ambient_color: Vec3,
    pub ambient_brightness: f32,
    pub ambient_tint: Vec4,
    pub skylight_color: Vec3,
    pub skylight_illuminance: f32,
    pub shadows_enabled: bool,
}

// Factor the complex ParamSet into a type alias and group related system
// parameters into a `SystemParam` to reduce function-argument count
type CelestialQuerySet<'w, 's> = ParamSet<'w, 's, (
    Query<'w, 's, (&'static mut Transform, &'static mut DirectionalLight), With<crate::Sun>>,
    Query<'w, 's, &'static mut Transform, With<crate::Moon>>,
    Query<'w, 's, (&'static mut Transform, &'static mut DirectionalLight), With<crate::Skylight>>,
)>;

#[derive(bevy::ecs::system::SystemParam)]
pub struct DaylightCtx<'w, 's> {
    pub atmosphere: AtmosphereMut<'w, Nishita>,
    pub celestial: CelestialQuerySet<'w, 's>,
    pub timer: ResMut<'w, CycleTimer>,
    pub startup: Res<'w, crate::StartupTimer>,
    pub time: Res<'w, Time>,
    pub ambient: ResMut<'w, AmbientLight>,
    pub voxel_materials: Option<ResMut<'w, Assets<ExtendedMaterial<StandardMaterial, VoxelMaterial>>>>,
    pub material_handle: Option<Res<'w, VoxelMaterialHandle>>,
    pub player_light: Query<'w, 's, &'static mut PointLight, With<crate::PlayerFillLight>>,
    pub settings: Res<'w, Settings>,
    pub prev: ResMut<'w, DaylightPrev>,
}

/// Update sun/moon/skylight and the shared ambient tint each frame.
///
/// This system advances the in-game time, computes a smooth day/night
/// interpolation and updates:
/// - the directional `Sun` light transform, color and illuminance,
/// - the `Skylight` directional light parameters,
/// - the global ambient light color/brightness,
/// - the `ambient_tint` field of the shared `VoxelMaterial` (if present).
pub fn daylight_cycle(
    mut ctx: DaylightCtx<'_, '_>, 
    ticks: Res<GameTicks>, 
    sys_log: Option<ResMut<SystemThreadLog>>
) {
    if let Some(mut l) = sys_log {
        l.record("daylight_cycle");
    }

    ctx.timer.0.tick(ctx.time.delta());

    if ctx.timer.0.finished() {
        let ticks_per_day = (crate::FULL_DAY_SECONDS * crate::GAME_TICK_RATE) as u64;
        let tick_idx = ticks.count % ticks_per_day;
        let frac = (tick_idx as f32) / (ticks_per_day as f32);
        let t = frac * std::f32::consts::TAU;

        let sun_height = t.sin();
        let is_night_global = sun_height < -0.05;

        ctx.atmosphere.sun_position = Vec3::new(0., t.sin(), t.cos());

        let sun_y = t.sin() * 400.0 + 100.0;
        let sun_z = t.cos() * 400.0;

        let mut pending_sk_update: Option<(Quat, Vec3, f32)> = None;

        if let Ok((mut light_trans, mut directional)) = ctx.celestial.p0().get_single_mut() {
            // always update transform (cheap) but throttle expensive property writes
            light_trans.rotation = Quat::from_rotation_x(-t);

            // Compute daylight info (fast) but only write heavy state when it meaningfully changes
            let info = stratum::lighting::compute_daylight(sun_height, ctx.startup.startup_complete);

            // tolerances to avoid noisy updates that force GPU/material work
            const COLOR_EPS: f32 = 0.01;
            const ILLUM_EPS: f32 = 1.0;
            const AMBIENT_BRIGHTNESS_EPS: f32 = 0.01;
            const TINT_EPS: f32 = 0.005;

            // --- Directional light (sun) ---
            let mut dir_changed = false;
            let shadows_allowed = ctx.settings.graphics.shadows;
            let new_shadows_enabled = info.shadows_enabled && shadows_allowed;

            if (info.sun_illuminance - ctx.prev.sun_illuminance).abs() > ILLUM_EPS {
                dir_changed = true;
            }
            if (info.sun_color.x - ctx.prev.sun_color.x).abs().max((info.sun_color.y - ctx.prev.sun_color.y).abs()).max((info.sun_color.z - ctx.prev.sun_color.z).abs()) > COLOR_EPS {
                dir_changed = true;
            }
            if new_shadows_enabled != ctx.prev.shadows_enabled { dir_changed = true; }

            if dir_changed {
                directional.illuminance = info.sun_illuminance;
                directional.color = Color::srgb(info.sun_color.x, info.sun_color.y, info.sun_color.z);
                directional.shadows_enabled = new_shadows_enabled;
                ctx.prev.sun_illuminance = info.sun_illuminance;
                ctx.prev.sun_color = info.sun_color;
                ctx.prev.shadows_enabled = new_shadows_enabled;
            }

            // --- Ambient light (global) ---
            let ambient_color_changed = (info.ambient_color.x - ctx.prev.ambient_color.x).abs().max((info.ambient_color.y - ctx.prev.ambient_color.y).abs()).max((info.ambient_color.z - ctx.prev.ambient_color.z).abs()) > COLOR_EPS;
            let ambient_bright_changed = (info.ambient_brightness - ctx.prev.ambient_brightness).abs() > AMBIENT_BRIGHTNESS_EPS;
            if ambient_color_changed || ambient_bright_changed {
                ctx.ambient.color = Color::srgb(info.ambient_color.x, info.ambient_color.y, info.ambient_color.z);
                ctx.ambient.brightness = info.ambient_brightness;
                ctx.prev.ambient_color = info.ambient_color;
                ctx.prev.ambient_brightness = info.ambient_brightness;
            }

            // --- Voxel material ambient tint ---
            if let (Some(mats), Some(mat_handle)) = (ctx.voxel_materials.as_mut(), ctx.material_handle.as_ref())
                && let Some(mat) = mats.get_mut(&mat_handle.0) {
                    let mut at = info.ambient_tint;
                    // Apply global ambient tint strength from settings (alpha multiplier)
                    at.w *= ctx.settings.graphics.ambient_tint_strength;

                    let tint_diff = (at.x - ctx.prev.ambient_tint.x).abs()
                        .max((at.y - ctx.prev.ambient_tint.y).abs())
                        .max((at.z - ctx.prev.ambient_tint.z).abs())
                        .max((at.w - ctx.prev.ambient_tint.w).abs());

                    if tint_diff > TINT_EPS {
                        mat.extension.ambient_tint = at;
                        ctx.prev.ambient_tint = at;
                    }
                }

            // skylight update (kept, but we track to avoid noisy future writes)
            pending_sk_update = Some((Quat::from_rotation_x(std::f32::consts::FRAC_PI_2), info.skylight_color, info.skylight_illuminance));
            ctx.prev.skylight_color = info.skylight_color;
            ctx.prev.skylight_illuminance = info.skylight_illuminance;
        }

        if let Ok(mut moon_transform) = ctx.celestial.p1().get_single_mut() {
            moon_transform.translation = Vec3::new(0.0, -sun_y, -sun_z);
        }

        if let Some((rot, sk_color, sk_ill)) = pending_sk_update
            && let Ok((mut sk_trans, mut sk_dir)) = ctx.celestial.p2().get_single_mut() {
                sk_trans.rotation = rot;
                sk_dir.color = Color::srgb(sk_color.x, sk_color.y, sk_color.z);
                sk_dir.illuminance = sk_ill;
        }

        if let Ok(mut pl) = ctx.player_light.get_single_mut() {
            if is_night_global { pl.intensity = 800.0; pl.range = 20.0; } else { pl.intensity = 0.0; }
        }
    }
}


