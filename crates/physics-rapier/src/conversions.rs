use std::f32::consts::TAU;

use game_types::Vec3;
use physics_api::{
    HeightFieldDesc, PhysicsError, PhysicsTransform, PlaneHalfExtents, PrimitiveShape, Quaternion,
};
use rapier3d::prelude::{Array2, Pose, Rotation, SharedShape, Vector};

/// Converts the API's engine-neutral vector into Rapier's vector type.
///
/// Both types use X/Y/Z coordinates measured in meters, so this copies each
/// component without changing the coordinate system.
pub(crate) fn to_vector(value: &Vec3) -> Vector {
    Vector::new(value.x, value.y, value.z)
}

/// Converts Rapier's vector back into the API's engine-neutral type.
pub(crate) fn from_vector(value: Vector) -> Vec3 {
    Vec3::new(value.x, value.y, value.z)
}

/// Converts the API's position and quaternion into Rapier's pose.
///
/// The API and Rapier both store quaternions as X/Y/Z/W. The input is checked
/// before this function is called; normalization protects Rapier against
/// small floating-point drift.
pub(crate) fn to_pose(transform: &PhysicsTransform) -> Pose {
    let rotation = Rotation::from_xyzw(
        transform.rotation.x,
        transform.rotation.y,
        transform.rotation.z,
        transform.rotation.w,
    )
    .normalize();
    Pose::from_parts(to_vector(&transform.position), rotation)
}

/// Converts a Rapier pose into the API-owned transform type.
pub(crate) fn from_pose(pose: &Pose) -> PhysicsTransform {
    PhysicsTransform::new(
        from_vector(pose.translation),
        Quaternion::new(
            pose.rotation.x,
            pose.rotation.y,
            pose.rotation.z,
            pose.rotation.w,
        ),
    )
}

/// Builds the Rapier shape and its local offset from an API shape.
///
/// Some shape definitions use a different origin in Rapier than the API does.
/// Returning the offset with the shape lets the creator place both together
/// while keeping this conversion logic in one place.
pub(crate) fn shape_and_offset(
    shape: &PrimitiveShape,
) -> Result<(SharedShape, Vector), PhysicsError> {
    let shape_result = match shape {
        PrimitiveShape::Box { half_extents } => {
            validate_positive_vec3(half_extents, "box half-extents")?;
            (
                SharedShape::cuboid(half_extents.x, half_extents.y, half_extents.z),
                Vector::ZERO,
            )
        }
        PrimitiveShape::Sphere { radius } => {
            validate_positive(*radius, "sphere radius")?;
            (SharedShape::ball(*radius), Vector::ZERO)
        }
        PrimitiveShape::Plane { half_extents } => {
            let plane_shape = match half_extents {
                None => SharedShape::halfspace(Vector::Y),
                Some(extents) => {
                    validate_plane_extents(extents)?;
                    // A finite plane is represented by a very thin box so its edges collide.
                    SharedShape::cuboid(extents.half_width_x, 0.005, extents.half_depth_z)
                }
            };
            (plane_shape, Vector::ZERO)
        }
        PrimitiveShape::Cylinder {
            radius,
            half_height,
        } => {
            validate_positive(*radius, "cylinder radius")?;
            validate_positive(*half_height, "cylinder half-height")?;
            (SharedShape::cylinder(*half_height, *radius), Vector::ZERO)
        }
        PrimitiveShape::Capsule {
            radius,
            half_height,
        } => {
            validate_positive(*radius, "capsule radius")?;
            if !half_height.is_finite() || *half_height < 0.0 {
                return Err(invalid(
                    "capsule half-height must be non-negative and finite",
                ));
            }
            (SharedShape::capsule_y(*half_height, *radius), Vector::ZERO)
        }
        PrimitiveShape::Torus {
            major_radius,
            tube_radius,
        } => {
            validate_positive(*major_radius, "torus major radius")?;
            validate_positive(*tube_radius, "torus tube radius")?;
            if major_radius <= tube_radius {
                return Err(invalid(
                    "torus major radius must be greater than its tube radius",
                ));
            }
            (torus_shape(*major_radius, *tube_radius), Vector::ZERO)
        }
        PrimitiveShape::HeightField(description) => heightfield_shape(description)?,
    };
    Ok(shape_result)
}

