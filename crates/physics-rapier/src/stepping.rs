use physics_api::PhysicsError;

use crate::world::RapierWorld;

/// Advances the whole Rapier world once using the caller's fixed timestep.
pub(super) fn step(
    world: &mut RapierWorld,
    fixed_time_step_seconds: f32,
) -> Result<(), PhysicsError> {
    if !fixed_time_step_seconds.is_finite() || fixed_time_step_seconds <= 0.0 {
        return Err(PhysicsError::InvalidTimeStep(fixed_time_step_seconds));
    }

    // Rapier reads the step duration from these parameters. The host chooses
    // the fixed duration; this adapter still advances exactly one step.
    world.integration_parameters.dt = fixed_time_step_seconds;
    // These Rapier subsystems work together to update motion, collisions, and
    // contacts. Keep this call centralized so every tick uses the same setup.
    world.pipeline.step(
        world.gravity,
        &world.integration_parameters,
        &mut world.islands,
        &mut world.broad_phase,
        &mut world.narrow_phase,
        &mut world.bodies,
        &mut world.colliders,
        &mut world.impulse_joints,
        &mut world.multibody_joints,
        &mut world.soft_bodies,
        &mut world.ccd_solver,
        &(),
        &(),
    );
    Ok(())
}
