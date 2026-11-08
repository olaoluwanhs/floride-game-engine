use game_types::Vec3;

/// A rotation represented by four numbers called a quaternion.
///
/// Quaternions describe 3D orientation without the problems that can happen
/// when combining three Euler angles. A valid rotation is normalized, meaning
/// its four values have a combined length of approximately one. Physics
/// implementations must validate rotation values they receive.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Quaternion {
    /// The X component of the rotation.
    pub x: f32,
    /// The Y component of the rotation.
    pub y: f32,
    /// The Z component of the rotation.
    pub z: f32,
    /// The W component of the rotation.
    pub w: f32,
}

impl Quaternion {
    /// A quaternion that means "no rotation".
    pub const IDENTITY: Self = Self {
        x: 0.0,
        y: 0.0,
        z: 0.0,
        w: 1.0,
    };

    /// Creates a quaternion from its four components.
    ///
    /// This stores the values without checking whether they form a valid
    /// rotation. The physics implementation checks values before use.
    pub fn new(x_value: f32, y_value: f32, z_value: f32, w_value: f32) -> Self {
        Self {
            x: x_value,
            y: y_value,
            z: z_value,
            w: w_value,
        }
    }
}

/// A position and orientation in the game world.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PhysicsTransform {
    /// The object's position in meters.
    pub position: Vec3,
    /// The object's orientation.
    pub rotation: Quaternion,
}

impl PhysicsTransform {
    /// A transform at the world origin with no rotation.
    pub const IDENTITY: Self = Self {
        position: Vec3::ZERO,
        rotation: Quaternion::IDENTITY,
    };

    /// Creates a transform from a position and rotation.
    pub fn new(position: Vec3, rotation: Quaternion) -> Self {
        Self { position, rotation }
    }
}
