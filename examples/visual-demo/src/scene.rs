use game_types::{EntityId, Vec3};
use macroquad::prelude::Color;
use physics_api::{
    CharacterDesc, DynamicBodyDesc, HeightFieldDesc, PhantomBehavior, PhantomColliderDesc,
    PhysicsError, PhysicsHandle, PhysicsTransform, PhysicsWorld, PlaneHalfExtents, PrimitiveShape,
    Quaternion, RayQuery, StaticColliderDesc,
};
use physics_rapier::RapierWorld;

// Stable EntityId values for demo objects. In our core architecture, game logic
// refers to objects using unique, stable u64 EntityIds rather than internal
// physics pointers.
const FLOOR_ENTITY: u64 = 1;
const CHARACTER_ENTITY: u64 = 2;
const FIRST_EXHIBIT_ENTITY: u64 = 10;

/// The role an object plays in the physics simulation.
///
/// In Rust, enums can hold data in their variants (algebraic data types / tagged unions).
/// For example, `Phantom(PhantomBehavior)` stores the sensor's specific behavior mode.
#[derive(Clone, Copy)]
enum ObjectKind {
    /// Immovable geometry that blocks characters and dynamic bodies (e.g. floors, walls).
    Static,
    /// Movable objects driven by mass, gravity, and physical collisions (e.g. rolling balls, crates).
    Dynamic,
    /// A player or NPC controlled via kinematic displacement rather than raw physical forces.
    Character,
    /// A non-solid volume that senses overlaps or ray hits without physically blocking movement.
    Phantom(PhantomBehavior),
}

/// Metadata tracked by the host application for displaying an object on screen.
struct DemoObject {
    label: &'static str,
    shape: PrimitiveShape,
    transform: PhysicsTransform,
    handle: PhysicsHandle,
    kind: ObjectKind,
    color: Color,
    spin_angle: f32,
}

/// A lightweight render snapshot of a single object sent to the renderer.
///
/// Notice this struct contains zero physics engine internals or Rapier dependencies;
/// it only describes what shape to draw, where it is in 3D space, and its color.
pub struct RenderItem {
    pub shape: PrimitiveShape,
    pub transform: PhysicsTransform,
    pub color: Color,
}

/// A raycast path to be drawn in the 3D scene for visual debugging and feedback.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RayVisual {
    /// Starting point of the ray in world coordinates.
    pub origin: Vec3,
    /// Ending point of the ray (either the impact point or maximum range).
    pub target: Vec3,
    /// Information about the surface impact, if the ray hit something.
    pub hit: Option<RayHitVisual>,
}

/// Visual details for a ray impact point.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RayHitVisual {
    /// Exact coordinate where the ray intersected the collider surface.
    pub point: Vec3,
    /// The surface normal vector perpendicular to the hit surface (unit length).
    pub normal: Vec3,
}

/// Manages the demo's physics simulation state and object registry.
///
/// `Scene` acts as the bridge: it uses the abstract `PhysicsWorld` trait (backed by `RapierWorld`)
/// to simulate movement and collisions, and exports plain data for rendering.
pub struct Scene {
    physics: RapierWorld,
    objects: Vec<DemoObject>,
    character_handle: PhysicsHandle,
    vertical_velocity: f32,
    is_grounded: bool,
    overlap_count: usize,
    phantom_ray_hit_count: usize,
    active_rays: Vec<RayVisual>,
}

