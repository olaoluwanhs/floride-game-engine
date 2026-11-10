//! Reusable spatial, line-of-sight, steering, and radial impulse utilities.
//!
//! These helpers sit cleanly on top of `game-types` and `physics-api` without imposing
//! any game-specific weapon, AI, or command rules.

use game_types::{EntityId, Vec3};
use physics_api::{PhysicsError, PhysicsWorld, RayQuery};

use crate::movement::MovementIntent;

/// Computes continuous linear attenuation from `1.0` at `distance = 0.0`
/// smoothly down to `0.0` at `distance >= max_radius`.
pub fn linear_falloff(distance: f32, max_radius: f32) -> f32 {
    if !distance.is_finite() || !max_radius.is_finite() || max_radius <= 0.0 {
        return 0.0;
    }
    (1.0 - (distance / max_radius)).clamp(0.0, 1.0)
}

/// Pushes all dynamic bodies within `radius` meters outward from `origin`.
///
/// Uses continuous linear distance falloff and mixes in `upward_lift` (along `+Y`)
/// so resting props overcome surface friction realistically. Returns the count
/// of dynamic bodies that received an impulse.
pub fn apply_radial_impulse<P: PhysicsWorld>(
    physics: &mut P,
    origin: &Vec3,
    radius: f32,
    max_impulse: f32,
    upward_lift: f32,
    direct_hit_entity: Option<EntityId>,
) -> Result<usize, PhysicsError> {
    if radius <= 0.0 || max_impulse <= 0.0 {
        return Ok(0);
    }

    let mut nearby = physics.dynamic_bodies_in_radius(origin, radius)?;

    // Include a large dynamic body struck on its corner even if its center is slightly farther out.
    if let Some(direct_id) = direct_hit_entity {
        if !nearby.iter().any(|(id, _, _)| *id == direct_id) {
            if let Some(handle) = physics.dynamic_body_handle(direct_id) {
                if let Ok(transform) = physics.body_transform(handle) {
                    nearby.push((direct_id, handle, transform.position));
                }
            }
        }
    }

    let mut affected = 0usize;
    for (_entity_id, body_handle, body_pos) in nearby {
        let offset = body_pos.subtract(origin);
        let dist = offset.length();
        let falloff = linear_falloff(dist, radius);
        if falloff <= 0.0 {
            continue;
        }

        let base_dir = offset.normalized_or(Vec3::UP);
        let blast_dir = Vec3::new(base_dir.x, base_dir.y + upward_lift, base_dir.z)
            .normalized_or(Vec3::UP);
        let impulse = blast_dir.scale(max_impulse * falloff);
        if physics.apply_impulse(body_handle, &impulse).is_ok() {
            affected += 1;
        }
    }

    Ok(affected)
}

/// Checks whether an unobstructed line of sight exists from `from` to `to`.
///
/// Returns `Ok(true)` if no physical collider blocks the segment before `to`
/// (or if the first hit is `target_entity` itself).
pub fn has_line_of_sight<P: PhysicsWorld>(
    physics: &P,
    from: &Vec3,
    to: &Vec3,
    ignored_entity: Option<EntityId>,
    target_entity: Option<EntityId>,
) -> Result<bool, PhysicsError> {
    let delta = to.subtract(from);
    let distance = delta.length();
    if distance <= 1.0e-4 {
        return Ok(true);
    }

    let query = RayQuery {
        origin: *from,
        direction: delta.scale(distance.recip()),
        max_distance: distance,
        ignored_entity,
    };

    match physics.raycast(&query)? {
        None => Ok(true),
        Some(hit) => Ok(target_entity.is_some() && hit.entity_id == target_entity),
    }
}

