//! Module: Biome definitions and registry
//!
//! Biomes define terrain generation parameters, block types, weather patterns,
//! and other environmental factors. Biome data is typically loaded from RON
//! files in the `data/biomes` directory and can be hot-reloaded during runtime.
//! The `BiomeRegistry` is exposed as a resource-like structure providing access
//! to biome definitions and helper utilities for sampling weather and picking
//! structures.
//!
//! # Examples
//!
//! ```rust
//! use voxel_game::biome::{BiomeRegistry, Biome};
//!
//! // Create an empty registry and insert a default "plains" biome so the
//! // example is self-contained and works in rustdoc tests.
//! let mut registry = BiomeRegistry::default();
//! registry.biomes.insert("plains".to_string(), Biome::default());
//!
//! // Lookup the biome and sample weather deterministically using a value in
//! // the 0..=1 range.
//! if let Some(plains) = registry.get("plains") {
//!     if let Some(weather) = registry.sample_weather_by_value(plains, 0.5) {
//!         println!("Sampled weather: {}", weather);
//!     }
//! }
//! ```


use bevy::prelude::Resource;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Reference to a block, either by numeric id (legacy) or by name (preferred)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum BlockRef {
    Id(u8),
    Name(String),
}

/// Definition struct for an ore vein
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Ore {
    #[serde(default)]
    pub name: Option<String>, //name of the ore (use name in the ore's RON file)
    #[serde(default)]
    pub min_y: i32, // The minimum Y level at which this ore can generate at
    #[serde(default)]
    pub max_y: i32, // The maximum Y level at which this ore can generate at
    #[serde(default)]
    pub density: f32, // The density of the ore vein (e.g., how likely it is to generate in a given chunk)
    #[serde(default)]
    pub vein_size: u32, // The maximum number of ore blocks that can be generated in a single vein
}

/// Definition struct for a structure that can spawn in a biome
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct StructureDef {
    pub name: String,
    #[serde(default)]
    pub function: bool, // Whether this structure has a special function or behavior
    #[serde(default)]
    pub function_name: Option<String>, // If function is true, the name of the function to call when spawning this structure
    #[serde(default)]
    pub density: f32, // The density of the structure (e.g., how likely it is to generate in a given chunk)
}

/// Definition struct for what mobds can spawn in biome and how frequently/quantity
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MobSpawn {
    pub name: String,
    #[serde(default)]
    pub spawn_weight: u32, // Relative weight for how likely this mob is to spawn compared to others in the same biome
}

/// Main biome definition struct
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Biome {
    pub id: i32,
    pub name: String,

    // Terrain generation
    pub height_scale: f32, // Overall vertical scale of the terrain in this biome
    pub height_offset: f32, // Base height offset for the terrain in this biome
    pub noise_scale: f32, // Scale of the noise function used for terrain generation (higher = more zoomed out)
    pub noise_octaves: u32, // Number of noise octaves for terrain generation (higher = more detail)
    pub noise_persistence: f32, // Persistence value for noise generation (higher = more influence from higher octaves)
    pub noise_lacunarity: f32, // Lacunarity value for noise generation (higher = more frequency increase per octave)

    /// Basic Environmental parameters
    #[serde(default)]
    pub temperature: f32, // Simple numeric temperature in Celsius, used for weather and biome classification
    pub humidity: f32, // Simple numeric humidity in 0..=1 range, used for weather and biome classification

    /// Block refs
    #[serde(default)]
    pub surface_block: Option<BlockRef>,
    #[serde(default)]
    pub soil_block: Option<BlockRef>,
    #[serde(default)]
    pub rock_block: Option<BlockRef>,

    #[serde(default)]
    pub water_level: i32, // The Y level at which water is present in this biome

    #[serde(default)]
    pub weather_chance: HashMap<String, f32>, // Map of weather type to chance (0..=1) for sampling weather in this biome

    /// Layered blocks top -> bottom
    #[serde(default)]
    pub block_layers: Vec<String>, // List of block names to use as layers when generating terrain in this biome (e.g., ["grass", "dirt", "stone"])

    #[serde(default)]
    pub ores: Vec<Ore>, // List of ore definitions for ores that can generate in this biome

    #[serde(default)]
    pub structures: Vec<StructureDef>, // List of structures that can spawn in this biome, with density values for sampling

    #[serde(default)]
    pub mobs: Vec<MobSpawn>, // List of mobs that can spawn in this biome, with spawn weights for sampling

    #[serde(default)]
    pub vegetation_density: f32, // Density of vegetation in this biome (e.g., grass, flowers)
    #[serde(default)]
    pub tree_density: f32, // Density of trees in this biome
    #[serde(default)]
    pub cave_density: f32, // Density of caves in this biome
}

/// Presents a default biome to use (plains)
impl Default for Biome {
    fn default() -> Self {
        Self {
            id: 4,
            name: "plains".to_string(),
            height_scale: 64.0,
            height_offset: 64.0,
            noise_scale: 0.1,
            noise_octaves: 4,
            noise_persistence: 0.5,
            noise_lacunarity: 2.0,
            temperature: 15.0,
            humidity: 0.5,
            surface_block: None,
            soil_block: None,
            rock_block: None,
            water_level: 0,
            weather_chance: {
                let mut m = HashMap::new();
                m.insert("rain".to_string(), 0.4);
                m.insert("snow".to_string(), 0.1);
                m.insert("thunder".to_string(), 0.2);
                m.insert("clear".to_string(), 0.9);
                m
            },
            block_layers: vec!["dirt".to_string(), "stone".to_string()],
            ores: Vec::new(),
            structures: Vec::new(),
            mobs: Vec::new(),
            vegetation_density: 0.6,
            tree_density: 0.1,
            cave_density: 0.05,
        }
    }
}

/// Registry for biomes, providing lookup and sampling utilities
#[derive(Resource, Default)]
pub struct BiomeRegistry {
    pub biomes: HashMap<String, Biome>,
}

/// Helper methods for biome lookup and sampling
/// Provides utilities to get biomes by name, sample weather based on biome
/// definitions, and pick structures to spawn based on biome configuration.
impl BiomeRegistry {
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&Biome> {
        self.biomes.get(name)
    }

    #[must_use]
    pub fn get_biome_at(&self, x: i32, z: i32) -> Option<&Biome> {
        use noise::{NoiseFn, Perlin};

        let perlin = Perlin::new(1337);
        let biome_noise = perlin.get([(f64::from(x)) * 0.01, (f64::from(z)) * 0.01]);

        let biome_name = match biome_noise {
            n if n < -0.4 => "tundra",
            n if n < -0.1 => "forest",
            n if n < 0.2 => "plains",
            n if n < 0.5 => "desert",
            _ => "jungle",
        };

        self.biomes.get(biome_name)
    }
}

pub mod loader;
