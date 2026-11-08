use game_types::Vec3;

/// A basic collision shape that can be shared by different kinds of physics objects.
///
/// The dimensions describe the shape in its own local space. The object's
/// `PhysicsTransform` places and rotates that shape in the world. The physics
/// implementation must check that every dimension is positive and finite.
///
/// A torus is included as a useful game shape, but it may need to be
/// approximated by several simpler colliders when a physics engine does not
/// support it directly. That engine-specific choice belongs in the adapter.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PrimitiveShape {
    /// A box. Each value is the distance from its center to one side.
    ///
    /// For example, half-extents `(2, 1, 3)` make a box 4 meters wide,
    /// 2 meters tall, and 6 meters deep.
    Box { half_extents: Vec3 },

    /// A ball described by its radius.
    Sphere { radius: f32 },

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
}