/// Produces a [`MovementIntent`] steering horizontally from `current_pos` toward `target_pos`.
///
/// When already within `stop_distance` horizontally, returns a zero-direction idle intent.
pub fn steer_toward(
    current_pos: &Vec3,
    target_pos: &Vec3,
    stop_distance: f32,
    sprint: bool,
) -> MovementIntent {
    let flat_delta = Vec3::new(
        target_pos.x - current_pos.x,
        0.0,
        target_pos.z - current_pos.z,
    );
    let stop_threshold = stop_distance.max(0.0);
    if flat_delta.length_squared() <= stop_threshold * stop_threshold {
        return MovementIntent::default();
    }

    MovementIntent {
        desired_direction: flat_delta.normalized_or_zero(),
        sprint,
        jump: false,
    }
}

#[cfg(test)]
mod tests {
    use super::{apply_radial_impulse, has_line_of_sight, linear_falloff, steer_toward};
    use game_types::{EntityId, Vec3};
    use physics_api::{
        CharacterDesc, CharacterMoveResult, DynamicBodyDesc, PhantomColliderDesc, PhantomOverlap,
        PhantomRayHit, PhysicsError, PhysicsHandle, PhysicsTransform, PhysicsWorld, RayHit,
        RayQuery, StaticColliderDesc,
    };

    struct MockWorld {
        bodies: Vec<(EntityId, PhysicsHandle, Vec3)>,
        impulses: Vec<(PhysicsHandle, Vec3)>,
        next_hit: Option<RayHit>,
    }

    impl PhysicsWorld for MockWorld {
        fn create_static_collider(&mut self, _: &StaticColliderDesc) -> Result<PhysicsHandle, PhysicsError> {
            Ok(PhysicsHandle::new(1))
        }
        fn remove_static_collider(&mut self, _: PhysicsHandle) -> Result<(), PhysicsError> { Ok(()) }
        fn create_phantom_collider(&mut self, _: &PhantomColliderDesc) -> Result<PhysicsHandle, PhysicsError> {
            Ok(PhysicsHandle::new(2))
        }
        fn set_phantom_transform(&mut self, _: PhysicsHandle, _: &PhysicsTransform) -> Result<(), PhysicsError> { Ok(()) }
        fn remove_phantom_collider(&mut self, _: PhysicsHandle) -> Result<(), PhysicsError> { Ok(()) }
        fn phantom_overlaps(&self, _: PhysicsHandle) -> Result<Vec<PhantomOverlap>, PhysicsError> { Ok(Vec::new()) }
        fn create_dynamic_body(&mut self, _: &DynamicBodyDesc) -> Result<PhysicsHandle, PhysicsError> {
            Ok(PhysicsHandle::new(3))
        }
        fn remove_dynamic_body(&mut self, _: PhysicsHandle) -> Result<(), PhysicsError> { Ok(()) }
        fn apply_impulse(&mut self, handle: PhysicsHandle, impulse: &Vec3) -> Result<(), PhysicsError> {
            self.impulses.push((handle, *impulse));
            Ok(())
        }
        fn apply_impulse_at_point(&mut self, _: PhysicsHandle, _: &Vec3, _: &Vec3) -> Result<(), PhysicsError> { Ok(()) }
        fn create_character(&mut self, _: &CharacterDesc) -> Result<PhysicsHandle, PhysicsError> {
            Ok(PhysicsHandle::new(4))
        }
        fn remove_character(&mut self, _: PhysicsHandle) -> Result<(), PhysicsError> { Ok(()) }
        fn move_character(&mut self, _: PhysicsHandle, _: &Vec3) -> Result<CharacterMoveResult, PhysicsError> {
            Ok(CharacterMoveResult { applied_displacement: Vec3::ZERO, is_grounded: true, collided: false })
        }
        fn is_grounded(&self, _: PhysicsHandle) -> Result<bool, PhysicsError> { Ok(true) }
        fn raycast(&self, _: &RayQuery) -> Result<Option<RayHit>, PhysicsError> {
            Ok(self.next_hit)
        }
        fn raycast_phantoms(&self, _: &RayQuery) -> Result<Vec<PhantomRayHit>, PhysicsError> { Ok(Vec::new()) }
        fn step(&mut self, _: f32) -> Result<(), PhysicsError> { Ok(()) }
        fn body_transform(&self, _: PhysicsHandle) -> Result<PhysicsTransform, PhysicsError> {
            Ok(PhysicsTransform::IDENTITY)
        }
        fn dynamic_bodies_in_radius(&self, origin: &Vec3, radius: f32) -> Result<Vec<(EntityId, PhysicsHandle, Vec3)>, PhysicsError> {
            let max_sq = radius * radius;
            Ok(self
                .bodies
                .iter()
                .filter(|(_, _, pos)| pos.subtract(origin).length_squared() <= max_sq)
                .copied()
                .collect())
        }
    }

