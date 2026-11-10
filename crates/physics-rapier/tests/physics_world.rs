use game_types::{EntityId, Vec3};
use physics_api::{
    CharacterDesc, DynamicBodyDesc, HeightFieldDesc, PhantomBehavior, PhantomColliderDesc,
    PhysicsTransform, PhysicsWorld, PlaneHalfExtents, PrimitiveShape, Quaternion, RayQuery,
    StaticColliderDesc,
};
use physics_rapier::RapierWorld;

fn transform_at(x: f32, y: f32, z: f32) -> PhysicsTransform {
    PhysicsTransform::new(Vec3::new(x, y, z), Quaternion::IDENTITY)
}

fn static_box(entity_id: u64, x: f32, y: f32, z: f32, half_extents: Vec3) -> StaticColliderDesc {
    StaticColliderDesc {
        entity_id: Some(EntityId::new(entity_id)),
        transform: transform_at(x, y, z),
        shape: PrimitiveShape::Box { half_extents },
    }
}

#[test]
fn character_moves_and_is_blocked_by_a_wall() {
    let mut world = RapierWorld::new();
    world
        .create_static_collider(&StaticColliderDesc {
            entity_id: None,
            transform: PhysicsTransform::IDENTITY,
            shape: PrimitiveShape::Plane { half_extents: None },
        })
        .unwrap();
    world
        .create_static_collider(&static_box(2, 2.0, 1.0, 0.0, Vec3::new(0.1, 1.0, 3.0)))
        .unwrap();
    let character = world
        .create_character(&CharacterDesc {
            entity_id: EntityId::new(1),
            position: Vec3::new(0.0, 1.0, 0.0),
            shape: PrimitiveShape::Capsule {
                radius: 0.5,
                half_height: 0.5,
            },
            max_slope_angle_radians: std::f32::consts::FRAC_PI_4,
            step_height: 0.25,
        })
        .unwrap();

    world.step(1.0 / 60.0).unwrap();
    let movement = world
        .move_character(character, &Vec3::new(5.0, 0.0, 0.0))
        .unwrap();

    assert!(movement.collided);
    assert!(movement.applied_displacement.x < 5.0);
    assert!(movement.applied_displacement.x > 0.0);
    assert!(movement.is_grounded);
    assert!(world.is_grounded(character).unwrap());
    world.step(1.0 / 60.0).unwrap();
    world.remove_character(character).unwrap();
}

#[test]
fn character_pushes_a_dynamic_box() {
    let mut world = RapierWorld::new();
    world
        .create_static_collider(&StaticColliderDesc {
            entity_id: Some(EntityId::new(100)),
            transform: PhysicsTransform::IDENTITY,
            shape: PrimitiveShape::Plane { half_extents: None },
        })
        .unwrap();

    let body = world
        .create_dynamic_body(&DynamicBodyDesc {
            entity_id: EntityId::new(101),
            transform: transform_at(2.0, 0.5, 0.0),
            shape: PrimitiveShape::Box {
                half_extents: Vec3::new(0.5, 0.5, 0.5),
            },
            mass_kg: 2.0,
            friction: 0.5,
            elasticity: 0.0,
        })
        .unwrap();
    let character = world
        .create_character(&CharacterDesc {
            entity_id: EntityId::new(102),
            position: Vec3::new(0.0, 1.0, 0.0),
            shape: PrimitiveShape::Capsule {
                radius: 0.5,
                half_height: 0.5,
            },
            max_slope_angle_radians: std::f32::consts::FRAC_PI_4,
            step_height: 0.0,
        })
        .unwrap();

    let initial_x = world.body_transform(body).unwrap().position.x;
    for _ in 0..8 {
        world
            .move_character(character, &Vec3::new(0.12, 0.0, 0.0))
            .unwrap();
        world.step(1.0 / 60.0).unwrap();
    }
    let distant_x = world.body_transform(body).unwrap().position.x;
    assert!(
        (distant_x - initial_x).abs() < 0.05,
        "dynamic box moved before the character reached it: x={distant_x}"
    );

    for _ in 8..20 {
        world
            .move_character(character, &Vec3::new(0.12, 0.0, 0.0))
            .unwrap();
        world.step(1.0 / 60.0).unwrap();
    }

    let moved_x = world.body_transform(body).unwrap().position.x;
    assert!(
        moved_x > initial_x + 0.05,
        "dynamic box stayed at x={initial_x}, expected movement after character contact; got x={moved_x}"
    );
}

