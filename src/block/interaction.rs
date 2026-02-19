//! Handle player interactions with blocks (breaking/placing) and updating chunk meshes accordingly
//! Performs raycasting from the player's view to determine which block is being targeted for interaction, and then updates the world state and rebuilds affected chunk meshes when blocks are added or removed.
//! This should only rebuild chunks that the player has changed/intereacted with (.e.g., break a block, that chunk needs to be rebuilt (don't rebuild all chunks))
//!
//! # Examples
//!
//! A small example that demonstrates how to use `raycast_block`. The example
//! constructs a minimal `World` and `BlockRegistry`, places a single block, and
//! verifies that `raycast_block` hits that block. This is compiled as a doctest.
//!
//! ```rust
//! use voxel_game::block::raycast_block;
//! use voxel_game::block::{BlockRegistry, Block};
//! use voxel_game::world::World;
//! use bevy::math::{Vec3, IVec3};
//!
//! // Create an empty world and a simple registry with one block (stone id=1).
//! let mut world = World::new();
//! let mut registry = BlockRegistry::default();
//! let mut stone = Block::default();
//! stone.name = "stone".to_string();
//! stone.id = 1;
//! registry.register(stone);
//!
//! // Place a block at (1,1,0).
//! world.set_block(1, 1, 0, registry.id_for_name("stone").unwrap(), &registry);
//!
//! // Raycast from z=-1 towards +z; should hit the placed block at (1,1,0).
//! let origin = Vec3::new(1.5, 1.5, -1.0);
//! let dir = Vec3::new(0.0, 0.0, 1.0);
//! let hit = raycast_block(&world, origin, dir, 10.0).expect("should hit block");
//! let (hit_pos, _place_pos) = hit;
//! assert_eq!(hit_pos, IVec3::new(1, 1, 0));
//! ```
use crate::atlas_builder::AtlasUVMap;
use crate::block::{blocks, BlockRegistry};
use crate::chunk::ChunkEntity;
use crate::chunk::VoxelMaterialHandle;
use crate::chunk::CHUNK_SIZE;
use crate::world::World;
use bevy::prelude::*;
use bevy::window::{CursorGrabMode, PrimaryWindow};

#[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
const CHUNK_SIZE_I32: i32 = CHUNK_SIZE as i32;

#[inline]
fn f32_floor_to_i32(v: f32) -> i32 {
    debug_assert!(
        v.is_finite() && (-2_147_483_648.0_f32..=2_147_483_647.0_f32).contains(&v),
        "coordinate out of i32 range",
    );
    #[allow(clippy::cast_possible_truncation)]
    {
        v.floor() as i32
    }
}



/// Performs raycasting from the player's view to determine 
/// which block is being targeted for interaction, and then updates the 
/// world state and rebuilds affected chunk meshes when blocks are added or removed.
///
/// # Arguments
/// * `world` - The game world containing block data and chunk information.
/// * `origin` - The starting point of the raycast (usually the player's camera position).
/// * `direction` - The direction vector of the raycast (usually the player's camera forward direction).
/// * `max_distance` - The maximum distance to check for block intersections (e.g., 5.0 for typical block interaction range).
///
/// # Returns
/// An `Option` containing a tuple of the hit block position and the adjacent air block position
///
/// # Example
/// ```
/// use voxel_game::block::raycast_block;
/// use voxel_game::world::World;
/// use voxel_game::block::{BlockRegistry, Block};
/// use bevy::math::{Vec3, IVec3};
///
/// let mut world = World::new();
/// let mut registry = BlockRegistry::default();
/// let mut stone = Block::default();
/// stone.name = "stone".to_string();
/// stone.id = 1;
/// registry.register(stone);
///
/// // place a block at (0,0,0)
/// world.set_block(0, 0, 0, registry.id_for_name("stone").unwrap(), &registry);
///
/// let origin = Vec3::new(0.5, 0.5, -1.0);
/// let dir = Vec3::new(0.0, 0.0, 1.0);
/// let (hit_pos, _place_pos) = raycast_block(&world, origin, dir, 10.0).expect("should hit");
/// assert_eq!(hit_pos, IVec3::new(0, 0, 0));
/// ```
#[must_use]
pub fn raycast_block(
    world: &World,
    origin: Vec3,
    direction: Vec3,
    max_distance: f32,
) -> Option<(IVec3, IVec3)> {
    let mut pos = origin;
    let step = direction.normalize() * 0.1;
    let mut last_air_pos = IVec3::new(
        f32_floor_to_i32(pos.x),
        f32_floor_to_i32(pos.y),
        f32_floor_to_i32(pos.z),
    );

    let mut distance = 0.0;
    while distance < max_distance {
        let block_pos = IVec3::new(
            f32_floor_to_i32(pos.x),
            f32_floor_to_i32(pos.y),
            f32_floor_to_i32(pos.z),
        );
        if world.get_block(block_pos.x, block_pos.y, block_pos.z) != blocks::AIR {
            return Some((block_pos, last_air_pos));
        }
        last_air_pos = block_pos;
        pos += step;
        distance += 0.1;
    }
    None
}

