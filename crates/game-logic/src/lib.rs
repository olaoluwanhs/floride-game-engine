//! Reusable, engine-independent gameplay systems.
//!
//! This crate contains shared mechanics such as character movement and kinematics.
//! It deliberately avoids game-specific rules (weapons, scores, specific items) and
//! does not impose a fixed command enum. Instead, systems consume generic intents
//! that individual games construct from their own player controls.

pub mod movement;

pub use movement::{
    MovementConfig, MovementIntent, MovementState, step_character_movement, update_velocity,
};
