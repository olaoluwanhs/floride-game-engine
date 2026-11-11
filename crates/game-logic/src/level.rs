//! Renderer-agnostic level world specification and physics spawning helpers.
//!
//! A [`LevelWorldSpec`] describes pure spatial geometry, dynamic props,
//! non-blocking trigger volumes, and authored placement markers (such as spawns
//! or waypoints). Game-specific crates wrap or embed [`LevelWorldSpec`] inside
//! their own level schemas alongside mode rules, enemies, and pickups.

use game_types::{EntityId, Vec3};
use physics_api::{
    DynamicBodyDesc, HeightFieldDesc, PhantomBehavior, PhantomColliderDesc, PhysicsError,
    PhysicsHandle, PhysicsTransform, PhysicsWorld, PrimitiveShape, Quaternion, StaticColliderDesc,
};
use serde::{Deserialize, Serialize};

/// Current schema version for [`LevelWorldSpec`].
pub const LEVEL_WORLD_SCHEMA_VERSION: u32 = 1;

/// Normalized RGBA color tint used for authoring and visualization metadata.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct LevelColor {
    /// Red channel in `0.0..=1.0`.
    pub r: f32,
    /// Green channel in `0.0..=1.0`.
    pub g: f32,
    /// Blue channel in `0.0..=1.0`.
    pub b: f32,
    /// Alpha channel in `0.0..=1.0`.
    pub a: f32,
}

impl LevelColor {
    /// Creates an opaque RGB color with alpha set to `1.0`.
    pub const fn rgb(r: f32, g: f32, b: f32) -> Self {
        Self { r, g, b, a: 1.0 }
    }

    /// Creates an RGBA color from normalized channel components.
    pub const fn rgba(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self { r, g, b, a }
    }
}

impl Default for LevelColor {
    fn default() -> Self {
        Self::rgb(0.6, 0.6, 0.65)
    }
}

/// An immovable collider in the level, such as ground planes, walls, or cover blocks.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LevelStaticObject {
    /// Optional game-owned entity ID so hits can identify this surface.
    #[serde(default)]
    pub id: Option<EntityId>,
    /// Human-readable label shown in editors and diagnostics.
    pub name: String,
    /// Collision shape and dimensions.
    pub shape: PrimitiveShape,
    /// World position and rotation.
    pub transform: PhysicsTransform,
    /// Suggested editor/visualizer color tint.
    #[serde(default)]
    pub color: LevelColor,
}

/// A movable physical prop in the level, such as a wooden crate or heavy barrel.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LevelDynamicObject {
    /// Game-owned entity ID used to track body transforms and damage hits.
    pub id: EntityId,
    /// Human-readable label shown in editors and diagnostics.
    pub name: String,
    /// Collision shape and dimensions.
    pub shape: PrimitiveShape,
    /// Starting world position and rotation.
    pub transform: PhysicsTransform,
    /// Body mass in kilograms (must be positive).
    pub mass_kg: f32,
    /// Surface friction coefficient.
    pub friction: f32,
    /// Restitution (bounciness) in `0.0..=1.0`.
    pub elasticity: f32,
    /// Suggested editor/visualizer color tint.
    #[serde(default)]
    pub color: LevelColor,
}

/// A non-blocking sensor or raycast volume placed in the world.
///
/// Games use trigger volumes for checkpoints, extraction zones, ambushes, or
/// shooting targets without obstructing character movement.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LevelTriggerVolume {
    /// Game-owned entity ID reported in phantom overlap or ray hits.
    pub id: EntityId,
    /// Unique identifier or friendly name (for example `"extraction_zone"`).
    pub name: String,
    /// Optional category tag (for example `"objective"`, `"checkpoint"`, `"hazard"`).
    #[serde(default)]
    pub tag: String,
    /// Volume shape and dimensions.
    pub shape: PrimitiveShape,
    /// World position and rotation.
    pub transform: PhysicsTransform,
    /// Whether this volume detects overlaps, raycasts, or both.
    pub behavior: PhantomBehavior,
    /// Suggested translucent tint in level editors or debug views.
    #[serde(default)]
    pub color: LevelColor,
}

/// A generic authored marker in world space, such as a spawn point or waypoint.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LevelMarker {
    /// Human-readable name for the marker.
    pub name: String,
    /// Marker role understood by the game layer (for example `"player_spawn"`).
    pub kind: String,
    /// Marker world position and facing rotation.
    pub transform: PhysicsTransform,
}

/// Renderer-independent specification of a level's physical world.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LevelWorldSpec {
    /// Format version for forward-compatible deserialization.
    #[serde(default = "default_schema_version")]
    pub schema_version: u32,
    /// Stable slug identifying this level.
    pub level_id: String,
    /// Display title for menus and editor headers.
    pub title: String,
    /// Static colliders such as floors, ramps, walls, and pillars.
    #[serde(default)]
    pub static_objects: Vec<LevelStaticObject>,
    /// Movable rigid bodies such as crates, barrels, and physics props.
    #[serde(default)]
    pub dynamic_objects: Vec<LevelDynamicObject>,
    /// Non-blocking phantom trigger volumes.
    #[serde(default)]
    pub triggers: Vec<LevelTriggerVolume>,
    /// Named markers for spawns, checkpoints, or patrol points.
    #[serde(default)]
    pub markers: Vec<LevelMarker>,
}

fn default_schema_version() -> u32 {
    LEVEL_WORLD_SCHEMA_VERSION
}

impl Default for LevelWorldSpec {
    fn default() -> Self {
        Self {
            schema_version: LEVEL_WORLD_SCHEMA_VERSION,
            level_id: "untitled_level".to_owned(),
            title: "Untitled Level".to_owned(),
            static_objects: Vec::new(),
            dynamic_objects: Vec::new(),
            triggers: Vec::new(),
            markers: Vec::new(),
        }
    }
}

impl LevelWorldSpec {
    /// Iterates over all markers matching `kind`.
    pub fn markers_of_kind<'a>(&'a self, kind: &'a str) -> impl Iterator<Item = &'a LevelMarker> {
        self.markers.iter().filter(move |marker| marker.kind == kind)
    }

    /// Returns the first marker matching `kind`, if present.
    pub fn first_marker(&self, kind: &str) -> Option<&LevelMarker> {
        self.markers.iter().find(|marker| marker.kind == kind)
    }

