//! Setup systems for initializing runtime resources.
//!
//! This module groups setup-related systems such as building/loading the
//! texture atlas, inserting chunk/world resources, and creating the shared
//! voxel material. These systems run at `Startup` and prepare data used by
//! other runtime systems.
use bevy::asset::AssetServer;
use bevy::prelude::*;
use stratum::atlas_builder::{AtlasBuilder, AtlasUVMap, AtlasTextureHandle};
use stratum::block::BlockRegistry;
use stratum::chunk::{ChunkEntities, MeshGenerationStats, PendingLodBuilds, LodStability};
use stratum::settings::Settings;
use std::sync::Arc;
use bevy::pbr::ExtendedMaterial;
use bevy::pbr::StandardMaterial;
use stratum::voxel_material::VoxelMaterial as VM;
use crate::TextureArrayReady;
use stratum::chunk::VoxelMaterialHandle;

/// Build the 2D atlas image from per-block textures and insert atlas resources.
///
/// This system invokes the `AtlasBuilder` to pack block textures into a single
/// atlas image, computes UV mappings, and inserts the following resources:
/// - `AtlasUVMap` with per-block UVs,
/// - `AtlasTextureHandle` (handle to the atlas image loaded into Bevy),
/// - `ChunkEntities`, `MeshGenerationStats`, `PendingLodBuilds`, `LodStability`.
///
/// # Arguments
/// - `commands`: Commands for inserting resources and spawning initial entities.
/// - `asset_server`: Used to load the produced atlas image into Bevy.
/// - `block_registry`: Registry of block types used to map textures to IDs.
#[allow(clippy::needless_pass_by_value)]
 pub fn setup_texture_array(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    block_registry: Res<BlockRegistry>,
    mut asset_paths: ResMut<stratum::debug::AssetPathRegistry>,
) {
    let texture_dir = std::path::Path::new("assets/textures/blocks");
    let atlas_output = std::path::Path::new("assets/textures/blocks/atlas.png");

    match AtlasBuilder::build_atlas_from_directory(texture_dir, atlas_output, Some(&block_registry))
    {
        Ok(atlas_info) => {
            let block_uvs = AtlasBuilder::map_blocks_to_atlas(&block_registry, &atlas_info);
            let uv_range = atlas_info.get_uv_range();
            let default_bounds = atlas_info.get_uv_bounds("default");
            let default_uvs = stratum::atlas_builder::BlockAtlasUVs {
                top: default_bounds,
                bottom: default_bounds,
                side: default_bounds,
            };

            commands.insert_resource(AtlasUVMap::new(
                Arc::new(block_uvs),
                uv_range,
                default_uvs,
            ));
            commands.insert_resource(ChunkEntities::default());
            commands.insert_resource(MeshGenerationStats::default());
            commands.insert_resource(PendingLodBuilds::default());
            commands.insert_resource(stratum::chunk::streaming::PendingMeshBuilds::default());
            commands.insert_resource(stratum::chunk::streaming::PendingMeshHandles::default());
            commands.insert_resource(stratum::chunk::streaming::MeshStreamingDiagnostics::default());
            commands.insert_resource(LodStability::default());

            let handle: Handle<Image> = asset_server.load("textures/blocks/atlas.png");
            asset_paths.0.insert(format!("{:?}", handle.clone()), "textures/blocks/atlas.png".to_string());
            commands.insert_resource(AtlasTextureHandle(handle));
        }
        Err(e) => {
            eprintln!("Failed to build atlas: {e}");
        }
    }
}

/// Create the shared voxel material once the atlas texture is ready. 
/// Waits for the `AtlasTextureHandle` resource is avaiable before setting it as 
/// the `StandardMaterial`.

