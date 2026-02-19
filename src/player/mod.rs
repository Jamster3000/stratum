//! Player components and systems (camera, movement, physics).
//!
//! The module provides the `Player` component and convenience re-exports for
//! the player-related systems.
//!
//! # Example:
//!
//! ```
//! // spawn an entity with camera and player state
//! commands.spawn((
//!     Camera3dBundle::default(),
//!     Player { velocity: Vec3::ZERO, on_ground: true, flying: false },
//!     PlayerLook::default(),
//! ));
//! // register systems
//! app.add_system(camera_look);
//! app.add_system(camera_movement);
//! app.add_system(player_physics);
//! ```
pub mod camera;
pub mod movement;
pub mod physics;

use bevy::prelude::*;

pub use camera::*;
pub use movement::*;
pub use physics::*;

/// Component tracking player state used by movement and physics systems.
#[derive(Component)]
pub struct Player {
    /// Current player velocity in world units per second.
    pub velocity: Vec3,
    /// Whether the player is currently considered on the ground.
    pub on_ground: bool,
    /// Whether the player is in flying mode (disables gravity).
    pub flying: bool,
}