    /// Queries the ground surface elevation at `(world_x, world_z)` across static
    /// [`PrimitiveShape::HeightField`] and [`PrimitiveShape::Plane`] colliders.
    pub fn surface_height_at(&self, world_x: f32, world_z: f32) -> Option<f32> {
        let mut highest: Option<f32> = None;

        for obj in &self.static_objects {
            let candidate = match &obj.shape {
                PrimitiveShape::HeightField(desc) => {
                    sample_heightfield_y(desc, &obj.transform, world_x, world_z)
                }
                PrimitiveShape::Plane { half_extents } => {
                    let local = obj.transform.rotation.inverse_rotate_vec(&Vec3::new(
                        world_x - obj.transform.position.x,
                        0.0,
                        world_z - obj.transform.position.z,
                    ));
                    let within = match half_extents {
                        Some(ext) => {
                            local.x.abs() <= ext.half_width_x && local.z.abs() <= ext.half_depth_z
                        }
                        None => true,
                    };
                    within.then_some(obj.transform.position.y)
                }
                _ => None,
            };

            if let Some(y) = candidate {
                highest = Some(highest.map_or(y, |prev| prev.max(y)));
            }
        }

        highest
    }

    /// Samples minimum, maximum, center elevation, and upward slope normal across an XZ footprint.
    pub fn sample_footprint_stats(
        &self,
        center_x: f32,
        center_z: f32,
        half_x: f32,
        half_z: f32,
    ) -> Option<TerrainFootprintStats> {
        let hx = half_x.abs().max(0.05);
        let hz = half_z.abs().max(0.05);
        let steps_x = ((hx * 2.0 / 0.5).ceil() as usize).clamp(2, 8);
        let steps_z = ((hz * 2.0 / 0.5).ceil() as usize).clamp(2, 8);

        let mut min_y = f32::INFINITY;
        let mut max_y = f32::NEG_INFINITY;
        let mut found = false;

        for iz in 0..=steps_z {
            let fz = (iz as f32 / steps_z as f32) * 2.0 - 1.0;
            let sz = center_z + fz * hz;
            for ix in 0..=steps_x {
                let fx = (ix as f32 / steps_x as f32) * 2.0 - 1.0;
                let sx = center_x + fx * hx;
                if let Some(y) = self.surface_height_at(sx, sz) {
                    min_y = min_y.min(y);
                    max_y = max_y.max(y);
                    found = true;
                }
            }
        }

        if !found {
            return None;
        }

        let center_y = self
            .surface_height_at(center_x, center_z)
            .unwrap_or((min_y + max_y) * 0.5);

        let eps = hx.min(hz).clamp(0.25, 1.5);
        let dx = match (
            self.surface_height_at(center_x - eps, center_z),
            self.surface_height_at(center_x + eps, center_z),
        ) {
            (Some(hl), Some(hr)) => (hr - hl) / (2.0 * eps),
            (None, Some(hr)) => (hr - center_y) / eps,
            (Some(hl), None) => (center_y - hl) / eps,
            (None, None) => 0.0,
        };
        let dz = match (
            self.surface_height_at(center_x, center_z - eps),
            self.surface_height_at(center_x, center_z + eps),
        ) {
            (Some(hd), Some(hu)) => (hu - hd) / (2.0 * eps),
            (None, Some(hu)) => (hu - center_y) / eps,
            (Some(hd), None) => (center_y - hd) / eps,
            (None, None) => 0.0,
        };

        let normal = Vec3::new(-dx, 1.0, -dz).normalized_or(Vec3::UP);

        Some(TerrainFootprintStats {
            min_y,
            max_y,
            center_y,
            normal,
        })
    }

    /// Computes a non-clipping, slope-aligned [`PhysicsTransform`] for a dynamic body
    /// (`Box`, `Sphere`, `Cylinder`, or `Capsule`) centered horizontally at `(center_x, center_z)`.
    pub fn seat_dynamic_transform(
        &self,
        shape: &PrimitiveShape,
        center_x: f32,
        center_z: f32,
    ) -> PhysicsTransform {
        self.seat_dynamic_transform_with_yaw(shape, center_x, center_z, 0.0)
    }

    /// Computes a non-clipping, slope-aligned [`PhysicsTransform`] for a dynamic body
    /// while preserving an authored horizontal facing rotation (`yaw_radians` around `+Y`).
    pub fn seat_dynamic_transform_with_yaw(
        &self,
        shape: &PrimitiveShape,
        center_x: f32,
        center_z: f32,
        yaw_radians: f32,
    ) -> PhysicsTransform {
        let clearance = 0.015;
        let yaw_q = Quaternion::from_euler_yxz(yaw_radians, 0.0, 0.0);
        let (hx, hz) = shape_rotated_footprint_extents(shape, &yaw_q);
        let stats = self
            .sample_footprint_stats(center_x, center_z, hx, hz)
            .unwrap_or_default();

        if let PrimitiveShape::Sphere { radius } = shape {
            let mut tf = PhysicsTransform::new(Vec3::new(center_x, 0.0, center_z), yaw_q);
            tf.position.y = self.sphere_clear_center_y(center_x, center_z, *radius, &stats, clearance);
            return tf;
        }

        let local_normal = yaw_q.inverse_rotate_vec(&stats.normal).normalized_or(Vec3::UP);
        let pitch = local_normal.z.atan2(local_normal.y);
        let cos_roll = (local_normal.y * local_normal.y + local_normal.z * local_normal.z)
            .sqrt();
        let roll = (-local_normal.x).atan2(cos_roll);
        let rotation = Quaternion::from_euler_yxz(yaw_radians, pitch, roll);
        let sample_points = shape_bottom_sample_points(shape);
        let mut req_center_y = stats.center_y + clearance;
        for local_pt in sample_points {
            let world_offset = rotation.rotate_vec(&local_pt);
            let ground_y = self
                .surface_height_at(center_x + world_offset.x, center_z + world_offset.z)
                .unwrap_or(stats.center_y);
            req_center_y = req_center_y.max(ground_y - world_offset.y + clearance);
        }

        PhysicsTransform::new(Vec3::new(center_x, req_center_y, center_z), rotation)
    }

    /// Preserves `transform.rotation` (all 3 Euler axes) and adjusts `transform.position.y`
    /// so every point on the rotated `shape` hull sits at least `clearance` meters above the floor.
    pub fn seat_transform_to_floor_preserving_rotation(
        &self,
        shape: &PrimitiveShape,
        transform: &mut PhysicsTransform,
        clearance: f32,
    ) {
        let (hx, hz) = shape_rotated_footprint_extents(shape, &transform.rotation);
        let stats = self
            .sample_footprint_stats(transform.position.x, transform.position.z, hx, hz)
            .unwrap_or_default();

        if let PrimitiveShape::Sphere { radius } = shape {
            transform.position.y = self.sphere_clear_center_y(
                transform.position.x,
                transform.position.z,
                *radius,
                &stats,
                clearance,
            );
            return;
        }

        let sample_points = shape_hull_sample_points(shape);
        let mut req_center_y = f32::NEG_INFINITY;
        for local_pt in sample_points {
            let world_offset = transform.rotation.rotate_vec(&local_pt);
            let ground_y = self
                .surface_height_at(
                    transform.position.x + world_offset.x,
                    transform.position.z + world_offset.z,
                )
                .unwrap_or(stats.center_y);
            req_center_y = req_center_y.max(ground_y - world_offset.y + clearance);
        }
        if req_center_y.is_finite() {
            transform.position.y = req_center_y;
        }
    }

