use physics_api::{PhantomRayHit, PhysicsError, RayHit, RayQuery};
use rapier3d::prelude::{Collider, ColliderHandle, QueryFilter, Ray};

use crate::{conversions, validation, world::RapierWorld};

/// Returns the nearest hit against solid colliders, or None when nothing is hit.
pub(super) fn raycast(
    world: &RapierWorld,
    query: &RayQuery,
) -> Result<Option<RayHit>, PhysicsError> {
    validation::validate_ray(query)?;
    // Exclude the caller's entity but keep anonymous colliders, such as walls.
    let predicate = |collider_handle: ColliderHandle, _collider: &Collider| {
        world
            .entities_by_collider
            .get(&collider_handle)
            .copied()
            .flatten()
            .map(|entity_id| Some(entity_id) != query.ignored_entity)
            .unwrap_or(true)
    };
    let query_pipeline = world.broad_phase.as_query_pipeline(
        world.narrow_phase.query_dispatcher(),
        &world.bodies,
        &world.colliders,
        QueryFilter::new().exclude_sensors().predicate(&predicate),
    );
    let ray = Ray::new(
        conversions::to_vector(&query.origin),
        conversions::to_vector(&query.direction),
    );
    let hit = query_pipeline
        .cast_ray_and_get_normal(&ray, query.max_distance, true)
        .map(|(collider_handle, intersection)| {
            let point = ray.origin + ray.dir * intersection.time_of_impact;
            RayHit {
                point: conversions::from_vector(point),
                surface_normal: conversions::from_vector(intersection.normal),
                distance: intersection.time_of_impact,
                entity_id: world
                    .entities_by_collider
                    .get(&collider_handle)
                    .copied()
                    .flatten(),
            }
        });
    Ok(hit)
}

/// Returns every ray hit on phantom targets, sorted from nearest to farthest.
///
/// Phantom hits are separate from physical hits: they do not stop the ray from
/// also finding a wall or other solid collider behind the sensor.
pub(super) fn raycast_phantoms(
    world: &RapierWorld,
    query: &RayQuery,
) -> Result<Vec<PhantomRayHit>, PhysicsError> {
    validation::validate_ray(query)?;
    let predicate = |collider_handle: ColliderHandle, _collider: &Collider| {
        world.phantom_ray_targets.contains(&collider_handle)
            && world
                .entities_by_collider
                .get(&collider_handle)
                .copied()
                .flatten()
                .map(|entity_id| Some(entity_id) != query.ignored_entity)
                .unwrap_or(false)
    };
    let query_pipeline = world.broad_phase.as_query_pipeline(
        world.narrow_phase.query_dispatcher(),
        &world.bodies,
        &world.colliders,
        QueryFilter::new().predicate(&predicate),
    );
    let ray = Ray::new(
        conversions::to_vector(&query.origin),
        conversions::to_vector(&query.direction),
    );
    let ray_origin = ray.origin;
    let ray_direction = ray.dir;
    let mut hits = query_pipeline
        .intersect_ray(ray, query.max_distance, true)
        .filter_map(|(collider_handle, _collider, intersection)| {
            let entity_id = world
                .entities_by_collider
                .get(&collider_handle)
                .copied()
                .flatten()?;
            let point = ray_origin + ray_direction * intersection.time_of_impact;
            Some(PhantomRayHit {
                entity_id,
                point: conversions::from_vector(point),
                surface_normal: conversions::from_vector(intersection.normal),
                distance: intersection.time_of_impact,
            })
        })
        .collect::<Vec<_>>();
    // Do not rely on Rapier's traversal order as a public API guarantee.
    hits.sort_by(|left, right| left.distance.total_cmp(&right.distance));
    Ok(hits)
}
