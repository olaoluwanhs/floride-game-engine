//! Pure-Rust cubic Bézier mathematics and adaptive sampling.
//!
//! Provides [`CubicBezier3`], evaluation of curve points and tangents,
//! local frame orientation calculation, and adaptive subdivision sampling.
//!
//! All coordinates are in meters, right-handed with `+Y` pointing up.

use game_types::Vec3;
use serde::{Deserialize, Serialize};

use crate::Quaternion;

/// A single cubic Bézier curve segment in 3D space.
///
/// `p0` is the starting point, `p1` and `p2` are control points,
/// and `p3` is the end point. Coordinates are stored in world meters with `+Y` up.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct CubicBezier3 {
    /// Starting position at `t = 0.0`.
    pub p0: [f32; 3],
    /// First tangent control point.
    pub p1: [f32; 3],
    /// Second tangent control point.
    pub p2: [f32; 3],
    /// Ending position at `t = 1.0`.
    pub p3: [f32; 3],
}

impl CubicBezier3 {
    /// Creates a new cubic Bézier segment from four 3D control points.
    pub const fn new(p0: [f32; 3], p1: [f32; 3], p2: [f32; 3], p3: [f32; 3]) -> Self {
        Self { p0, p1, p2, p3 }
    }

    /// Evaluates the 3D position along the curve at parameter `t` in `0.0..=1.0`.
    pub fn point(&self, t: f32) -> Vec3 {
        bezier_point(self, t)
    }

    /// Evaluates the normalized 3D tangent vector along the curve at parameter `t` in `0.0..=1.0`.
    pub fn tangent(&self, t: f32) -> Vec3 {
        bezier_tangent(self, t)
    }

    /// Extrudes a new connected cubic Bézier segment continuing from `p3` along the exit tangent.
    ///
    /// The new segment starts at `self.p3`, and places its handles and endpoint along the forward
    /// tangent with the given `segment_length` (in meters).
    pub fn extrude_continuation(&self, segment_length: f32) -> Self {
        let p3 = Vec3::new(self.p3[0], self.p3[1], self.p3[2]);
        let exit_tan = self.tangent(1.0);
        let handle_dist = (segment_length * 0.33).max(0.2);

        let new_p0 = self.p3;
        let new_p1_v = p3.add(&exit_tan.scale(handle_dist));
        let new_p2_v = p3.add(&exit_tan.scale(segment_length - handle_dist));
        let new_p3_v = p3.add(&exit_tan.scale(segment_length));

        Self {
            p0: new_p0,
            p1: [new_p1_v.x, new_p1_v.y, new_p1_v.z],
            p2: [new_p2_v.x, new_p2_v.y, new_p2_v.z],
            p3: [new_p3_v.x, new_p3_v.y, new_p3_v.z],
        }
    }
}

/// An adaptively sampled point along a Bézier curve.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BezierSample {
    /// Curve parameter `t` in `0.0..=1.0`.
    pub t: f32,
    /// 3D position in world meters.
    pub position: Vec3,
    /// Normalized forward tangent vector.
    pub tangent: Vec3,
    /// Cumulative arc-length distance from the start of the curve in meters.
    pub arc_length: f32,
}

/// Evaluates a point along a cubic Bézier curve at parameter `t` in `0.0..=1.0`.
///
/// Uses the cubic Bernstein polynomial:
/// `B(t) = (1-t)³·P0 + 3(1-t)²t·P1 + 3(1-t)t²·P2 + t³·P3`
pub fn bezier_point(curve: &CubicBezier3, t: f32) -> Vec3 {
    let t = t.clamp(0.0, 1.0);
    let u = 1.0 - t;
    let u2 = u * u;
    let u3 = u2 * u;
    let t2 = t * t;
    let t3 = t2 * t;

    let p0 = Vec3::new(curve.p0[0], curve.p0[1], curve.p0[2]);
    let p1 = Vec3::new(curve.p1[0], curve.p1[1], curve.p1[2]);
    let p2 = Vec3::new(curve.p2[0], curve.p2[1], curve.p2[2]);
    let p3 = Vec3::new(curve.p3[0], curve.p3[1], curve.p3[2]);

    p0.scale(u3)
        .add(&p1.scale(3.0 * u2 * t))
        .add(&p2.scale(3.0 * u * t2))
        .add(&p3.scale(t3))
}

/// Evaluates the first derivative (tangent vector) of a cubic Bézier curve at parameter `t` in `0.0..=1.0`.
///
/// Returns a normalized direction vector. Falls back to `+Z` for degenerate points.
/// `B'(t) = 3(1-t)²(P1-P0) + 6(1-t)t(P2-P1) + 3t²(P3-P2)`
pub fn bezier_tangent(curve: &CubicBezier3, t: f32) -> Vec3 {
    let t = t.clamp(0.0, 1.0);
    let u = 1.0 - t;
    let u2 = u * u;
    let t2 = t * t;

    let p0 = Vec3::new(curve.p0[0], curve.p0[1], curve.p0[2]);
    let p1 = Vec3::new(curve.p1[0], curve.p1[1], curve.p1[2]);
    let p2 = Vec3::new(curve.p2[0], curve.p2[1], curve.p2[2]);
    let p3 = Vec3::new(curve.p3[0], curve.p3[1], curve.p3[2]);

    let d0 = p1.subtract(&p0);
    let d1 = p2.subtract(&p1);
    let d2 = p3.subtract(&p2);

    let tangent = d0
        .scale(3.0 * u2)
        .add(&d1.scale(6.0 * u * t))
        .add(&d2.scale(3.0 * t2));

    tangent.normalized_or(Vec3::new(0.0, 0.0, 1.0))
}