    fn sphere_clear_center_y(
        &self,
        center_x: f32,
        center_z: f32,
        radius: f32,
        stats: &TerrainFootprintStats,
        clearance: f32,
    ) -> f32 {
        let r = radius.abs().max(0.05);
        let steps = 4usize;
        let mut req_center_y = stats.center_y + r / stats.normal.y.max(0.25) + clearance;
        for iz in 0..=steps {
            let dz = ((iz as f32 / steps as f32) * 2.0 - 1.0) * r;
            for ix in 0..=steps {
                let dx = ((ix as f32 / steps as f32) * 2.0 - 1.0) * r;
                let r_sq = dx * dx + dz * dz;
                if r_sq > r * r {
                    continue;
                }
                if let Some(gy) = self.surface_height_at(center_x + dx, center_z + dz) {
                    let sphere_bottom_drop = (r * r - r_sq).sqrt();
                    req_center_y = req_center_y.max(gy + sphere_bottom_drop + clearance);
                }
            }
        }
        req_center_y
    }

    /// Seats an immovable static prop on terrain:
    /// - When upright (only Yaw rotation), keeps its Yaw while extending its foundation bottom down
    ///   to `stats.min_y` and placing its top at `stats.max_y + desired_height`.
    /// - When tilted (non-zero Pitch or Roll, e.g., an angled ramp), preserves full 3D rotation and
    ///   authored `desired_height` while seating its lowest rotated point cleanly on the floor.
    ///
    /// Floor shapes (`Plane`, `HeightField`) are left untouched.
    pub fn seat_static_upright(&self, obj: &mut LevelStaticObject, desired_height: f32) {
        if matches!(
            obj.shape,
            PrimitiveShape::Plane { .. } | PrimitiveShape::HeightField(..)
        ) {
            return;
        }
        self.seat_upright_shape_to_floor(
            &mut obj.shape,
            &mut obj.transform,
            desired_height.max(0.4),
            0.2,
            0.25,
        );
    }

    /// Seats a non-blocking trigger volume across `[min_y .. max_y + desired_height]`
    /// beneath its rotated XZ footprint, preserving Yaw (or full 3D rotation if tilted).
    pub fn seat_trigger_to_floor(&self, trig: &mut LevelTriggerVolume, desired_height: f32) {
        self.seat_upright_shape_to_floor(
            &mut trig.shape,
            &mut trig.transform,
            desired_height.max(0.5),
            0.25,
            0.25,
        );
    }

    fn seat_upright_shape_to_floor(
        &self,
        shape: &mut PrimitiveShape,
        transform: &mut PhysicsTransform,
        desired_height: f32,
        min_box_half_y: f32,
        min_cyl_half_y: f32,
    ) {
        let (yaw, pitch, roll) = transform.rotation.to_euler_yxz();
        let is_upright = pitch.abs() < 1.0e-3 && roll.abs() < 1.0e-3;

        if !is_upright {
            match shape {
                PrimitiveShape::Box { half_extents } => {
                    half_extents.y = (desired_height * 0.5).max(min_box_half_y);
                }
                PrimitiveShape::Cylinder { half_height, .. } => {
                    *half_height = (desired_height * 0.5).max(min_cyl_half_y);
                }
                _ => {}
            }
            self.seat_transform_to_floor_preserving_rotation(shape, transform, 0.0);
            return;
        }

        let yaw_q = Quaternion::from_euler_yxz(yaw, 0.0, 0.0);
        let (hx, hz) = shape_rotated_footprint_extents(shape, &yaw_q);
        let stats = self
            .sample_footprint_stats(transform.position.x, transform.position.z, hx, hz)
            .unwrap_or_default();
        let bottom = stats.min_y;
        let top = stats.max_y + desired_height;
        match shape {
            PrimitiveShape::Box { half_extents } => {
                half_extents.y = ((top - bottom) * 0.5).max(min_box_half_y);
                transform.position.y = (top + bottom) * 0.5;
                transform.rotation = yaw_q;
            }
            PrimitiveShape::Cylinder { half_height, .. } => {
                *half_height = ((top - bottom) * 0.5).max(min_cyl_half_y);
                transform.position.y = (top + bottom) * 0.5;
                transform.rotation = yaw_q;
            }
            PrimitiveShape::Sphere { radius } => {
                transform.position.y = stats.max_y + radius.abs();
                transform.rotation = yaw_q;
            }
            PrimitiveShape::Capsule {
                radius,
                half_height,
            } => {
                transform.position.y = stats.max_y + half_height.abs() + radius.abs();
                transform.rotation = yaw_q;
            }
            _ => {}
        }
    }
}

/// Ground elevation summary across an XZ footprint, returned by [`LevelWorldSpec::sample_footprint_stats`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TerrainFootprintStats {
    /// Lowest ground point beneath the footprint.
    pub min_y: f32,
    /// Highest ground point beneath the footprint.
    pub max_y: f32,
    /// Ground point at the horizontal center of the footprint.
    pub center_y: f32,
    /// Average unit upward surface normal of the ground under the footprint.
    pub normal: Vec3,
}

impl Default for TerrainFootprintStats {
    fn default() -> Self {
        Self {
            min_y: 0.0,
            max_y: 0.0,
            center_y: 0.0,
            normal: Vec3::UP,
        }
    }
}

/// Computes axis-aligned `(half_x, half_z)` footprint extents of `shape` after applying `rotation`.
pub fn shape_rotated_footprint_extents(
    shape: &PrimitiveShape,
    rotation: &Quaternion,
) -> (f32, f32) {
    match shape {
        PrimitiveShape::Box { half_extents } => {
            let ax = rotation.rotate_vec(&Vec3::new(half_extents.x.abs(), 0.0, 0.0));
            let ay = rotation.rotate_vec(&Vec3::new(0.0, half_extents.y.abs(), 0.0));
            let az = rotation.rotate_vec(&Vec3::new(0.0, 0.0, half_extents.z.abs()));
            (
                ax.x.abs() + ay.x.abs() + az.x.abs(),
                ax.z.abs() + ay.z.abs() + az.z.abs(),
            )
        }
        PrimitiveShape::Cylinder {
            radius,
            half_height,
        } => {
            let r = radius.abs();
            let hh = half_height.abs();
            let axis = rotation.rotate_vec(&Vec3::UP);
            (
                hh * axis.x.abs() + r * (1.0 - axis.x * axis.x).max(0.0).sqrt(),
                hh * axis.z.abs() + r * (1.0 - axis.z * axis.z).max(0.0).sqrt(),
            )
        }
        PrimitiveShape::Capsule {
            radius,
            half_height,
        } => {
            let r = radius.abs();
            let hh = half_height.abs();
            let axis = rotation.rotate_vec(&Vec3::UP);
            (hh * axis.x.abs() + r, hh * axis.z.abs() + r)
        }
        PrimitiveShape::Sphere { radius } => (radius.abs(), radius.abs()),
        PrimitiveShape::Torus {
            major_radius,
            tube_radius,
        } => {
            let ring_r = major_radius.abs() + tube_radius.abs();
            let tube_r = tube_radius.abs();
            let axis = rotation.rotate_vec(&Vec3::UP);
            (
                tube_r * axis.x.abs() + ring_r * (1.0 - axis.x * axis.x).max(0.0).sqrt(),
                tube_r * axis.z.abs() + ring_r * (1.0 - axis.z * axis.z).max(0.0).sqrt(),
            )
        }
        _ => (0.5, 0.5),
    }
}