impl Scene {
    /// Builds the showcase scene, creating static exhibits, dynamic bodies, a player capsule,
    /// and rotating phantom sensors in the physics world.
    pub fn new() -> Result<Self, String> {
        let mut physics = RapierWorld::new();
        let mut objects = Vec::new();

        // 1. Add the immovable ground floor.
        add_static(
            &mut physics,
            &mut objects,
            "Ground",
            FLOOR_ENTITY,
            PrimitiveShape::Box {
                half_extents: Vec3::new(13.0, 0.25, 10.0),
            },
            transform(0.0, -0.25, 0.0),
            color(0.20, 0.24, 0.29, 1.0),
        )?;

        // 2. Add an exhibit row: each shape is added twice.
        // Once as a static (blue) exhibit at z = 5.5, and once as a dynamic (orange) body
        // dropping from the air at z = 1.5 to demonstrate rigid-body physics.
        let shapes = example_shapes();
        for (index, (label, shape)) in shapes.iter().enumerate() {
            let x = exhibit_x(index);
            let y = if matches!(shape, PrimitiveShape::Plane { .. }) {
                0.02
            } else {
                0.75
            };
            add_static(
                &mut physics,
                &mut objects,
                label,
                FIRST_EXHIBIT_ENTITY + index as u64,
                shape.clone(),
                transform(x, y, 5.5),
                color(0.28, 0.56, 0.90, 1.0),
            )?;

            add_dynamic(
                &mut physics,
                &mut objects,
                label,
                FIRST_EXHIBIT_ENTITY + 20 + index as u64,
                shape.clone(),
                transform(x, 4.0, 1.5),
                color(0.95, 0.59, 0.24, 1.0),
            )?;
        }

        // 3. Add the player character (capsule shape) using a kinematic character controller.
        let character_shape = PrimitiveShape::Capsule {
            radius: 0.45,
            half_height: 0.55,
        };
        let character_handle = physics
            .create_character(&CharacterDesc {
                entity_id: EntityId::new(CHARACTER_ENTITY),
                position: Vec3::new(0.0, 1.1, -7.0),
                shape: character_shape.clone(),
                max_slope_angle_radians: std::f32::consts::FRAC_PI_4, // 45 degree slope limit
                step_height: 0.25, // Can step up curbs/ledges up to 25cm high
            })
            .map_err(display_error)?;
        objects.push(DemoObject {
            label: "Player character",
            shape: character_shape,
            transform: transform(0.0, 1.1, -7.0),
            handle: character_handle,
            kind: ObjectKind::Character,
            color: color(0.95, 0.91, 0.42, 1.0),
            spin_angle: 0.0,
        });

        // 4. Add phantom colliders demonstrating the three sensor modes:
        // A. OverlapSensor: Senses objects touching its volume (like a coin or doorway trigger).
        add_phantom(
            &mut physics,
            &mut objects,
            "Overlap sensor",
            80,
            PrimitiveShape::Sphere { radius: 1.0 },
            transform(-6.0, 2.0, 1.5),
            PhantomBehavior::OverlapSensor,
            color(0.34, 0.90, 0.60, 0.34),
        )?;

        // B. RaycastTarget: Invisible/non-solid to objects, but intercepts line-of-sight raycasts
        //    (useful for hitboxes on holographic shields or interactive consoles).
        add_phantom(
            &mut physics,
            &mut objects,
            "Raycast target",
            81,
            PrimitiveShape::Torus {
                major_radius: 0.8,
                tube_radius: 0.22,
            },
            transform(0.0, 1.3, -3.0),
            PhantomBehavior::RaycastTarget,
            color(0.96, 0.40, 0.72, 0.38),
        )?;

        // C. OverlapSensorAndRaycastTarget: Senses both physical body overlaps and raycasts.
        add_phantom(
            &mut physics,
            &mut objects,
            "Overlap + ray target",
            82,
            PrimitiveShape::HeightField(HeightFieldDesc {
                rows: 3,
                columns: 3,
                height_samples: vec![0.0, 0.15, 0.0, 0.1, 0.35, 0.1, 0.0, 0.15, 0.0],
                cell_size_x: 0.75,
                cell_size_z: 0.75,
                thickness: Some(0.25),
            }),
            transform(5.0, 1.3, -3.0),
            PhantomBehavior::OverlapSensorAndRaycastTarget,
            color(0.30, 0.85, 0.96, 0.40),
        )?;

        Ok(Self {
            physics,
            objects,
            character_handle,
            vertical_velocity: 0.0,
            is_grounded: false,
            overlap_count: 0,
            phantom_ray_hit_count: 0,
            active_rays: Vec::new(),
        })
    }

