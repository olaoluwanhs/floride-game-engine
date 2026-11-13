use game_types::Vec3 as GameVec3;
use macroquad::models::draw_line_3d;
use macroquad::prelude::{
    Camera3D, Color, DARKGRAY, GRAY, KeyCode, Mesh, Quat, Vec2, Vec3, Vertex, WHITE, draw_mesh,
    draw_text, is_key_down, is_mouse_button_down, mouse_delta_position, mouse_wheel, set_camera,
    set_default_camera, vec3,
};
use physics_api::{CubicBezier3, HeightFieldDesc, PhysicsTransform, PrimitiveShape};

use crate::scene::{RayVisual, RenderItem};

// Polygon resolution for curved 3D shapes.
const SHAPE_SEGMENTS: usize = 16;
const SPHERE_RINGS: usize = 10;

/// Configurable camera parameters governing how the view is positioned relative to the target entity.
///
/// Instead of a rigid mode enum, every camera perspective (first-person, third-person over-the-shoulder,
/// free orbit, or custom) is described by continuous physical offsets.
#[derive(Debug, Clone, PartialEq)]
pub struct CameraSettings {
    /// Vertical height offset above the target entity origin in meters (e.g. eye level = 1.6m).
    pub height_offset: f32,

    /// Distance behind the target along the reverse look direction in meters.
    /// - `0.0`: Pure first-person shooter perspective.
    /// - `> 0.0`: Third-person follow / chase perspective.
    pub distance: f32,

    /// Lateral shoulder offset in meters.
    /// - `+0.45`: Over-the-right-shoulder (standard tactical shooter).
    /// - `-0.45`: Over-the-left-shoulder.
    /// - `0.0`: Centered.
    pub shoulder_offset: f32,

    /// Vertical field of view in degrees.
    pub fov_degrees: f32,

    /// Whether the target entity's mesh should be rendered.
    /// In first person, this is false so the player's own head capsule does not clip through the lens.
    pub show_player_mesh: bool,

    /// Minimum zoom distance when using the mouse wheel.
    pub min_distance: f32,

    /// Maximum zoom distance when using the mouse wheel.
    pub max_distance: f32,
}

impl CameraSettings {
    /// Preset: First-Person Shooter perspective (at character eye level, 0m distance).
    pub fn first_person() -> Self {
        Self {
            height_offset: 1.6,
            distance: 0.0,
            shoulder_offset: 0.0,
            fov_degrees: 75.0,
            show_player_mesh: false,
            min_distance: 0.0,
            max_distance: 0.0,
        }
    }

    /// Preset: Third-Person over-the-shoulder perspective.
    pub fn third_person() -> Self {
        Self {
            height_offset: 1.5,
            distance: 2.5,
            shoulder_offset: 0.45,
            fov_degrees: 70.0,
            show_player_mesh: true,
            min_distance: 0.8,
            max_distance: 8.0,
        }
    }

    /// Preset: Centered third-person / orbital inspection view.
    pub fn orbit() -> Self {
        Self {
            height_offset: 1.0,
            distance: 20.0,
            shoulder_offset: 0.0,
            fov_degrees: 60.0,
            show_player_mesh: true,
            min_distance: 2.0,
            max_distance: 48.0,
        }
    }

    /// Custom mode helper allowing setting arbitrary offsets.
    pub fn custom(height_offset: f32, distance: f32, shoulder_offset: f32) -> Self {
        Self {
            height_offset,
            distance,
            shoulder_offset,
            fov_degrees: 70.0,
            show_player_mesh: distance > 0.3,
            min_distance: 0.0,
            max_distance: 50.0,
        }
    }
}

impl Default for CameraSettings {
    fn default() -> Self {
        Self::orbit()
    }
}

/// A versatile 3D camera supporting first-person, third-person, and orbital viewing modes.
pub struct GameCamera {
    pub yaw: f32,
    pub pitch: f32,
    pub target: Vec3,
    pub settings: CameraSettings,
}

/// Alias for backwards compatibility with the original visual demo.
pub type OrbitCamera = GameCamera;

impl Default for GameCamera {
    fn default() -> Self {
        Self {
            yaw: 0.75,
            pitch: 0.55,
            target: vec3(0.0, 1.0, 0.0),
            settings: CameraSettings::orbit(),
        }
    }
}