/// Calculates the position of the next block in a direction from origin.
/// This is primarily used for determining where to place a block
/// when the player right-clicks to place their chosen block.
///
/// # Arguments
/// * `origin` - The starting point (usually the player's camera position).
/// * `direction` - The direction vector (usually the player's camera forward direction).
///
/// # Returns
/// The block position (as `IVec3`) where a new block should be placed adjacent to
/// the block being targeted for interaction.
///
/// # Example
/// ```
/// use voxel_game::block::next_block_pos;
/// use bevy::math::{Vec3, IVec3};
///
/// let origin = Vec3::new(1.5, 1.5, -1.0);
/// let dir = Vec3::new(0.0, 0.0, 1.0);
/// let place_pos = next_block_pos(origin, dir);
/// // The ray moves a tiny bit forward in +z, so it stays in block (1, 1, -1).
/// let expected = IVec3::new(1, 1, -1);
/// if place_pos != expected {
///     panic!("next_block_pos returned {:?}, expected {:?}", place_pos, expected);
/// }
/// ```
#[must_use]
pub fn next_block_pos(origin: Vec3, direction: Vec3) -> IVec3 {
    let step = direction.normalize() * 0.1;
    let p = origin + step;
    IVec3::new(f32_floor_to_i32(p.x), f32_floor_to_i32(p.y), f32_floor_to_i32(p.z))
}

