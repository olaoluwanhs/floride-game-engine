use game_types::Vec3;

use crate::{
    CharacterDesc, CharacterMoveResult, DynamicBodyDesc, PhantomColliderDesc, PhantomOverlap,
    PhantomRayHit, PhysicsError, PhysicsHandle, PhysicsTransform, RayHit, RayQuery,
    StaticColliderDesc,
};

/// The physics operations needed by the game.
///
/// A physics engine adapter implements this trait. The game calls these named
/// methods without depending on the adapter's engine-specific types. Each
/// method returns a `Result`: success contains the requested value, while an
/// error explains why the operation could not be completed.
pub trait PhysicsWorld {
    /// Adds an immovable object with the supplied primitive shape.
    ///
    /// If its description has an entity ID, raycasts can return that ID when
    /// they hit this object so the game can react to the hit.
    fn create_static_collider(
        &mut self,
        description: &StaticColliderDesc,
    ) -> Result<PhysicsHandle, PhysicsError>;

    /// Removes an immovable object previously added to this world.
    fn remove_static_collider(
        &mut self,
        collider_handle: PhysicsHandle,
    ) -> Result<(), PhysicsError>;

    /// Adds a non-blocking phantom collider and returns its physics handle.
    fn create_phantom_collider(
        &mut self,
        description: &PhantomColliderDesc,
    ) -> Result<PhysicsHandle, PhysicsError>;

    /// Removes a phantom collider previously added to this world.
    fn remove_phantom_collider(
        &mut self,
        phantom_handle: PhysicsHandle,
    ) -> Result<(), PhysicsError>;

    /// Returns the entities currently inside or touching an overlap sensor.
    ///
    /// Use this with phantoms configured as `OverlapSensor` or
    /// `OverlapSensorAndRaycastTarget`. Each returned ID is a game entity, not a
    /// physics-engine handle.
    fn phantom_overlaps(
        &self,
        phantom_handle: PhysicsHandle,
    ) -> Result<Vec<PhantomOverlap>, PhysicsError>;

    /// Adds a movable object, such as a cardboard box or a tire.
    ///
    /// After it is created, gravity and collisions can move and rotate it during
    /// `step`. Use `body_transform` to read its current position and rotation.
    fn create_dynamic_body(
        &mut self,
        description: &DynamicBodyDesc,
    ) -> Result<PhysicsHandle, PhysicsError>;

    /// Removes a movable object previously added to this world.
    fn remove_dynamic_body(&mut self, body_handle: PhysicsHandle) -> Result<(), PhysicsError>;

    /// Applies a quick push to a movable object at its center of mass.
    ///
    /// The impulse changes the object's velocity. Its units are kilogram-meters
    /// per second. Use `apply_impulse_at_point` when the push should also make
    /// the object rotate.
    fn apply_impulse(
        &mut self,
        body_handle: PhysicsHandle,
        impulse: &Vec3,
    ) -> Result<(), PhysicsError>;

    /// Applies a quick push at a position in the world, which can also rotate the object.
    ///
    /// `world_position` is the point where the impact happens, in world meters.
    /// For example, a bullet hitting away from the center can both move and spin
    /// a cardboard box. The impulse units are kilogram-meters per second.
    fn apply_impulse_at_point(
        &mut self,
        body_handle: PhysicsHandle,
        impulse: &Vec3,
        world_position: &Vec3,
    ) -> Result<(), PhysicsError>;

    /// Adds a player or enemy with the supplied character collision shape.
    fn create_character(
        &mut self,
        description: &CharacterDesc,
    ) -> Result<PhysicsHandle, PhysicsError>;

    /// Removes a character previously added to this world.
    fn remove_character(&mut self, character_handle: PhysicsHandle) -> Result<(), PhysicsError>;

    /// Moves a character by a requested displacement, in meters, for this tick.
    ///
    /// The displacement is not a velocity. The implementation adjusts it to
    /// keep the character from passing through colliders and returns the
    /// movement that was actually applied. Call this before `step` for the
    /// current game tick.
    fn move_character(
        &mut self,
        character_handle: PhysicsHandle,
        desired_displacement: &Vec3,
    ) -> Result<CharacterMoveResult, PhysicsError>;

    /// Reports whether a character is touching the ground.
    fn is_grounded(&self, character_handle: PhysicsHandle) -> Result<bool, PhysicsError>;

    /// Returns the closest hit for a ray, or `None` when it hits nothing.
    ///
    /// The ray direction must have a length of one. Its maximum distance must
    /// be positive. This query returns physical colliders, not phantom colliders.
    /// The `Option<RayHit>` return value is `Some` when a physical object was hit
    /// and `None` when no physical object was hit.
    fn raycast(&self, query: &RayQuery) -> Result<Option<RayHit>, PhysicsError>;

    /// Returns every phantom ray intersection, ordered from nearest to farthest.
    ///
    /// Only phantoms configured as `RaycastTarget` or
    /// `OverlapSensorAndRaycastTarget` are included. These hits do not stop a
    /// physical raycast. The game can handle the phantom interactions and still
    /// call `raycast` to find the physical object behind them.
    fn raycast_phantoms(&self, query: &RayQuery) -> Result<Vec<PhantomRayHit>, PhysicsError>;

    /// Advances the physics simulation by one fixed time step, in seconds.
    ///
    /// Call this once per game tick, after applying character movement for that
    /// tick. The time step must be positive and finite; the game runtime chooses
    /// the fixed value (initially 1/60 of a second).
    fn step(&mut self, fixed_time_step_seconds: f32) -> Result<(), PhysicsError>;

    /// Returns the latest position and orientation for an object in this world.
    fn body_transform(&self, body_handle: PhysicsHandle) -> Result<PhysicsTransform, PhysicsError>;
}