    /// Advances the simulation by one fixed time step (e.g. 1/60th of a second).
    ///
    /// This is where game logic executes:
    /// 1. Jump request and gravity are integrated into the character's vertical velocity.
    /// 2. The kinematic character controller evaluates requested movement against colliders.
    /// 3. Phantom sensors are rotated around their Y axes.
    /// 4. The physics world steps forward to resolve dynamic body forces and collisions.
    /// 5. Overlaps and raycast hits are collected to update the HUD metrics.
    pub fn tick(
        &mut self,
        movement: Vec3,
        jump_requested: bool,
        time_step: f32,
    ) -> Result<(), String> {
        // --- 1. JUMPING AND GRAVITY ---
        // If the player pressed Space and is currently standing on a surface, apply an upward velocity.
        if jump_requested && self.is_grounded {
            self.vertical_velocity = 5.0; // Initial upward velocity in meters per second
            self.is_grounded = false;
        }

        // Apply downward acceleration due to Earth-standard gravity (9.81 m/s²).
        // v = v0 - g * dt
        self.vertical_velocity -= 9.81 * time_step;

        // --- 2. KINEMATIC CHARACTER MOVEMENT ---
        // Notice that we pass displacement (meters traveled in this tick), NOT velocity.
        // Horizontal speed is 4.0 meters per second.
        let character_displacement = Vec3::new(
            movement.x * 4.0 * time_step,
            self.vertical_velocity * time_step,
            movement.z * 4.0 * time_step,
        );

        // `move_character` handles collision sliding, stepping up stairs, and ground detection.
        // It returns a `CharacterMoveResult` telling us the actual movement applied and if we hit ground.
        let movement_result = self
            .physics
            .move_character(self.character_handle, &character_displacement)
            .map_err(display_error)?;

        // If the character has landed on the floor, zero out downward vertical velocity so gravity
        // does not build up an enormous downward speed while standing still.
        if movement_result.is_grounded && self.vertical_velocity < 0.0 {
            self.vertical_velocity = 0.0;
        }
        self.is_grounded = movement_result.is_grounded;

        // --- 3. ANIMATE / ROTATE PHANTOM SENSORS ---
        // Phantoms are sensors, but they can still move and rotate! Here we spin each phantom
        // around its Y axis to show that dynamic triggers/hitboxes track their transforms correctly.
        for object in &mut self.objects {
            let ObjectKind::Phantom(_) = object.kind else {
                continue;
            };
            object.spin_angle += 0.8 * time_step;
            let half_angle = object.spin_angle * 0.5;

            // In 3D math, rotations are best represented using Quaternions to avoid gimbal lock.
            // A rotation of angle θ around unit axis (0, 1, 0) is:
            // x = 0, y = sin(θ/2), z = 0, w = cos(θ/2)
            object.transform.rotation =
                Quaternion::new(0.0, half_angle.sin(), 0.0, half_angle.cos());

            // Inform the physics world of the new sensor transform.
            self.physics
                .set_phantom_transform(object.handle, &object.transform)
                .map_err(display_error)?;
        }

        // --- 4. ADVANCE PHYSICS SIMULATION ---
        // This steps dynamic bodies, simulates gravity on rolling/falling props, and resolves contacts.
        self.physics.step(time_step).map_err(display_error)?;

        // --- 5. POLL SENSORS AND QUERIES ---
        self.update_phantom_readouts()?;
        Ok(())
    }

