use game_types::{EntityId, Vec3};

use crate::{PhysicsTransform, PrimitiveShape};

/// An identifier for an object stored by a physics implementation.
///
/// A physics handle is different from an `EntityId`. The game owns an
/// `EntityId`; a physics implementation creates a `PhysicsHandle` when an
/// object is added to its world. Game code should keep the handle returned by
/// `PhysicsWorld` and pass it back to that same physics world. Handles are
/// temporary runtime values and must not be written into save files.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PhysicsHandle {
    value: u64,
}

impl PhysicsHandle {
    /// Creates a handle value.
    ///
    /// Physics implementations use this when they assign handles. Game code
    /// normally receives handles from methods such as `create_character` and
    /// should not create them itself.
    pub fn new(handle_value: u64) -> Self {
        Self {
            value: handle_value,
        }
    }

    /// Returns the number stored in this handle.
    pub fn get(&self) -> u64 {
        self.value
    }
}

/// Description of an immovable object in the world, such as a wall, barrier, or floor.
///
/// Give the collider an entity ID when game logic may need to identify it,
/// such as when it can take damage or break. Anonymous scenery can use `None`.
#[derive(Debug, Clone, PartialEq)]
pub struct StaticColliderDesc {
    /// The game-owned ID, or `None` when the collider is anonymous scenery.
    pub entity_id: Option<EntityId>,
    /// The shape's position and orientation in the world.
    pub transform: PhysicsTransform,
    /// The collision shape used for this object.
    pub shape: PrimitiveShape,
}

/// Settings for a movable physical object such as a box, barrel, or tire.
///
/// Unlike a static collider, a dynamic body can move when it falls, collides,
/// or receives an impulse. The physics world applies gravity and collisions to
/// it during each call to `step`. Mass is measured in kilograms. Friction must
/// be zero or greater; elasticity (bounciness) must be between zero and one.
/// All numeric values must be finite.
#[derive(Debug, Clone, PartialEq)]
pub struct DynamicBodyDesc {
    /// The game-owned ID used to refer to this object in ray hits and events.
    pub entity_id: EntityId,
    /// The object's starting position and orientation.
    pub transform: PhysicsTransform,
    /// The object's collision shape and dimensions.
    pub shape: PrimitiveShape,
    /// How much the object weighs, in kilograms. Must be positive.
    pub mass_kg: f32,
    /// How much the object's surface resists sliding against other surfaces.
    pub friction: f32,
    /// How much the object bounces when it collides with another surface.
    ///
    /// This is called the coefficient of restitution in physics documentation.
    pub elasticity: f32,
}

/// Description of a player or enemy moved with the character controller.
///
/// The game owns movement rules such as gravity and jump strength. The physics
/// implementation uses the selected shape for collision-aware character motion.
/// A capsule is a common choice for a player because its rounded ends move
/// smoothly over floors and small obstacles. The slope limit is in radians.
#[derive(Debug, Clone, PartialEq)]
pub struct CharacterDesc {
    /// The game-owned ID used to identify this character in ray hits.
    pub entity_id: EntityId,
    /// The character's starting position in meters.
    pub position: Vec3,
    /// The collision shape used for this character.
    pub shape: PrimitiveShape,
    /// The steepest slope, in radians, the character controller can climb.
    pub max_slope_angle_radians: f32,
    /// The maximum vertical step, in meters, the character controller can climb.
    pub step_height: f32,
}

/// The result of asking physics to move a character for one simulation step.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CharacterMoveResult {
    /// How far the physics implementation actually moved the character, in meters.
    pub applied_displacement: Vec3,
    /// Whether the character is touching ground after this movement.
    pub is_grounded: bool,
    /// Whether a collider blocked or redirected any of the requested movement.
    pub collided: bool,
}

/// Settings for a raycast, such as a weapon shot or line-of-sight check.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RayQuery {
    /// The ray's starting position in meters.
    pub origin: Vec3,
    /// The direction the ray travels. Implementations require this to be normalized.
    pub direction: Vec3,
    /// The maximum distance the ray travels, in meters.
    pub max_distance: f32,
    /// An entity to ignore, usually the entity that fired the ray.
    ///
    /// `Option<EntityId>` means this value can either contain an ID (`Some`) or
    /// contain no ID (`None`).
    pub ignored_entity: Option<EntityId>,
}

/// Information about the closest object hit by a raycast.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RayHit {
    /// The point where the ray touched the collider, in meters.
    pub point: Vec3,
    /// The direction perpendicular to the touched surface.
    pub surface_normal: Vec3,
    /// The distance from the ray's origin to the hit point, in meters.
    pub distance: f32,
    /// The game entity hit, or `None` when the collider has no game entity ID.
    ///
    /// Static walls and barriers can have IDs too, so game logic can identify
    /// them and decide whether they should react to a hit.
    pub entity_id: Option<EntityId>,
}