impl GameCamera {
    /// Creates a camera with the specified settings.
    pub fn new(settings: CameraSettings) -> Self {
        Self {
            yaw: 0.0,
            pitch: 0.0,
            target: Vec3::ZERO,
            settings,
        }
    }

    /// Returns the current horizontal view angle in radians.
    pub fn yaw(&self) -> f32 {
        self.yaw
    }

    /// Returns the current vertical view angle in radians.
    pub fn pitch(&self) -> f32 {
        self.pitch
    }

    /// Sets the focal target point (e.g. player position) that the camera tracks.
    pub fn set_target(&mut self, target: Vec3) {
        self.target = target;
    }

    /// Updates camera rotation and zoom distance based on keyboard arrow keys and mouse dragging.
    pub fn update(&mut self, frame_time: f32) {
        let mut yaw_change = 0.0;
        let mut pitch_change = 0.0;

        // Arrow keys allow keyboard-only rotating.
        if is_key_down(KeyCode::Left) {
            yaw_change += frame_time;
        }
        if is_key_down(KeyCode::Right) {
            yaw_change -= frame_time;
        }
        if is_key_down(KeyCode::Up) {
            pitch_change += frame_time;
        }
        if is_key_down(KeyCode::Down) {
            pitch_change -= frame_time;
        }

        // Right mouse button dragging allows smooth mouse looking/orbiting.
        if is_mouse_button_down(macroquad::prelude::MouseButton::Right) {
            let mouse_delta = mouse_delta_position();
            yaw_change -= mouse_delta.x * 2.5;
            pitch_change -= mouse_delta.y * 2.5;
        }

        self.yaw += yaw_change;

        // Allow free vertical pitch rotation:
        // - Negative pitch tilts camera high above the character (bird's-eye view looking down).
        // - Positive pitch tilts camera below character looking up.
        // Clamped at ±1.54 rad (±88.2°) to prevent vertical gimbal lock with the up vector.
        self.pitch = (self.pitch + pitch_change).clamp(-1.54, 1.54);

        // Mouse wheel controls zoom distance if zoomable.
        if self.settings.max_distance > self.settings.min_distance {
            let (_, wheel_y) = mouse_wheel();
            self.settings.distance = (self.settings.distance - wheel_y * 1.5)
                .clamp(self.settings.min_distance, self.settings.max_distance);
            // Hide player capsule when zoomed very close to eye level.
            self.settings.show_player_mesh = self.settings.distance > 0.3;
        }
    }

    /// Returns the normalized 3D forward view direction vector.
    pub fn forward_direction(&self) -> Vec3 {
        let (sin_yaw, cos_yaw) = self.yaw.sin_cos();
        let (sin_pitch, cos_pitch) = self.pitch.sin_cos();
        vec3(sin_yaw * cos_pitch, sin_pitch, cos_yaw * cos_pitch)
    }

    /// Computes the 3D position and orientation for Macroquad's `Camera3D`.
    pub fn camera(&self) -> Camera3D {
        let (sin_yaw, cos_yaw) = self.yaw.sin_cos();
        let (sin_pitch, cos_pitch) = self.pitch.sin_cos();

        // Forward unit vector pointing along the view direction (+Z forward when yaw=0).
        let forward = vec3(sin_yaw * cos_pitch, sin_pitch, cos_yaw * cos_pitch);
        let right = vec3(-cos_yaw, 0.0, sin_yaw);
        let up = vec3(0.0, 1.0, 0.0);

        // Anchor is at the target + height offset + lateral shoulder offset.
        let anchor = self.target
            + vec3(0.0, self.settings.height_offset, 0.0)
            + right * self.settings.shoulder_offset;

        // Position is pulled back along the reverse forward vector by distance.
        let eye_position = anchor - forward * self.settings.distance;
        let look_target = eye_position + forward * 10.0;

        Camera3D {
            position: eye_position,
            target: look_target,
            up,
            fovy: self.settings.fov_degrees.to_radians(),
            ..Default::default()
        }
    }
}

