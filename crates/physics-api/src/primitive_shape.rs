use game_types::Vec3;
use serde::{Deserialize, Serialize};

use crate::bezier::CubicBezier3;
use crate::height_field_type::HeightFieldDesc;

/// Half-width and half-depth for a finite plane.
///
/// The plane itself lies in the local XZ plane, so it has no Y extent.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PlaneHalfExtents {
    /// Distance from the plane's center to its edge along local X, in meters.
    pub half_width_x: f32,
    /// Distance from the plane's center to its edge along local Z, in meters.
    pub half_depth_z: f32,
}

/// A collision shape that can be shared by different kinds of physics objects.
///
/// The dimensions describe the shape in its own local space. The object's
/// `PhysicsTransform` places and rotates that shape in the world. The physics
/// implementation must check that dimensions are positive and finite. A
/// heightfield contains owned sample data, so shapes and descriptions that
/// contain one can be cloned but cannot be copied implicitly.
///
/// A torus is included as a useful game shape, but it may need to be
/// approximated by several simpler colliders when a physics engine does not
/// support it directly. That engine-specific choice belongs in the adapter.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PrimitiveShape {
    /// A box. Each value is the distance from its center to one side.
    ///
    /// For example, half-extents `(2, 1, 3)` make a box 4 meters wide,
    /// 2 meters tall, and 6 meters deep.
    Box { half_extents: Vec3 },

    /// A ball described by its radius.
    Sphere { radius: f32 },

    /// A flat surface through the shape's local origin.
    ///
    /// The surface lies in the local XZ plane and faces positive Y. Set
    /// `half_extents` to `None` for an infinite plane, or to `Some(...)` for a
    /// finite rectangle. The object's transform places and rotates the plane.
    /// Finite half-extents must be positive and finite.
    Plane {
        half_extents: Option<PlaneHalfExtents>,
    },

    /// A cylinder whose central axis is the shape's local Y axis.
    ///
    /// `half_height` is half of the cylinder's full length.
    Cylinder { radius: f32, half_height: f32 },

    /// A capsule whose central axis is the shape's local Y axis.
    ///
    /// A capsule has a straight middle section with a rounded half-sphere at
    /// each end. `half_height` is half the length of the straight middle
    /// section; the radius is in addition to that length.
    Capsule { radius: f32, half_height: f32 },

    /// A ring-shaped torus lying in the shape's local XZ plane.
    ///
    /// `major_radius` measures from the center of the hole to the center of the
    /// tube. `tube_radius` is the radius of the tube itself. To keep an open
    /// hole in the middle, `major_radius` must be greater than `tube_radius`.
    /// The physics adapter will choose how to represent this shape in its engine.
    Torus { major_radius: f32, tube_radius: f32 },

    /// A grid of heights that describes an uneven surface.
    ///
    /// Heightfields can be requested for static objects, movable bodies,
    /// characters, or phantoms through this shared shape type. The physics
    /// adapter decides which uses its engine supports.
    HeightField(HeightFieldDesc),

    /// A rectangular cross-section swept along a sequence of cubic Bézier splines.
    ///
    /// Useful for bridges, highways, ramps, compound curved walls, and walkways.
    BezierExtrudedRect {
        /// Sequence of connected 3D cubic Bézier segments in local shape space.
        segments: Vec<CubicBezier3>,
        /// Half-width of the rectangular cross-section in meters (local horizontal extent).
        half_width: f32,
        /// Half-thickness of the rectangular cross-section in meters (local vertical extent).
        half_thickness: f32,
        /// Target arc-length per physics collider segment in meters.
        max_segment_len_m: f32,
        /// Maximum angular deviation in degrees between adjacent tangents before subdividing.
        max_tangent_angle_deg: f32,
    },

    /// A circular cross-section swept along a sequence of cubic Bézier splines.
    ///
    /// Useful for curved pipes, industrial conduits, railings, and suspension cables.
    BezierExtrudedCircle {
        /// Sequence of connected 3D cubic Bézier segments in local shape space.
        segments: Vec<CubicBezier3>,
        /// Radius of the circular cross-section in meters.
        radius: f32,
        /// Target arc-length per physics collider segment in meters.
        max_segment_len_m: f32,
        /// Maximum angular deviation in degrees between adjacent tangents before subdividing.
        max_tangent_angle_deg: f32,
    },
}
