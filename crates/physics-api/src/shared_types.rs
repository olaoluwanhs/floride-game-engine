use game_types::Vec3;
use serde::{Deserialize, Serialize};

/// A rotation represented by four numbers called a quaternion.
///
/// Quaternions describe 3D orientation without the problems that can happen
/// when combining three Euler angles. A valid rotation is normalized, meaning
/// its four values have a combined length of approximately one. Physics
/// implementations must validate rotation values they receive.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
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

    /// Returns a unit-length quaternion, falling back to [`Quaternion::IDENTITY`] if degenerate.
    pub fn normalized(&self) -> Self {
        let len_sq = self.x * self.x + self.y * self.y + self.z * self.z + self.w * self.w;
        if len_sq.is_finite() && len_sq > 1.0e-8 {
            let inv = len_sq.sqrt().recip();
            Self::new(self.x * inv, self.y * inv, self.z * inv, self.w * inv)
        } else {
            Self::IDENTITY
        }
    }

    /// Returns the conjugate (inverse rotation for unit quaternions).
    pub fn conjugate(&self) -> Self {
        Self::new(-self.x, -self.y, -self.z, self.w)
    }

    /// Rotates a 3D vector by this quaternion (`q * v * q^-1`).
    pub fn rotate_vec(&self, vector: &Vec3) -> Vec3 {
        let unit = self.normalized();
        let q_vec = Vec3::new(unit.x, unit.y, unit.z);
        let t = q_vec.cross_product(vector).scale(2.0);
        let cross_qt = q_vec.cross_product(&t);
        vector.add(&t.scale(unit.w)).add(&cross_qt)
    }

    /// Rotates a 3D vector by the inverse of this quaternion (`q^-1 * v * q`).
    ///
    /// Useful for converting world offsets into an object's local coordinate frame.
    pub fn inverse_rotate_vec(&self, vector: &Vec3) -> Vec3 {
        self.conjugate().rotate_vec(vector)
    }

    /// Computes a rotation aligning the local `+Y` (up) axis onto `normal`.
    ///
    /// Useful for orienting surface decals, blemishes, or upright props flush against any face.
    pub fn align_up_to(normal: &Vec3) -> Self {
        let n = normal.normalized_or(Vec3::UP);
        let dot = n.y;
        if dot > 0.9999 {
            return Self::IDENTITY;
        }
        if dot < -0.9999 {
            return Self::new(1.0, 0.0, 0.0, 0.0);
        }
        // Cross(UP, n) = (n.z, 0.0, -n.x)
        Self::new(n.z, 0.0, -n.x, 1.0 + dot).normalized()
    }
}

/// A position and orientation in the game world.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
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

    /// Creates an unrotated transform at `position`.
    pub fn from_position(position: Vec3) -> Self {
        Self {
            position,
            rotation: Quaternion::IDENTITY,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Quaternion;
    use game_types::Vec3;

    #[test]
    fn quaternion_rotates_and_inverts_vectors() {
        let half_angle = std::f32::consts::FRAC_PI_4; // 90 deg around Y
        let rot = Quaternion::new(0.0, half_angle.sin(), 0.0, half_angle.cos());
        let world = rot.rotate_vec(&Vec3::new(0.0, 0.0, 1.0));
        assert!((world.x - 1.0).abs() < 1e-5);
        assert!(world.z.abs() < 1e-5);

        let recovered = rot.inverse_rotate_vec(&world);
        assert!(recovered.distance_to(&Vec3::new(0.0, 0.0, 1.0)) < 1e-5);
    }

    #[test]
    fn quaternion_align_up_to_matches_target_normal() {
        let forward_normal = Vec3::new(0.0, 0.0, 1.0);
        let q = Quaternion::align_up_to(&forward_normal);
        let aligned = q.rotate_vec(&Vec3::UP);
        assert!(aligned.distance_to(&forward_normal) < 1e-5);

        let down_q = Quaternion::align_up_to(&Vec3::new(0.0, -1.0, 0.0));
        let down = down_q.rotate_vec(&Vec3::UP);
        assert!((down.y + 1.0).abs() < 1e-5);
    }
}