/// Draws all 3D scene objects, debug raycasts, and the optional reference grid.
///
/// Notice how decoupled this is: `draw_scene` receives an immutable slice of `RenderItem`s
/// and draws them. It has no knowledge of Rapier, simulation steps, or physics forces.
pub fn draw_scene(items: &[RenderItem], rays: &[RayVisual], camera: &OrbitCamera) {
    draw_scene_with_grid(items, rays, camera, true);
}

/// Draws all 3D scene objects and debug raycasts without drawing a `y = 0` reference grid.
///
/// Useful for level editors or terrain views that supply their own elevation grid.
pub fn draw_scene_without_grid(items: &[RenderItem], rays: &[RayVisual], camera: &OrbitCamera) {
    draw_scene_with_grid(items, rays, camera, false);
}

fn draw_scene_with_grid(
    items: &[RenderItem],
    rays: &[RayVisual],
    camera: &OrbitCamera,
    show_grid: bool,
) {
    let camera = camera.camera();
    set_camera(&camera);
    if show_grid {
        draw_grid(28, 1.0, GRAY, DARKGRAY);
    }

    for item in items {
        if let PrimitiveShape::HeightField(description) = &item.shape {
            draw_heightfield_chunked(description, &item.transform, item.color);
        } else {
            let mesh = shape_mesh(&item.shape, &item.transform, item.color);
            draw_mesh(&mesh);
        }
    }

    // --- DRAW RAYCAST RAYS, HIT IMPACTS, AND SURFACE NORMALS ---
    for ray in rays {
        let start = to_macroquad(ray.origin);
        let end = to_macroquad(ray.target);

        // 1. Ray path represented by a bright red line
        draw_line_3d(start, end, Color::new(1.0, 0.2, 0.2, 0.95));

        if let Some(hit) = &ray.hit {
            let hit_point = to_macroquad(hit.point);
            let normal_direction = to_macroquad(hit.normal);

            // 2. Impact point marker: 3D crosshair centered at the contact point (bright yellow)
            draw_hit_marker(hit_point, Color::new(1.0, 0.9, 0.2, 1.0));

            // 3. Surface normal: line pointing outward perpendicular to the touched surface (bright green)
            let normal_end = hit_point + normal_direction * 0.65;
            draw_line_3d(hit_point, normal_end, Color::new(0.2, 1.0, 0.35, 1.0));
        }
    }

    set_default_camera();
}

/// Draws a 3D crosshair at the ray contact point.
fn draw_hit_marker(point: Vec3, color: Color) {
    let radius = 0.08;
    draw_line_3d(
        point - vec3(radius, 0.0, 0.0),
        point + vec3(radius, 0.0, 0.0),
        color,
    );
    draw_line_3d(
        point - vec3(0.0, radius, 0.0),
        point + vec3(0.0, radius, 0.0),
        color,
    );
    draw_line_3d(
        point - vec3(0.0, 0.0, radius),
        point + vec3(0.0, 0.0, radius),
        color,
    );
}

/// Draws the 2D user interface / HUD overlay displaying controls and sensor metrics.
pub fn draw_hud(lines: Vec<String>, width: f32) {
    let mut y = 24.0;
    for line in lines {
        draw_text(&line, 20.0, y, 20.0, WHITE);
        y += 25.0;
    }
    draw_text(
        "Arrow keys orbit | Right-drag orbit | Mouse wheel zoom",
        20.0,
        140.0,
        18.0,
        WHITE,
    );
    draw_text(
        "Red lines: ray paths | Yellow cross: hit impact | Green lines: surface normals",
        20.0,
        165.0,
        16.0,
        Color::new(0.9, 0.9, 0.45, 1.0),
    );
    let footer = format!("FPS: {}", macroquad::prelude::get_fps());
    draw_text(&footer, width - 95.0, 24.0, 18.0, WHITE);
}

/// Displays an on-screen error banner if the physics world fails during initialization or tick.
pub fn draw_error(error: &str) {
    set_default_camera();
    draw_text(
        "Physics showcase could not continue:",
        24.0,
        42.0,
        26.0,
        WHITE,
    );
    draw_text(error, 24.0, 78.0, 20.0, Color::new(1.0, 0.35, 0.35, 1.0));
}