    /// Queries the physics engine for phantom sensor triggers and raycast intersections.
    fn update_phantom_readouts(&mut self) -> Result<(), String> {
        self.overlap_count = 0;
        self.phantom_ray_hit_count = 0;
        self.active_rays.clear();

        for object in &self.objects {
            let ObjectKind::Phantom(behavior) = object.kind else {
                continue;
            };

            // Query overlap sensors to count how many entities are currently touching them.
            if matches!(
                behavior,
                PhantomBehavior::OverlapSensor | PhantomBehavior::OverlapSensorAndRaycastTarget
            ) {
                self.overlap_count += self
                    .physics
                    .phantom_overlaps(object.handle)
                    .map_err(display_error)?
                    .len();
            }

            // Cast a vertical downward ray towards the phantom to test raycast detection.
            if matches!(
                behavior,
                PhantomBehavior::RaycastTarget | PhantomBehavior::OverlapSensorAndRaycastTarget
            ) {
                // Compute the target position on the rotating object.
                let local_ray_offset = match &object.shape {
                    PrimitiveShape::Torus { major_radius, .. } => {
                        Vec3::new(*major_radius, 0.0, 0.0)
                    }
                    PrimitiveShape::HeightField(description) => Vec3::new(
                        description.cell_size_x * (description.columns - 1) as f32 * 0.5,
                        0.0,
                        description.cell_size_z * (description.rows - 1) as f32 * 0.5,
                    ),
                    _ => Vec3::ZERO,
                };
                let yaw = 2.0
                    * object
                        .transform
                        .rotation
                        .y
                        .atan2(object.transform.rotation.w);
                let (sin_yaw, cos_yaw) = yaw.sin_cos();
                let ray_world_x = cos_yaw * local_ray_offset.x + sin_yaw * local_ray_offset.z;
                let ray_world_z = -sin_yaw * local_ray_offset.x + cos_yaw * local_ray_offset.z;

                // Cast a line from 3 meters above downward through the sensor.
                let query = RayQuery {
                    origin: Vec3::new(
                        object.transform.position.x + ray_world_x,
                        object.transform.position.y + 3.0,
                        object.transform.position.z + ray_world_z,
                    ),
                    direction: Vec3::new(0.0, -1.0, 0.0),
                    max_distance: 6.0,
                    ignored_entity: None,
                };

                // `raycast_phantoms` returns all phantom hits without being blocked by physical colliders.
                let hits = self
                    .physics
                    .raycast_phantoms(&query)
                    .map_err(display_error)?;
                self.phantom_ray_hit_count += hits.len();

                // Record the ray path and hit impact normal for visual debugging.
                let (target, hit_visual) = if let Some(first_hit) = hits.first() {
                    (
                        first_hit.point,
                        Some(RayHitVisual {
                            point: first_hit.point,
                            normal: first_hit.surface_normal,
                        }),
                    )
                } else {
                    let max_displacement = query.direction.scale(query.max_distance);
                    (query.origin.add(&max_displacement), None)
                };

                self.active_rays.push(RayVisual {
                    origin: query.origin,
                    target,
                    hit: hit_visual,
                });
            }
        }

        // --- 6. PLAYER AIM / LINE-OF-SIGHT RAY ---
        // Cast a forward ray from the player character capsule towards +Z into the scene.
        // This lets the user see real-time physical raycasts against static and dynamic
        // colliders as they move around with WASD!
        let character_transform = self
            .physics
            .body_transform(self.character_handle)
            .map_err(display_error)?;
        let player_ray_origin = Vec3::new(
            character_transform.position.x,
            character_transform.position.y + 0.35,
            character_transform.position.z,
        );
        let player_ray_dir = Vec3::new(0.0, 0.0, 1.0);
        let player_query = RayQuery {
            origin: player_ray_origin,
            direction: player_ray_dir,
            max_distance: 18.0,
            ignored_entity: Some(EntityId::new(CHARACTER_ENTITY)),
        };

        let physical_hit = self.physics.raycast(&player_query).map_err(display_error)?;
        let (target, hit_visual) = if let Some(hit) = physical_hit {
            (
                hit.point,
                Some(RayHitVisual {
                    point: hit.point,
                    normal: hit.surface_normal,
                }),
            )
        } else {
            let max_displacement = player_ray_dir.scale(player_query.max_distance);
            (player_ray_origin.add(&max_displacement), None)
        };

        self.active_rays.push(RayVisual {
            origin: player_ray_origin,
            target,
            hit: hit_visual,
        });

        Ok(())
    }

    /// Returns the active rays and hit impact points to be drawn by the renderer this frame.
    pub fn ray_visuals(&self) -> &[RayVisual] {
        &self.active_rays
    }

    /// Generates the list of visual items to be drawn by the renderer this frame.
    ///
    /// Notice how state is retrieved:
    /// - Static objects and Phantoms keep their pre-configured or scripted transform.
    /// - Dynamic rigid bodies and Character controllers query `body_transform` from the physics engine
    ///   to get the exact position and orientation calculated by the simulation.
    pub fn render_items(&self) -> Result<Vec<RenderItem>, String> {
        self.objects
            .iter()
            .map(|object| {
                let transform = match object.kind {
                    ObjectKind::Static | ObjectKind::Phantom(_) => object.transform,
                    ObjectKind::Dynamic | ObjectKind::Character => self
                        .physics
                        .body_transform(object.handle)
                        .map_err(display_error)?,
                };
                Ok(RenderItem {
                    shape: object.shape.clone(),
                    transform,
                    color: object.color,
                })
            })
            .collect()
    }

    /// Formats current simulation status for the on-screen heads-up display (HUD).
    pub fn status_lines(&self) -> Vec<String> {
        let shape_names = self
            .objects
            .iter()
            .filter(|object| matches!(object.kind, ObjectKind::Static) && object.label != "Ground")
            .map(|object| object.label)
            .collect::<Vec<_>>()
            .join(", ");
        vec![
            "Blue: fixed | Orange: dynamic | Yellow: controlled character | Translucent: phantoms"
                .to_owned(),
            format!("Static and dynamic rows, left to right: {shape_names}"),
            format!(
                "Phantom results: {} overlapping entities | {} phantom ray hits",
                self.overlap_count, self.phantom_ray_hit_count
            ),
            "Phantoms: overlap-only sphere, ray-only torus, combined thick heightfield (all rotate)"
                .to_owned(),
            format!(
                "Character: {} | WASD move, Space jump",
                if self.is_grounded { "grounded" } else { "airborne" }
            ),
        ]
    }
}

