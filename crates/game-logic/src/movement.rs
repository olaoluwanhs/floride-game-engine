//! Reusable character movement and kinematics.
//!
//! This module calculates velocity and displacement from general movement intents.
//! It does not read keyboard inputs directly and does not know about specific game
//! commands. Instead, a game translates its player actions into a [`MovementIntent`],
//! and this module calculates how the character accelerates, decelerates, jumps,
//! and falls.

use game_types::Vec3;
use physics_api::{CharacterMoveResult, PhysicsError, PhysicsHandle, PhysicsWorld};
use serde::{Deserialize, Serialize};

/// A game-neutral request describing how an entity wants to move during a tick.
///
/// Games convert their own input bindings or AI decisions into this struct.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MovementIntent {
    /// Normalized horizontal direction vector in world coordinates.
    ///
    /// For example, `(0, 0, 1)` represents forward and `(1, 0, 0)` represents right.
    /// The Y component is ignored because vertical movement is governed by jumping
    /// and gravity. If the entity wants to stand still, use [`Vec3::ZERO`].
    pub desired_direction: Vec3,

    /// Whether the entity is requesting to move at sprint speed.
    pub sprint: bool,

    /// Whether the entity is requesting to jump during this tick.
    pub jump: bool,
}

impl Default for MovementIntent {
    fn default() -> Self {
        Self {
            desired_direction: Vec3::ZERO,
            sprint: false,
            jump: false,
        }
    }
}

/// Tunable settings that define how responsive or heavy movement feels.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct MovementConfig {
    /// Normal walking speed on the ground, in meters per second.
    pub walk_speed: f32,

    /// Sprinting speed on the ground, in meters per second.
    pub sprint_speed: f32,

    /// How quickly the entity reaches desired speed on the ground, in meters per second squared.
    pub acceleration: f32,

    /// How quickly the entity slows to a stop on the ground with no input, in meters per second squared.
    pub friction: f32,

    /// Multiplier (between 0.0 and 1.0) applied to acceleration while airborne.
    pub air_control: f32,

    /// Initial upward velocity applied when jumping, in meters per second.
    pub jump_impulse: f32,

    /// Downward acceleration applied while airborne, in meters per second squared.
    pub gravity: f32,

    /// Maximum downward falling speed, in meters per second.
    pub terminal_velocity: f32,
}

impl Default for MovementConfig {
    fn default() -> Self {
        Self {
            walk_speed: 5.0,
            sprint_speed: 8.0,
            acceleration: 40.0,
            friction: 20.0,
            air_control: 0.2,
            jump_impulse: 6.0,
            gravity: 15.0,
            terminal_velocity: 40.0,
        }
    }
}

/// Dynamic movement properties preserved between simulation ticks.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MovementState {
    /// Current velocity vector in meters per second.
    pub velocity: Vec3,

    /// Whether the entity is currently resting on the ground.
    pub is_grounded: bool,
}

impl Default for MovementState {
    fn default() -> Self {
        Self {
            velocity: Vec3::ZERO,
            is_grounded: false,
        }
    }
}

impl MovementState {
    /// Creates a movement state with the specified velocity and grounded status.
    pub fn new(velocity: Vec3, is_grounded: bool) -> Self {
        Self {
            velocity,
            is_grounded,
        }
    }
}

/// Calculates the updated velocity for an entity based on its configuration, current state, and intent.
///
/// This is a pure mathematical calculation. It does not call into the physics engine
/// or modify game state.
pub fn update_velocity(
    config: &MovementConfig,
    state: &MovementState,
    intent: &MovementIntent,
    delta_time: f32,
) -> Vec3 {
    let new_horizontal = calculate_horizontal_velocity(config, state, intent, delta_time);
    let new_vertical = calculate_vertical_velocity(config, state, intent, delta_time);

    Vec3::new(new_horizontal.x, new_vertical, new_horizontal.z)
}

