use criterion::{black_box, criterion_group, criterion_main, Criterion};
use std::path::Path;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use std::fs;

use stratum::atlas_builder::{AtlasBuilder, AtlasInfo, AtlasUVMap, BlockAtlasUVs};
use stratum::block::loader as block_loader;
use stratum::block::BlockRegistry;
use stratum::chunk::Chunk;
use stratum::world::World;
use stratum::player::Player;
use stratum::player::camera::PlayerLook;
use stratum::player::physics as player_physics_mod;

/// Test out small camera movement deltas
fn bench_camera_look_clamp(c: &mut Criterion) {
    c.bench_function("camera_look_clamp", |b| {
        b.iter(|| {
            let mut look = PlayerLook::default();
            // simulate many small mouse moves
            for i in 0..1_000usize {
                let dx = ((i * 13) % 17) as f32 * 0.1;
                let dy = ((i * 7) % 23) as f32 * 0.2 - 5.0;
                look.apply_delta(black_box(bevy::math::Vec2::new(dx, dy)));
            }
            black_box((look.yaw, look.pitch));
        })
    });
}

/// Test out large/extreme camera movement deltas
fn bench_camera_look_extreme(c: &mut Criterion) {
    c.bench_function("camera_look_extreme", |b| {
        b.iter(|| {
            let mut look = PlayerLook::default();
            // alternate very large movements to exercise clamps and signs
            for i in 0..1_000usize {
                let d = if (i & 1) == 0 { 1000.0 } else { -1000.0 };
                look.apply_delta(black_box(bevy::math::Vec2::new(d, -d)));
            }
            black_box((look.yaw, look.pitch));
        })
    });
}

/// Randomized camera movement deltas (deterministic LCG) to approximate variable input
fn bench_camera_look_random(c: &mut Criterion) {
    c.bench_function("camera_look_random", |b| {
        b.iter(|| {
            let mut look = PlayerLook::default();
            let mut state: u32 = 0x12345678;
            for _ in 0..1_000usize {
                state = state.wrapping_mul(1664525).wrapping_add(1013904223);
                let dx = (((state >> 16) & 0x7fff) as f32 / 32767.0) * 200.0 - 100.0;
                state = state.wrapping_mul(1664525).wrapping_add(1013904223);
                let dy = (((state >> 16) & 0x7fff) as f32 / 32767.0) * 200.0 - 100.0;
                look.apply_delta(black_box(bevy::math::Vec2::new(dx, dy)));
            }
            black_box((look.yaw, look.pitch));
        })
    });
}

/// Benchmark chunk generation for many chunks in a loop.
fn bench_chunk_generate(c: &mut Criterion) {
    let registry: BlockRegistry = block_loader::load_blocks_from_dir("data/blocks");

    c.bench_function("chunk_generate", |b| {
        b.iter(|| {
            for i in 0..100 {
                let mut cchunk = Chunk::new();
                cchunk.generate((i % 10) as i32, (i / 10) as i32, &registry);
                black_box(&cchunk);
            }
        })
    });
}

/// Lighting math microbenchmark — exercises the pure daylight computation
fn bench_lighting_math(c: &mut Criterion) {
    c.bench_function("lighting_math", |b| {
        b.iter(|| {
            for i in 0..1_000usize {
                let t = (i as f32 / 1_000.0) * std::f32::consts::TAU;
                let sun_h = t.sin();
                // exercise both startup=false and startup=true
                let _ = stratum::lighting::compute_daylight(black_box(sun_h), black_box(false));
                let _ = stratum::lighting::compute_daylight(black_box(sun_h), black_box(true));
            }
        })
    });
}