/// Helper function providing a catalog of primitive shapes to showcase in the demo.
fn example_shapes() -> Vec<(&'static str, PrimitiveShape)> {
    vec![
        (
            "Box",
            PrimitiveShape::Box {
                half_extents: Vec3::new(0.65, 0.65, 0.65),
            },
        ),
        ("Sphere", PrimitiveShape::Sphere { radius: 0.65 }),
        (
            "Plane",
            PrimitiveShape::Plane {
                half_extents: Some(PlaneHalfExtents {
                    half_width_x: 0.8,
                    half_depth_z: 0.8,
                }),
            },
        ),
        (
            "Cylinder",
            PrimitiveShape::Cylinder {
                radius: 0.55,
                half_height: 0.7,
            },
        ),
        (
            "Capsule",
            PrimitiveShape::Capsule {
                radius: 0.45,
                half_height: 0.5,
            },
        ),
        (
            "Torus",
            PrimitiveShape::Torus {
                major_radius: 0.65,
                tube_radius: 0.2,
            },
        ),
        (
            "Thick heightfield",
            PrimitiveShape::HeightField(HeightFieldDesc {
                rows: 3,
                columns: 3,
                height_samples: vec![0.0, 0.2, 0.0, 0.15, 0.5, 0.15, 0.0, 0.2, 0.0],
                cell_size_x: 0.7,
                cell_size_z: 0.7,
                thickness: Some(0.25),
            }),
        ),
    ]
}

/// Registers a static collider in the physics engine and records its display properties.
///
/// Static colliders have zero mass and infinite inertia; they never move in response
/// to collisions, gravity, or impulses.
fn add_static(
    physics: &mut RapierWorld,
    objects: &mut Vec<DemoObject>,
    label: &'static str,
    entity_id: u64,
    shape: PrimitiveShape,
    transform: PhysicsTransform,
    color: Color,
) -> Result<(), String> {
    let handle = physics
        .create_static_collider(&StaticColliderDesc {
            entity_id: Some(EntityId::new(entity_id)),
            transform,
            shape: shape.clone(),
        })
        .map_err(display_error)?;
    objects.push(DemoObject {
        label,
        shape,
        transform,
        handle,
        kind: ObjectKind::Static,
        color,
        spin_angle: 0.0,
    });
    Ok(())
}

/// Registers a dynamic rigid body in the physics engine and records its display properties.
///
/// Dynamic bodies have mass and inertia. They are affected by gravity, can push and be pushed
/// by other bodies or characters, and simulate friction and restitution (bounciness).
fn add_dynamic(
    physics: &mut RapierWorld,
    objects: &mut Vec<DemoObject>,
    label: &'static str,
    entity_id: u64,
    shape: PrimitiveShape,
    transform: PhysicsTransform,
    color: Color,
) -> Result<(), String> {
    let handle = physics
        .create_dynamic_body(&DynamicBodyDesc {
            entity_id: EntityId::new(entity_id),
            transform,
            shape: shape.clone(),
            mass_kg: 2.0,
            friction: 0.6,
            elasticity: 0.1,
        })
        .map_err(display_error)?;
    objects.push(DemoObject {
        label,
        shape,
        transform,
        handle,
        kind: ObjectKind::Dynamic,
        color,
        spin_angle: 0.0,
    });
    Ok(())
}

/// Registers a phantom sensor in the physics engine and records its display properties.
///
/// Phantoms are non-solid triggers. They do not generate collision contact impulses,
/// but can trigger overlap events or be targeted by raycasts depending on `PhantomBehavior`.
#[allow(clippy::too_many_arguments)]
fn add_phantom(
    physics: &mut RapierWorld,
    objects: &mut Vec<DemoObject>,
    label: &'static str,
    entity_id: u64,
    shape: PrimitiveShape,
    transform: PhysicsTransform,
    behavior: PhantomBehavior,
    color: Color,
) -> Result<(), String> {
    let handle = physics
        .create_phantom_collider(&PhantomColliderDesc {
            entity_id: EntityId::new(entity_id),
            transform,
            shape: shape.clone(),
            behavior,
        })
        .map_err(display_error)?;
    objects.push(DemoObject {
        label,
        shape,
        transform,
        handle,
        kind: ObjectKind::Phantom(behavior),
        color,
        spin_angle: 0.0,
    });
    Ok(())
}

