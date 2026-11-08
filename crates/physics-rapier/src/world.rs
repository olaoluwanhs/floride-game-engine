use std::collections::{HashMap, HashSet};

use game_types::{EntityId, Vec3};
use physics_api::{PhysicsError, PhysicsHandle};
use rapier3d::control::KinematicCharacterController;
use rapier3d::prelude::{
    BroadPhaseBvh, CCDSolver, ColliderHandle, ColliderSet, ImpulseJointSet, IntegrationParameters,
    IslandManager, MultibodyJointSet, NarrowPhase, PhysicsPipeline, RigidBodyHandle, RigidBodySet,
    SoftBodySet, Vector,
};

use crate::{characters, dynamic_bodies, phantom_colliders, queries, static_colliders, stepping};

/// A physics world backed by Rapier 3D.
///
/// `physics_api::PhysicsWorld` is a Rust trait: it is the shared interface the
/// game can call without knowing which physics engine is underneath. The
/// `impl physics_api::PhysicsWorld for RapierWorld` block below is the concrete
/// implementation of that interface, in the same role as a TypeScript class
/// declaring `implements PhysicsWorld`.
///
/// The fields hold Rapier's simulation state and the adapter's own lookup
/// tables. Keeping them together means each trait call operates on the same
/// physics world. The public methods come from the trait; this type also
/// provides constructors for the default or a custom gravity vector.
pub struct RapierWorld {
    /// Downward acceleration applied by Rapier to dynamic bodies.
    pub(crate) gravity: Vector,
    /// Coordinates the full simulation update each time `step` is called.
    pub(crate) pipeline: PhysicsPipeline,
    /// Holds simulation settings, including the current fixed step duration.
    pub(crate) integration_parameters: IntegrationParameters,
    /// Tracks active bodies during simulation.
    pub(crate) islands: IslandManager,
    /// Finds nearby collider pairs efficiently before precise collision checks.
    pub(crate) broad_phase: BroadPhaseBvh,
    /// Stores precise contacts and overlap results for colliding shapes.
    pub(crate) narrow_phase: NarrowPhase,
    /// Stores Rapier rigid bodies.
    pub(crate) bodies: RigidBodySet,
    /// Stores the collision shapes attached to bodies or placed in the world.
    pub(crate) colliders: ColliderSet,
    /// Rapier constraint collections required by its simulation pipeline.
    pub(crate) impulse_joints: ImpulseJointSet,
    pub(crate) multibody_joints: MultibodyJointSet,
    pub(crate) soft_bodies: SoftBodySet,
    /// Handles continuous collision detection during simulation steps.
    pub(crate) ccd_solver: CCDSolver,
    /// Connects public physics handles to the Rapier object(s) they identify.
    pub(crate) objects: HashMap<PhysicsHandle, WorldObject>,
    /// Recovers a game-owned entity ID from a Rapier collider hit or overlap.
    pub(crate) entities_by_collider: HashMap<ColliderHandle, Option<EntityId>>,
    /// Records which sensor colliders are phantom objects.
    pub(crate) phantom_colliders: HashSet<ColliderHandle>,
    /// Limits phantom raycasts to sensors explicitly configured as ray targets.
    pub(crate) phantom_ray_targets: HashSet<ColliderHandle>,
    /// Monotonically assigns adapter-owned handles rather than exposing Rapier handles.
    pub(crate) next_handle: u64,
}

/// The Rapier handle or handles associated with one public physics handle.
///
/// A dynamic body and a character each need both a rigid body and a collider.
/// A static collider and a phantom need only a collider. Keeping these
/// associations private to the adapter prevents Rapier handles leaking into
/// the game-facing API.
pub(crate) enum WorldObject {
    StaticCollider(ColliderHandle),
    PhantomCollider(ColliderHandle),
    DynamicBody {
        body: RigidBodyHandle,
        collider: ColliderHandle,
    },
    Character {
        body: RigidBodyHandle,
        collider: ColliderHandle,
        controller: KinematicCharacterController,
        is_grounded: bool,
    },
}

impl RapierWorld {
    /// Creates an empty world with gravity set to `(0, -9.81, 0)`.
    ///
    /// This is the approximate Earth-gravity default in the project's
    /// Y-up, meters-based coordinate system.
    pub fn new() -> Self {
        Self::from_gravity(Vector::new(0.0, -9.81, 0.0))
    }

    /// Creates an empty world with the supplied finite gravity vector.
    ///
    /// The vector is validated before conversion so NaN or infinite values
    /// cannot enter Rapier's simulation state.
    pub fn with_gravity(gravity: Vec3) -> Result<Self, PhysicsError> {
        crate::validation::validate_vec3(&gravity, "gravity")?;
        Ok(Self::from_gravity(crate::conversions::to_vector(&gravity)))
    }

    fn from_gravity(gravity: Vector) -> Self {
        // Each Rapier collection starts empty. The adapter's maps start empty
        // too, and the first public physics handle is 1 (zero stays unused).
        Self {
            gravity,
            pipeline: PhysicsPipeline::new(),
            integration_parameters: IntegrationParameters::default(),
            islands: IslandManager::new(),
            broad_phase: BroadPhaseBvh::new(),
            narrow_phase: NarrowPhase::new(),
            bodies: RigidBodySet::new(),
            colliders: ColliderSet::new(),
            impulse_joints: ImpulseJointSet::new(),
            multibody_joints: MultibodyJointSet::new(),
            soft_bodies: SoftBodySet::new(),
            ccd_solver: CCDSolver::new(),
            objects: HashMap::new(),
            entities_by_collider: HashMap::new(),
            phantom_colliders: HashSet::new(),
            phantom_ray_targets: HashSet::new(),
            next_handle: 1,
        }
    }