#[test]
fn raycast_returns_entity_and_respects_the_ignored_entity() {
    let mut world = RapierWorld::new();
    let target = world
        .create_static_collider(&static_box(5, 0.0, 0.0, 4.0, Vec3::new(1.0, 1.0, 0.25)))
        .unwrap();
    world.step(1.0 / 60.0).unwrap();
    let ray = RayQuery {
        origin: Vec3::ZERO,
        direction: Vec3::new(0.0, 0.0, 1.0),
        max_distance: 10.0,
        ignored_entity: None,
    };

    let hit = world.raycast(&ray).unwrap().unwrap();
    assert_eq!(hit.entity_id, Some(EntityId::new(5)));
    assert!((hit.distance - 3.75).abs() < 0.01);

    let ignored_ray = RayQuery {
        ignored_entity: Some(EntityId::new(5)),
        ..ray
    };
    assert!(world.raycast(&ignored_ray).unwrap().is_none());
    world.remove_static_collider(target).unwrap();
}

#[test]
fn phantom_reports_overlaps_and_raycast_hits() {
    let mut world = RapierWorld::new();
    let phantom = world
        .create_phantom_collider(&PhantomColliderDesc {
            entity_id: EntityId::new(10),
            transform: transform_at(0.0, 0.0, 0.0),
            shape: PrimitiveShape::Sphere { radius: 2.0 },
            behavior: PhantomBehavior::OverlapSensorAndRaycastTarget,
        })
        .unwrap();
    world
        .create_dynamic_body(&DynamicBodyDesc {
            entity_id: EntityId::new(11),
            transform: transform_at(0.0, 0.0, 0.0),
            shape: PrimitiveShape::Sphere { radius: 0.5 },
            mass_kg: 1.0,
            friction: 0.5,
            elasticity: 0.0,
        })
        .unwrap();
    world
        .create_character(&CharacterDesc {
            entity_id: EntityId::new(12),
            position: Vec3::ZERO,
            shape: PrimitiveShape::Capsule {
                radius: 0.25,
                half_height: 0.5,
            },
            max_slope_angle_radians: std::f32::consts::FRAC_PI_4,
            step_height: 0.0,
        })
        .unwrap();
    world.step(1.0 / 60.0).unwrap();

    let overlaps = world.phantom_overlaps(phantom).unwrap();
    assert_eq!(overlaps.len(), 2);
    assert_eq!(overlaps[0].phantom_entity_id, EntityId::new(10));
    assert_eq!(overlaps[0].other_entity_id, EntityId::new(11));
    assert_eq!(overlaps[1].other_entity_id, EntityId::new(12));

    let hits = world
        .raycast_phantoms(&RayQuery {
            origin: Vec3::new(-4.0, 0.0, 0.0),
            direction: Vec3::new(1.0, 0.0, 0.0),
            max_distance: 8.0,
            ignored_entity: None,
        })
        .unwrap();
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].entity_id, EntityId::new(10));
    world.remove_phantom_collider(phantom).unwrap();
    assert!(world.phantom_overlaps(phantom).is_err());
}

#[test]
fn phantom_transform_can_be_updated() {
    let mut world = RapierWorld::new();
    let phantom = world
        .create_phantom_collider(&PhantomColliderDesc {
            entity_id: EntityId::new(13),
            transform: transform_at(0.0, 0.0, 0.0),
            shape: PrimitiveShape::Sphere { radius: 1.0 },
            behavior: PhantomBehavior::RaycastTarget,
        })
        .unwrap();

    world
        .set_phantom_transform(phantom, &transform_at(3.0, 2.0, 1.0))
        .unwrap();
    let moved_transform = world.body_transform(phantom).unwrap();
    assert_eq!(moved_transform.position, Vec3::new(3.0, 2.0, 1.0));

    let error = world.set_phantom_transform(
        phantom,
        &PhysicsTransform::new(Vec3::ZERO, Quaternion::new(0.0, 0.0, 0.0, 0.0)),
    );
    assert!(error.is_err());
}