    #[test]
    fn linear_falloff_reduces_smoothly_to_zero() {
        assert!((linear_falloff(0.0, 10.0) - 1.0).abs() < 1e-6);
        assert!((linear_falloff(5.0, 10.0) - 0.5).abs() < 1e-6);
        assert_eq!(linear_falloff(10.0, 10.0), 0.0);
        assert_eq!(linear_falloff(15.0, 10.0), 0.0);
    }

    #[test]
    fn apply_radial_impulse_scales_by_distance() {
        let h_near = PhysicsHandle::new(10);
        let h_mid = PhysicsHandle::new(11);
        let mut world = MockWorld {
            bodies: vec![
                (EntityId::new(1), h_near, Vec3::new(2.0, 0.0, 0.0)),
                (EntityId::new(2), h_mid, Vec3::new(6.0, 0.0, 0.0)),
            ],
            impulses: Vec::new(),
            next_hit: None,
        };

        let affected = apply_radial_impulse(&mut world, &Vec3::ZERO, 10.0, 50.0, 0.45, None).unwrap();
        assert_eq!(affected, 2);
        assert!(world.impulses[0].1.length() > world.impulses[1].1.length());
    }

    #[test]
    fn line_of_sight_detects_walls_vs_target() {
        let target = EntityId::new(50);
        let wall = EntityId::new(99);

        let clear = MockWorld { bodies: Vec::new(), impulses: Vec::new(), next_hit: None };
        assert!(has_line_of_sight(&clear, &Vec3::ZERO, &Vec3::new(0.0, 0.0, 5.0), None, Some(target)).unwrap());

        let hit_target = MockWorld {
            bodies: Vec::new(),
            impulses: Vec::new(),
            next_hit: Some(RayHit { point: Vec3::ZERO, surface_normal: Vec3::UP, distance: 4.0, entity_id: Some(target) }),
        };
        assert!(has_line_of_sight(&hit_target, &Vec3::ZERO, &Vec3::new(0.0, 0.0, 5.0), None, Some(target)).unwrap());

        let blocked = MockWorld {
            bodies: Vec::new(),
            impulses: Vec::new(),
            next_hit: Some(RayHit { point: Vec3::ZERO, surface_normal: Vec3::UP, distance: 2.0, entity_id: Some(wall) }),
        };
        assert!(!has_line_of_sight(&blocked, &Vec3::ZERO, &Vec3::new(0.0, 0.0, 5.0), None, Some(target)).unwrap());
    }

    #[test]
    fn steer_toward_normalizes_and_stops_within_radius() {
        let intent = steer_toward(&Vec3::ZERO, &Vec3::new(3.0, 10.0, 4.0), 1.0, true);
        assert!(intent.sprint);
        assert!((intent.desired_direction.x - 0.6).abs() < 1e-5);
        assert!((intent.desired_direction.z - 0.8).abs() < 1e-5);
        assert_eq!(intent.desired_direction.y, 0.0);

        let stopped = steer_toward(&Vec3::ZERO, &Vec3::new(0.5, 5.0, 0.5), 1.5, true);
        assert_eq!(stopped.desired_direction, Vec3::ZERO);
        assert!(!stopped.sprint);
    }
}
