/// Three numbers that represent a position, direction, or movement amount.
///
/// Game coordinates use meters for distance, and the Y axis points up. This
/// type is defined by our game rather than by a renderer or physics engine, so
/// it can be shared with different applications.
///
/// Each component is an `f32`, which is a 32-bit floating-point number. This
/// type does not check whether a number is infinite or `NaN`; code receiving
/// values from outside the game must validate them. The type also does not
/// define which direction is forward. Player movement will define that later.
///
/// `derive` asks Rust to provide common behaviors: `Debug` helps display a
/// vector while debugging, `Default` creates a vector whose components are all
/// zero, `Clone` and `Copy` allow vectors to be copied, and `PartialEq` allows
/// their components to be compared in tests.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct Vec3 {
    /// The X component, commonly used for left and right in world coordinates.
    pub x: f32,
    /// The Y component, used for height because the game uses Y as up.
    pub y: f32,
    /// The Z component. Player movement will define its forward/back meaning.
    pub z: f32,
}

impl Vec3 {
    /// A vector with all three components set to zero.
    pub const ZERO: Self = Self {
        x: 0.0,
        y: 0.0,
        z: 0.0,
    };

    /// Creates a vector using the X, Y, and Z numbers provided.
    pub fn new(x_value: f32, y_value: f32, z_value: f32) -> Self {
        Self {
            x: x_value,
            y: y_value,
            z: z_value,
        }
    }

    /// Adds another vector to this one, component by component.
    ///
    /// The `&self` and `&Vec3` parts mean this function reads both input vectors
    /// without taking ownership of them. It returns a new vector and leaves the
    /// original vectors unchanged. For example, `first.add(&second)` adds the
    /// X values, the Y values, and the Z values separately.
    pub fn add(&self, other: &Self) -> Self {
        Self::new(self.x + other.x, self.y + other.y, self.z + other.z)
    }

    /// Subtracts another vector from this one, component by component.
    ///
    /// Like `add`, the references let this function read the inputs without
    /// taking ownership. The returned vector is new; neither input is changed.
    pub fn subtract(&self, other: &Self) -> Self {
        Self::new(self.x - other.x, self.y - other.y, self.z - other.z)
    }

    /// Multiplies each component by the same number and returns a new vector.
    ///
    /// Scaling a direction can change its length. When the vector represents a
    /// position, the result is that position multiplied by `factor`.
    pub fn scale(&self, factor: f32) -> Self {
        Self::new(self.x * factor, self.y * factor, self.z * factor)
    }

    /// Multiplies matching components and adds those results into one number.
    ///
    /// The result describes how much the vectors point in similar directions:
    /// zero means perpendicular, positive means generally aligned, and negative
    /// means generally opposed.
    pub fn dot_product(&self, other: &Self) -> f32 {
        (self.x * other.x) + (self.y * other.y) + (self.z * other.z)
    }

    /// Returns the vector's length multiplied by itself.
    ///
    /// This is useful for comparing distances without calculating a square
    /// root. For example, compare two `length_squared` results to find which
    /// vector is shorter. The result is a squared distance, not a distance.
    pub fn length_squared(&self) -> f32 {
        self.dot_product(self)
    }
}

#[cfg(test)]
mod tests {
    use super::Vec3;

    #[test]
    fn named_vector_methods_return_expected_values() {
        let starting_vector = Vec3::new(1.0, 2.0, 3.0);
        let other_vector = Vec3::new(4.0, 5.0, 6.0);

        let added_vector = starting_vector.add(&other_vector);
        let subtracted_vector = other_vector.subtract(&starting_vector);
        let scaled_vector = starting_vector.scale(2.0);

        assert_eq!(added_vector, Vec3::new(5.0, 7.0, 9.0));
        assert_eq!(subtracted_vector, Vec3::new(3.0, 3.0, 3.0));
        assert_eq!(scaled_vector, Vec3::new(2.0, 4.0, 6.0));
        assert_eq!(starting_vector.length_squared(), 14.0);
    }
}