/// Converts a physics `PrimitiveShape` into a 3D Macroquad polygon `Mesh`.
fn shape_mesh(shape: &PrimitiveShape, transform: &PhysicsTransform, color: Color) -> Mesh {
    let mut builder = MeshBuilder::new(transform, color);
    match shape {
        PrimitiveShape::Box { half_extents } => {
            builder.add_box(to_macroquad(*half_extents));
        }
        PrimitiveShape::Sphere { radius } => {
            builder.add_sphere(Vec3::ZERO, *radius);
        }
        PrimitiveShape::Plane { half_extents } => {
            let (half_x, half_z) = half_extents
                .map(|extents| (extents.half_width_x, extents.half_depth_z))
                .unwrap_or((2.5, 2.5));
            builder.add_box(vec3(half_x, 0.015, half_z));
        }
        PrimitiveShape::Cylinder {
            radius,
            half_height,
        } => builder.add_cylinder(*radius, *half_height),
        PrimitiveShape::Capsule {
            radius,
            half_height,
        } => builder.add_capsule(*radius, *half_height),
        PrimitiveShape::Torus {
            major_radius,
            tube_radius,
        } => builder.add_torus(*major_radius, *tube_radius),
        PrimitiveShape::HeightField(description) => builder.add_heightfield(description),
        PrimitiveShape::BezierExtrudedRect {
            segments,
            half_width,
            half_thickness,
            max_segment_len_m,
            max_tangent_angle_deg,
        } => {
            builder.add_bezier_rect(
                segments,
                *half_width,
                *half_thickness,
                *max_segment_len_m,
                *max_tangent_angle_deg,
            );
        }
        PrimitiveShape::BezierExtrudedCircle {
            segments,
            radius,
            max_segment_len_m,
            max_tangent_angle_deg,
        } => {
            builder.add_bezier_circle(
                segments,
                *radius,
                *max_segment_len_m,
                *max_tangent_angle_deg,
            );
        }
    }
    builder.finish()
}

/// Helper for generating indexed 3D triangle meshes.
///
/// 3D GPUs draw surfaces as triangles. Instead of sending duplicate 3D points for every
/// triangle, indexed meshes maintain a list of unique `vertices` and a list of `indices`
/// (triplets of integers specifying which three vertices make up each triangle).
struct MeshBuilder<'a> {
    transform: &'a PhysicsTransform,
    rotation: Quat,
    color: Color,
    vertices: Vec<Vertex>,
    indices: Vec<u16>,
}

impl<'a> MeshBuilder<'a> {
    fn new(transform: &'a PhysicsTransform, color: Color) -> Self {
        Self {
            transform,
            rotation: Quat::from_xyzw(
                transform.rotation.x,
                transform.rotation.y,
                transform.rotation.z,
                transform.rotation.w,
            )
            .normalize(),
            color,
            vertices: Vec::new(),
            indices: Vec::new(),
        }
    }

    /// Generates the 6 faces (12 triangles) of a 3D box.
    fn add_box(&mut self, half_extents: Vec3) {
        let corners = [
            vec3(-half_extents.x, -half_extents.y, -half_extents.z),
            vec3(half_extents.x, -half_extents.y, -half_extents.z),
            vec3(half_extents.x, half_extents.y, -half_extents.z),
            vec3(-half_extents.x, half_extents.y, -half_extents.z),
            vec3(-half_extents.x, -half_extents.y, half_extents.z),
            vec3(half_extents.x, -half_extents.y, half_extents.z),
            vec3(half_extents.x, half_extents.y, half_extents.z),
            vec3(-half_extents.x, half_extents.y, half_extents.z),
        ];
        let faces = [
            [0, 1, 2, 3],
            [5, 4, 7, 6],
            [4, 0, 3, 7],
            [1, 5, 6, 2],
            [3, 2, 6, 7],
            [4, 5, 1, 0],
        ];
        for [a, b, c, d] in faces {
            self.add_quad(corners[a], corners[b], corners[c], corners[d]);
        }
    }

