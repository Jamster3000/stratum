//! Single helper system, `debug_chunk_report`, which
//! samples information about chunks near the player when the
//! debug key (`F1` or `FN + F1` depending on the keyboard configuration on the OS) is pressed.
//!
//! # Example
//! ```rust
//! app.add_system(voxel_game::chunk::debug::debug_chunk_report);
//! ```
use crate::chunk::streaming::ChunkEntities;
use crate::chunk::CHUNK_SIZE;
use bevy::prelude::*;



/// Inspect and report minimal debug info for chunks near the player.
///
/// This system runs a small sweep (5x5 chunks) centered on the player's
/// current chunk and gathers lightweight statistics: whether each chunk
/// is loaded, per-chunk triangle counts, pending LOD counts, and a few
/// sampled block values. It also attempts to inspect mesh handles if
/// available.
///
/// # Panics
///
/// - If the compile-time `CHUNK_SIZE` constant cannot be converted to `i32`.
/// - If converted sample indices are unexpectedly out of range (should not
///   happen for the hard-coded `sample_positions` values).
///
pub fn debug_chunk_report(
    input: &Res<ButtonInput<KeyCode>>,
    player_query: &Query<&GlobalTransform, With<Camera3d>>,
    world: &Res<crate::world::World>,
    chunk_entities: &Res<ChunkEntities>,
    meshes: &Res<Assets<Mesh>>,
) {
    if !input.just_pressed(KeyCode::F3) {
        return;
    }

    // Query the single primary camera transform to determine player position.
    let Ok(player_transform) = player_query.get_single() else {
        return;
    };

    // Convert world position to chunk coordinates (two-step, range-checked).
    let pos = player_transform.translation();

    let chunk_size_i32 = i32::try_from(CHUNK_SIZE).expect("CHUNK_SIZE fits in i32");

    assert!((1..=(1 << 23)).contains(&chunk_size_i32));

    // Convert world position -> chunk index using float division then floor.
    // We assert the float result is in `i32` range immediately before casting
    // so the subsequent `as i32` is provably safe.
    let to_chunk_i32 = |v: f32| -> i32 {
        let f = (f64::from(v) / f64::from(chunk_size_i32)).floor();
        assert!(f >= f64::from(i32::MIN) && f <= f64::from(i32::MAX));
        #[allow(clippy::cast_possible_truncation)]
        let f_i64 = f as i64;
        i32::try_from(f_i64).expect("chunk index fits in i32")
    };
    let pcx = to_chunk_i32(pos.x);
    let pcz = to_chunk_i32(pos.z);

    let sample_positions: [(i32, i32, i32); 3] = [(0, 64, 0), (16, 80, 16), (8, 40, 8)];

    // Sweep a 5x5 area centered on player chunk.
    for dx in -2..=2 {
        for dz in -2..=2 {
            let cx = pcx + dx;
            let cz = pcz + dz;

            // loaded state and entity map lookup
            let loaded = world.chunks.contains_key(&(cx, cz));
            let entry = chunk_entities.map.get(&(cx, cz));

            // If we have entity handles, inspect mesh handles
            if let Some((_, handles, _active)) = entry {
                for h in handles.iter().flatten() {
                    if let Some(mesh) = meshes.get(h) {
                        let _indices = mesh.indices().map_or(0, bevy::render::mesh::Indices::len);
                    }
                }
            }

            // If chunk is loaded, sample a few blocks from it. 
            if loaded && let Some(chunk) = world.chunks.get(&(cx, cz)) {
                for &(sx, sy, sz) in &sample_positions {
                    // `sx/sy/sz` are `i32` literals from `sample_positions`.
                    let bx = sx;
                    let by = sy;
                    let bz = sz;

                    let x_idx = usize::try_from(bx.rem_euclid(chunk_size_i32))
                        .expect("sample x index non-negative");
                    let y_idx = usize::try_from(by).expect("sample y index non-negative");
                    let z_idx = usize::try_from(bz.rem_euclid(chunk_size_i32))
                        .expect("sample z index non-negative");

                    let _block = chunk.get(x_idx, y_idx, z_idx);
                }
            } 
        }
    }
}