use game_types::Vec3;
use physics_api::{CharacterDesc, DynamicBodyDesc, PhysicsError, RayQuery, StaticColliderDesc};

/// Rejects NaN and infinite vector components before passing values to Rapier.
pub(crate) fn validate_vec3(value: &Vec3, name: &str) -> Result<(), PhysicsError> {
    if value.x.is_finite() && value.y.is_finite() && value.z.is_finite() {
        Ok(())
    } else {
        Err(PhysicsError::InvalidDescription(format!(
            "{name} must contain only finite numbers"
        )))
    }
}

/// Checks a static collider's position, rotation, and shape dimensions.
pub(crate) fn validate_static(description: &StaticColliderDesc) -> Result<(), PhysicsError> {
    crate::conversions::validate_transform(&description.transform)?;
    crate::conversions::validate_shape(&description.shape)
}

/// Checks a dynamic body's geometry and the physical values Rapier simulates.
pub(crate) fn validate_dynamic(description: &DynamicBodyDesc) -> Result<(), PhysicsError> {
    crate::conversions::validate_transform(&description.transform)?;
    crate::conversions::validate_shape(&description.shape)?;
    if !description.mass_kg.is_finite() || description.mass_kg <= 0.0 {
        return Err(PhysicsError::InvalidDescription(
            "dynamic body mass must be positive and finite".into(),
        ));
    }
    if !description.friction.is_finite() || description.friction < 0.0 {
        return Err(PhysicsError::InvalidDescription(
            "dynamic body friction must be non-negative and finite".into(),
        ));
    }
    if !description.elasticity.is_finite() || !(0.0..=1.0).contains(&description.elasticity) {
        return Err(PhysicsError::InvalidDescription(
            "dynamic body elasticity must be finite and between zero and one".into(),
        ));
    }
    Ok(())
}

/// Checks controller settings and validates the character's collision shape.
pub(crate) fn validate_character(description: &CharacterDesc) -> Result<(), PhysicsError> {
    validate_vec3(&description.position, "character position")?;
    crate::conversions::validate_shape(&description.shape)?;
    if !description.max_slope_angle_radians.is_finite()
        || !(0.0..=std::f32::consts::FRAC_PI_2).contains(&description.max_slope_angle_radians)
    {
        return Err(PhysicsError::InvalidDescription(
            "character maximum slope must be finite and between zero and 90 degrees".into(),
        ));
    }
    if !description.step_height.is_finite() || description.step_height < 0.0 {
        return Err(PhysicsError::InvalidDescription(
            "character step height must be non-negative and finite".into(),
        ));
    }
    Ok(())
}

/// Validates a ray, including the API requirement that its direction is unit length.
pub(crate) fn validate_ray(query: &RayQuery) -> Result<(), PhysicsError> {
    validate_vec3(&query.origin, "ray origin")?;
    validate_vec3(&query.direction, "ray direction")?;
    let direction_length_squared = query.direction.length_squared();
    if !query.max_distance.is_finite() || query.max_distance <= 0.0 {
        return Err(PhysicsError::InvalidDescription(
            "ray maximum distance must be positive and finite".into(),
        ));
    }
    if !direction_length_squared.is_finite() || (direction_length_squared - 1.0).abs() > 1.0e-3 {
        return Err(PhysicsError::InvalidDescription(
            "ray direction must have a length of one".into(),
        ));
    }
    Ok(())
}