#[test]
fn updating_heightfield_phantom_transform_uses_the_public_origin() {
    let mut world = RapierWorld::new();
    let phantom = world
        .create_phantom_collider(&PhantomColliderDesc {
            entity_id: EntityId::new(14),
            transform: PhysicsTransform::IDENTITY,
            shape: PrimitiveShape::HeightField(HeightFieldDesc {
                rows: 2,
                columns: 2,
                height_samples: vec![0.0; 4],
                cell_size_x: 1.0,
                cell_size_z: 1.0,
                thickness: None,
            }),
            behavior: PhantomBehavior::RaycastTarget,
        })
        .unwrap();
    let rotation = Quaternion::new(
        0.0,
        std::f32::consts::FRAC_1_SQRT_2,
        0.0,
        std::f32::consts::FRAC_1_SQRT_2,
    );
    world
        .set_phantom_transform(
            phantom,
            &PhysicsTransform::new(Vec3::new(3.0, 2.0, 1.0), rotation),
        )
        .unwrap();

    let moved_transform = world.body_transform(phantom).unwrap();
    assert!((moved_transform.position.x - 3.0).abs() < 0.001);
    assert!((moved_transform.position.y - 2.0).abs() < 0.001);
    assert!((moved_transform.position.z - 1.0).abs() < 0.001);
}

#[test]
fn dynamic_body_responds_to_impulses_and_can_be_removed() {
    let mut world = RapierWorld::new();
    let body = world
        .create_dynamic_body(&DynamicBodyDesc {
            entity_id: EntityId::new(20),
            transform: transform_at(0.0, 5.0, 0.0),
            shape: PrimitiveShape::Box {
                half_extents: Vec3::new(0.5, 0.5, 0.5),
            },
            mass_kg: 2.0,
            friction: 0.5,
            elasticity: 0.1,
        })
        .unwrap();
    world
        .apply_impulse(body, &Vec3::new(2.0, 0.0, 0.0))
        .unwrap();
    world
        .apply_impulse_at_point(body, &Vec3::new(0.0, 0.0, 1.0), &Vec3::new(0.0, 5.0, 0.5))
        .unwrap();
    world.step(1.0 / 60.0).unwrap();

    let transform = world.body_transform(body).unwrap();
    assert!(transform.position.x > 0.0);
    assert!(transform.position.y < 5.0);
    world.remove_dynamic_body(body).unwrap();
    assert!(world.body_transform(body).is_err());
}

#[test]
fn plane_heightfield_torus_and_finite_plane_are_accepted_as_static_shapes() {
    let mut world = RapierWorld::new();
    let descriptions = [
        StaticColliderDesc {
            entity_id: None,
            transform: transform_at(0.0, 0.0, 0.0),
            shape: PrimitiveShape::Plane {
                half_extents: Some(PlaneHalfExtents {
                    half_width_x: 3.0,
                    half_depth_z: 3.0,
                }),
            },
        },
        StaticColliderDesc {
            entity_id: None,
            transform: transform_at(5.0, 0.0, 0.0),
            shape: PrimitiveShape::HeightField(HeightFieldDesc {
                rows: 2,
                columns: 3,
                height_samples: vec![0.0, 0.25, 0.0, 0.0, 0.25, 0.0],
                cell_size_x: 1.0,
                cell_size_z: 1.0,
                thickness: None,
            }),
        },
        StaticColliderDesc {
            entity_id: None,
            transform: transform_at(10.0, 0.0, 0.0),
            shape: PrimitiveShape::Torus {
                major_radius: 1.0,
                tube_radius: 0.2,
            },
        },
    ];

    for description in &descriptions {
        world.create_static_collider(description).unwrap();
    }
    world.step(1.0 / 60.0).unwrap();
}

