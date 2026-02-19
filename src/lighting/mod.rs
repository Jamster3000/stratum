use bevy::prelude::*;

/// Result of the daylight math for a single time/sample.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DaylightInfo {
    pub solar: f32,                    // normalized solar altitude (0..1)
    pub is_night: bool,
    pub night_factor: f32,             // dusk->night interpolation (0..1)

    pub sun_color: Vec3,
    pub sun_illuminance: f32,
    pub shadows_enabled: bool,

    pub ambient_color: Vec3,
    pub ambient_brightness: f32,
    pub ambient_tint: Vec4,

    pub skylight_color: Vec3,
    pub skylight_illuminance: f32,
}

/// Smoothstep helper used by the daylight math.
#[inline]
pub fn smoothstep(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Compute the lighting parameters for a given `sun_height`.
///
/// - `sun_height` is expected to be the sine of the solar angle (range -1..1);
/// - `startup_complete` enables shadow logic that is only applied after
///   startup.
///
/// This is pure, deterministic math and is safe to call from benches/tests.
#[must_use]
pub fn compute_daylight(sun_height: f32, startup_complete: bool) -> DaylightInfo {
    let solar = (sun_height + 1.0) * 0.5;
    let dusk_u = ((0.15 - sun_height) / 0.20).clamp(0.0, 1.0);
    let night_factor = smoothstep(dusk_u);
    let is_night = sun_height < -0.05;

    // directional (sun) illuminance
    let day_illuminance = if sun_height < 0.06 {
        let tt = (sun_height + 0.06) / 0.12;
        400.0 + smoothstep(tt) * 400.0
    } else {
        let day_intensity = 1_200.0 + (sun_height.max(0.0).powf(1.8) * 3_500.0);
        day_intensity.min(8_000.0)
    };
    let sun_illuminance = day_illuminance * (1.0 - night_factor);

    // sun / day color interpolation
    let day_color = if sun_height < 0.15 {
        let t = smoothstep((sun_height + 0.05) / 0.20);
        let horizon = Vec3::new(1.0, 0.5, 0.3);
        let morning = Vec3::new(1.0, 0.85, 0.7);
        horizon.lerp(morning, t)
    } else if sun_height < 0.4 {
        let t = smoothstep((sun_height - 0.15) / 0.25);
        let morning = Vec3::new(1.0, 0.85, 0.7);
        let day = Vec3::new(1.0, 0.98, 0.95);
        morning.lerp(day, t)
    } else {
        Vec3::new(1.0, 0.98, 0.95)
    };
    let night_color = Vec3::new(0.6, 0.65, 0.85);
    let sun_color = day_color.lerp(night_color, night_factor);

    let shadows_enabled = startup_complete && sun_height > 0.08;

    // ambient color/brightness
    let ambient_color = if is_night {
        Vec3::new(0.04, 0.06, 0.10)
    } else {
        Vec3::new(0.95, 0.95, 1.0).lerp(sun_color, 0.08)
    };

    let mut ambient_brightness = if is_night {
        0.12
    } else if sun_height < 0.15 {
        let t = smoothstep((sun_height + 0.05) / 0.20);
        0.12 + t * 0.28
    } else {
        let day_ambient = 0.32 + (sun_height - 0.15) * 0.18;
        day_ambient.min(0.65)
    };

    if shadows_enabled && !is_night {
        ambient_brightness = ambient_brightness.max(0.2);
    }

    // ambient tint used by voxel material
    let base_dark = Vec3::splat(0.02);
    let shadow_rgb = base_dark * (1.0 + (1.0 - solar) * 0.5) + sun_color * 0.02;
    let alpha = 0.70 + (1.0 - solar) * 0.1;
    let ambient_tint = Vec4::new(shadow_rgb.x, shadow_rgb.y, shadow_rgb.z, alpha);

    // skylight (fill) color & illuminance
    let (skylight_color, skylight_illuminance) = if is_night {
        (Vec3::ZERO, 0.0)
    } else {
        let sky_fill_factor = 0.25 + sun_height.max(0.0) * 0.45;
        let sk_ill = ((ambient_brightness * 400.0).max(20.0)) * sky_fill_factor;
        let sk_col = ambient_color * 0.6 + Vec3::new(0.06, 0.07, 0.09);
        (sk_col, sk_ill)
    };

    DaylightInfo {
        solar,
        is_night,
        night_factor,
        sun_color,
        sun_illuminance,
        shadows_enabled,
        ambient_color,
        ambient_brightness,
        ambient_tint,
        skylight_color,
        skylight_illuminance,
    }
}