/// Mesh generation under different input densities / LODs
fn bench_mesh_variants(c: &mut Criterion) {
    let registry: BlockRegistry = block_loader::load_blocks_from_dir("data/blocks");

    // Construct atlas map (small deterministic atlas)
    let mut positions = std::collections::HashMap::new();
    positions.insert("dirt".to_string(), (0u32, 0u32, 0u32));
    positions.insert("grass_dirt_top".to_string(), (16u32, 0u32, 1u32));
    let atlas = AtlasInfo { width: 48, height: 16, tex_size: 16, texture_positions: positions };
    let block_uvs = AtlasBuilder::map_blocks_to_atlas(&registry, &atlas);
    let default_bounds = atlas.get_uv_bounds("default");
    let default_uvs = BlockAtlasUVs { top: default_bounds, bottom: default_bounds, side: default_bounds };
    let atlas_map = AtlasUVMap::new(Arc::new(block_uvs), atlas.get_uv_range(), default_uvs);

    let dirt_id = registry.id_for_name("dirt").unwrap_or(registry.missing_id());

    c.bench_function("mesh_variants_density", |b| {
        b.iter(|| {
            // empty chunk
            let empty = Chunk::new();
            black_box(empty.build_mesh(&registry, &atlas_map, 0));

            // solid chunk (no exposed faces)
            let mut solid = Chunk::new();
            for x in 0..stratum::chunk::CHUNK_SIZE {
                for y in 0..stratum::world::MAX_HEIGHT {
                    for z in 0..stratum::chunk::CHUNK_SIZE {
                        solid.set(x, y, z, dirt_id);
                    }
                }
            }
            black_box(solid.build_mesh(&registry, &atlas_map, 0));

            // checker pattern (many exposed faces)
            let mut checker = Chunk::new();
            for x in 0..stratum::chunk::CHUNK_SIZE {
                for z in 0..stratum::chunk::CHUNK_SIZE {
                    if (x + z) % 2 == 0 {
                        for y in 0..(stratum::chunk::CHUNK_SIZE / 2) {
                            checker.set(x, y, z, dirt_id);
                        }
                    }
                }
            }
            black_box(checker.build_mesh(&registry, &atlas_map, 0));
        })
    });
}

fn bench_mesh_lod_variants(c: &mut Criterion) {
    let registry: BlockRegistry = block_loader::load_blocks_from_dir("data/blocks");

    // Construct atlas map (small deterministic atlas)
    let mut positions = std::collections::HashMap::new();
    positions.insert("dirt".to_string(), (0u32, 0u32, 0u32));
    positions.insert("grass_dirt_top".to_string(), (16u32, 0u32, 1u32));
    let atlas = AtlasInfo { width: 48, height: 16, tex_size: 16, texture_positions: positions };
    let block_uvs = AtlasBuilder::map_blocks_to_atlas(&registry, &atlas);
    let default_bounds = atlas.get_uv_bounds("default");
    let default_uvs = BlockAtlasUVs { top: default_bounds, bottom: default_bounds, side: default_bounds };
    let atlas_map = AtlasUVMap::new(Arc::new(block_uvs), atlas.get_uv_range(), default_uvs);

    // Prepare a realistic generated chunk (heavy mesh)
    let mut heavy = Chunk::new();
    heavy.generate(0, 0, &registry);

    c.bench_function("mesh_lod_variants", |b| {
        b.iter(|| {
            for lod in 0..=3u8 {
                black_box(heavy.build_mesh(&registry, &atlas_map, lod));
            }
        })
    });
}

/// Benchmark mesh generation for a single chunk
fn bench_mesh_generation(c: &mut Criterion) {
    let registry: BlockRegistry = block_loader::load_blocks_from_dir("data/blocks");

    let mut positions = std::collections::HashMap::new();
    positions.insert("dirt".to_string(), (0u32, 0u32, 0u32));
    positions.insert("grass_dirt_top".to_string(), (16u32, 0u32, 1u32));
    positions.insert("grass_dirt_side".to_string(), (32u32, 0u32, 2u32));
    let atlas = AtlasInfo {
        width: 48,
        height: 16,
        tex_size: 16,
        texture_positions: positions,
    };

    let block_uvs = AtlasBuilder::map_blocks_to_atlas(&registry, &atlas);
    let default_bounds = atlas.get_uv_bounds("default");
    let default_uvs = BlockAtlasUVs { top: default_bounds, bottom: default_bounds, side: default_bounds };
    let atlas_map = AtlasUVMap::new(Arc::new(block_uvs), atlas.get_uv_range(), default_uvs);

    c.bench_function("mesh_generation_single_chunk", |b| {
        b.iter(|| {
            let mut chunk = Chunk::new();
            chunk.generate(0, 0, &registry);
            let (mesh, tri) = chunk.build_mesh(&registry, &atlas_map, 0);
            black_box((mesh, tri));
        })
    });
}

