use game_types::Vec3 as GameVec3;
use macroquad::models::draw_line_3d;
use macroquad::prelude::{
    Camera3D, Color, DARKGRAY, GRAY, KeyCode, Mesh, Quat, Vec2, Vec3, Vertex, WHITE, draw_mesh,
    draw_text, is_key_down, is_mouse_button_down, mouse_delta_position, mouse_wheel, set_camera,
    set_default_camera, vec3,
};
use physics_api::{HeightFieldDesc, PhysicsTransform, PrimitiveShape};

use crate::scene::{RayVisual, RenderItem};

// Polygon resolution for curved 3D shapes.
const SHAPE_SEGMENTS: usize = 16;
const SPHERE_RINGS: usize = 10;

/// An orbit camera that rotates around a central target point.
///
/// In 3D graphics, an orbit camera is easiest to control using spherical coordinates:
/// - `yaw`: Horizontal rotation angle around the vertical Y axis (looking left/right).
/// - `pitch`: Vertical elevation angle (looking up/down).
/// - `distance`: Radial distance from the camera eye to the focal target.
pub struct OrbitCamera {
    yaw: f32,
    pitch: f32,
    distance: f32,
    target: Vec3,
}

impl Default for OrbitCamera {
    fn default() -> Self {
        Self {
            yaw: 0.75,
            pitch: 0.55,
            distance: 30.0,
            target: vec3(0.0, 1.0, 0.0),
        }
    }
}

impl OrbitCamera {
    /// Returns the current horizontal view angle in radians.
    ///
    /// Used by `main.rs` to ensure WASD player movement follows the camera's perspective.
    pub fn yaw(&self) -> f32 {
        self.yaw
    }

    /// Updates camera rotation and distance based on keyboard arrow keys and mouse dragging.
    pub fn update(&mut self, frame_time: f32) {
        let mut yaw_change = 0.0;
        let mut pitch_change = 0.0;

        // Arrow keys allow keyboard-only orbiting.
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

        // Right mouse button dragging allows smooth mouse orbiting.
        if is_mouse_button_down(macroquad::prelude::MouseButton::Right) {
            let mouse_delta = mouse_delta_position();
            yaw_change -= mouse_delta.x * 2.5;
            pitch_change += mouse_delta.y * 2.5;
        }

        self.yaw += yaw_change;
        // Clamp pitch so the camera cannot flip upside down or pass below the floor.
        self.pitch = (self.pitch + pitch_change).clamp(0.15, 1.35);

        // Mouse wheel controls zoom distance.
        let (_, wheel_y) = mouse_wheel();
        self.distance = (self.distance - wheel_y * 1.5).clamp(12.0, 48.0);
    }

    /// Converts spherical coordinates (yaw, pitch, distance) into Cartesian (X, Y, Z) coordinates
    /// and configures a Macroquad `Camera3D`.
    fn camera(&self) -> Camera3D {
        let horizontal_distance = self.distance * self.pitch.cos();
        let position = self.target
            + vec3(
                self.yaw.sin() * horizontal_distance,
                self.pitch.sin() * self.distance,
                self.yaw.cos() * horizontal_distance,
            );
        Camera3D {
            position,
            target: self.target,
            up: vec3(0.0, 1.0, 0.0),
            ..Default::default()
        }
    }
}

/// Draws all 3D scene objects and the coordinate grid.
///
/// Notice how decoupled this is: `draw_scene` receives an immutable slice of `RenderItem`s
/// and draws them. It has no knowledge of Rapier, simulation steps, or physics forces.
pub fn draw_scene(items: &[RenderItem], rays: &[RayVisual], camera: &OrbitCamera) {
    let camera = camera.camera();
    set_camera(&camera);
    draw_grid(28, 1.0, GRAY, DARKGRAY);

    for item in items {
        let mesh = shape_mesh(&item.shape, &item.transform, item.color);
        draw_mesh(&mesh);
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
        let point = |row: usize, column: usize| {
            let height = description.height_samples[row * description.columns + column];
            vec3(
                column as f32 * description.cell_size_x,
                height,
                row as f32 * description.cell_size_z,
            )
        };
        for row in 0..description.rows - 1 {
            for column in 0..description.columns - 1 {
                let p00 = point(row, column);
                let p10 = point(row + 1, column);
                let p01 = point(row, column + 1);
                let p11 = point(row + 1, column + 1);
                self.add_triangle(p00, p10, p01);
                self.add_triangle(p10, p11, p01);
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
            for column in 0..description.columns - 1 {
                self.add_quad(
                    point(0, column),
                    bottom_point(point(0, column), thickness),
                    bottom_point(point(0, column + 1), thickness),
                    point(0, column + 1),
                );
                let last_row = description.rows - 1;
                self.add_quad(
                    point(last_row, column + 1),
                    bottom_point(point(last_row, column + 1), thickness),
                    bottom_point(point(last_row, column), thickness),
                    point(last_row, column),
                );
            }
            for row in 0..description.rows - 1 {
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

    /// Adds a triangle to the mesh, transforming each vertex from local model space into world space.
    fn add_triangle(&mut self, first: Vec3, second: Vec3, third: Vec3) {
        let base = self.vertices.len() as u16;
        for point in [first, second, third] {
            // Apply rotation and world position:
            // world_pos = (rotation * local_point) + position
            let world_position = self.rotation * point + to_macroquad(self.transform.position);
            self.vertices
                .push(Vertex::new2(world_position, Vec2::ZERO, self.color));
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