/// Computes the X coordinate spacing for the row of shape exhibits.
fn exhibit_x(index: usize) -> f32 {
    (index as f32 - 3.0) * 3.0
}

/// Helper creating a `PhysicsTransform` with identity (unrotated) orientation.
fn transform(x: f32, y: f32, z: f32) -> PhysicsTransform {
    PhysicsTransform::new(Vec3::new(x, y, z), Quaternion::IDENTITY)
}

/// Helper creating a Macroquad `Color`.
fn color(red: f32, green: f32, blue: f32, alpha: f32) -> Color {
    Color::new(red, green, blue, alpha)
}

/// Converts a strongly typed `PhysicsError` into a user-facing error message string.
fn display_error(error: PhysicsError) -> String {
    error.to_string()
}

#[cfg(test)]
mod tests {
    use super::Scene;
    use game_types::Vec3;
    use physics_api::{PhantomBehavior, PrimitiveShape};

    #[test]
    fn showcase_covers_all_shapes_and_phantom_behaviors() {
        let mut scene = Scene::new().expect("showcase setup should be valid");
        for _ in 0..180 {
            scene
                .tick(Vec3::ZERO, false, 1.0 / 60.0)
                .expect("showcase physics tick should succeed");
        }

        assert!(
            scene.overlap_count > 0,
            "overlap sensor should detect an object"
        );
        assert_eq!(
            scene.phantom_ray_hit_count, 2,
            "ray-target phantoms should be hit by the showcase rays"
        );
        assert!(
            scene
                .objects
                .iter()
                .any(|object| matches!(object.kind, super::ObjectKind::Static))
        );
        assert!(
            scene
                .objects
                .iter()
                .any(|object| matches!(object.kind, super::ObjectKind::Dynamic))
        );
        assert!(
            scene
                .objects
                .iter()
                .any(|object| matches!(object.kind, super::ObjectKind::Character))
        );

        let phantom_objects = scene
            .objects
            .iter()
            .filter(|object| matches!(object.kind, super::ObjectKind::Phantom(_)))
            .collect::<Vec<_>>();
        assert_eq!(phantom_objects.len(), 3);
        assert!(
            phantom_objects
                .iter()
                .all(|object| object.transform.rotation.y.abs() > 0.1),
            "each phantom should have a visibly changing rotation"
        );
        assert!(phantom_objects.iter().any(|object| matches!(
            object.kind,
            super::ObjectKind::Phantom(PhantomBehavior::OverlapSensor)
        )));
        assert!(phantom_objects.iter().any(|object| matches!(
            object.kind,
            super::ObjectKind::Phantom(PhantomBehavior::RaycastTarget)
        )));
        assert!(phantom_objects.iter().any(|object| matches!(
            object.kind,
            super::ObjectKind::Phantom(PhantomBehavior::OverlapSensorAndRaycastTarget)
        )));

        let items = scene
            .render_items()
            .expect("all showcase objects should have transforms");
        let shapes = items.iter().map(|item| &item.shape).collect::<Vec<_>>();
        assert!(
            shapes
                .iter()
                .any(|shape| matches!(shape, PrimitiveShape::Box { .. }))
        );
        assert!(
            shapes
                .iter()
                .any(|shape| matches!(shape, PrimitiveShape::Sphere { .. }))
        );
        assert!(
            shapes
                .iter()
                .any(|shape| matches!(shape, PrimitiveShape::Plane { .. }))
        );
        assert!(
            shapes
                .iter()
                .any(|shape| matches!(shape, PrimitiveShape::Cylinder { .. }))
        );
        assert!(
            shapes
                .iter()
                .any(|shape| matches!(shape, PrimitiveShape::Capsule { .. }))
        );
        assert!(
            shapes
                .iter()
                .any(|shape| matches!(shape, PrimitiveShape::Torus { .. }))
        );
        assert!(
            shapes
                .iter()
                .any(|shape| matches!(shape, PrimitiveShape::HeightField(_)))
        );

        let rays = scene.ray_visuals();
        assert_eq!(
            rays.len(),
            3,
            "should produce 2 phantom rays and 1 player ray"
        );
        assert!(
            rays.iter().any(|ray| ray.hit.is_some()),
            "at least one ray should register a hit impact"
        );
        for ray in rays {
            if let Some(hit) = &ray.hit {
                assert!(
                    hit.normal.length_squared() > 0.5,
                    "hit surface normal should be a non-zero direction"
                );
            }
        }
    }
}
