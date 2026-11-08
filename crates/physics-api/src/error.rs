use std::fmt;

use crate::PhysicsHandle;

/// An error returned when a physics operation cannot be completed.
///
/// Physics implementations use the same error type so the game does not need
/// to know which engine produced the error. An implementation should put a
/// useful, engine-independent explanation in `InvalidDescription` or
/// `BackendFailure`.
#[derive(Debug, Clone, PartialEq)]
pub enum PhysicsError {
    /// The supplied handle does not identify an object in this physics world.
    InvalidHandle(PhysicsHandle),
    /// A collider, character, ray, or other input contains invalid values.
    InvalidDescription(String),
    /// The requested simulation step is zero, negative, infinite, or NaN.
    InvalidTimeStep(f32),
    /// The physics implementation failed for another reason.
    BackendFailure(String),
}

impl fmt::Display for PhysicsError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PhysicsError::InvalidHandle(handle) => {
                write!(formatter, "unknown physics handle {}", handle.get())
            }
            PhysicsError::InvalidDescription(message) => {
                write!(formatter, "invalid physics input: {message}")
            }
            PhysicsError::InvalidTimeStep(time_step) => {
                write!(formatter, "invalid physics time step: {time_step}")
            }
            PhysicsError::BackendFailure(message) => {
                write!(formatter, "physics operation failed: {message}")
            }
        }
    }
}

impl std::error::Error for PhysicsError {}