pub(crate) fn validate_transform(transform: &PhysicsTransform) -> Result<(), PhysicsError> {
    crate::validation::validate_vec3(&transform.position, "transform position")?;
    let rotation = transform.rotation;
    if !rotation.x.is_finite()
        || !rotation.y.is_finite()
        || !rotation.z.is_finite()
        || !rotation.w.is_finite()
    {
        return Err(invalid("rotation must contain only finite numbers"));
    }
    let length_squared = rotation.x * rotation.x
        + rotation.y * rotation.y
        + rotation.z * rotation.z
        + rotation.w * rotation.w;
    if !length_squared.is_finite() || (length_squared - 1.0).abs() > 1.0e-3 {
        return Err(invalid("rotation quaternion must be normalized"));
    }
    Ok(())
}

/// Validates a shape by attempting the same conversion used at creation time.
pub(crate) fn validate_shape(shape: &PrimitiveShape) -> Result<(), PhysicsError> {
    shape_and_offset(shape).map(|_| ())
}

fn heightfield_shape(description: &HeightFieldDesc) -> Result<(SharedShape, Vector), PhysicsError> {
    if description.rows < 2 || description.columns < 2 {
        return Err(invalid(
            "heightfield must have at least two rows and two columns",
        ));
    }
    let expected_samples = description
        .expected_sample_count()
        .ok_or_else(|| invalid("heightfield dimensions are too large"))?;
    if description.height_samples.len() != expected_samples {
        return Err(invalid(
            "heightfield sample count does not match its dimensions",
        ));
    }
    if !description.cell_size_x.is_finite()
        || description.cell_size_x <= 0.0
        || !description.cell_size_z.is_finite()
        || description.cell_size_z <= 0.0
    {
        return Err(invalid(
            "heightfield cell sizes must be positive and finite",
        ));
    }
    if description
        .height_samples
        .iter()
        .any(|height| !height.is_finite())
    {
        return Err(invalid("heightfield samples must all be finite"));
    }

    let width = description.cell_size_x * (description.columns - 1) as f32;
    let depth = description.cell_size_z * (description.rows - 1) as f32;
    if !width.is_finite() || !depth.is_finite() {
        return Err(invalid("heightfield dimensions are too large"));
    }

    // Rapier centers its heightfield at the midpoint. This offset puts its
    // first sample back at the API's local origin (X=0, Z=0).
    let offset = Vector::new(width * 0.5, 0.0, depth * 0.5);
    let shape = match description.thickness {
        Some(thickness) => {
            validate_positive(thickness, "heightfield thickness")?;
            thick_heightfield_shape(description, width, depth, thickness)?
        }
        None => {
            // The API lays samples out row-by-row (rows advance along Z,
            // columns along X). Preserve that layout in Rapier's heightfield.
            let heights = Array2::from_fn(description.rows, description.columns, |row, column| {
                description.height_samples[row * description.columns + column]
            });
            SharedShape::heightfield(heights, Vector::new(width, 1.0, depth))
        }
    };
    Ok((shape, offset))
}