/// This function handles the player interactionss with blocks
/// (breaking with left-click, placing with right-click) and
/// updates the world state and rebuilds affected chunk meshes accordingly.
///
/// This is basically the main function that ties together
/// raycasting, world updates, chunk mesh rebuild and interaction logic.
///
/// # Arguments
/// * `mouse_button` - Resource tracking mouse button input state.
/// * `world` - Mutable reference to the game world for updating block data.
/// * `meshes` - Mutable reference to the asset collection for chunk meshes, used for updating meshes when blocks change.
/// * `camera_query` - Query to get the player's camera transform for raycasting.
/// * `chunk_query` - Query to find chunk entities for rebuilding meshes.
/// * `window_query` - Query to access the primary window for checking cursor state.
/// * `block_registry` - Resource containing block definitions, used for looking up block ids and
/// * `commands` - Commands for spawning/updating entities when rebuilding chunk meshes.
/// * `chunk_entities` - Resource tracking which chunk entities exist and their mesh handles, used for updating meshes when blocks change.
/// * `stats` - Resource for tracking mesh generation stats, updated when chunks are rebuilt.
/// * `layer_map` - Optional resource containing the atlas UV mapping, needed for rebuilding chunk meshes with correct texture coordinates.
/// * `material_handle` - Optional resource containing the voxel material handle, needed for rebuilding chunk meshes with the correct material.
///
/// # Example
/// ```rust
/// use voxel_game::block::raycast_block;
/// use voxel_game::block::{BlockRegistry, Block};
/// use voxel_game::world::World;
/// use bevy::math::{Vec3, IVec3};
///
/// let mut world = World::new();
/// let mut registry = BlockRegistry::default();
/// let mut stone = Block::default();
/// stone.name = "stone".to_string();
/// stone.id = 1;
/// registry.register(stone);
///
/// // place a block at (0,0,0)
/// world.set_block(0, 0, 0, registry.id_for_name("stone").unwrap(), &registry);
///
/// let origin = Vec3::new(0.5, 0.5, -1.0);
/// let dir = Vec3::new(0.0, 0.0, 1.0);
/// let (hit_pos, _place_pos) = raycast_block(&world, origin, dir, 10.0).expect("should hit");
/// assert_eq!(hit_pos, IVec3::new(0, 0, 0));
/// ```
#[derive(bevy::ecs::system::SystemParam)]
pub struct BlockInteractionCtx<'w, 's> {
    pub mouse_button: Res<'w, ButtonInput<MouseButton>>,
    pub world: ResMut<'w, World>,
    pub meshes: ResMut<'w, Assets<Mesh>>,
    pub block_registry: Res<'w, BlockRegistry>,
    pub chunk_entities: ResMut<'w, crate::chunk::streaming::ChunkEntities>,
    pub stats: ResMut<'w, crate::chunk::MeshGenerationStats>,
    pub layer_map: Option<Res<'w, AtlasUVMap>>,
    pub material_handle: Option<Res<'w, VoxelMaterialHandle>>,
    pub camera_query: Query<'w, 's, &'static Transform, With<Camera3d>>,
    pub chunk_query: Query<'w, 's, (&'static ChunkEntity, Entity)>,
    pub window_query: Query<'w, 's, &'static Window, With<PrimaryWindow>>,
    pub commands: Commands<'w, 's>,
}

/// Function to handle player interactions with blocks (breaking/placing)
/// and updating the world state and chunk meshes accordingly.
///
/// # Arguments
/// * `ctx` - A `BlockInteractionCtx` containing all necessary resources and queries for handling block interactions and updating chunk meshes.
pub fn block_interaction(mut ctx: BlockInteractionCtx) {
    let Some(layer_map) = ctx.layer_map.as_ref() else {
        return; // The atlas map isn't ready yet
    };

    let window = ctx.window_query.single();
    if window.cursor.grab_mode != CursorGrabMode::Locked {
        return;
    }

    let Some(mat_handle) = ctx.material_handle.as_ref() else {
        return;
    };

    let camera = ctx.camera_query.single();
    let direction = camera.forward();
    let origin = camera.translation;

    if let Some((hit_pos, place_pos)) = raycast_block(&ctx.world, origin, *direction, 5.0) {
        // Break block
        if ctx.mouse_button.just_pressed(MouseButton::Left) {
            let cx = hit_pos.x.div_euclid(CHUNK_SIZE_I32);
            let cz = hit_pos.z.div_euclid(CHUNK_SIZE_I32);
            if ctx.world.set_block(hit_pos.x, hit_pos.y, hit_pos.z, blocks::AIR, &ctx.block_registry)
                .is_some()
            {
                // Rebuild affected chunks
                rebuild_all_affected_chunks(
                    &ctx.world,
                    cx,
                    cz,
                    hit_pos,
                    &mut ctx.commands,
                    &mut ctx.meshes,
                    &mut ctx.chunk_query,
                    &ctx.block_registry,
                    layer_map,
                    mat_handle,
                    &mut ctx.chunk_entities,
                    &mut ctx.stats,
                );
            }
        }

        // Place block
        if ctx.mouse_button.just_pressed(MouseButton::Right) {
            let py = origin.y;
            let feet = f32_floor_to_i32(py - 1.7);
            let head = f32_floor_to_i32(py);
            let px = f32_floor_to_i32(origin.x);
            let pz = f32_floor_to_i32(origin.z);
            let intersect = place_pos.x == px
                && place_pos.z == pz
                && place_pos.y >= feet
                && place_pos.y <= head;

            if !intersect {
                let cx = place_pos.x.div_euclid(CHUNK_SIZE_I32);
                let cz = place_pos.z.div_euclid(CHUNK_SIZE_I32);

                // used as a temp feature for being able to place blocks
                // This will need to change at some point to allow placing a
                // variety of blocks rather than just dirt specifically
                let dirt_id = ctx
                    .block_registry
                    .id_for_name("dirt")
                    .unwrap_or(ctx.block_registry.missing_id());

                if ctx
                    .world
                    .set_block(
                        place_pos.x,
                        place_pos.y,
                        place_pos.z,
                        dirt_id,
                        &ctx.block_registry,
                    )
                    .is_some()
                {
                    rebuild_all_affected_chunks(
                        &ctx.world,
                        cx,
                        cz,
                        place_pos,
                        &mut ctx.commands,
                        &mut ctx.meshes,
                        &mut ctx.chunk_query,
                        &ctx.block_registry,
                        layer_map,
                        mat_handle,
                        &mut ctx.chunk_entities,
                        &mut ctx.stats,
                    );
                }
            }
        }
    }
}

/// Rebuilds the visual mesh for all chunks that are affected by block change
/// (e.g., the chunk containing the changed block and any adjacent chunks if the changed block is on a chunk boundary).
/// This function is called after a block is added or removed to ensure 
/// that the visual representation of the world is updated to reflect the change.
///
/// # Arguments
/// * `world` - The game world containing block data and chunk information.
/// * `chunk_x` - The x coordinate of the chunk containing the changed block.
/// * `chunk_z` - The z coordinate of the chunk containing the changed block.
/// * `block_pos` - The world position of the block that was changed, used to determine if adjacent chunks also need to be rebuilt.
/// * `commands` - Commands for spawning/updating entities when rebuilding chunk meshes.
/// * `meshes` - Mutable reference to the asset collection for chunk meshes, used for updating meshes when blocks change.
/// * `chunk_query` - Query to find chunk entities for rebuilding meshes.
/// * `block_registry` - Resource containing block definitions, used for looking up block ids and properties when rebuilding meshes.
/// * `layer_map` - Resource containing the atlas UV mapping, needed for rebuilding chunk meshes with correct texture coordinates.
/// * `material_handle` - Resource containing the voxel material handle, needed for rebuilding chunk meshes with the correct material.
/// * `chunk_entities` - Resource tracking which chunk entities exist and their mesh handles, used for updating meshes when blocks change.
/// * `stats` - Resource for tracking mesh generation stats, updated when chunks are rebuilt.
#[allow(clippy::too_many_arguments)]
fn rebuild_all_affected_chunks(
    world: &World,
    chunk_x: i32,
    chunk_z: i32,
    block_pos: IVec3,
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    chunk_query: &mut Query<(&ChunkEntity, Entity)>,
    block_registry: &BlockRegistry,
    layer_map: &AtlasUVMap,
    mat_handle: &VoxelMaterialHandle,
    chunk_entities: &mut crate::chunk::streaming::ChunkEntities,
    stats: &mut crate::chunk::MeshGenerationStats,
) {
    // Rebuild the visual mesh for the chunk containing the changed block
    rebuild_chunk_visual(
        world,
        chunk_x,
        chunk_z,
        commands,
        meshes,
        chunk_query,
        block_registry,
        layer_map,
        mat_handle,
        chunk_entities,
        stats,
    );

    let local_x = block_pos.x.rem_euclid(CHUNK_SIZE_I32);
    let local_z = block_pos.z.rem_euclid(CHUNK_SIZE_I32);

    if local_x == 0 {
        rebuild_chunk_visual(
            world,
            chunk_x - 1,
            chunk_z,
            commands,
            meshes,
            chunk_query,
            block_registry,
            layer_map,
            mat_handle,
            chunk_entities,
            stats,
        );
    }
    if local_x == (CHUNK_SIZE_I32 - 1) {
        rebuild_chunk_visual(
            world,
            chunk_x + 1,
            chunk_z,
            commands,
            meshes,
            chunk_query,
            block_registry,
            layer_map,
            mat_handle,
            chunk_entities,
            stats,
        );
    }
    if local_z == 0 {
        rebuild_chunk_visual(
            world,
            chunk_x,
            chunk_z - 1,
            commands,
            meshes,
            chunk_query,
            block_registry,
            layer_map,
            mat_handle,
            chunk_entities,
            stats,
        );
    }
    if local_z == (CHUNK_SIZE_I32 - 1) {
        rebuild_chunk_visual(
            world,
            chunk_x,
            chunk_z + 1,
            commands,
            meshes,
            chunk_query,
            block_registry,
            layer_map,
            mat_handle,
            chunk_entities,
            stats,
        );
    }
}

/// Rebuilds the visual mesh for a single chunk at the given chunk coordinates.
/// This is called for the chunk containing the changed block and any adjacent chunks if the changed block
/// is on a chunk boundary. It generates a new mesh based on the current block data in the world and updates the corresponding chunk entity with the new mesh.
///
/// # Arguments
/// * `world` - The game world containing block data and chunk information.
/// * `chunk_x` - The x coordinate of the chunk to rebuild.
/// * `chunk_z` - The z coordinate of the chunk to rebuild.
/// * `commands` - Commands for spawning/updating entities when rebuilding chunk meshes.
/// * `meshes` - Mutable reference to the asset collection for chunk meshes, used for updating meshes when blocks change.
/// * `chunk_query` - Query to find chunk entities for rebuilding meshes.
/// * `block_registry` - Resource containing block definitions, used for looking up block ids and properties when rebuilding meshes.
/// * `layer_map` - Resource containing the atlas UV mapping, needed for rebuilding chunk meshes with correct texture coordinates.
/// * `material_handle` - Resource containing the voxel material handle, needed for rebuilding chunk meshes with the correct material.
/// * `chunk_entities` - Resource tracking which chunk entities exist and their mesh handles, used for updating meshes when blocks change.
/// * `stats` - Resource for tracking mesh generation stats, updated when chunks are rebuilt.
#[allow(clippy::too_many_arguments)]
fn rebuild_chunk_visual(
    world: &World,
    chunk_x: i32,
    chunk_z: i32,
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    _chunk_query: &mut Query<(&ChunkEntity, Entity)>,
    block_registry: &BlockRegistry,
    layer_map: &AtlasUVMap,
    mat_handle: &VoxelMaterialHandle,
    chunk_entities: &mut crate::chunk::streaming::ChunkEntities,
    stats: &mut crate::chunk::MeshGenerationStats,
) {
    // Look for the chunk data in these given chunk coords
    // There's nothing to rebuild if chunk isn't loaded
    let Some(chunk) = world.chunks.get(&(chunk_x, chunk_z)) else {
        return;
    };

    // Build new mesh (include neighboring chunks snapshot for correct face culling)
    let mut neigh: std::collections::HashMap<(i32, i32), crate::chunk::Chunk> = std::collections::HashMap::new();
    for (dx, dz) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
        if let Some(n) = world.chunks.get(&(chunk_x + dx, chunk_z + dz)) {
            neigh.insert((chunk_x + dx, chunk_z + dz), n.clone());
        }
    }
    let (mesh, tri_count) = chunk.build_mesh(block_registry, layer_map, 0, (chunk_x, chunk_z), if neigh.is_empty() { None } else { Some(neigh) });

