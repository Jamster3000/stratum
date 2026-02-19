use bevy::diagnostic::{FrameTimeDiagnosticsPlugin, LogDiagnosticsPlugin};
use bevy::pbr::{ExtendedMaterial, MaterialPlugin, StandardMaterial};
use bevy::prelude::*;
use bevy::window::{PresentMode, Window, WindowPlugin};
use bevy_atmosphere::prelude::*;
use stratum::biome::loader as biome_loader;
use stratum::block::loader as block_loader;
use stratum::settings::loader as settings_loader;
use stratum::block::block_interaction;
use stratum::chunk::{stream_chunks, ChunkStreamingConfig, PendingChunks, StartupTimer};
use stratum::chunk::frustum::cull_chunk_entities_system;
use stratum::player::{camera_look, camera_movement, cursor_grab, player_physics};
use stratum::ui::{
    render_chunk_grid, setup_debug_overlay, spawn_debug_overlay,
    toggle_debug_grid, toggle_debug_overlay, update_debug_overlay,
};
use stratum::voxel_material::VoxelMaterial;

mod app;
use stratum::debug::DebugDumpPlugin;
use app::{
    ensure_atlas_sampler,
    setup_texture_array,
    setup_voxel_material,
    setup,
    daylight_cycle,
    update_player_fill_light,
};

#[derive(Component)]
struct Sun;

#[derive(Component)]
struct Moon;

#[derive(Component)]
struct Skylight;

#[derive(Component)]
struct PlayerFillLight;

// Game tick constants
pub const GAME_TICK_RATE: f32 = 20.0;
pub const FULL_DAY_SECONDS: f32 = 48.0 * 60.0;

#[derive(Resource)]
struct CycleTimer(Timer);

#[derive(Resource)]
struct TickTimer(Timer);

#[derive(Resource, Default)]
struct GameTicks { pub count: u64 }

#[derive(Resource, Default)]
struct TextureArrayReady(bool);

#[derive(Resource, Default)]
struct AtlasSamplerReady(bool);

fn game_tick_system(mut ticks: ResMut<GameTicks>, mut timer: ResMut<TickTimer>, time: Res<Time>) {
    if timer.0.tick(time.delta()).just_finished() {
        ticks.count = ticks.count.wrapping_add(1);
    }
}

fn main() {
    let settings = settings_loader::load_settings_from_dir("data/settings");
    let settings_watcher = settings_loader::setup_settings_watcher("data/settings")
        .unwrap_or_else(|_| settings_loader::SettingsWatcher::stub());

    let mut app = App::new();

    app.add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                position: WindowPosition::Centered(MonitorSelection::Primary),
                present_mode: PresentMode::AutoNoVsync,
                ..default()
            }),
            ..default()
        }))
        .add_plugins(MaterialPlugin::<
            ExtendedMaterial<StandardMaterial, VoxelMaterial>,
        >::default())
        .add_plugins(FrameTimeDiagnosticsPlugin)
        .add_plugins(LogDiagnosticsPlugin::default())
        .add_plugins(DebugDumpPlugin);

    // Asset path registry used by the debug dump to map handles -> source paths
    app.insert_resource(stratum::debug::AssetPathRegistry::default());

    if settings.atmosphere.enabled {
        app.add_plugins(AtmospherePlugin)
            .insert_resource(AtmosphereModel::default())
            .insert_resource(AtmosphereSettings {
                resolution: settings.atmosphere.resolution,
                dithering: settings.atmosphere.dithering,
                ..Default::default()
            });
    }

    app.insert_resource(ChunkStreamingConfig::default());
    app.insert_resource(PendingChunks::default());
    app.insert_resource(StartupTimer {
        elapsed: 0.0,
        startup_complete: false,
    });
    app.insert_resource(CycleTimer(Timer::from_seconds(0.10, TimerMode::Repeating)));
    app.insert_resource(TickTimer(Timer::from_seconds(1.0 / GAME_TICK_RATE, TimerMode::Repeating)));
    app.insert_resource(GameTicks::default());
    app.insert_resource(app::lighting::DaylightPrev::default());
    app.insert_resource(TextureArrayReady::default());
    app.insert_resource(AtlasSamplerReady::default());
    app.insert_resource(biome_loader::load_biomes_from_dir("data/biomes"));
    app.insert_resource(
        biome_loader::setup_biome_watcher("data/biomes").unwrap_or_else(|_| {
            biome_loader::BiomeWatcher::stub()
        }),
    );
    app.insert_resource(block_loader::load_blocks_from_dir("data/blocks"));
    app.insert_resource(
        block_loader::setup_block_watcher("data/blocks").unwrap_or_else(|_| {
            block_loader::BlockWatcher::stub()
        }),
    );

    app.insert_resource(settings.clone());
    app.insert_resource(settings_watcher);

    app.add_systems(Startup, setup_debug_overlay);
    app.add_systems(Startup, spawn_debug_overlay);
    app.add_systems(Startup, setup);
    app.add_systems(Startup, setup_texture_array);
    app.add_systems(PreUpdate, game_tick_system);
    app.add_systems(Update, setup_voxel_material);
    app.add_systems(Update, ensure_atlas_sampler);
    app.add_systems(Update, stream_chunks);
    app.add_systems(Update, cull_chunk_entities_system);
    app.add_systems(Update, toggle_debug_overlay);
    app.add_systems(Update, toggle_debug_grid);
    app.add_systems(Update, update_debug_overlay);
    app.add_systems(Update, render_chunk_grid);

    // Add daylight and atmosphere sync
    if settings.atmosphere.enabled {
        app.add_systems(Update, daylight_cycle);
        app.add_systems(Update, crate::app::sync_atmosphere_settings);
    }

    app.add_systems(Update, crate::app::sync_streaming_settings);
    app.add_systems(Update, crate::app::sync_vsync_settings);

    app.add_systems(Update, biome_loader::check_biome_changes);
    app.add_systems(Update, block_loader::check_block_changes);
    app.add_systems(Update, settings_loader::check_settings_changes);
    app.add_systems(Update, camera_movement);
    app.add_systems(Update, camera_look);
    app.add_systems(Update, cursor_grab);
    app.add_systems(Update, player_physics);
    app.add_systems(Update, block_interaction);
    app.add_systems(Update, update_player_fill_light);

    app.run();
}