fn shape_bottom_sample_points(shape: &PrimitiveShape) -> Vec<Vec3> {
    let mut points = Vec::new();
    match shape {
        PrimitiveShape::Box { half_extents } => {
            let hx = half_extents.x.abs();
            let hy = -half_extents.y.abs();
            let hz = half_extents.z.abs();
            for iz in [-1.0_f32, 0.0, 1.0] {
                for ix in [-1.0_f32, 0.0, 1.0] {
                    points.push(Vec3::new(ix * hx, hy, iz * hz));
                }
            }
        }
        PrimitiveShape::Cylinder {
            radius,
            half_height,
        } => {
            let r = radius.abs();
            let hy = -half_height.abs();
            points.push(Vec3::new(0.0, hy, 0.0));
            for s in 0..8 {
                let angle = std::f32::consts::TAU * s as f32 / 8.0;
                points.push(Vec3::new(r * angle.cos(), hy, r * angle.sin()));
            }
        }
        PrimitiveShape::Capsule {
            radius,
            half_height,
        } => {
            points.push(Vec3::new(0.0, -(half_height.abs() + radius.abs()), 0.0));
        }
        _ => points.push(Vec3::ZERO),
    }
    points
}

fn shape_hull_sample_points(shape: &PrimitiveShape) -> Vec<Vec3> {
    let mut points = shape_bottom_sample_points(shape);
    match shape {
        PrimitiveShape::Box { half_extents } => {
            let hx = half_extents.x.abs();
            let hy = half_extents.y.abs();
            let hz = half_extents.z.abs();
            for iz in [-1.0_f32, 0.0, 1.0] {
                for ix in [-1.0_f32, 0.0, 1.0] {
                    points.push(Vec3::new(ix * hx, hy, iz * hz));
                }
            }
        }
        PrimitiveShape::Cylinder {
            radius,
            half_height,
        } => {
            let r = radius.abs();
            let hy = half_height.abs();
            points.push(Vec3::new(0.0, hy, 0.0));
            for s in 0..8 {
                let angle = std::f32::consts::TAU * s as f32 / 8.0;
                points.push(Vec3::new(r * angle.cos(), hy, r * angle.sin()));
            }
        }
        PrimitiveShape::Capsule {
            radius,
            half_height,
        } => {
            points.push(Vec3::new(0.0, half_height.abs() + radius.abs(), 0.0));
        }
        _ => {}
    }
    points
}

/// Procedural terrain shapes for generating uneven heightmap floors.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TerrainPreset {
    /// Level grid with all samples initialized to `0.0` (ready for manual sculpting).
    FlatGrid,
    /// Smooth undulating hills suitable for long sightline outdoor combat.
    RollingHills,
    /// Elevated outer rim sloping down toward a central tactical basin.
    CentralValley,
    /// Twin flanking ridges separated by a lower central lane.
    TwinRidges,
    /// Stepped multi-tier plateaus connecting low and high ground.
    TerracedArena,
}

impl Default for TerrainPreset {
    fn default() -> Self {
        Self::RollingHills
    }
}

/// Parameters controlling procedural heightfield terrain generation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TerrainFloorOptions {
    /// Object name assigned to the generated [`LevelStaticObject`].
    pub name: String,
    /// Number of sample rows along Z (clamped to `2..=45`).
    pub rows: usize,
    /// Number of sample columns along X (clamped to `2..=45`).
    pub columns: usize,
    /// Spacing between adjacent columns along X, in meters.
    pub cell_size_x: f32,
    /// Spacing between adjacent rows along Z, in meters.
    pub cell_size_z: f32,
    /// Peak height variation in meters above/below `base_y`.
    pub amplitude: f32,
    /// Spatial pattern frequency multiplier (`1.0` = standard arena scale).
    pub frequency: f32,
    /// Base elevation offset in meters.
    pub base_y: f32,
    /// Optional solid downward extrusion thickness in meters.
    pub thickness: Option<f32>,
    /// Suggested editor/visualizer color tint.
    pub color: LevelColor,
}

impl Default for TerrainFloorOptions {
    fn default() -> Self {
        Self {
            name: "terrain_floor".to_owned(),
            rows: 29,
            columns: 29,
            cell_size_x: 2.5,
            cell_size_z: 2.5,
            amplitude: 2.4,
            frequency: 1.0,
            base_y: 0.0,
            thickness: Some(1.5),
            color: LevelColor::rgb(0.28, 0.36, 0.26),
        }
    }
}

/// Generates a centered uneven heightmap floor as a [`LevelStaticObject`].
///
/// The resulting heightfield spans `[ -width/2 .. +width/2 ]` in X and
/// `[ -depth/2 .. +depth/2 ]` in Z around `(0, base_y, 0)`.
pub fn generate_heightfield_floor(
    preset: TerrainPreset,
    options: &TerrainFloorOptions,
) -> LevelStaticObject {
    let rows = options.rows.clamp(2, 45);
    let columns = options.columns.clamp(2, 45);
    let cell_size_x = options.cell_size_x.max(0.25);
    let cell_size_z = options.cell_size_z.max(0.25);
    let amplitude = options.amplitude.max(0.0);
    let frequency = options.frequency.max(0.1);

    let mut samples = Vec::with_capacity(rows * columns);
    let max_row = (rows - 1) as f32;
    let max_col = (columns - 1) as f32;
    let pi = std::f32::consts::PI;

    for r in 0..rows {
        let nz = (r as f32 / max_row) * 2.0 - 1.0;
        for c in 0..columns {
            let nx = (c as f32 / max_col) * 2.0 - 1.0;
            let raw_height = match preset {
                TerrainPreset::FlatGrid => 0.0,
                TerrainPreset::RollingHills => {
                    let wave_a = (nx * pi * frequency * 1.25).sin()
                        * (nz * pi * frequency * 1.15).cos();
                    let wave_b = ((nx + nz) * pi * frequency * 0.85).cos();
                    amplitude * (0.6 * wave_a + 0.4 * wave_b)
                }
                TerrainPreset::CentralValley => {
                    let radial = (nx * nx + nz * nz).sqrt().min(1.2);
                    amplitude * (radial.powf(1.65) - 0.35)
                }
                TerrainPreset::TwinRidges => {
                    let left_ridge = (-((nx + 0.6) * 3.2).powi(2)).exp();
                    let right_ridge = (-((nx - 0.6) * 3.2).powi(2)).exp();
                    let pass_mod = 0.65 + 0.35 * (nz * pi * frequency).cos();
                    amplitude * (left_ridge + right_ridge) * pass_mod
                }
                TerrainPreset::TerracedArena => {
                    let radial = (nx * nx + nz * nz).sqrt().min(1.2);
                    let stepped = (radial * 4.0).floor() / 4.0;
                    let mound = ((nx * pi * 0.7).cos() * (nz * pi * 0.7).cos()).max(0.0);
                    amplitude * (0.55 * stepped + 0.45 * (mound * 3.0).floor() / 3.0)
                }
            };
            samples.push(raw_height);
        }
    }

    let width = cell_size_x * max_col;
    let depth = cell_size_z * max_row;
    let thickness = options.thickness.filter(|t| t.is_finite() && *t > 0.0);

    LevelStaticObject {
        id: None,
        name: options.name.clone(),
        shape: PrimitiveShape::HeightField(HeightFieldDesc {
            rows,
            columns,
            height_samples: samples,
            cell_size_x,
            cell_size_z,
            thickness,
        }),
        transform: PhysicsTransform::from_position(Vec3::new(
            -width * 0.5,
            options.base_y,
            -depth * 0.5,
        )),
        color: options.color,
    }
}