/// Moves a character through the physics world using the calculated movement kinematics.
///
/// This helper performs the full tick step for a character:
/// 1. Updates velocity via [`update_velocity`].
/// 2. Multiplies velocity by `delta_time` to determine desired displacement in meters.
/// 3. Calls [`PhysicsWorld::move_character`] to resolve collisions.
/// 4. Updates `state.is_grounded` and sets `state.velocity` according to collision results.
pub fn step_character_movement<P: PhysicsWorld>(
    physics: &mut P,
    handle: PhysicsHandle,
    config: &MovementConfig,
    state: &mut MovementState,
    intent: &MovementIntent,
    delta_time: f32,
) -> Result<CharacterMoveResult, PhysicsError> {
    let next_velocity = update_velocity(config, state, intent, delta_time);
    let desired_displacement = next_velocity.scale(delta_time);

    let move_result = physics.move_character(handle, &desired_displacement)?;

    state.is_grounded = move_result.is_grounded;

    // If touching the ground, prevent downward velocity from accumulating into the floor.
    if move_result.is_grounded && next_velocity.y < 0.0 {
        state.velocity = Vec3::new(next_velocity.x, 0.0, next_velocity.z);
    } else {
        state.velocity = next_velocity;
    }

    Ok(move_result)
}

fn calculate_horizontal_velocity(
    config: &MovementConfig,
    state: &MovementState,
    intent: &MovementIntent,
    delta_time: f32,
) -> Vec3 {
    let current_horizontal = Vec3::new(state.velocity.x, 0.0, state.velocity.z);
    let horizontal_direction = Vec3::new(intent.desired_direction.x, 0.0, intent.desired_direction.z);
    let direction_length_squared = horizontal_direction.length_squared();

    let has_directional_input = direction_length_squared > 0.0001;

    if has_directional_input {
        let target_speed = if intent.sprint {
            config.sprint_speed
        } else {
            config.walk_speed
        };

        // Normalize direction if it is not already unit length.
        let direction_length = direction_length_squared.sqrt();
        let normalized_direction = horizontal_direction.scale(1.0 / direction_length);
        let target_velocity = normalized_direction.scale(target_speed);

        let velocity_difference = target_velocity.subtract(&current_horizontal);
        let difference_length_squared = velocity_difference.length_squared();

        if difference_length_squared <= 0.0001 {
            return target_velocity;
        }

        let difference_length = difference_length_squared.sqrt();

        let base_acceleration = if state.is_grounded {
            config.acceleration
        } else {
            config.acceleration * config.air_control
        };

        let max_speed_change = base_acceleration * delta_time;

        if max_speed_change >= difference_length {
            target_velocity
        } else {
            let change_ratio = max_speed_change / difference_length;
            let velocity_delta = velocity_difference.scale(change_ratio);
            current_horizontal.add(&velocity_delta)
        }
    } else if state.is_grounded {
        // No directional input on the ground: apply friction to slow to a stop.
        let current_speed_squared = current_horizontal.length_squared();
        if current_speed_squared <= 0.0001 {
            return Vec3::ZERO;
        }

        let current_speed = current_speed_squared.sqrt();
        let friction_reduction = config.friction * delta_time;

        if friction_reduction >= current_speed {
            Vec3::ZERO
        } else {
            let remaining_speed_ratio = (current_speed - friction_reduction) / current_speed;
            current_horizontal.scale(remaining_speed_ratio)
        }
    } else {
        // Airborne with no input: maintain horizontal momentum.
        current_horizontal
    }
}

