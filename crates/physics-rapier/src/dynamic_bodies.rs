use game_types::Vec3;
use physics_api::{DynamicBodyDesc, PhysicsError, PhysicsHandle, PhysicsTransform};
use rapier3d::prelude::{ColliderBuilder, Pose, RigidBodyBuilder, RigidBodyHandle};

use crate::{
    conversions, validation,
    world::{RapierWorld, WorldObject},
};

/// Creates a moving Rapier body and attaches its collision shape.
pub(super) fn create(
    world: &mut RapierWorld,
    description: &DynamicBodyDesc,
) -> Result<PhysicsHandle, PhysicsError> {
    validation::validate_dynamic(description)?;
    let handle = world.allocate_handle()?;
    let (shape, local_offset) = conversions::shape_and_offset(&description.shape)?;
    let body_position = conversions::to_pose(&description.transform);
    let body = RigidBodyBuilder::dynamic().pose(body_position).build();
    let body_handle = world.bodies.insert(body);
    let collider = ColliderBuilder::new(shape)
        // The collider supplies the mass and surface properties; its parent
        // rigid body owns the object's motion.
        .position(Pose::from_translation(local_offset))
        .mass(description.mass_kg)
        .friction(description.friction)
        .restitution(description.elasticity)
        .build();
    let collider_handle =
        world
            .colliders
            .insert_with_parent(collider, body_handle, &mut world.bodies);

    world
        .entities_by_collider
        .insert(collider_handle, Some(description.entity_id));
    world.objects.insert(
        handle,
        WorldObject::DynamicBody {
            body: body_handle,
            collider: collider_handle,
        },
    );
    Ok(handle)
}

/// Removes the body and asks Rapier to remove its attached colliders as well.
pub(super) fn remove(world: &mut RapierWorld, handle: PhysicsHandle) -> Result<(), PhysicsError> {
    let (body_handle, collider_handle) = match world.objects.get(&handle) {
        Some(WorldObject::DynamicBody { body, collider }) => (*body, *collider),
        _ => return Err(PhysicsError::InvalidHandle(handle)),
    };

    world
        .bodies
        .remove(
            body_handle,
            &mut world.islands,
            &mut world.colliders,
            &mut world.impulse_joints,
            &mut world.multibody_joints,
            &mut world.soft_bodies,
            true,
        )
        .ok_or(PhysicsError::InvalidHandle(handle))?;
    world.objects.remove(&handle);
    world.entities_by_collider.remove(&collider_handle);
    Ok(())
}

/// Applies a push through the center of mass, changing linear velocity.
pub(super) fn apply_impulse(
    world: &mut RapierWorld,
    handle: PhysicsHandle,
    impulse: &Vec3,
) -> Result<(), PhysicsError> {
    validation::validate_vec3(impulse, "impulse")?;
    let body_handle = dynamic_body_handle(world, handle)?;
    let body = world
        .bodies
        .get_mut(body_handle)
        .ok_or(PhysicsError::InvalidHandle(handle))?;
    body.apply_impulse(conversions::to_vector(impulse), true);
    Ok(())
}

/// Applies a push at a world-space point; an off-center push also causes rotation.
pub(super) fn apply_impulse_at_point(
    world: &mut RapierWorld,
    handle: PhysicsHandle,
    impulse: &Vec3,
    world_position: &Vec3,
) -> Result<(), PhysicsError> {
    validation::validate_vec3(impulse, "impulse")?;
    validation::validate_vec3(world_position, "impulse position")?;
    let body_handle = dynamic_body_handle(world, handle)?;
    let body = world
        .bodies
        .get_mut(body_handle)
        .ok_or(PhysicsError::InvalidHandle(handle))?;
    body.apply_impulse_at_point(
        conversions::to_vector(impulse),
        conversions::to_vector(world_position),
        true,
    );
    Ok(())
}

/// Reads the current transform for any live object represented by this handle.
pub(super) fn body_transform(
    world: &RapierWorld,
    handle: PhysicsHandle,
) -> Result<PhysicsTransform, PhysicsError> {
    let object = world
        .objects
        .get(&handle)
        .ok_or(PhysicsError::InvalidHandle(handle))?;
    let pose = match object {
        WorldObject::StaticCollider(collider) | WorldObject::PhantomCollider(collider) => world
            .colliders
            .get(*collider)
            .map(|collider| collider.position()),
        WorldObject::DynamicBody { body, .. } | WorldObject::Character { body, .. } => {
            world.bodies.get(*body).map(|body| body.position())
        }
    }
    .ok_or(PhysicsError::InvalidHandle(handle))?;
    Ok(conversions::from_pose(pose))
}

/// Returns a Rapier body only for a handle that identifies a dynamic body.
///
/// This prevents an impulse operation from accidentally treating a static
/// collider, phantom, or kinematic character as a dynamic object.
fn dynamic_body_handle(
    world: &RapierWorld,
    handle: PhysicsHandle,
) -> Result<RigidBodyHandle, PhysicsError> {
    match world.objects.get(&handle) {
        Some(WorldObject::DynamicBody { body, .. }) => Ok(*body),
        _ => Err(PhysicsError::InvalidHandle(handle)),
    }
}
