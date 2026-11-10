use serde::{Deserialize, Serialize};

/// Height samples and spacing that describe a grid-shaped collision surface.
///
/// The first sample is at local X=0 and Z=0. Columns move along positive X;
/// rows move along positive Z. The samples are stored one row at a time, so
/// each consecutive group of `columns` values describes one row. Every sample
/// is a height in meters along local Y.
///
/// `thickness` optionally closes the surface downward along local Y. When set,
/// it must be positive and finite. This creates a solid collision shape with
/// the sampled surface on top, plus an underside and sides.
///
/// A valid heightfield needs at least two rows and two columns, exactly
/// `rows * columns` finite height samples, and positive finite spacing values.
/// Whether a physics engine supports this shape in a particular object type is
/// decided by that engine's adapter.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HeightFieldDesc {
    /// Number of rows, which extend along the local Z axis.
    pub rows: usize,
    /// Number of columns in each row, which extend along the local X axis.
    pub columns: usize,
    /// Heights in meters, listed one row at a time.
    pub height_samples: Vec<f32>,
    /// Distance in meters between neighboring columns.
    pub cell_size_x: f32,
    /// Distance in meters between neighboring rows.
    pub cell_size_z: f32,
    /// Optional solid thickness extending downward from the sampled surface.
    ///
    /// `None` keeps this as a surface. `Some(value)` requires a positive,
    /// finite value in meters.
    pub thickness: Option<f32>,
}

impl HeightFieldDesc {
    /// Returns the number of height samples that a valid grid must contain.
    pub fn expected_sample_count(&self) -> Option<usize> {
        self.rows.checked_mul(self.columns)
    }
}

#[cfg(test)]
mod tests {
    use super::HeightFieldDesc;

    #[test]
    fn heightfield_reports_its_expected_sample_count() {
        let heightfield = HeightFieldDesc {
            rows: 3,
            columns: 4,
            height_samples: vec![0.0; 12],
            cell_size_x: 2.0,
            cell_size_z: 2.0,
            thickness: None,
        };

        assert_eq!(heightfield.expected_sample_count(), Some(12));
    }
}