/// Samples the exact triangle height of a [`HeightFieldDesc`] at world `(world_x, world_z)`.
///
/// Uses the same `(row+1, col) <-> (row, col+1)` diagonal split as Rapier and
/// `floride-visual-demo` so physics placement matches rendered terrain triangles.
pub fn sample_heightfield_y(
    desc: &HeightFieldDesc,
    transform: &PhysicsTransform,
    world_x: f32,
    world_z: f32,
) -> Option<f32> {
    if desc.rows < 2
        || desc.columns < 2
        || desc.cell_size_x <= 0.0
        || desc.cell_size_z <= 0.0
        || desc.height_samples.len() != desc.rows * desc.columns
    {
        return None;
    }

    let delta = Vec3::new(
        world_x - transform.position.x,
        0.0,
        world_z - transform.position.z,
    );
    let local = transform.rotation.inverse_rotate_vec(&delta);

    let max_x = desc.cell_size_x * (desc.columns - 1) as f32;
    let max_z = desc.cell_size_z * (desc.rows - 1) as f32;
    if local.x < 0.0 || local.x > max_x || local.z < 0.0 || local.z > max_z {
        return None;
    }

    let gx = local.x / desc.cell_size_x;
    let gz = local.z / desc.cell_size_z;
    let col = (gx.floor() as usize).min(desc.columns - 2);
    let row = (gz.floor() as usize).min(desc.rows - 2);
    let u = (gx - col as f32).clamp(0.0, 1.0);
    let v = (gz - row as f32).clamp(0.0, 1.0);

    let h00 = desc.height_samples[row * desc.columns + col];
    let h01 = desc.height_samples[row * desc.columns + (col + 1)];
    let h10 = desc.height_samples[(row + 1) * desc.columns + col];
    let h11 = desc.height_samples[(row + 1) * desc.columns + (col + 1)];

    // Diagonal from p10 (u=0, v=1) to p01 (u=1, v=0):
    let local_h = if u + v <= 1.0 {
        h00 + u * (h01 - h00) + v * (h10 - h00)
    } else {
        h11 + (1.0 - u) * (h10 - h11) + (1.0 - v) * (h01 - h11)
    };

    Some(transform.position.y + local_h)
}

/// Runtime record for a spawned static collider from [`LevelWorldSpec`].
#[derive(Debug, Clone, PartialEq)]
pub struct SpawnedStaticObject {
    /// Physics handle returned by [`PhysicsWorld::create_static_collider`].
    pub handle: PhysicsHandle,
    /// Original static object specification.
    pub spec: LevelStaticObject,
}

/// Runtime record for a spawned dynamic rigid body from [`LevelWorldSpec`].
#[derive(Debug, Clone, PartialEq)]
pub struct SpawnedDynamicBody {
    /// Game entity ID assigned to the dynamic body.
    pub entity_id: EntityId,
    /// Physics handle returned by [`PhysicsWorld::create_dynamic_body`].
    pub handle: PhysicsHandle,
    /// Original dynamic object specification.
    pub spec: LevelDynamicObject,
}

/// Runtime record for a spawned phantom trigger volume from [`LevelWorldSpec`].
#[derive(Debug, Clone, PartialEq)]
pub struct SpawnedTrigger {
    /// Game entity ID assigned to the phantom collider.
    pub entity_id: EntityId,
    /// Physics handle returned by [`PhysicsWorld::create_phantom_collider`].
    pub handle: PhysicsHandle,
    /// Original trigger specification.
    pub spec: LevelTriggerVolume,
}

/// Collection of physics handles created by [`spawn_level_world`].
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SpawnedLevelWorld {
    /// All static colliders spawned from the level specification.
    pub static_objects: Vec<SpawnedStaticObject>,
    /// All dynamic rigid bodies spawned from the level specification.
    pub dynamic_bodies: Vec<SpawnedDynamicBody>,
    /// All phantom trigger volumes spawned from the level specification.
    pub triggers: Vec<SpawnedTrigger>,
}

impl SpawnedLevelWorld {
    /// Finds a spawned trigger volume by its authored `name`.
    pub fn trigger_by_name(&self, name: &str) -> Option<&SpawnedTrigger> {
        self.triggers
            .iter()
            .find(|trigger| trigger.spec.name == name)
    }

    /// Finds a spawned trigger volume by its [`EntityId`].
    pub fn trigger_by_id(&self, entity_id: EntityId) -> Option<&SpawnedTrigger> {
        self.triggers
            .iter()
            .find(|trigger| trigger.entity_id == entity_id)
    }
}

/// Populates `physics` with all static colliders, dynamic bodies, and trigger
/// volumes described in `spec`.
pub fn spawn_level_world(
    physics: &mut dyn PhysicsWorld,
    spec: &LevelWorldSpec,
) -> Result<SpawnedLevelWorld, PhysicsError> {
    let mut spawned = SpawnedLevelWorld::default();

    for static_obj in &spec.static_objects {
        let handle = physics.create_static_collider(&StaticColliderDesc {
            entity_id: static_obj.id,
            transform: static_obj.transform,
            shape: static_obj.shape.clone(),
        })?;
        spawned.static_objects.push(SpawnedStaticObject {
            handle,
            spec: static_obj.clone(),
        });
    }

    for dynamic_obj in &spec.dynamic_objects {
        let handle = physics.create_dynamic_body(&DynamicBodyDesc {
            entity_id: dynamic_obj.id,
            transform: dynamic_obj.transform,
            shape: dynamic_obj.shape.clone(),
            mass_kg: dynamic_obj.mass_kg,
            friction: dynamic_obj.friction,
            elasticity: dynamic_obj.elasticity,
        })?;
        spawned.dynamic_bodies.push(SpawnedDynamicBody {
            entity_id: dynamic_obj.id,
            handle,
            spec: dynamic_obj.clone(),
        });
    }

    for trigger in &spec.triggers {
        let handle = physics.create_phantom_collider(&PhantomColliderDesc {
            entity_id: trigger.id,
            transform: trigger.transform,
            shape: trigger.shape.clone(),
            behavior: trigger.behavior,
        })?;
        spawned.triggers.push(SpawnedTrigger {
            entity_id: trigger.id,
            handle,
            spec: trigger.clone(),
        });
    }

    Ok(spawned)
}