    // Update the mesh stats
    stats.update_chunk((chunk_x, chunk_z), tri_count);

    let max_lods = crate::chunk::MAX_LODS;

    if tri_count == 0 {
        if let Some((entity, handles, _)) = chunk_entities.map.remove(&(chunk_x, chunk_z)) {
            commands.entity(entity).despawn();
            for mh in handles.into_iter().flatten() { meshes.remove(&mh); }
        }
        return;
    }

    // Look up the entity & per-LOD mesh handle we store for this chunk
    // If an entry exists, we will update existing entity/mesh
    // otherwise spawn a new entity and insert a new entry
    if let Some((entity, handles, active_lod)) = chunk_entities.map.get_mut(&(chunk_x, chunk_z)) {
        if handles.len() < max_lods {
            handles.resize(max_lods, None);
        }

        // if there's already a mesh handle for LOD 0, replace the
        // Existing mesh asset in-place to avoidd any re-allocating handles
        if let Some(existing_handle) = handles[0].as_ref() {
            if let Some(existing_mesh) = meshes.get_mut(existing_handle) {
                *existing_mesh = mesh;
            } else {
                // The handle was found but the asset was not
                // add a new asset and store its handle
                let new_handle = meshes.add(mesh);
                handles[0] = Some(new_handle.clone());
                commands.entity(*entity).insert(new_handle.clone());
            }

            // Ensure the entity has mesh so bevy can render it
            commands
                .entity(*entity)
                .insert(handles[0].as_ref().unwrap().clone());
            
            // Mark LOD as 0 since this function would only be applied to chunks the player is close enough to
            *active_lod = 0;
        } else {
            let new_handle = meshes.add(mesh);
            handles[0] = Some(new_handle.clone());
            commands.entity(*entity).insert(new_handle.clone());
            *active_lod = 0;
        }
    } else {
        // Spawn new entity and record per-LOD handles
        let mesh_handle = meshes.add(mesh);
        let mut handles = vec![None; max_lods];
        handles[0] = Some(mesh_handle.clone());
        
        // World-space chunk origin (f32) — single binding avoids similar-name warnings
        #[allow(clippy::cast_precision_loss)]
        let chunk_world_pos = Vec3::new(
            (chunk_x * CHUNK_SIZE_I32) as f32,
            0.0,
            (chunk_z * CHUNK_SIZE_I32) as f32,
        );
        
        let entity = commands
            .spawn((
                MaterialMeshBundle {
                    mesh: mesh_handle.clone(),
                    material: mat_handle.0.clone(),
                        transform: Transform::from_xyz(
                            chunk_world_pos.x,
                            0.0,
                            chunk_world_pos.z,
                        ),
                    ..default()
                },
                ChunkEntity { chunk_x, chunk_z },
            ))
            .id();

        chunk_entities
            .map
            .insert((chunk_x, chunk_z), (entity, handles, 0));
    }
}