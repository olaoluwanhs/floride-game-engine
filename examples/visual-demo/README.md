# FPS Core Visual Demo

This Macroquad application shows the physics API running through the Rapier
adapter. It includes static and dynamic examples of every primitive shape, a
controllable capsule character, and rotating phantom sensors using all three
phantom behaviors.

Run it from the `fps-core` repository root:

```sh
cargo run -p visual-demo
```

Controls:

- **W/A/S/D**: move the character
- **Space**: jump
- **Arrow keys** or **right mouse drag**: orbit the camera
- **Mouse wheel**: zoom

The on-screen counters show phantom overlaps and phantom raycast hits. Phantoms
are sensors: they detect but do not physically block the character or bodies.

Raycast Visuals:

- **Red lines**: Ray trajectories (vertical sensor rays tracking rotating phantoms, and a line-of-sight ray from the player character).
- **Yellow crosshairs**: Exact impact contact points on intersected surfaces.
- **Green lines**: Surface normal vectors pointing outward perpendicular to the touched geometry.