#[test]
fn dynamic_bodies_and_characters_accept_heightfields_and_planes() {
    let heightfield = PrimitiveShape::HeightField(HeightFieldDesc {
        rows: 2,
        columns: 2,
        height_samples: vec![0.0, 0.2, 0.1, 0.3],
        cell_size_x: 1.0,
        cell_size_z: 1.0,
        thickness: Some(0.25),
    });

    let mut heightfield_world = RapierWorld::new();
    let dynamic_heightfield = heightfield_world
        .create_dynamic_body(&DynamicBodyDesc {
            entity_id: EntityId::new(40),
            transform: transform_at(0.0, 5.0, 0.0),
            shape: heightfield.clone(),
            mass_kg: 2.0,
            friction: 0.5,
            elasticity: 0.0,
        })
        .unwrap();
    heightfield_world.step(1.0 / 60.0).unwrap();
    assert!(
        heightfield_world
            .body_transform(dynamic_heightfield)
            .unwrap()
            .position
            .y
            < 5.0
    );

    let mut plane_world = RapierWorld::new();
    let dynamic_infinite_plane = plane_world
        .create_dynamic_body(&DynamicBodyDesc {
            entity_id: EntityId::new(41),
            transform: transform_at(10.0, 5.0, 0.0),
            shape: PrimitiveShape::Plane { half_extents: None },
            mass_kg: 2.0,
            friction: 0.5,
            elasticity: 0.0,
        })
        .unwrap();
    plane_world.step(1.0 / 60.0).unwrap();
    assert!(
        plane_world
            .body_transform(dynamic_infinite_plane)
            .unwrap()
            .position
            .y
            < 5.0
    );

    let character_shapes = [
        PrimitiveShape::Plane { half_extents: None },
        PrimitiveShape::Torus {
            major_radius: 1.0,
            tube_radius: 0.2,
        },
        heightfield,
    ];
    for (index, shape) in character_shapes.into_iter().enumerate() {
        let mut character_world = RapierWorld::new();
        character_world
            .create_static_collider(&static_box(
                50 + index as u64,
                2.0,
                1.0,
                0.0,
                Vec3::new(0.1, 1.0, 2.0),
            ))
            .unwrap();
        let character = character_world
            .create_character(&CharacterDesc {
                entity_id: EntityId::new(42 + index as u64),
                position: Vec3::new(0.0, 1.0, 0.0),
                shape,
                max_slope_angle_radians: std::f32::consts::FRAC_PI_4,
                step_height: 0.0,
            })
            .unwrap();
        character_world.step(1.0 / 60.0).unwrap();
        character_world
            .move_character(character, &Vec3::new(3.0, 0.0, 0.0))
            .unwrap();
    }
}

#[test]
fn thick_heightfield_has_top_and_underside_and_works_as_a_phantom() {
    let mut world = RapierWorld::new();
    let shape = PrimitiveShape::HeightField(HeightFieldDesc {
        rows: 2,
        columns: 2,
        height_samples: vec![0.0; 4],
        cell_size_x: 1.0,
        cell_size_z: 1.0,
        thickness: Some(0.25),
    });
    let collider = world
        .create_static_collider(&StaticColliderDesc {
            entity_id: Some(EntityId::new(60)),
            transform: PhysicsTransform::IDENTITY,
            shape: shape.clone(),
        })
        .unwrap();
    let phantom = world
        .create_phantom_collider(&PhantomColliderDesc {
            entity_id: EntityId::new(61),
            transform: transform_at(3.0, 0.0, 0.0),
            shape: shape.clone(),
            behavior: PhantomBehavior::OverlapSensorAndRaycastTarget,
        })
        .unwrap();

    world.step(1.0 / 60.0).unwrap();
    let top_hit = world
        .raycast(&RayQuery {
            origin: Vec3::new(0.5, 1.0, 0.5),
            direction: Vec3::new(0.0, -1.0, 0.0),
            max_distance: 2.0,
            ignored_entity: None,
        })
        .unwrap()
        .unwrap();
    assert!((top_hit.distance - 1.0).abs() < 0.01);

    let underside_hit = world
        .raycast(&RayQuery {
            origin: Vec3::new(0.5, -1.0, 0.5),
            direction: Vec3::new(0.0, 1.0, 0.0),
            max_distance: 2.0,
            ignored_entity: None,
        })
        .unwrap()
        .unwrap();
    assert!((underside_hit.distance - 0.75).abs() < 0.01);

    let phantom_hits = world
        .raycast_phantoms(&RayQuery {
            origin: Vec3::new(3.5, 1.0, 0.5),
            direction: Vec3::new(0.0, -1.0, 0.0),
            max_distance: 2.0,
            ignored_entity: None,
        })
        .unwrap();
    assert_eq!(phantom_hits.len(), 1);
    assert_eq!(phantom_hits[0].entity_id, EntityId::new(61));
    world.remove_static_collider(collider).unwrap();
    world.remove_phantom_collider(phantom).unwrap();
}