#[cfg(test)]
mod tests {
    use super::{
        LEVEL_WORLD_SCHEMA_VERSION, LevelColor, LevelDynamicObject, LevelMarker, LevelStaticObject,
        LevelTriggerVolume, LevelWorldSpec, TerrainFloorOptions, TerrainPreset,
        generate_heightfield_floor, spawn_level_world,
    };
    use game_types::{EntityId, Vec3};
    use physics_api::{
        CharacterDesc, CharacterMoveResult, DynamicBodyDesc, HeightFieldDesc, PhantomBehavior,
        PhantomColliderDesc, PhantomOverlap, PhantomRayHit, PhysicsError, PhysicsHandle,
        PhysicsTransform, PhysicsWorld, PlaneHalfExtents, PrimitiveShape, Quaternion, RayHit,
        RayQuery, StaticColliderDesc,
    };

    #[derive(Default)]
    struct RecordingWorld {
        next_handle: u64,
        static_count: usize,
        dynamic_count: usize,
        phantom_count: usize,
    }

    impl RecordingWorld {
        fn alloc_handle(&mut self) -> PhysicsHandle {
            self.next_handle += 1;
            PhysicsHandle::new(self.next_handle)
        }
    }

    impl PhysicsWorld for RecordingWorld {
        fn create_static_collider(
            &mut self,
            _: &StaticColliderDesc,
        ) -> Result<PhysicsHandle, PhysicsError> {
            self.static_count += 1;
            Ok(self.alloc_handle())
        }
        fn remove_static_collider(&mut self, _: PhysicsHandle) -> Result<(), PhysicsError> {
            Ok(())
        }
        fn create_phantom_collider(
            &mut self,
            _: &PhantomColliderDesc,
        ) -> Result<PhysicsHandle, PhysicsError> {
            self.phantom_count += 1;
            Ok(self.alloc_handle())
        }
        fn set_phantom_transform(
            &mut self,
            _: PhysicsHandle,
            _: &PhysicsTransform,
        ) -> Result<(), PhysicsError> {
            Ok(())
        }
        fn remove_phantom_collider(&mut self, _: PhysicsHandle) -> Result<(), PhysicsError> {
            Ok(())
        }
        fn phantom_overlaps(&self, _: PhysicsHandle) -> Result<Vec<PhantomOverlap>, PhysicsError> {
            Ok(Vec::new())
        }
        fn create_dynamic_body(
            &mut self,
            _: &DynamicBodyDesc,
        ) -> Result<PhysicsHandle, PhysicsError> {
            self.dynamic_count += 1;
            Ok(self.alloc_handle())
        }
        fn remove_dynamic_body(&mut self, _: PhysicsHandle) -> Result<(), PhysicsError> {
            Ok(())
        }
        fn apply_impulse(&mut self, _: PhysicsHandle, _: &Vec3) -> Result<(), PhysicsError> {
            Ok(())
        }
        fn apply_impulse_at_point(
            &mut self,
            _: PhysicsHandle,
            _: &Vec3,
            _: &Vec3,
        ) -> Result<(), PhysicsError> {
            Ok(())
        }
        fn create_character(&mut self, _: &CharacterDesc) -> Result<PhysicsHandle, PhysicsError> {
            Ok(self.alloc_handle())
        }
        fn remove_character(&mut self, _: PhysicsHandle) -> Result<(), PhysicsError> {
            Ok(())
        }
        fn move_character(
            &mut self,
            _: PhysicsHandle,
            _: &Vec3,
        ) -> Result<CharacterMoveResult, PhysicsError> {
            Ok(CharacterMoveResult {
                applied_displacement: Vec3::ZERO,
                is_grounded: true,
                collided: false,
            })
        }
        fn is_grounded(&self, _: PhysicsHandle) -> Result<bool, PhysicsError> {
            Ok(true)
        }
        fn raycast(&self, _: &RayQuery) -> Result<Option<RayHit>, PhysicsError> {
            Ok(None)
        }
        fn raycast_phantoms(&self, _: &RayQuery) -> Result<Vec<PhantomRayHit>, PhysicsError> {
            Ok(Vec::new())
        }
        fn step(&mut self, _: f32) -> Result<(), PhysicsError> {
            Ok(())
        }
        fn body_transform(&self, _: PhysicsHandle) -> Result<PhysicsTransform, PhysicsError> {
            Ok(PhysicsTransform::IDENTITY)
        }
        fn dynamic_bodies_in_radius(
            &self,
            _: &Vec3,
            _: f32,
        ) -> Result<Vec<(EntityId, PhysicsHandle, Vec3)>, PhysicsError> {
            Ok(Vec::new())
        }
    }

