//! Rapier-backed implementation of the engine-independent physics API.
//!
//! The `physics-api` crate defines the `PhysicsWorld` trait and the shared
//! descriptions such as `CharacterDesc` and `RayQuery`. This crate supplies
//! one concrete type, [`RapierWorld`], that implements that trait using Rapier.
//! Callers use the API types; Rapier-specific types stay inside this crate.
//!
//! The implementation is split by responsibility:
//! - `world` owns the Rapier simulation and implements the API trait.
//! - `static_colliders`, `dynamic_bodies`, `phantom_colliders`, and `characters`
//!   implement creation and operations for each kind of world object.
//! - `queries` handles physical and phantom raycasts.
//! - `stepping` advances the simulation.
//! - `conversions` and `validation` translate and check the API's data.

mod characters;
mod conversions;
mod dynamic_bodies;
mod phantom_colliders;
mod queries;
mod static_colliders;
mod stepping;
mod validation;
mod world;

pub use world::RapierWorld;