    /// Generates an approximate sphere using latitude (phi) and longitude (theta) rings.
    fn add_sphere(&mut self, center: Vec3, radius: f32) {
        for ring in 0..SPHERE_RINGS {
            let phi0 = std::f32::consts::PI * ring as f32 / SPHERE_RINGS as f32;
            let phi1 = std::f32::consts::PI * (ring + 1) as f32 / SPHERE_RINGS as f32;
            for segment in 0..SHAPE_SEGMENTS {
                let theta0 = std::f32::consts::TAU * segment as f32 / SHAPE_SEGMENTS as f32;
                let theta1 = std::f32::consts::TAU * (segment + 1) as f32 / SHAPE_SEGMENTS as f32;
                let point = |phi: f32, theta: f32| {
                    center
                        + vec3(
                            radius * phi.sin() * theta.cos(),
                            radius * phi.cos(),
                            radius * phi.sin() * theta.sin(),
                        )
                };
                let top_left = point(phi0, theta0);
                let top_right = point(phi0, theta1);
                let bottom_right = point(phi1, theta1);
                let bottom_left = point(phi1, theta0);
                self.add_triangle(top_left, bottom_left, bottom_right);
                self.add_triangle(top_left, bottom_right, top_right);
            }
        }
    }

    /// Generates a cylinder consisting of side quads and top/bottom circular end-caps.
    fn add_cylinder(&mut self, radius: f32, half_height: f32) {
        for segment in 0..SHAPE_SEGMENTS {
            let angle0 = std::f32::consts::TAU * segment as f32 / SHAPE_SEGMENTS as f32;
            let angle1 = std::f32::consts::TAU * (segment + 1) as f32 / SHAPE_SEGMENTS as f32;
            let bottom0 = vec3(radius * angle0.cos(), -half_height, radius * angle0.sin());
            let bottom1 = vec3(radius * angle1.cos(), -half_height, radius * angle1.sin());
            let top0 = vec3(radius * angle0.cos(), half_height, radius * angle0.sin());
            let top1 = vec3(radius * angle1.cos(), half_height, radius * angle1.sin());
            self.add_quad(bottom0, bottom1, top1, top0);
            self.add_triangle(Vec3::ZERO.with_y(-half_height), bottom1, bottom0);
            self.add_triangle(Vec3::ZERO.with_y(half_height), top0, top1);
        }
    }

    /// Generates a capsule: a central cylinder capped with two half-spheres.
    fn add_capsule(&mut self, radius: f32, half_height: f32) {
        self.add_cylinder(radius, half_height);
        self.add_sphere(vec3(0.0, half_height, 0.0), radius);
        self.add_sphere(vec3(0.0, -half_height, 0.0), radius);
    }

    /// Generates a torus (doughnut shape) parametrized by a major ring radius and tube radius.
    fn add_torus(&mut self, major_radius: f32, tube_radius: f32) {
        const RING_STEPS: usize = 24;
        const TUBE_STEPS: usize = 10;
        for ring in 0..RING_STEPS {
            let ring0 = std::f32::consts::TAU * ring as f32 / RING_STEPS as f32;
            let ring1 = std::f32::consts::TAU * (ring + 1) as f32 / RING_STEPS as f32;
            for tube in 0..TUBE_STEPS {
                let tube0 = std::f32::consts::TAU * tube as f32 / TUBE_STEPS as f32;
                let tube1 = std::f32::consts::TAU * (tube + 1) as f32 / TUBE_STEPS as f32;
                let point = |ring_angle: f32, tube_angle: f32| {
                    let radial = major_radius + tube_radius * tube_angle.cos();
                    vec3(
                        radial * ring_angle.cos(),
                        tube_radius * tube_angle.sin(),
                        radial * ring_angle.sin(),
                    )
                };
                self.add_quad(
                    point(ring0, tube0),
                    point(ring1, tube0),
                    point(ring1, tube1),
                    point(ring0, tube1),
                );
            }
        }
    }

    /// Generates a heightfield terrain grid with optional thickness walls.
    fn add_heightfield(&mut self, description: &HeightFieldDesc) {
        if description.rows < 2 || description.columns < 2 {
            return;
        }
        self.add_heightfield_rows(description, 0, description.rows - 1);
    }