/// Adaptively subdivides a cubic Bézier curve into discrete samples.
///
/// Splits segments recursively until both the chord length is less than or equal to
/// `max_segment_len_m` and the angular change between adjacent tangents is less than
/// or equal to `max_tangent_angle_rad`.
pub fn adaptive_sample(
    curve: &CubicBezier3,
    max_segment_len_m: f32,
    max_tangent_angle_rad: f32,
) -> Vec<BezierSample> {
    let max_len = if max_segment_len_m.is_finite() && max_segment_len_m > 0.01 {
        max_segment_len_m
    } else {
        0.5
    };
    let max_angle = if max_tangent_angle_rad.is_finite() && max_tangent_angle_rad > 0.001 {
        max_tangent_angle_rad
    } else {
        10.0f32.to_radians()
    };

    let mut t_values = vec![0.0f32];
    subdivide_interval(curve, 0.0, 1.0, max_len, max_angle, 0, &mut t_values);
    t_values.push(1.0);

    let mut samples = Vec::with_capacity(t_values.len());
    let mut total_len = 0.0f32;
    let mut prev_pos = bezier_point(curve, 0.0);

    for (i, &t) in t_values.iter().enumerate() {
        let pos = if t == 0.0 {
            prev_pos
        } else if (t - 1.0).abs() < 1e-6 {
            Vec3::new(curve.p3[0], curve.p3[1], curve.p3[2])
        } else {
            bezier_point(curve, t)
        };
        let tan = bezier_tangent(curve, t);
        if i > 0 {
            total_len += pos.subtract(&prev_pos).length();
        }
        samples.push(BezierSample {
            t,
            position: pos,
            tangent: tan,
            arc_length: total_len,
        });
        prev_pos = pos;
    }

    samples
}

fn subdivide_interval(
    curve: &CubicBezier3,
    t0: f32,
    t1: f32,
    max_len: f32,
    max_angle: f32,
    depth: usize,
    out: &mut Vec<f32>,
) {
    if depth > 12 || (t1 - t0) < 1e-4 {
        return;
    }
    let p0 = bezier_point(curve, t0);
    let p1 = bezier_point(curve, t1);
    let chord_len = p1.subtract(&p0).length();

    let mid_t = (t0 + t1) * 0.5;
    let tan0 = bezier_tangent(curve, t0);
    let tan1 = bezier_tangent(curve, t1);
    let tan_mid = bezier_tangent(curve, mid_t);

    let dot0 = tan0.dot_product(&tan_mid).clamp(-1.0, 1.0);
    let dot1 = tan_mid.dot_product(&tan1).clamp(-1.0, 1.0);
    let angle0 = dot0.acos();
    let angle1 = dot1.acos();
    let angle_diff = angle0.max(angle1);

    if chord_len > max_len || angle_diff > max_angle {
        subdivide_interval(curve, t0, mid_t, max_len, max_angle, depth + 1, out);
        out.push(mid_t);
        subdivide_interval(curve, mid_t, t1, max_len, max_angle, depth + 1, out);
    }
}

