//! User interface helpers: HUD, debug overlay and utilities.
//!
//! This module implements a simple debug overlay, an optional chunk grid
//! renderer for debugging, and spawning of a crosshair UI element. The
//! overlay periodically displays FPS, triangle counts, player position and
//! biome information.

use crate::player::Player;
use crate::world::World;
use bevy::diagnostic::{Diagnostic, DiagnosticsStore};
use bevy::prelude::*;
use crate::chunk::{CHUNK_DIM, CHUNK_LAYERS_Y};

/// State for the debug overlay visibility.
#[derive(Resource, Default)]
pub struct DebugOverlayState {
    /// Whether the overlay is currently visible.
    pub visible: bool,
}

#[derive(Resource, Default)]
pub struct DebugOverlayTimer(pub Timer);

#[derive(Resource, Default)]
pub struct DebugGridVisible(pub bool);

/// Insert debug overlay resources into the `Commands` world.
///
/// # Arguments
/// * `commands` - `Commands` to insert resources (timer, state, grid visibility)
pub fn setup_debug_overlay(mut commands: Commands) {
    commands.insert_resource(DebugOverlayTimer(Timer::from_seconds(
        0.5,
        TimerMode::Repeating,
    )));
    commands.insert_resource(DebugOverlayState::default());
    commands.insert_resource(DebugGridVisible::default());
}

/// Toggle the debug overlay visibility when F1 is pressed.
///
/// # Arguments
/// * `state` - mutable `DebugOverlayState` resource
/// * `input` - keyboard input resource
#[allow(clippy::needless_pass_by_value)]
pub fn toggle_debug_overlay(
    mut state: ResMut<DebugOverlayState>,
    input: Res<ButtonInput<KeyCode>>,
) {
    if input.just_pressed(KeyCode::F1) {
        state.visible = !state.visible;
    }
}

#[allow(clippy::needless_pass_by_value)]
pub fn toggle_debug_grid(mut grid: ResMut<DebugGridVisible>, input: Res<ButtonInput<KeyCode>>) {
    if input.just_pressed(KeyCode::F2) {
        grid.0 = !grid.0;
    }
}


/// Update the debug overlay text once every interval.
///
/// # Arguments
/// * `diagnostics` - diagnostics store (frame time / FPS)
/// * `state` - debug overlay visibility state
/// * `world` - optional `World` resource for chunk count
/// * `biome_registry` - to sample biome at player position
/// * `time` - time resource for timers
/// * `timer` - mutable overlay timer resource
/// * `query` - text query identifying the debug overlay UI text element
/// * `player_query` - query for player position and facing
/// * `mesh_stats` - optional mesh stats for triangle counts
#[derive(bevy::ecs::system::SystemParam)]
pub struct DebugOverlayCtx<'w, 's> {
    pub diagnostics: Res<'w, DiagnosticsStore>,
    pub state: Res<'w, DebugOverlayState>,
    pub world: Option<Res<'w, World>>,
    pub biome_registry: Res<'w, crate::biome::BiomeRegistry>,
    pub time: Res<'w, Time>,
    pub timer: ResMut<'w, DebugOverlayTimer>,
    pub query: Query<'w, 's, &'static mut Text, With<DebugOverlayText>>,
    pub player_query: Query<'w, 's, (&'static GlobalTransform, &'static Transform), With<Player>>,
    pub mesh_stats: Option<Res<'w, crate::chunk::MeshGenerationStats>>,
}

/// Constantly update the debug overlay text with debug information.
/// The overlay updates at a fixed interval to avoid the overhead
/// of querying diagnostics and world state every frame.
///
/// # Arguments
/// * `ctx` - system parameters grouped into a context struct for cleaner function signature
#[allow(clippy::cast_possible_truncation)]
pub fn update_debug_overlay(mut ctx: DebugOverlayCtx<'_, '_>) {
    if !ctx.timer.0.tick(ctx.time.delta()).just_finished() {
        return;
    }

    let Ok(mut text) = ctx.query.get_single_mut() else { return };

    if !ctx.state.visible {
        text.sections[0].value = String::new();
        return;
    }

    let fps = ctx
        .diagnostics
        .get(&bevy::diagnostic::FrameTimeDiagnosticsPlugin::FPS)
        .and_then(Diagnostic::smoothed)
        .unwrap_or(0.0);

    let frame_time = ctx
        .diagnostics
        .get(&bevy::diagnostic::FrameTimeDiagnosticsPlugin::FRAME_TIME)
        .and_then(Diagnostic::smoothed)
        .unwrap_or(0.0);

    let chunk_count = ctx.world.as_ref().map_or(0, |w| w.chunks.len());

    // Get player position and direction
    let (pos_str, direction) = if let Ok((global_transform, transform)) = ctx.player_query.get_single() {
        let pos = global_transform.translation();

        // Calculate compass direction from player's forward vector
        let forward = transform.forward();
        let angle = forward.x.atan2(forward.z).to_degrees();

        // Convert angle to compass direction
        let compass = if (-22.5..22.5).contains(&angle) {
            "E →"
        } else if (22.5..67.5).contains(&angle) {
            "SE ↘"
        } else if (67.5..112.5).contains(&angle) {
            "S ↓"
        } else if (112.5..157.5).contains(&angle) {
            "SW ↙"
        } else if !(-157.5..157.5).contains(&angle) {
            "W ←"
        } else if (-157.5..-112.5).contains(&angle) {
            "NW ↖"
        } else if (-112.5..-67.5).contains(&angle) {
            "N ↑"
        } else {
            "NE ↗"
        };

        // Get biome at player position
        let chunk_x = (pos.x / 32.0).floor() as i32;
        let chunk_z = (pos.z / 32.0).floor() as i32;
        let biome_name = ctx
            .biome_registry
            .get_biome_at(chunk_x, chunk_z)
            .map_or("unknown", |b| b.name.as_str());

        (
            format!("Pos: ({:.1}, {:.1}, {:.1})", pos.x, pos.y, pos.z),
            format!("Direction: {compass} | Biome: {biome_name}"),
        )
    } else {
        ("Pos: N/A".to_string(), "Direction: N/A".to_string())
    };

    let mesh_triangles = ctx.mesh_stats.as_ref().map_or(0, |s| s.total_triangles);
    let mesh_quads = mesh_triangles / 2;

    text.sections[0].value = format!(
        "FPS: {:.1}\nFrame Time: {:.2} ms\nChunks: {}\nTriangles: {} (Quads: {})\n{}\n{}",
        fps,
        frame_time * 1000.0,
        chunk_count,
        mesh_triangles,
        mesh_quads,
        pos_str,
        direction
    );
}

