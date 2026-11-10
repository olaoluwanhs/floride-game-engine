//! Reusable, engine-independent gameplay systems.
//!
//! This crate contains shared mechanics such as character movement and kinematics.
//! It deliberately avoids game-specific rules (weapons, scores, specific items) and
//! does not impose a fixed command enum. Instead, systems consume generic intents
//! that individual games construct from their own player controls.

pub mod level;
pub mod movement;
pub mod spatial;
pub mod vitals;

pub use level::{
    LEVEL_WORLD_SCHEMA_VERSION, LevelColor, LevelDynamicObject, LevelMarker, LevelStaticObject,
    LevelTriggerVolume, LevelWorldSpec, SpawnedDynamicBody, SpawnedLevelWorld, SpawnedStaticObject,
    SpawnedTrigger, TerrainFloorOptions, TerrainPreset, generate_heightfield_floor,
    sample_heightfield_y, spawn_level_world,
};
pub use movement::{
    MovementConfig, MovementIntent, MovementState, step_character_movement, update_velocity,
};
pub use spatial::{apply_radial_impulse, has_line_of_sight, linear_falloff, steer_toward};
pub use vitals::{DamageOutcome, HealthState};
