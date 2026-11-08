use physics_api::{PhysicsError, PhysicsHandle, StaticColliderDesc};
use rapier3d::prelude::{ColliderBuilder, Pose};

use crate::{
    conversions, validation,
    world::{RapierWorld, WorldObject},
};

/// Creates an immovable Rapier collider and remembers its public API handle.
pub(super) fn create(
    world: &mut RapierWorld,
    description: &StaticColliderDesc,
) -> Result<PhysicsHandle, PhysicsError> {
    validation::validate_static(description)?;
    let handle = world.allocate_handle()?;
    let (shape, local_offset) = conversions::shape_and_offset(&description.shape)?;
    // Compose the shape's local offset with its requested world transform.
    let position =
        conversions::to_pose(&description.transform) * Pose::from_translation(local_offset);
    let collider = ColliderBuilder::new(shape).position(position).build();
    let collider_handle = world.colliders.insert(collider);

    // Keep the game ID for later ray hits; None identifies anonymous scenery.
    world
        .entities_by_collider
        .insert(collider_handle, description.entity_id);
    world
        .objects
        .insert(handle, WorldObject::StaticCollider(collider_handle));
    Ok(handle)
}

/// Removes a static collider and its ID mapping; other handle kinds are rejected.
pub(super) fn remove(world: &mut RapierWorld, handle: PhysicsHandle) -> Result<(), PhysicsError> {
    let collider_handle = match world.objects.get(&handle) {
        Some(WorldObject::StaticCollider(collider_handle)) => *collider_handle,
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
    Ok(())
}