/// Builds a closed collision shape from the same two triangles Rapier uses in
/// each heightfield cell. Each top triangle is extruded downward to make a
/// convex triangular prism; the prisms are then grouped into one compound.
fn thick_heightfield_shape(
    description: &HeightFieldDesc,
    width: f32,
    depth: f32,
    thickness: f32,
) -> Result<SharedShape, PhysicsError> {
    let part_count = (description.rows - 1)
        .checked_mul(description.columns - 1)
        .and_then(|cell_count| cell_count.checked_mul(2))
        .ok_or_else(|| invalid("heightfield has too many cells"))?;
    let mut parts = Vec::with_capacity(part_count);

    for row in 0..description.rows - 1 {
        for column in 0..description.columns - 1 {
            let p00 = heightfield_point(description, row, column, width, depth);
            let p10 = heightfield_point(description, row + 1, column, width, depth);
            let p01 = heightfield_point(description, row, column + 1, width, depth);
            let p11 = heightfield_point(description, row + 1, column + 1, width, depth);

            // Rapier's default heightfield split uses the diagonal from p10 to
            // p01. Matching it keeps the thick top aligned with the thin form.
            add_heightfield_prism(&mut parts, [p00, p10, p01], thickness)?;
            add_heightfield_prism(&mut parts, [p10, p11, p01], thickness)?;
        }
    }

    Ok(SharedShape::compound(parts))
}

fn heightfield_point(
    description: &HeightFieldDesc,
    row: usize,
    column: usize,
    width: f32,
    depth: f32,
) -> Vector {
    let sample_index = row * description.columns + column;
    Vector::new(
        column as f32 * description.cell_size_x - width * 0.5,
        description.height_samples[sample_index],
        row as f32 * description.cell_size_z - depth * 0.5,
    )
}

fn add_heightfield_prism(
    parts: &mut Vec<(Pose, SharedShape)>,
    top_triangle: [Vector; 3],
    thickness: f32,
) -> Result<(), PhysicsError> {
    let mut prism_vertices = Vec::with_capacity(6);
    for point in top_triangle {
        let bottom_y = point.y - thickness;
        if !bottom_y.is_finite() {
            return Err(invalid("heightfield thickness produces invalid geometry"));
        }
        prism_vertices.push(point);
        prism_vertices.push(Vector::new(point.x, bottom_y, point.z));
    }

    let prism = SharedShape::convex_hull(&prism_vertices)
        .ok_or_else(|| invalid("could not create a convex heightfield cell"))?;
    parts.push((Pose::IDENTITY, prism));
    Ok(())
}

fn torus_shape(major_radius: f32, tube_radius: f32) -> SharedShape {
    const SEGMENTS: usize = 32;
    let mut parts = Vec::with_capacity(SEGMENTS);
    // Rapier does not provide a torus primitive. Short capsules approximate
    // the ring; overlapping their ends avoids gaps between neighboring pieces.
    for segment in 0..SEGMENTS {
        let start_angle = TAU * segment as f32 / SEGMENTS as f32;
        let end_angle = TAU * (segment + 1) as f32 / SEGMENTS as f32;
        let start = Vector::new(
            major_radius * start_angle.cos(),
            0.0,
            major_radius * start_angle.sin(),
        );
        let end = Vector::new(
            major_radius * end_angle.cos(),
            0.0,
            major_radius * end_angle.sin(),
        );
        parts.push((
            Pose::IDENTITY,
            SharedShape::capsule(start, end, tube_radius),
        ));
    }
    SharedShape::compound(parts)
}

fn validate_plane_extents(extents: &PlaneHalfExtents) -> Result<(), PhysicsError> {
    validate_positive(extents.half_width_x, "plane half-width")?;
    validate_positive(extents.half_depth_z, "plane half-depth")
}

fn validate_positive(value: f32, description: &str) -> Result<(), PhysicsError> {
    if value.is_finite() && value > 0.0 {
        Ok(())
    } else {
        Err(invalid(format!(
            "{description} must be positive and finite"
        )))
    }
}

fn validate_positive_vec3(value: &Vec3, description: &str) -> Result<(), PhysicsError> {
    if value.x.is_finite()
        && value.x > 0.0
        && value.y.is_finite()
        && value.y > 0.0
        && value.z.is_finite()
        && value.z > 0.0
    {
        Ok(())
    } else {
        Err(invalid(format!(
            "{description} must be positive and finite"
        )))
    }
}

fn invalid(message: impl Into<String>) -> PhysicsError {
    PhysicsError::InvalidDescription(message.into())
}