    #[test]
    fn spawn_level_world_registers_all_object_categories_and_lookups() {
        let mut world = RecordingWorld::default();
        let spec = LevelWorldSpec {
            schema_version: LEVEL_WORLD_SCHEMA_VERSION,
            level_id: "arena_01".to_owned(),
            title: "Training Arena".to_owned(),
            static_objects: vec![LevelStaticObject {
                id: Some(EntityId::new(1)),
                name: "floor".to_owned(),
                shape: PrimitiveShape::Plane {
                    half_extents: Some(PlaneHalfExtents {
                        half_width_x: 20.0,
                        half_depth_z: 20.0,
                    }),
                },
                transform: PhysicsTransform::IDENTITY,
                color: LevelColor::rgb(0.3, 0.35, 0.4),
            }],
            dynamic_objects: vec![LevelDynamicObject {
                id: EntityId::new(10),
                name: "crate_a".to_owned(),
                shape: PrimitiveShape::Box {
                    half_extents: Vec3::new(0.5, 0.5, 0.5),
                },
                transform: PhysicsTransform::from_position(Vec3::new(2.0, 0.5, 0.0)),
                mass_kg: 12.0,
                friction: 0.7,
                elasticity: 0.1,
                color: LevelColor::rgb(0.65, 0.45, 0.25),
            }],
            triggers: vec![LevelTriggerVolume {
                id: EntityId::new(20),
                name: "extract".to_owned(),
                tag: "objective".to_owned(),
                shape: PrimitiveShape::Cylinder {
                    half_height: 1.5,
                    radius: 2.0,
                },
                transform: PhysicsTransform::from_position(Vec3::new(0.0, 1.5, -10.0)),
                behavior: PhantomBehavior::OverlapSensor,
                color: LevelColor::rgba(0.2, 0.85, 0.35, 0.35),
            }],
            markers: vec![LevelMarker {
                name: "start".to_owned(),
                kind: "player_spawn".to_owned(),
                transform: PhysicsTransform::from_position(Vec3::new(0.0, 1.0, 5.0)),
            }],
        };

        let spawned = spawn_level_world(&mut world, &spec).expect("spawn should succeed");
        assert_eq!(world.static_count, 1);
        assert_eq!(world.dynamic_count, 1);
        assert_eq!(world.phantom_count, 1);
        assert_eq!(spawned.static_objects.len(), 1);
        assert_eq!(spawned.dynamic_bodies.len(), 1);
        assert_eq!(spawned.triggers.len(), 1);
        assert_eq!(
            spawned.trigger_by_name("extract").map(|t| t.entity_id),
            Some(EntityId::new(20))
        );
        assert_eq!(
            spawned
                .trigger_by_id(EntityId::new(20))
                .map(|t| t.spec.name.as_str()),
            Some("extract")
        );
        assert_eq!(
            spec.first_marker("player_spawn").map(|m| m.name.as_str()),
            Some("start")
        );
    }

    #[test]
    fn terrain_heightfield_generation_and_sampling_match_triangle_plane() {
        let floor = generate_heightfield_floor(
            TerrainPreset::RollingHills,
            &TerrainFloorOptions {
                rows: 11,
                columns: 11,
                cell_size_x: 2.0,
                cell_size_z: 2.0,
                amplitude: 2.0,
                frequency: 1.0,
                base_y: 0.5,
                thickness: Some(1.0),
                ..TerrainFloorOptions::default()
            },
        );
        let mut world_spec = LevelWorldSpec {
            static_objects: vec![floor.clone()],
            ..LevelWorldSpec::default()
        };
        assert!(world_spec.surface_height_at(0.0, 0.0).is_some());
        assert!(world_spec.surface_height_at(50.0, 50.0).is_none());

        // Exact triangle check on a 2x2 grid: h00=0, h01=2, h10=4, h11=10
        let custom_hf = LevelStaticObject {
            id: None,
            name: "slope".to_owned(),
            shape: PrimitiveShape::HeightField(HeightFieldDesc {
                rows: 2,
                columns: 2,
                height_samples: vec![0.0, 2.0, 4.0, 10.0],
                cell_size_x: 2.0,
                cell_size_z: 2.0,
                thickness: None,
            }),
            transform: PhysicsTransform::IDENTITY,
            color: LevelColor::default(),
        };
        world_spec.static_objects = vec![custom_hf];
        // u=0.25, v=0.25 -> u+v <= 1 -> 0 + 0.25*(2-0) + 0.25*(4-0) = 1.5
        let h_lower = world_spec.surface_height_at(0.5, 0.5).unwrap();
        assert!((h_lower - 1.5).abs() < 1e-5);
        // u=0.75, v=0.75 -> u+v > 1 -> 10 + 0.25*(4-10) + 0.25*(2-10) = 6.5
        let h_upper = world_spec.surface_height_at(1.5, 1.5).unwrap();
        assert!((h_upper - 6.5).abs() < 1e-5);
    }

    #[test]
    fn footprint_stats_and_dynamic_slope_seating_prevent_clipping_and_floating() {
        // Sloped terrain rising linearly along +X: slope dy/dx = 0.5
        let rows = 5usize;
        let columns = 5usize;
        let mut samples = Vec::with_capacity(rows * columns);
        for _r in 0..rows {
            for c in 0..columns {
                samples.push(c as f32 * 1.0); // step 2m -> slope = 0.5
            }
        }
        let world = LevelWorldSpec {
            static_objects: vec![LevelStaticObject {
                id: None,
                name: "ramp".to_owned(),
                shape: PrimitiveShape::HeightField(HeightFieldDesc {
                    rows,
                    columns,
                    height_samples: samples,
                    cell_size_x: 2.0,
                    cell_size_z: 2.0,
                    thickness: Some(1.0),
                }),
                transform: PhysicsTransform::IDENTITY,
                color: LevelColor::default(),
            }],
            ..LevelWorldSpec::default()
        };

        let stats = world
            .sample_footprint_stats(4.0, 4.0, 1.0, 1.0)
            .expect("stats inside ramp");
        assert!((stats.min_y - 1.5).abs() < 1e-4);
        assert!((stats.max_y - 2.5).abs() < 1e-4);
        assert!(stats.normal.x < -0.3 && stats.normal.y > 0.8);

        let crate_shape = PrimitiveShape::Box {
            half_extents: Vec3::new(0.6, 0.6, 0.6),
        };
        let seated = world.seat_dynamic_transform(&crate_shape, 4.0, 4.0);
        // Bottom corners of the rotated crate must all lie cleanly at or slightly above the slope.
        for sx in [-0.6_f32, 0.6] {
            for sz in [-0.6_f32, 0.6] {
                let offset = seated.rotation.rotate_vec(&Vec3::new(sx, -0.6, sz));
                let world_corner = seated.position.add(&offset);
                let ground = world
                    .surface_height_at(world_corner.x, world_corner.z)
                    .unwrap();
                assert!(world_corner.y >= ground);
                assert!(world_corner.y - ground < 0.05);
            }
        }
    }