    /// Generates heightfield geometry for row indices `row_start..row_end` (cell rows).
    ///
    /// Keeping row chunks small prevents Macroquad's default 5,000-index draw buffer
    /// from truncating large terrains mid-mesh.
    fn add_heightfield_rows(
        &mut self,
        description: &HeightFieldDesc,
        row_start: usize,
        row_end: usize,
    ) {
        let point = |row: usize, column: usize| {
            let height = description.height_samples[row * description.columns + column];
            vec3(
                column as f32 * description.cell_size_x,
                height,
                row as f32 * description.cell_size_z,
            )
        };
        for row in row_start..row_end {
            for column in 0..description.columns - 1 {
                let p00 = point(row, column);
                let p10 = point(row + 1, column);
                let p01 = point(row, column + 1);
                let p11 = point(row + 1, column + 1);
                self.add_terrain_triangle(p00, p10, p01);
                self.add_terrain_triangle(p10, p11, p01);
                if let Some(thickness) = description.thickness {
                    self.add_triangle(
                        bottom_point(p00, thickness),
                        bottom_point(p01, thickness),
                        bottom_point(p10, thickness),
                    );
                    self.add_triangle(
                        bottom_point(p10, thickness),
                        bottom_point(p01, thickness),
                        bottom_point(p11, thickness),
                    );
                }
            }
        }

        if let Some(thickness) = description.thickness {
            if row_start == 0 {
                for column in 0..description.columns - 1 {
                    self.add_quad(
                        point(0, column),
                        bottom_point(point(0, column), thickness),
                        bottom_point(point(0, column + 1), thickness),
                        point(0, column + 1),
                    );
                }
            }
            let max_cell_row = description.rows - 1;
            if row_end == max_cell_row {
                for column in 0..description.columns - 1 {
                    self.add_quad(
                        point(max_cell_row, column + 1),
                        bottom_point(point(max_cell_row, column + 1), thickness),
                        bottom_point(point(max_cell_row, column), thickness),
                        point(max_cell_row, column),
                    );
                }
            }
            for row in row_start..row_end {
                self.add_quad(
                    point(row + 1, 0),
                    bottom_point(point(row + 1, 0), thickness),
                    bottom_point(point(row, 0), thickness),
                    point(row, 0),
                );
                let last_column = description.columns - 1;
                self.add_quad(
                    point(row, last_column),
                    bottom_point(point(row, last_column), thickness),
                    bottom_point(point(row + 1, last_column), thickness),
                    point(row + 1, last_column),
                );
            }
        }
    }

    /// Adds a 4-vertex quadrilateral by splitting it into two triangles (first-second-third, first-third-fourth).
    fn add_quad(&mut self, first: Vec3, second: Vec3, third: Vec3, fourth: Vec3) {
        self.add_triangle(first, second, third);
        self.add_triangle(first, third, fourth);
    }

    /// Adds a terrain triangle modulated by face slope and elevation so uneven hills stand out.
    fn add_terrain_triangle(&mut self, first: Vec3, second: Vec3, third: Vec3) {
        let edge_a = second - first;
        let edge_b = third - first;
        let mut normal = edge_a.cross(edge_b).normalize_or_zero();
        if normal.y < 0.0 {
            normal = -normal;
        }
        let sun = vec3(0.45, 0.85, 0.35).normalize();
        let light = 0.62 + 0.38 * normal.dot(sun).max(0.0);
        let avg_y = (first.y + second.y + third.y) / 3.0;
        let height_tint = (avg_y * 0.035).clamp(-0.12, 0.18);
        let shaded = Color::new(
            (self.color.r * light + height_tint * 0.5).clamp(0.0, 1.0),
            (self.color.g * light + height_tint * 0.8).clamp(0.0, 1.0),
            (self.color.b * light + height_tint * 0.3).clamp(0.0, 1.0),
            self.color.a,
        );
        self.add_tinted_triangle(first, second, third, shaded);
    }

    /// Adds a triangle to the mesh, transforming each vertex from local model space into world space.
    fn add_triangle(&mut self, first: Vec3, second: Vec3, third: Vec3) {
        self.add_tinted_triangle(first, second, third, self.color);
    }

    fn add_tinted_triangle(&mut self, first: Vec3, second: Vec3, third: Vec3, color: Color) {
        let base = self.vertices.len() as u16;
        for point in [first, second, third] {
            // Apply rotation and world position:
            // world_pos = (rotation * local_point) + position
            let world_position = self.rotation * point + to_macroquad(self.transform.position);
            self.vertices
                .push(Vertex::new2(world_position, Vec2::ZERO, color));
        }
        self.indices.extend_from_slice(&[base, base + 1, base + 2]);
    }