/// Constructs a rotation [`Quaternion`] aligning local `+Z` (forward) with `tangent`,
/// keeping local `+Y` (up) aligned with world `+Y` where possible.
pub fn tangent_to_rotation(tangent: &Vec3) -> Quaternion {
    let t = tangent.normalized_or(Vec3::new(0.0, 0.0, 1.0));
    let up = if t.y.abs() > 0.99 {
        Vec3::new(0.0, 0.0, 1.0)
    } else {
        Vec3::UP
    };
    let right = up.cross_product(&t).normalized_or(Vec3::new(1.0, 0.0, 0.0));
    let true_up = t.cross_product(&right).normalized_or(Vec3::UP);

    // Matrix columns: X = right, Y = true_up, Z = tangent
    let r00 = right.x;
    let r10 = right.y;
    let r20 = right.z;

    let r01 = true_up.x;
    let r11 = true_up.y;
    let r21 = true_up.z;

    let r02 = t.x;
    let r12 = t.y;
    let r22 = t.z;

    let trace = r00 + r11 + r22;
    if trace > 0.0 {
        let s = (trace + 1.0).sqrt() * 2.0;
        let inv_s = 1.0 / s;
        Quaternion::new(
            (r21 - r12) * inv_s,
            (r02 - r20) * inv_s,
            (r10 - r01) * inv_s,
            0.25 * s,
        )
        .normalized()
    } else if (r00 > r11) && (r00 > r22) {
        let s = (1.0 + r00 - r11 - r22).max(1e-8).sqrt() * 2.0;
        let inv_s = 1.0 / s;
        Quaternion::new(
            0.25 * s,
            (r01 + r10) * inv_s,
            (r02 + r20) * inv_s,
            (r21 - r12) * inv_s,
        )
        .normalized()
    } else if r11 > r22 {
        let s = (1.0 + r11 - r00 - r22).max(1e-8).sqrt() * 2.0;
        let inv_s = 1.0 / s;
        Quaternion::new(
            (r01 + r10) * inv_s,
            0.25 * s,
            (r12 + r21) * inv_s,
            (r02 - r20) * inv_s,
        )
        .normalized()
    } else {
        let s = (1.0 + r22 - r00 - r11).max(1e-8).sqrt() * 2.0;
        let inv_s = 1.0 / s;
        Quaternion::new(
            (r02 + r20) * inv_s,
            (r12 + r21) * inv_s,
            0.25 * s,
            (r10 - r01) * inv_s,
        )
        .normalized()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bezier_endpoints() {
        let curve = CubicBezier3::new(
            [0.0, 1.0, 2.0],
            [1.0, 5.0, 2.0],
            [3.0, 5.0, 2.0],
            [4.0, 1.0, 2.0],
        );

        let start = bezier_point(&curve, 0.0);
        assert!((start.x - 0.0).abs() < 1e-6);
        assert!((start.y - 1.0).abs() < 1e-6);
        assert!((start.z - 2.0).abs() < 1e-6);

        let end = bezier_point(&curve, 1.0);
        assert!((end.x - 4.0).abs() < 1e-6);
        assert!((end.y - 1.0).abs() < 1e-6);
        assert!((end.z - 2.0).abs() < 1e-6);
    }

    #[test]
    fn test_bezier_tangent_endpoints() {
        let curve = CubicBezier3::new(
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [2.0, 1.0, 0.0],
            [3.0, 1.0, 0.0],
        );

        let tan_start = bezier_tangent(&curve, 0.0);
        assert!((tan_start.x - 1.0).abs() < 1e-5);
        assert!(tan_start.y.abs() < 1e-5);
        assert!(tan_start.z.abs() < 1e-5);

        let tan_end = bezier_tangent(&curve, 1.0);
        assert!((tan_end.x - 1.0).abs() < 1e-5);
        assert!(tan_end.y.abs() < 1e-5);
        assert!(tan_end.z.abs() < 1e-5);
    }

    #[test]
    fn test_adaptive_sample_straight_line() {
        let curve = CubicBezier3::new(
            [0.0, 0.0, 0.0],
            [0.0, 0.0, 10.0],
            [0.0, 0.0, 20.0],
            [0.0, 0.0, 30.0],
        );

        let samples = adaptive_sample(&curve, 5.0, 10.0f32.to_radians());
        assert!(samples.len() >= 6);
        assert_eq!(samples.first().unwrap().t, 0.0);
        assert_eq!(samples.last().unwrap().t, 1.0);
        assert!((samples.last().unwrap().arc_length - 30.0).abs() < 1e-2);
    }

    #[test]
    fn test_adaptive_sample_curved_arc() {
        // Quarter circle arc in XZ plane
        let r = 10.0;
        let c = 0.55228475 * r; // standard cubic bezier approximation for quarter circle
        let curve = CubicBezier3::new([r, 0.0, 0.0], [r, 0.0, c], [c, 0.0, r], [0.0, 0.0, r]);

        let samples = adaptive_sample(&curve, 2.0, 5.0f32.to_radians());
        // For 5 deg tolerance, 90 deg curve subdivides to at least 16 segments (17 sample points)
        assert!(samples.len() >= 17);
        assert_eq!(samples.first().unwrap().t, 0.0);
        assert_eq!(samples.last().unwrap().t, 1.0);

        // Arc length of quarter circle radius 10 is 0.5 * PI * 10 ≈ 15.7079
        let expected_arc_len = 0.5 * std::f32::consts::PI * r;
        let arc_diff = (samples.last().unwrap().arc_length - expected_arc_len).abs();
        assert!(arc_diff < 0.2, "arc length difference was {arc_diff}");
    }

    #[test]
    fn test_tangent_to_rotation_aligns_forward() {
        let tangent = Vec3::new(0.0, 0.0, 1.0);
        let rot = tangent_to_rotation(&tangent);
        let forward = rot.rotate_vec(&Vec3::new(0.0, 0.0, 1.0));
        assert!((forward.z - 1.0).abs() < 1e-4);

        let angled_tangent = Vec3::new(1.0, 0.0, 1.0).normalized_or(Vec3::UP);
        let rot_angled = tangent_to_rotation(&angled_tangent);
        let forward_angled = rot_angled.rotate_vec(&Vec3::new(0.0, 0.0, 1.0));
        assert!((forward_angled.x - angled_tangent.x).abs() < 1e-4);
        assert!((forward_angled.z - angled_tangent.z).abs() < 1e-4);
    }
}