fn calculate_vertical_velocity(
    config: &MovementConfig,
    state: &MovementState,
    intent: &MovementIntent,
    delta_time: f32,
) -> f32 {
    if state.is_grounded && intent.jump {
        // Initiate jump from the ground.
        config.jump_impulse
    } else if state.is_grounded {
        // While resting on the ground without jumping, vertical velocity is zero.
        0.0
    } else {
        // Airborne: accelerate downward due to gravity, capped at terminal velocity.
        let current_y = state.velocity.y;
        let next_y = current_y - (config.gravity * delta_time);
        next_y.max(-config.terminal_velocity)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accelerates_from_rest_towards_walk_speed() {
        let config = MovementConfig {
            walk_speed: 10.0,
            acceleration: 20.0,
            ..Default::default()
        };
        let state = MovementState {
            velocity: Vec3::ZERO,
            is_grounded: true,
        };
        let intent = MovementIntent {
            desired_direction: Vec3::new(1.0, 0.0, 0.0),
            sprint: false,
            jump: false,
        };

        // After 0.1s at 20 m/s^2, speed should increase by 2.0 m/s along X.
        let updated_velocity = update_velocity(&config, &state, &intent, 0.1);
        assert!((updated_velocity.x - 2.0).abs() < 1e-4);
        assert_eq!(updated_velocity.y, 0.0);
        assert_eq!(updated_velocity.z, 0.0);
    }

    #[test]
    fn caps_at_target_walk_speed() {
        let config = MovementConfig {
            walk_speed: 5.0,
            acceleration: 100.0,
            ..Default::default()
        };
        let state = MovementState {
            velocity: Vec3::ZERO,
            is_grounded: true,
        };
        let intent = MovementIntent {
            desired_direction: Vec3::new(0.0, 0.0, 1.0),
            sprint: false,
            jump: false,
        };

        // Large delta time would overshoot without clamping; should clamp at walk_speed.
        let updated_velocity = update_velocity(&config, &state, &intent, 1.0);
        assert_eq!(updated_velocity.x, 0.0);
        assert_eq!(updated_velocity.y, 0.0);
        assert!((updated_velocity.z - 5.0).abs() < 1e-4);
    }

    #[test]
    fn applies_friction_to_stop_entity_when_no_input() {
        let config = MovementConfig {
            friction: 10.0,
            ..Default::default()
        };
        let state = MovementState {
            velocity: Vec3::new(4.0, 0.0, 0.0),
            is_grounded: true,
        };
        let intent = MovementIntent::default();

        // 0.2s of 10 m/s^2 friction reduces speed by 2.0 m/s (from 4.0 to 2.0).
        let step1 = update_velocity(&config, &state, &intent, 0.2);
        assert!((step1.x - 2.0).abs() < 1e-4);

        // Another 0.3s of 10 m/s^2 friction would reduce by 3.0 m/s, bringing it to a full stop (0.0).
        let state2 = MovementState {
            velocity: step1,
            is_grounded: true,
        };
        let step2 = update_velocity(&config, &state2, &intent, 0.3);
        assert_eq!(step2, Vec3::ZERO);
    }

    #[test]
    fn jump_triggers_impulse_only_when_grounded() {
        let config = MovementConfig {
            jump_impulse: 7.5,
            ..Default::default()
        };

        // Grounded jump succeeds.
        let grounded_state = MovementState {
            velocity: Vec3::ZERO,
            is_grounded: true,
        };
        let jump_intent = MovementIntent {
            jump: true,
            ..Default::default()
        };
        let velocity_jump = update_velocity(&config, &grounded_state, &jump_intent, 0.016);
        assert_eq!(velocity_jump.y, 7.5);

        // Mid-air jump attempt is ignored and gravity applies instead.
        let airborne_state = MovementState {
            velocity: Vec3::new(0.0, 5.0, 0.0),
            is_grounded: false,
        };
        let velocity_airborne = update_velocity(&config, &airborne_state, &jump_intent, 0.1);
        // y should be 5.0 - (15.0 * 0.1) = 3.5
        assert!((velocity_airborne.y - 3.5).abs() < 1e-4);
    }

    #[test]
    fn gravity_respects_terminal_velocity() {
        let config = MovementConfig {
            gravity: 20.0,
            terminal_velocity: 30.0,
            ..Default::default()
        };
        let state = MovementState {
            velocity: Vec3::new(0.0, -25.0, 0.0),
            is_grounded: false,
        };
        let intent = MovementIntent::default();

        // Falling for 1.0s at 20 m/s^2 would reach -45 m/s, but should clamp at -30 m/s.
        let updated = update_velocity(&config, &state, &intent, 1.0);
        assert_eq!(updated.y, -30.0);
    }

    #[test]
    fn air_control_reduces_steering_acceleration() {
        let config = MovementConfig {
            walk_speed: 10.0,
            acceleration: 40.0,
            air_control: 0.25, // 40.0 * 0.25 = 10.0 m/s^2 effective acceleration
            ..Default::default()
        };
        let state = MovementState {
            velocity: Vec3::ZERO,
            is_grounded: false,
        };
        let intent = MovementIntent {
            desired_direction: Vec3::new(1.0, 0.0, 0.0),
            ..Default::default()
        };

        // 0.1s at 10.0 m/s^2 = 1.0 m/s speed change in the air.
        let updated = update_velocity(&config, &state, &intent, 0.1);
        assert!((updated.x - 1.0).abs() < 1e-4);
    }
}