    /// Consumes the builder to produce a Macroquad `Mesh` ready for the GPU.
    fn finish(self) -> Mesh {
        Mesh {
            vertices: self.vertices,
            indices: self.indices,
            texture: None,
        }
    }

    fn add_bezier_rect(
        &mut self,
        segments: &[CubicBezier3],
        half_width: f32,
        half_thickness: f32,
        max_segment_len_m: f32,
        max_tangent_angle_deg: f32,
    ) {
        use physics_api::bezier::{adaptive_sample, tangent_to_rotation};
        let max_angle_rad = max_tangent_angle_deg.to_radians();

        for curve in segments {
            let samples = adaptive_sample(curve, max_segment_len_m, max_angle_rad);
            for window in samples.windows(2) {
                let (curr, next) = (&window[0], &window[1]);
                let curr_rot_q = tangent_to_rotation(&curr.tangent);
                let next_rot_q = tangent_to_rotation(&next.tangent);
                let curr_rot = Quat::from_xyzw(
                    curr_rot_q.x,
                    curr_rot_q.y,
                    curr_rot_q.z,
                    curr_rot_q.w,
                )
                .normalize();
                let next_rot = Quat::from_xyzw(
                    next_rot_q.x,
                    next_rot_q.y,
                    next_rot_q.z,
                    next_rot_q.w,
                )
                .normalize();

                let right_curr = curr_rot * vec3(half_width, 0.0, 0.0);
                let up_curr = curr_rot * vec3(0.0, half_thickness, 0.0);
                let right_next = next_rot * vec3(half_width, 0.0, 0.0);
                let up_next = next_rot * vec3(0.0, half_thickness, 0.0);

                let cp = to_macroquad(curr.position);
                let np = to_macroquad(next.position);

                let c_tl = cp - right_curr + up_curr;
                let c_tr = cp + right_curr + up_curr;
                let c_br = cp + right_curr - up_curr;
                let c_bl = cp - right_curr - up_curr;

                let n_tl = np - right_next + up_next;
                let n_tr = np + right_next + up_next;
                let n_br = np + right_next - up_next;
                let n_bl = np - right_next - up_next;

                self.add_quad(c_tl, c_tr, n_tr, n_tl);
                self.add_quad(c_bl, n_bl, n_br, c_br);
                self.add_quad(c_tl, n_tl, n_bl, c_bl);
                self.add_quad(c_tr, c_br, n_br, n_tr);
            }
        }
    }

    fn add_bezier_circle(
        &mut self,
        segments: &[CubicBezier3],
        radius: f32,
        max_segment_len_m: f32,
        max_tangent_angle_deg: f32,
    ) {
        use physics_api::bezier::{adaptive_sample, tangent_to_rotation};
        let max_angle_rad = max_tangent_angle_deg.to_radians();

        for curve in segments {
            let samples = adaptive_sample(curve, max_segment_len_m, max_angle_rad);
            for window in samples.windows(2) {
                let (curr, next) = (&window[0], &window[1]);
                let curr_rot_q = tangent_to_rotation(&curr.tangent);
                let next_rot_q = tangent_to_rotation(&next.tangent);
                let curr_rot = Quat::from_xyzw(
                    curr_rot_q.x,
                    curr_rot_q.y,
                    curr_rot_q.z,
                    curr_rot_q.w,
                )
                .normalize();
                let next_rot = Quat::from_xyzw(
                    next_rot_q.x,
                    next_rot_q.y,
                    next_rot_q.z,
                    next_rot_q.w,
                )
                .normalize();

                let cp = to_macroquad(curr.position);
                let np = to_macroquad(next.position);

                const SIDES: usize = 12;
                for i in 0..SIDES {
                    let a0 = std::f32::consts::TAU * i as f32 / SIDES as f32;
                    let a1 = std::f32::consts::TAU * (i + 1) as f32 / SIDES as f32;

                    let c0 = cp + curr_rot * vec3(radius * a0.cos(), radius * a0.sin(), 0.0);
                    let c1 = cp + curr_rot * vec3(radius * a1.cos(), radius * a1.sin(), 0.0);
                    let n0 = np + next_rot * vec3(radius * a0.cos(), radius * a0.sin(), 0.0);
                    let n1 = np + next_rot * vec3(radius * a1.cos(), radius * a1.sin(), 0.0);

                    self.add_quad(c0, c1, n1, n0);
                }
            }
        }
    }
}

