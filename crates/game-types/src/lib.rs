//! Shared types used by the game core and its physics adapter.
//!
//! These types deliberately avoid renderer- and physics-engine-specific data
//! so different hosts can exchange the same game state.

mod entity_id;
mod vec3;

pub use entity_id::EntityId;
pub use vec3::Vec3;
