use game_types::{EntityId, Vec3};

use crate::{PhysicsTransform, PrimitiveShape};

/// Selects what a non-blocking phantom collider can detect.
///
/// A phantom collider has a shape and position, but it does not physically push
/// or stop other objects. Use an overlap behavior to notice entities inside it,
/// or a raycast behavior to notice rays passing through it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PhantomBehavior {
    /// Detects entities that are currently inside or touching this phantom.
    OverlapSensor,
    /// Can be found by a phantom raycast but does not detect overlaps.
    RaycastTarget,
    /// Detects both overlapping entities and rays passing through it.
    OverlapSensorAndRaycastTarget,
}

/// Description of a non-blocking shape that can still detect game interactions.
///
/// Unlike a static collider or dynamic body, a phantom does not block movement
/// or respond to physical forces. Its behavior decides whether it detects
/// overlaps, raycasts, or both. Every phantom has an entity ID so the game can
/// tell which object caused an interaction.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PhantomColliderDesc {
    /// The game-owned ID reported when this phantom is detected.
    pub entity_id: EntityId,
    /// The phantom's position and orientation in the world.
    pub transform: PhysicsTransform,
    /// The shape used to detect overlaps or ray intersections.
    pub shape: PrimitiveShape,
    /// Whether this phantom reacts to overlaps, raycasts, or both.
    pub behavior: PhantomBehavior,
}

/// An entity currently detected inside a phantom sensor.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PhantomOverlap {
    /// The ID of the phantom sensor.
    pub phantom_entity_id: EntityId,
    /// The ID of the other game entity inside or touching the sensor.
    pub other_entity_id: EntityId,
}

/// A ray intersection with a phantom that is configured as a raycast target.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PhantomRayHit {
    /// The ID of the phantom that the ray passed through.
    pub entity_id: EntityId,
    /// The point where the ray entered the phantom shape, in meters.
    pub point: Vec3,
    /// The direction perpendicular to the surface at the hit point.
    pub surface_normal: Vec3,
    /// The distance from the ray's origin to this intersection, in meters.
    pub distance: f32,
}

#[cfg(test)]
mod tests {
    use super::{PhantomBehavior, PhantomColliderDesc};
    use crate::{PhysicsTransform, PrimitiveShape};
    use game_types::{EntityId, Vec3};

    #[test]
    fn phantom_description_keeps_its_entity_and_behavior() {
        let phantom_id = EntityId::new(31);
        let phantom = PhantomColliderDesc {
            entity_id: phantom_id,
            transform: PhysicsTransform::IDENTITY,
            shape: PrimitiveShape::Sphere { radius: 2.0 },
            behavior: PhantomBehavior::OverlapSensorAndRaycastTarget,
        };

        assert_eq!(phantom.entity_id, phantom_id);
        assert_eq!(
            phantom.behavior,
            PhantomBehavior::OverlapSensorAndRaycastTarget
        );
        assert_eq!(phantom.transform.position, Vec3::ZERO);
    }
}