fn draw_heightfield_chunked(
    description: &HeightFieldDesc,
    transform: &PhysicsTransform,
    color: Color,
) {
    if description.rows < 2 || description.columns < 2 {
        return;
    }
    let total_cell_rows = description.rows - 1;
    let cells_per_row = (description.columns - 1).max(1);
    let max_indices_per_chunk = 3600usize;
    let indices_per_row = cells_per_row * if description.thickness.is_some() { 12 } else { 6 } + 24;
    let rows_per_chunk = (max_indices_per_chunk / indices_per_row.max(1)).clamp(1, total_cell_rows);

    let mut row_start = 0;
    while row_start < total_cell_rows {
        let row_end = (row_start + rows_per_chunk).min(total_cell_rows);
        let mut builder = MeshBuilder::new(transform, color);
        builder.add_heightfield_rows(description, row_start, row_end);
        draw_mesh(&builder.finish());
        row_start = row_end;
    }
}

fn with_y(point: Vec3, y: f32) -> Vec3 {
    vec3(point.x, y, point.z)
}

fn bottom_point(point: Vec3, thickness: f32) -> Vec3 {
    with_y(point, point.y - thickness)
}

fn to_macroquad(vector: GameVec3) -> Vec3 {
    vec3(vector.x, vector.y, vector.z)
}

fn draw_grid(slices: u32, spacing: f32, axes_color: Color, other_color: Color) {
    macroquad::models::draw_grid(slices, spacing, axes_color, other_color);
}

/// Renders 3D Bézier spline control points, tangent handle bars, and anchor knots in world space.
///
/// Just like in Blender:
/// - Anchor Knots (P0, P3) are drawn as yellow wireframe spheres.
/// - Tangent Handles (P1, P2) are drawn as cyan/orange diamond markers.
/// - Handle bars are drawn as colored dashed lines connecting the knot to its tangent handles.
/// - The currently selected point is highlighted in pulsing bright green.
pub fn draw_bezier_spline_gizmos(
    segments: &[CubicBezier3],
    transform: &PhysicsTransform,
    selected_seg: usize,
    selected_point_idx: usize, // 0: P0, 1: P1, 2: P2, 3: P3
) {
    use macroquad::models::{draw_line_3d, draw_sphere_wires};
    use macroquad::prelude::{GOLD, GREEN, ORANGE, SKYBLUE};

    let world_rot = Quat::from_xyzw(
        transform.rotation.x,
        transform.rotation.y,
        transform.rotation.z,
        transform.rotation.w,
    );
    let world_pos = to_macroquad(transform.position);

    let to_world = |pt: [f32; 3]| -> Vec3 {
        let local = vec3(pt[0], pt[1], pt[2]);
        world_pos + world_rot * local
    };

    for (seg_idx, curve) in segments.iter().enumerate() {
        let p0 = to_world(curve.p0);
        let p1 = to_world(curve.p1);
        let p2 = to_world(curve.p2);
        let p3 = to_world(curve.p3);

        // Draw handle bars (P0 -> P1, and P3 -> P2)
        draw_line_3d(p0, p1, SKYBLUE);
        draw_line_3d(p3, p2, ORANGE);

        // Draw knots (P0, P3)
        let p0_selected = seg_idx == selected_seg && selected_point_idx == 0;
        let p3_selected = seg_idx == selected_seg && selected_point_idx == 3;
        draw_sphere_wires(p0, if p0_selected { 0.22 } else { 0.14 }, None, if p0_selected { GREEN } else { GOLD });
        draw_sphere_wires(p3, if p3_selected { 0.22 } else { 0.14 }, None, if p3_selected { GREEN } else { GOLD });

        // Draw handles (P1, P2)
        let p1_selected = seg_idx == selected_seg && selected_point_idx == 1;
        let p2_selected = seg_idx == selected_seg && selected_point_idx == 2;
        draw_sphere_wires(p1, if p1_selected { 0.18 } else { 0.10 }, None, if p1_selected { GREEN } else { SKYBLUE });
        draw_sphere_wires(p2, if p2_selected { 0.18 } else { 0.10 }, None, if p2_selected { GREEN } else { ORANGE });
    }
}
