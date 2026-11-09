use physics_api::{
    PhantomBehavior, PhantomColliderDesc, PhantomOverlap, PhysicsError, PhysicsHandle,
};
use rapier3d::prelude::{ActiveCollisionTypes, ColliderBuilder, Pose};

use crate::{
    conversions,
    world::{RapierWorld, WorldObject},
};

/// Creates a non-blocking Rapier sensor and records how the API wants to query it.
pub(super) fn create(
    world: &mut RapierWorld,
    description: &PhantomColliderDesc,
) -> Result<PhysicsHandle, PhysicsError> {
    conversions::validate_transform(&description.transform)?;
    conversions::validate_shape(&description.shape)?;
    let handle = world.allocate_handle()?;
    let (shape, local_offset) = conversions::shape_and_offset(&description.shape)?;
    let position =
        conversions::to_pose(&description.transform) * Pose::from_translation(local_offset);
    let collider = ColliderBuilder::new(shape)
        .position(position)
        .sensor(true)
        // Rapier does not enable every sensor/body pairing by default. These
        // flags let sensors see fixed terrain and kinematic characters too.
        .active_collision_types(
            ActiveCollisionTypes::default()
                | ActiveCollisionTypes::KINEMATIC_FIXED
                | ActiveCollisionTypes::FIXED_FIXED,
        )
        .build();
    let collider_handle = world.colliders.insert(collider);

    world
        .entities_by_collider
        .insert(collider_handle, Some(description.entity_id));
    world.phantom_colliders.insert(collider_handle);
    if matches!(
        description.behavior,
        PhantomBehavior::RaycastTarget | PhantomBehavior::OverlapSensorAndRaycastTarget
    ) {
        // Only these two behaviors should appear in phantom ray queries.
        world.phantom_ray_targets.insert(collider_handle);
    }
    world.objects.insert(
        handle,
        WorldObject::PhantomCollider {
            collider: collider_handle,
            local_offset,
        },
    );
    Ok(handle)
}

/// Updates the phantom's requested world transform while preserving the
/// shape-local offset used by heightfields.
pub(super) fn set_transform(
    world: &mut RapierWorld,
    handle: PhysicsHandle,
    transform: &physics_api::PhysicsTransform,
) -> Result<(), PhysicsError> {
    let (collider_handle, local_offset) = match world.objects.get(&handle) {
        Some(WorldObject::PhantomCollider {
            collider,
            local_offset,
        }) => (*collider, *local_offset),
        _ => return Err(PhysicsError::InvalidHandle(handle)),
    };
    conversions::validate_transform(transform)?;
    let collider = world
        .colliders
        .get_mut(collider_handle)
        .ok_or(PhysicsError::InvalidHandle(handle))?;
    let position = conversions::to_pose(transform) * Pose::from_translation(local_offset);
    collider.set_position(position);
    Ok(())
}

/// Removes a phantom and all of its adapter-side lookup entries.
pub(super) fn remove(world: &mut RapierWorld, handle: PhysicsHandle) -> Result<(), PhysicsError> {
    let collider_handle = match world.objects.get(&handle) {
        Some(WorldObject::PhantomCollider { collider, .. }) => *collider,
        _ => return Err(PhysicsError::InvalidHandle(handle)),
    };

    world
        .colliders
        .remove(
            collider_handle,
            &mut world.islands,
            &mut world.bodies,
            &mut world.soft_bodies,
            true,
        )
        .ok_or(PhysicsError::InvalidHandle(handle))?;
    world.objects.remove(&handle);
    world.entities_by_collider.remove(&collider_handle);
    world.phantom_colliders.remove(&collider_handle);
    world.phantom_ray_targets.remove(&collider_handle);
    Ok(())
}

/// Returns entity IDs currently intersecting this sensor.
///
/// Rapier updates its intersection graph during `step`; the results therefore
/// describe the last completed physics step.
pub(super) fn overlaps(
    world: &RapierWorld,
    handle: PhysicsHandle,
) -> Result<Vec<PhantomOverlap>, PhysicsError> {
    let phantom_collider = match world.objects.get(&handle) {
        Some(WorldObject::PhantomCollider { collider, .. }) => *collider,
        _ => return Err(PhysicsError::InvalidHandle(handle)),
    };
    let phantom_entity_id = world
        .entities_by_collider
        .get(&phantom_collider)
        .copied()
        .flatten()
        .ok_or(PhysicsError::InvalidHandle(handle))?;

    let mut overlaps = Vec::new();
    for (first, second, is_intersecting) in
        world.narrow_phase.intersection_pairs_with(phantom_collider)
    {
        if !is_intersecting {
            continue;
        }
        let other_collider = if first == phantom_collider {
            second
        } else {
            first
        };
        if let Some(Some(other_entity_id)) = world.entities_by_collider.get(&other_collider) {
            overlaps.push(PhantomOverlap {
                phantom_entity_id,
                other_entity_id: *other_entity_id,
            });
        }
    }
    // Several colliders may belong to one entity; return that entity once and
    // use a stable order so callers get predictable output.
    overlaps.sort_by_key(|overlap| overlap.other_entity_id.get());
    overlaps.dedup_by_key(|overlap| overlap.other_entity_id);
    Ok(overlaps)
}