/// # Arguments
/// - `commands`: Commands for inserting the `VoxelMaterialHandle` resource.
/// - `materials`: Asset storage for creating the `ExtendedMaterial` that includes the atlas texture.
/// - `atlas_texture`: Resource containing the handle to the atlas texture; required to create the material.
/// - `ready`: Mutable resource to track whether the voxel material has been created, preventing redundant creation.
/// - `existing_material`: Optional resource to check if the voxel material already exists, preventing redundant creation if the system runs multiple times.
/// - `settings`: Optional resource for accessing graphics settings that may influence material properties (e.g., ambient tint strength).
#[allow(clippy::needless_pass_by_value)]
pub fn setup_voxel_material(
    mut commands: Commands,
    mut materials: ResMut<Assets<ExtendedMaterial<StandardMaterial, VM>>>,
    atlas_texture: Option<Res<AtlasTextureHandle>>,
    mut ready: ResMut<TextureArrayReady>,
    existing_material: Option<Res<VoxelMaterialHandle>>,
    settings: Option<Res<Settings>>,
) {
    if ready.0 || existing_material.is_some() {
        return;
    }

    let Some(tex_handle) = atlas_texture else { return; };

    let tint_alpha = settings
        .as_ref()
        .map(|s| s.graphics.ambient_tint_strength)
        .unwrap_or(1.0);

    let base_ambient = Vec4::new(0.03, 0.03, 0.035, 0.75 * tint_alpha);

    let material = ExtendedMaterial {
        base: StandardMaterial {
            base_color: Color::WHITE,
            perceptual_roughness: 0.8,
            metallic: 0.0,
            ..default()
        },
        extension: VM {
            atlas_texture: tex_handle.0.clone(),
            ambient_tint: base_ambient,
        },
    };
    let mat_handle = materials.add(material);
    commands.insert_resource(VoxelMaterialHandle(mat_handle));
    ready.0 = true;
}

/// Perform initial synchronous world generation and spawn core entities.
///
/// This startup system generates a small local world (used for safe spawn
/// placement), inserts the generated `World` resource, spawns directional
/// lights for sun and skylight, the player camera, a player-local fill
/// light, and a non-emissive moon mesh.
///
/// # Arguments
/// - `commands`: Commands used to spawn entities and insert resources.
/// - `meshes`: Asset storage for creating meshes (moon sphere).
/// - `materials`: Asset storage for standard materials.
/// - `block_registry`: Registry used by terrain generation.
#[allow(clippy::needless_pass_by_value, clippy::cast_precision_loss)]
pub fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    block_registry: Res<BlockRegistry>,
) {
    let mut initial_world = stratum::world::World::new();
    let block_registry = (*block_registry).clone();
    for cx in -1..=1 {
        for cz in -1..=1 {
            let mut c = stratum::chunk::Chunk::new();
            c.generate(cx, cz, &block_registry);
            initial_world.chunks.insert((cx, cz), c);
        }
    }

    let mut spawn_y = 25.0f32;
    if let Some(center_chunk) = initial_world.chunks.get(&(0, 0)) {
        let max_h = i32::try_from(stratum::world::MAX_HEIGHT).expect("MAX_HEIGHT fits in i32");
        for y in (0..max_h).rev() {
            let b = center_chunk.get(0usize, usize::try_from(y).expect("y non-negative"), 0usize);
            if b != stratum::block::blocks::AIR {
                spawn_y = (y as f32) + 3.0;
                break;
            }
        }
    }

    commands.insert_resource(initial_world);

    commands.spawn((
        DirectionalLightBundle {
            directional_light: DirectionalLight {
                shadows_enabled: false,
                ..default()
            },
            ..default()
        },
        crate::Sun,
    ));

    commands.spawn((
        DirectionalLightBundle {
            directional_light: DirectionalLight {
                shadows_enabled: false,
                illuminance: 1200.0,
                color: Color::srgb(0.72, 0.78, 0.90),
                ..default()
            },
            transform: Transform::from_rotation(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
            ..default()
        },
        crate::Skylight,
    ));

    let _cam = commands
        .spawn((
            Camera3dBundle {
                transform: Transform::from_xyz(0.0, spawn_y, 0.0),
                ..default()
            },
            stratum::player::Player {
                velocity: Vec3::ZERO,
                on_ground: false,
                flying: false,
            },
            bevy_atmosphere::prelude::AtmosphereCamera::default(),
            stratum::player::PlayerLook::default(),
        ))
        .id();

    commands.spawn((
        PointLightBundle {
            point_light: PointLight {
                intensity: 4000.0,
                range: 60.0,
                color: Color::srgb(0.9, 0.92, 1.0),
                shadows_enabled: false,
                ..default()
            },
            transform: Transform::from_translation(Vec3::new(0.0, spawn_y, 0.0)),
            ..default()
        },
        crate::PlayerFillLight,
    ));

    stratum::ui::spawn_crosshair(&mut commands);

    commands.insert_resource(AmbientLight {
        color: Color::WHITE,
        brightness: 0.7,
    });

    commands.spawn((
        PbrBundle {
            mesh: meshes.add(Sphere { radius: 3.0 }.mesh().uv(8, 8)),
            material: materials.add(StandardMaterial {
                base_color: Color::srgb(0.9, 0.9, 1.0),
                emissive: LinearRgba::rgb(0.0, 0.0, 0.0),
                unlit: false,
                ..default()
            }),
            transform: Transform::from_xyz(0.0, -300.0, 0.0),
            ..default()
        },
        crate::Moon,
    ));
}