    #[test]
    fn static_upright_and_trigger_seating_span_slope_footprint_without_floating() {
        // Sloped terrain rising linearly along +X: slope dy/dx = 0.5 (height = x * 0.5).
        let rows = 5usize;
        let columns = 5usize;
        let mut samples = Vec::with_capacity(rows * columns);
        for _r in 0..rows {
            for c in 0..columns {
                samples.push(c as f32 * 1.0);
            }
        }
        let world = LevelWorldSpec {
            static_objects: vec![LevelStaticObject {
                id: None,
                name: "ramp".to_owned(),
                shape: PrimitiveShape::HeightField(HeightFieldDesc {
                    rows,
                    columns,
                    height_samples: samples,
                    cell_size_x: 2.0,
                    cell_size_z: 2.0,
                    thickness: Some(1.0),
                }),
                transform: PhysicsTransform::IDENTITY,
                color: LevelColor::default(),
            }],
            ..LevelWorldSpec::default()
        };

        // Calling seat_static_upright on the floor itself must leave it untouched.
        let mut floor_copy = world.static_objects[0].clone();
        world.seat_static_upright(&mut floor_copy, 2.0);
        assert_eq!(floor_copy, world.static_objects[0]);

        // Static wall spanning X in [3.0, 5.0]: min_y = 1.5, max_y = 2.5, desired_h = 2.0 -> [1.5 .. 4.5]
        let mut cover = LevelStaticObject {
            id: Some(EntityId::new(10)),
            name: "wall".to_owned(),
            shape: PrimitiveShape::Box {
                half_extents: Vec3::new(1.0, 1.0, 1.0),
            },
            transform: PhysicsTransform::from_position(Vec3::new(4.0, 0.0, 4.0)),
            color: LevelColor::default(),
        };
        world.seat_static_upright(&mut cover, 2.0);
        assert_eq!(cover.transform.rotation, Quaternion::IDENTITY);
        let PrimitiveShape::Box { half_extents } = cover.shape else {
            panic!("expected box");
        };
        assert!(((cover.transform.position.y - half_extents.y) - 1.5).abs() < 1e-4);
        assert!(((cover.transform.position.y + half_extents.y) - 4.5).abs() < 1e-4);

        // Trigger cylinder spanning X in [3.0, 5.0]: [min_y .. max_y + 3.0] -> [1.5 .. 5.5]
        let mut trig = LevelTriggerVolume {
            id: EntityId::new(20),
            name: "zone".to_owned(),
            tag: "checkpoint".to_owned(),
            shape: PrimitiveShape::Cylinder {
                half_height: 1.5,
                radius: 1.0,
            },
            transform: PhysicsTransform::from_position(Vec3::new(4.0, 0.0, 4.0)),
            behavior: PhantomBehavior::OverlapSensor,
            color: LevelColor::default(),
        };
        world.seat_trigger_to_floor(&mut trig, 3.0);
        assert_eq!(trig.transform.rotation, Quaternion::IDENTITY);
        let PrimitiveShape::Cylinder { half_height, .. } = trig.shape else {
            panic!("expected cylinder");
        };
        assert!(((trig.transform.position.y - half_height) - 1.5).abs() < 1e-4);
        assert!(((trig.transform.position.y + half_height) - 5.5).abs() < 1e-4);
    }

    #[test]
    fn rotated_static_walls_ramps_and_yawed_dynamic_bodies_preserve_rotation_on_slopes() {
        let rows = 5usize;
        let columns = 5usize;
        let mut samples = Vec::with_capacity(rows * columns);
        for _r in 0..rows {
            for c in 0..columns {
                samples.push(c as f32 * 1.0); // height = 0.5 * x
            }
        }
        let world = LevelWorldSpec {
            static_objects: vec![LevelStaticObject {
                id: None,
                name: "ramp".to_owned(),
                shape: PrimitiveShape::HeightField(HeightFieldDesc {
                    rows,
                    columns,
                    height_samples: samples,
                    cell_size_x: 2.0,
                    cell_size_z: 2.0,
                    thickness: Some(1.0),
                }),
                transform: PhysicsTransform::IDENTITY,
                color: LevelColor::default(),
            }],
            ..LevelWorldSpec::default()
        };

        // Thin wall (half_x=0.5, half_z=2.0) rotated 90 deg yaw -> world X span is +-2.0 around x=4.0:
        // X in [2.0, 6.0] -> min_y = 1.0, max_y = 3.0, desired_h = 2.0 -> [1.0 .. 5.0]
        let yaw_90 = Quaternion::from_euler_yxz(std::f32::consts::FRAC_PI_2, 0.0, 0.0);
        let mut rotated_wall = LevelStaticObject {
            id: Some(EntityId::new(11)),
            name: "rotated_wall".to_owned(),
            shape: PrimitiveShape::Box {
                half_extents: Vec3::new(0.5, 1.0, 2.0),
            },
            transform: PhysicsTransform::new(Vec3::new(4.0, 0.0, 4.0), yaw_90),
            color: LevelColor::default(),
        };
        world.seat_static_upright(&mut rotated_wall, 2.0);
        let (ry, rp, rr) = rotated_wall.transform.rotation.to_euler_yxz();
        assert!((ry - std::f32::consts::FRAC_PI_2).abs() < 1e-4);
        assert!(rp.abs() < 1e-4 && rr.abs() < 1e-4);
        let PrimitiveShape::Box { half_extents } = rotated_wall.shape else {
            panic!("expected box");
        };
        assert!(((rotated_wall.transform.position.y - half_extents.y) - 1.0).abs() < 1e-4);
        assert!(((rotated_wall.transform.position.y + half_extents.y) - 5.0).abs() < 1e-4);

        // Intentionally pitched ramp (pitch = 20 deg) must preserve pitch & clear ground.
        let pitch_20 = Quaternion::from_euler_yxz(0.0, 20.0_f32.to_radians(), 0.0);
        let mut pitched_ramp = LevelStaticObject {
            id: Some(EntityId::new(12)),
            name: "angled_ramp".to_owned(),
            shape: PrimitiveShape::Box {
                half_extents: Vec3::new(1.0, 0.5, 2.0),
            },
            transform: PhysicsTransform::new(Vec3::new(4.0, 0.0, 4.0), pitch_20),
            color: LevelColor::default(),
        };
        world.seat_static_upright(&mut pitched_ramp, 1.0);
        let (_, rp2, _) = pitched_ramp.transform.rotation.to_euler_yxz();
        assert!((rp2 - 20.0_f32.to_radians()).abs() < 1e-4);
        let mut lowest_gap = f32::INFINITY;
        for sx in [-1.0_f32, 1.0] {
            for sy in [-0.5_f32, 0.5] {
                for sz in [-2.0_f32, 2.0] {
                    let off = pitched_ramp
                        .transform
                        .rotation
                        .rotate_vec(&Vec3::new(sx, sy, sz));
                    let wp = pitched_ramp.transform.position.add(&off);
                    let gy = world.surface_height_at(wp.x, wp.z).unwrap();
                    assert!(wp.y >= gy - 1e-4);
                    lowest_gap = lowest_gap.min(wp.y - gy);
                }
            }
        }
        assert!(lowest_gap < 0.05);

        // Dynamic crate with 45 deg yaw keeps its yaw while conforming to slope.
        let crate_shape = PrimitiveShape::Box {
            half_extents: Vec3::new(0.6, 0.6, 0.6),
        };
        let yaw_45 = 45.0_f32.to_radians();
        let dyn_tf = world.seat_dynamic_transform_with_yaw(&crate_shape, 4.0, 4.0, yaw_45);
        let (dy, _, _) = dyn_tf.rotation.to_euler_yxz();
        assert!((dy - yaw_45).abs() < 0.05);
        for sx in [-0.6_f32, 0.6] {
            for sz in [-0.6_f32, 0.6] {
                let off = dyn_tf.rotation.rotate_vec(&Vec3::new(sx, -0.6, sz));
                let wp = dyn_tf.position.add(&off);
                let gy = world.surface_height_at(wp.x, wp.z).unwrap();
                assert!(wp.y >= gy);
                assert!(wp.y - gy < 0.05);
            }
        }
    }
}
