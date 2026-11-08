use game_types::Vec3;
use physics_api::{CharacterDesc, CharacterMoveResult, PhysicsError, PhysicsHandle};
use rapier3d::control::{CharacterAutostep, CharacterLength, KinematicCharacterController};
use rapier3d::prelude::{ColliderBuilder, Pose, QueryFilter, RigidBodyBuilder};

use crate::{
    conversions, validation,
    world::{RapierWorld, WorldObject},
};

/// Creates a kinematic body used for game-directed, collision-aware movement.
pub(super) fn create(
    world: &mut RapierWorld,
    description: &CharacterDesc,
) -> Result<PhysicsHandle, PhysicsError> {
    validation::validate_character(description)?;
    let handle = world.allocate_handle()?;
    let (shape, local_offset) = conversions::shape_and_offset(&description.shape)?;
    let body = RigidBodyBuilder::kinematic_position_based()
        .translation(conversions::to_vector(&description.position))
        .build();
    let body_handle = world.bodies.insert(body);
    let collider = ColliderBuilder::new(shape)
        .position(Pose::from_translation(local_offset))
        .build();
    let collider_handle =
        world
            .colliders
            .insert_with_parent(collider, body_handle, &mut world.bodies);

    let controller = KinematicCharacterController {
        max_slope_climb_angle: description.max_slope_angle_radians,
        // A zero step height disables Rapier's optional stair-stepping behavior.
        autostep: (description.step_height > 0.0).then_some(CharacterAutostep {
            max_height: CharacterLength::Absolute(description.step_height),
            min_width: CharacterLength::Relative(0.5),
            include_dynamic_bodies: false,
        }),
        ..KinematicCharacterController::default()
    };

    world
        .entities_by_collider
        .insert(collider_handle, Some(description.entity_id));
    world.objects.insert(
        handle,
        WorldObject::Character {
            body: body_handle,
            collider: collider_handle,
            controller,
            is_grounded: false,
        },
    );
    Ok(handle)
}

/// Removes the character's Rapier body and its attached collider.
pub(super) fn remove(world: &mut RapierWorld, handle: PhysicsHandle) -> Result<(), PhysicsError> {
    let (body_handle, collider_handle) = match world.objects.get(&handle) {
        Some(WorldObject::Character { body, collider, .. }) => (*body, *collider),
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

/// Computes a collision-safe displacement for the character.
///
/// The caller supplies distance to move this tick, not speed. Rapier sweeps the
/// character shape through the scene and adjusts movement around obstacles.
/// Gravity, jump strength, and movement speed remain gameplay rules outside
/// this physics adapter.
pub(super) fn move_character(
    world: &mut RapierWorld,
    handle: PhysicsHandle,
    desired_displacement: &Vec3,
) -> Result<CharacterMoveResult, PhysicsError> {
    validation::validate_vec3(desired_displacement, "character displacement")?;
    let (body_handle, collider_handle, controller) = match world.objects.get(&handle) {
        Some(WorldObject::Character {
            body,
            collider,
            controller,
            ..
        }) => (*body, *collider, *controller),
        _ => return Err(PhysicsError::InvalidHandle(handle)),
    };
    let character_body = world
        .bodies
        .get(body_handle)
        .ok_or(PhysicsError::InvalidHandle(handle))?;
    let character_collider = world
        .colliders
        .get(collider_handle)
        .ok_or(PhysicsError::InvalidHandle(handle))?;
    let query_pipeline = world.broad_phase.as_query_pipeline(
        world.narrow_phase.query_dispatcher(),
        &world.bodies,
        &world.colliders,
        QueryFilter::new()
            .exclude_sensors()
            .exclude_collider(collider_handle),
    );
    let mut collided = false;
    // The controller calls this for encountered contacts. The API returns a
    // summary flag, so the detailed contact information is not stored here.
    let movement = controller.move_shape(
        world.integration_parameters.dt,
        &query_pipeline,
        character_collider.shape(),
        character_body.position(),
        conversions::to_vector(desired_displacement),
        |_| collided = true,
    );
    let applied_displacement = movement.translation;
    let is_grounded = movement.grounded;

    let body = world
        .bodies
        .get_mut(body_handle)
        .ok_or(PhysicsError::InvalidHandle(handle))?;
    body.set_next_kinematic_translation(body.translation() + applied_displacement);

    // Rapier applies the kinematic target on the next step. Save the grounded
    // result now so the API can answer `is_grounded` for this movement.
    if let Some(WorldObject::Character {
        is_grounded: grounded_state,
        ..
    }) = world.objects.get_mut(&handle)
    {
        *grounded_state = is_grounded;
    }

    Ok(CharacterMoveResult {
        applied_displacement: conversions::from_vector(applied_displacement),
        is_grounded,
        collided,
    })
}

/// Returns the grounded state from the character's most recent movement check.
pub(super) fn is_grounded(
    world: &RapierWorld,
    handle: PhysicsHandle,
) -> Result<bool, PhysicsError> {
    match world.objects.get(&handle) {
        Some(WorldObject::Character { is_grounded, .. }) => Ok(*is_grounded),
        _ => Err(PhysicsError::InvalidHandle(handle)),
    }
}
