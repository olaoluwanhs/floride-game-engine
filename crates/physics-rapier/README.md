# physics-rapier

This crate implements `physics-api` with Rapier 3D. Rapier types and handles
stay inside this adapter; callers use the engine-independent types from
`physics-api`.

`RapierWorld` provides all operations in `PhysicsWorld`: static and sensor
colliders, dynamic rigid bodies and impulses, kinematic character movement,
ray queries, fixed-step simulation, and transform lookup. The default gravity
is `(0, -9.81, 0)` meters per second squared; use `RapierWorld::with_gravity`
to supply a different finite vector.

Supported shape conversions include boxes, spheres, cylinders, capsules,
planes, torus approximations, and heightfields. A finite plane is represented
by a thin box. A torus is represented by 32 overlapping capsule segments.
Heightfields can also be used for dynamic bodies and characters. A heightfield
with `thickness: None` remains a surface. Setting a positive `thickness`
builds a closed solid collision shape: each grid cell is split into the same
two triangles as Rapier's heightfield, then extruded downward into convex
prisms and grouped together. This gives the shape a bent top, underside, and
sides, and works for static colliders, dynamic bodies, characters, and
phantoms. More grid cells create more convex pieces, increasing collision
work. Invalid dimensions and non-finite inputs still return errors.

Phantom overlap results reflect the most recently completed `step`. Queries
use Rapier's broad-phase data, so call `step` after adding or moving objects
before relying on raycasts or character movement against the updated scene.