#[derive(Component)]
pub struct DebugOverlayText;

/// Render a wireframe chunk grid for debugging purposes.
///
/// # Arguments
/// * `commands` - `Commands` for spawning the grid UI elements.
/// * `asset_server` - asset server for loading fonts and textures.
/// * `asset_paths` - registry for mapping asset handles to paths for debugging.
#[allow(clippy::needless_pass_by_value)]
pub fn spawn_debug_overlay(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut asset_paths: ResMut<crate::debug::AssetPathRegistry>,
) {
    let font_handle: Handle<Font> = asset_server.load("fonts/OpenSans.ttf");
    asset_paths.0.insert(format!("{:?}", font_handle.clone()), "fonts/OpenSans.ttf".to_string());

    commands.spawn((
        TextBundle {
            text: Text::from_section(
                "",
                TextStyle {
                    font: font_handle,
                    font_size: 18.0,
                    color: Color::srgb(1.0, 1.0, 0.0),
                },
            ),
            style: Style {
                position_type: PositionType::Absolute,
                left: Val::Px(10.0),
                top: Val::Px(10.0),
                ..default()
            },
            ..default()
        },
        DebugOverlayText,
    ));
}

/// Render a wireframe chunk grid for debugging purposes.
///
/// # Arguments
/// * `grid` - `DebugGridVisible` resource controlling whether grid is shown
/// * `gizmos` - gizmo drawing context
/// * `world` - `World` resource providing chunk coordinates
#[allow(clippy::needless_pass_by_value)]
#[allow(clippy::cast_precision_loss, clippy::items_after_statements)]
pub fn render_chunk_grid(
    grid: Res<DebugGridVisible>,
    mut gizmos: Gizmos,
    world: Res<World>,
    player_query: Query<&GlobalTransform, With<Player>>,
) {
    if !grid.0 {
        return;
    }

    const CHUNK_SIZE_F32: f32 = 32.0;
    const GRID_RADIUS_CHUNKS: i32 = 12;      // tune (how many chunks around player to draw)
    const DETAILED_RADIUS: i32 = 2;          // draw per-layer only very close; otherwise draw a single column box.
    const MAX_RENDER_CHUNKS: usize = 1024;   // safety cap
    let green = Color::srgb(0.0, 1.0, 0.0);

    let world_height_blocks = (CHUNK_DIM * CHUNK_LAYERS_Y) as f32;
    let stack_base = -world_height_blocks * 0.5;

    // player-centred culling
    let (player_cx, player_cz) = player_query
        .get_single()
        .map(|t| {
            let p = t.translation();
            ((p.x / CHUNK_SIZE_F32).floor() as i32, (p.z / CHUNK_SIZE_F32).floor() as i32)
        })
        .unwrap_or((0, 0));

    let mut drawn = 0usize;
    for chunk_coords in world.chunks.keys() {
        if drawn >= MAX_RENDER_CHUNKS {
            break;
        }

        let dx = chunk_coords.0 - player_cx;
        let dz = chunk_coords.1 - player_cz;
        if dx.abs() > GRID_RADIUS_CHUNKS || dz.abs() > GRID_RADIUS_CHUNKS {
            continue;
        }

        let cx = chunk_coords.0 as f32;
        let cz = chunk_coords.1 as f32;
        let x_min = cx * CHUNK_SIZE_F32;
        let x_max = x_min + CHUNK_SIZE_F32;
        let z_min = cz * CHUNK_SIZE_F32;
        let z_max = z_min + CHUNK_SIZE_F32;

        let full_bottom = stack_base + 0.5;
        let full_top = stack_base + (CHUNK_LAYERS_Y as f32 * CHUNK_DIM as f32) + 0.5;

        // detailed per-layer only very close; otherwise draw a single column box.
        if dx.abs().max(dz.abs()) <= DETAILED_RADIUS {
            for layer in 0..CHUNK_LAYERS_Y {
                let layer_y = stack_base + (layer as f32 * CHUNK_DIM as f32) + 0.5;

                // bottom rect
                gizmos.line(Vec3::new(x_min, layer_y, z_min), Vec3::new(x_max, layer_y, z_min), green);
                gizmos.line(Vec3::new(x_max, layer_y, z_min), Vec3::new(x_max, layer_y, z_max), green);
                gizmos.line(Vec3::new(x_max, layer_y, z_max), Vec3::new(x_min, layer_y, z_max), green);
                gizmos.line(Vec3::new(x_min, layer_y, z_max), Vec3::new(x_min, layer_y, z_min), green);

                // top rect
                let y_top = layer_y + CHUNK_DIM as f32;
                gizmos.line(Vec3::new(x_min, y_top, z_min), Vec3::new(x_max, y_top, z_min), green);
                gizmos.line(Vec3::new(x_max, y_top, z_min), Vec3::new(x_max, y_top, z_max), green);
                gizmos.line(Vec3::new(x_max, y_top, z_max), Vec3::new(x_min, y_top, z_max), green);
                gizmos.line(Vec3::new(x_min, y_top, z_max), Vec3::new(x_min, y_top, z_min), green);

                // vertical edges
                gizmos.line(Vec3::new(x_min, layer_y, z_min), Vec3::new(x_min, y_top, z_min), green);
                gizmos.line(Vec3::new(x_max, layer_y, z_min), Vec3::new(x_max, y_top, z_min), green);
                gizmos.line(Vec3::new(x_max, layer_y, z_max), Vec3::new(x_max, y_top, z_max), green);
                gizmos.line(Vec3::new(x_min, layer_y, z_max), Vec3::new(x_min, y_top, z_max), green);
            }
        } else {
            // single bounding-box for the whole column
            // bottom rect
            gizmos.line(Vec3::new(x_min, full_bottom, z_min), Vec3::new(x_max, full_bottom, z_min), green);
            gizmos.line(Vec3::new(x_max, full_bottom, z_min), Vec3::new(x_max, full_bottom, z_max), green);
            gizmos.line(Vec3::new(x_max, full_bottom, z_max), Vec3::new(x_min, full_bottom, z_max), green);
            gizmos.line(Vec3::new(x_min, full_bottom, z_max), Vec3::new(x_min, full_bottom, z_min), green);

            // top rect
            gizmos.line(Vec3::new(x_min, full_top, z_min), Vec3::new(x_max, full_top, z_min), green);
            gizmos.line(Vec3::new(x_max, full_top, z_min), Vec3::new(x_max, full_top, z_max), green);
            gizmos.line(Vec3::new(x_max, full_top, z_max), Vec3::new(x_min, full_top, z_max), green);
            gizmos.line(Vec3::new(x_min, full_top, z_max), Vec3::new(x_min, full_top, z_min), green);

            // 4 vertical edges
            gizmos.line(Vec3::new(x_min, full_bottom, z_min), Vec3::new(x_min, full_top, z_min), green);
            gizmos.line(Vec3::new(x_max, full_bottom, z_min), Vec3::new(x_max, full_top, z_min), green);
            gizmos.line(Vec3::new(x_max, full_bottom, z_max), Vec3::new(x_max, full_top, z_max), green);
            gizmos.line(Vec3::new(x_min, full_bottom, z_max), Vec3::new(x_min, full_top, z_max), green);
        }

        // faint center guide line
        let alpha = if dx.abs().max(dz.abs()) <= DETAILED_RADIUS { 0.35 } else { 0.15 };
        let center_x = (x_min + x_max) * 0.5;
        let center_z = (z_min + z_max) * 0.5;
        gizmos.line(Vec3::new(center_x, full_bottom, center_z), Vec3::new(center_x, full_top, center_z), Color::srgba(0.0, 1.0, 0.0, alpha));

        drawn += 1;
    }
}

/// Spawn a crosshair UI element centered on the screen.
///
/// # Arguments
/// * `commands` - mutable `Commands` used to spawn UI nodes
pub fn spawn_crosshair(commands: &mut Commands) {
    commands
        .spawn(NodeBundle {
            style: Style {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            ..default()
        })
        .with_children(|p| {
            p.spawn(NodeBundle {
                style: Style {
                    position_type: PositionType::Absolute,
                    width: Val::Px(20.0),
                    height: Val::Px(2.0),
                    ..default()
                },
                background_color: Color::WHITE.into(),
                ..default()
            });
            p.spawn(NodeBundle {
                style: Style {
                    position_type: PositionType::Absolute,
                    width: Val::Px(2.0),
                    height: Val::Px(20.0),
                    ..default()
                },
                background_color: Color::WHITE.into(),
                ..default()
            });
        });
}