/// Benchmark the atlas building process from block textures on disk.
fn bench_atlas_build(c: &mut Criterion) {
    let texture_dir = Path::new("assets/textures/blocks");
    let tmp = std::env::temp_dir();
    let epoch = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
    let out = tmp.join(format!("stratum_bench_atlas_{}.png", epoch));

    c.bench_function("atlas_build_from_directory", |b| {
        b.iter(|| {
            let _ = AtlasBuilder::build_atlas_from_directory(texture_dir, &out, None).unwrap();
            // cleanup to avoid leaving large files
            let _ = fs::remove_file(&out);
            let _ = fs::remove_file(out.with_extension("ron"));
        })
    });
}

/// Benchmark loading and generating a large area of chunks to simulate startup streaming.
fn bench_chunk_streaming_startup(c: &mut Criterion) {
    let registry: BlockRegistry = block_loader::load_blocks_from_dir("data/blocks");

    c.bench_function("chunk_streaming_startup_17x17", |b| {
        b.iter(|| {
            let mut world = World::new();
            let radius: i32 = 8; // produces (2r+1)^2 = 289 chunks
            for cx in -radius..=radius {
                for cz in -radius..=radius {
                    let mut c = Chunk::new();
                    c.generate(cx, cz, &registry);
                    world.chunks.insert((cx, cz), c);
                }
            }
            black_box(&world);
        })
    });
}

/// Benchmark simulating many player physics steps in a generated world.
fn bench_player_physics_sim(c: &mut Criterion) {
    // Realistic physics stepping over a generated world
    let mut world = World::new();
    let registry: BlockRegistry = block_loader::load_blocks_from_dir("data/blocks");

    // Generate a small 5x5 area
    for cx in -2..=2 {
        for cz in -2..=2 {
            let mut c = Chunk::new();
            c.generate(cx, cz, &registry);
            world.chunks.insert((cx, cz), c);
        }
    }

    c.bench_function("player_physics_many_steps", |b| {
        b.iter(|| {
            let mut tf = bevy::prelude::Transform::from_xyz(0.0, 30.0, 0.0);
            let mut player = Player { velocity: bevy::prelude::Vec3::ZERO, on_ground: false, flying: false };
            let dt = 1.0f32 / 60.0f32;
            let kb = Default::default();

            for _ in 0..5_000 {
                player_physics_mod::physics_step(&mut tf, &mut player, &world, dt, &kb, bevy::prelude::KeyCode::Tab, bevy::prelude::KeyCode::Space);
            }

            black_box((tf, player));
        })
    });
}

#[test]
fn __bench_smoke_test() {
    // make sure test harness runs this file
    assert!(true);
}

fn bench_dummy(c: &mut Criterion) { c.bench_function("dummy", |b| b.iter(|| { black_box(1 + 1); })); }

criterion_group! {
    name = benches;
    config = Criterion::default().sample_size(200);
    targets =
        bench_dummy,
        bench_camera_look_clamp,
        bench_camera_look_extreme,
        bench_camera_look_random,
        bench_lighting_math,
        bench_chunk_generate,
        bench_mesh_generation,
        bench_mesh_variants,
        bench_mesh_lod_variants,
        bench_atlas_build,
        bench_chunk_streaming_startup,
        bench_player_physics_sim
}
criterion_main!(benches);