#[cfg(test)]
mod tests {
    use super::{CharacterDesc, DynamicBodyDesc, PhysicsHandle};
    use crate::{
        HeightFieldDesc, PhysicsTransform, PrimitiveShape, Quaternion, StaticColliderDesc,
    };
    use game_types::{EntityId, Vec3};

    #[test]
    fn physics_handle_returns_its_number() {
        let handle = PhysicsHandle::new(12);

        assert_eq!(handle.get(), 12);
    }

    #[test]
    fn identity_transform_is_at_origin_without_rotation() {
        let transform = PhysicsTransform::IDENTITY;

        assert_eq!(transform.position, Vec3::ZERO);
        assert_eq!(transform.rotation, Quaternion::IDENTITY);
    }

    #[test]
    fn dynamic_body_description_keeps_its_shape_and_mass() {
        let body = DynamicBodyDesc {
            entity_id: EntityId::new(7),
            transform: PhysicsTransform::IDENTITY,
            shape: PrimitiveShape::Box {
                half_extents: Vec3::new(0.5, 0.5, 0.5),
            },
            mass_kg: 2.0,
            friction: 0.5,
            elasticity: 0.1,
        };

        assert_eq!(body.entity_id, EntityId::new(7));
        assert_eq!(body.mass_kg, 2.0);
        assert_eq!(
            body.shape,
            PrimitiveShape::Box {
                half_extents: Vec3::new(0.5, 0.5, 0.5),
            }
        );
    }

    #[test]
    fn dynamic_body_and_character_can_use_heightfield_shapes() {
        let heightfield = HeightFieldDesc {
            rows: 2,
            columns: 2,
            height_samples: vec![0.0, 1.0, 1.0, 0.0],
            cell_size_x: 1.0,
            cell_size_z: 1.0,
        };
        let dynamic_body = DynamicBodyDesc {
            entity_id: EntityId::new(8),
            transform: PhysicsTransform::IDENTITY,
            shape: PrimitiveShape::HeightField(heightfield.clone()),
            mass_kg: 2.0,
            friction: 0.5,
            elasticity: 0.1,
        };
        let character = CharacterDesc {
            entity_id: EntityId::new(9),
            position: Vec3::ZERO,
            shape: PrimitiveShape::HeightField(heightfield),
            max_slope_angle_radians: 0.8,
            step_height: 0.25,
        };

        assert!(matches!(dynamic_body.shape, PrimitiveShape::HeightField(_)));
        assert!(matches!(character.shape, PrimitiveShape::HeightField(_)));
    }

    #[test]
    fn static_collider_description_keeps_its_entity_id() {
        let barrier_id = EntityId::new(21);
        let barrier = StaticColliderDesc {
            entity_id: Some(barrier_id),
            transform: PhysicsTransform::IDENTITY,
            shape: PrimitiveShape::Box {
                half_extents: Vec3::new(1.0, 2.0, 0.25),
            },
        };

        assert_eq!(barrier.entity_id, Some(barrier_id));
    }

    #[test]
    fn static_collider_can_use_a_heightfield_shape() {
        let ground = StaticColliderDesc {
            entity_id: None,
            transform: PhysicsTransform::IDENTITY,
            shape: PrimitiveShape::HeightField(HeightFieldDesc {
                rows: 2,
                columns: 2,
                height_samples: vec![0.0, 1.0, 1.0, 0.0],
                cell_size_x: 1.0,
                cell_size_z: 1.0,
            }),
        };

        assert!(matches!(ground.shape, PrimitiveShape::HeightField(_)));
    }

    #[test]
    fn static_collider_can_use_a_plane_primitive() {
        let ground = StaticColliderDesc {
            entity_id: None,
            transform: PhysicsTransform::IDENTITY,
            shape: PrimitiveShape::Plane {
                half_extents: Some(crate::PlaneHalfExtents {
                    half_width_x: 5.0,
                    half_depth_z: 3.0,
                }),
            },
        };

        assert_eq!(ground.transform.position, Vec3::ZERO);
    }

    #[test]
    fn static_collider_can_use_an_infinite_plane() {
        let ground = StaticColliderDesc {
            entity_id: None,
            transform: PhysicsTransform::IDENTITY,
            shape: PrimitiveShape::Plane { half_extents: None },
        };

        assert!(matches!(
            ground.shape,
            PrimitiveShape::Plane { half_extents: None }
        ));
    }
}