#[test]
fn heightfield_thickness_must_be_positive_and_finite() {
    let mut world = RapierWorld::new();
    for thickness in [0.0, -0.1, f32::NAN, f32::INFINITY] {
        let result = world.create_static_collider(&StaticColliderDesc {
            entity_id: None,
            transform: PhysicsTransform::IDENTITY,
            shape: PrimitiveShape::HeightField(HeightFieldDesc {
                rows: 2,
                columns: 2,
                height_samples: vec![0.0; 4],
                cell_size_x: 1.0,
                cell_size_z: 1.0,
                thickness: Some(thickness),
            }),
        });
        assert!(result.is_err(), "accepted invalid thickness {thickness}");
    }
}

#[test]
fn invalid_descriptions_and_wrong_handle_kinds_return_errors() {
    let mut world = RapierWorld::new();
    assert!(world.step(0.0).is_err());
    assert!(world.step(f32::INFINITY).is_err());

    let static_collider = world
        .create_static_collider(&static_box(30, 0.0, 0.0, 0.0, Vec3::new(1.0, 1.0, 1.0)))
        .unwrap();
    assert!(world.remove_dynamic_body(static_collider).is_err());
    assert!(
        world
            .raycast(&RayQuery {
                origin: Vec3::ZERO,
                direction: Vec3::new(2.0, 0.0, 0.0),
                max_distance: 5.0,
                ignored_entity: None,
            })
            .is_err()
    );
    world.remove_static_collider(static_collider).unwrap();
}

#[test]
fn dynamic_body_entity_lookup_and_radius_query() {
    let mut world = RapierWorld::new();
    let static_id = EntityId::new(40);
    let near_id = EntityId::new(41);
    let far_id = EntityId::new(42);

    world
        .create_static_collider(&static_box(40, 0.0, 0.0, 0.0, Vec3::new(1.0, 1.0, 1.0)))
        .unwrap();
    let near_handle = world
        .create_dynamic_body(&DynamicBodyDesc {
            entity_id: near_id,
            transform: transform_at(2.0, 1.0, 0.0),
            shape: PrimitiveShape::Sphere { radius: 0.5 },
            mass_kg: 1.5,
            friction: 0.5,
            elasticity: 0.2,
        })
        .unwrap();
    world
        .create_dynamic_body(&DynamicBodyDesc {
            entity_id: far_id,
            transform: transform_at(15.0, 1.0, 0.0),
            shape: PrimitiveShape::Sphere { radius: 0.5 },
            mass_kg: 1.5,
            friction: 0.5,
            elasticity: 0.2,
        })
        .unwrap();

    // Static colliders must not resolve as dynamic bodies.
    assert_eq!(world.dynamic_body_handle(static_id), None);
    assert_eq!(world.dynamic_body_handle(near_id), Some(near_handle));

    let nearby = world
        .dynamic_bodies_in_radius(&Vec3::new(0.0, 1.0, 0.0), 4.0)
        .unwrap();
    assert_eq!(nearby.len(), 1);
    assert_eq!(nearby[0].0, near_id);
    assert_eq!(nearby[0].1, near_handle);
}
