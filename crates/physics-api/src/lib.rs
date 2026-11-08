//! Engine-independent physics types and the contract a physics implementation follows.
//!
//! This crate describes the physics operations needed by the game. It does not
//! perform physics itself and does not depend on Rapier or another engine.

mod error;
mod phantom_types;
mod physics_types;
mod primitive_shape;
mod shared_types;
mod world;

pub use error::PhysicsError;
pub use phantom_types::{PhantomBehavior, PhantomColliderDesc, PhantomOverlap, PhantomRayHit};
pub use physics_types::{
    CharacterDesc, CharacterMoveResult, DynamicBodyDesc, PhysicsHandle, RayHit, RayQuery,
    StaticColliderDesc,
};
pub use primitive_shape::PrimitiveShape;
pub use shared_types::{PhysicsTransform, Quaternion};
pub use world::PhysicsWorld;