    pub(crate) fn allocate_handle(&mut self) -> Result<PhysicsHandle, PhysicsError> {
        let value = self.next_handle;
        // Refuse to wrap around: reusing an old number could make a stale
        // physics handle accidentally refer to a different object.
        self.next_handle = value
            .checked_add(1)
            .ok_or_else(|| PhysicsError::BackendFailure("physics handle space exhausted".into()))?;
        Ok(PhysicsHandle::new(value))
    }
}

impl Default for RapierWorld {
    fn default() -> Self {
        Self::new()
    }
}

// This is the key adapter boundary. The trait comes from `physics-api`, while
// `RapierWorld` and all Rapier-specific state live in this crate. The methods
// delegate to small modules so this trait implementation remains easy to scan.
impl physics_api::PhysicsWorld for RapierWorld {
    /// Creates a fixed Rapier collider and stores the returned handles.
    fn create_static_collider(
        &mut self,
        description: &physics_api::StaticColliderDesc,
    ) -> Result<PhysicsHandle, PhysicsError> {
        static_colliders::create(self, description)
    }

    /// Removes only a handle that identifies a static collider in this world.
    fn remove_static_collider(
        &mut self,
        collider_handle: PhysicsHandle,
    ) -> Result<(), PhysicsError> {
        static_colliders::remove(self, collider_handle)
    }

    /// Creates a non-blocking sensor for overlap detection and/or phantom rays.
    fn create_phantom_collider(
        &mut self,
        description: &physics_api::PhantomColliderDesc,
    ) -> Result<PhysicsHandle, PhysicsError> {
        phantom_colliders::create(self, description)
    }

    /// Removes a phantom sensor and its adapter lookup data.
    fn remove_phantom_collider(
        &mut self,
        phantom_handle: PhysicsHandle,
    ) -> Result<(), PhysicsError> {
        phantom_colliders::remove(self, phantom_handle)
    }

    /// Converts Rapier's sensor intersection pairs into game entity IDs.
    fn phantom_overlaps(
        &self,
        phantom_handle: PhysicsHandle,
    ) -> Result<Vec<physics_api::PhantomOverlap>, PhysicsError> {
        phantom_colliders::overlaps(self, phantom_handle)
    }

    /// Creates a dynamic rigid body and its collision shape.
    fn create_dynamic_body(
        &mut self,
        description: &physics_api::DynamicBodyDesc,
    ) -> Result<PhysicsHandle, PhysicsError> {
        dynamic_bodies::create(self, description)
    }

    /// Removes the dynamic body and its attached collider.
    fn remove_dynamic_body(&mut self, body_handle: PhysicsHandle) -> Result<(), PhysicsError> {
        dynamic_bodies::remove(self, body_handle)
    }

    /// Adds an instantaneous push at the body's center of mass.
    fn apply_impulse(
        &mut self,
        body_handle: PhysicsHandle,
        impulse: &Vec3,
    ) -> Result<(), PhysicsError> {
        dynamic_bodies::apply_impulse(self, body_handle, impulse)
    }

    /// Adds an instantaneous push at a world-space point, which can also rotate the body.
    fn apply_impulse_at_point(
        &mut self,
        body_handle: PhysicsHandle,
        impulse: &Vec3,
        world_position: &Vec3,
    ) -> Result<(), PhysicsError> {
        dynamic_bodies::apply_impulse_at_point(self, body_handle, impulse, world_position)
    }

    /// Creates a kinematic character; movement rules remain owned by the game.
    fn create_character(
        &mut self,
        description: &physics_api::CharacterDesc,
    ) -> Result<PhysicsHandle, PhysicsError> {
        characters::create(self, description)
    }

    /// Removes the character's kinematic body and collider.
    fn remove_character(&mut self, character_handle: PhysicsHandle) -> Result<(), PhysicsError> {
        characters::remove(self, character_handle)
    }

    /// Performs collision-aware movement for the requested displacement.
    fn move_character(
        &mut self,
        character_handle: PhysicsHandle,
        desired_displacement: &Vec3,
    ) -> Result<physics_api::CharacterMoveResult, PhysicsError> {
        characters::move_character(self, character_handle, desired_displacement)
    }

    /// Returns the grounded state from the character's latest movement check.
    fn is_grounded(&self, character_handle: PhysicsHandle) -> Result<bool, PhysicsError> {
        characters::is_grounded(self, character_handle)
    }

    /// Raycasts against solid colliders, excluding phantom sensors.
    fn raycast(
        &self,
        query: &physics_api::RayQuery,
    ) -> Result<Option<physics_api::RayHit>, PhysicsError> {
        queries::raycast(self, query)
    }

    /// Raycasts only against phantoms configured as ray targets.
    fn raycast_phantoms(
        &self,
        query: &physics_api::RayQuery,
    ) -> Result<Vec<physics_api::PhantomRayHit>, PhysicsError> {
        queries::raycast_phantoms(self, query)
    }

    /// Advances Rapier once by the caller-provided fixed timestep.
    fn step(&mut self, fixed_time_step_seconds: f32) -> Result<(), PhysicsError> {
        stepping::step(self, fixed_time_step_seconds)
    }

    /// Reads the current transform and converts it back to API-owned types.
    fn body_transform(
        &self,
        body_handle: PhysicsHandle,
    ) -> Result<physics_api::PhysicsTransform, PhysicsError> {
        dynamic_bodies::body_transform(self, body_handle)
    }
}
