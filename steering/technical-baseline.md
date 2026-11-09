# Technical Baseline

This is a compact summary, not a replacement for `docs/Rust_Game_Core_Architecture.md`.

## What we build and what we reuse

- Put engine-independent mechanics that can serve multiple games in `game-logic`, depending only on `game-types` and `physics-api`. Movement systems consume general intents rather than a fixed game-command list.
- Keep each game's commands/actions, input bindings, combo sequences and responses in that game's crate. Games can define any number of actions and variable-length combos; `game-types` and `game-logic` must not impose a universal command enum.
- Standardize host integration through `game-api::GameAdapter`: each game implements the same create/input/tick/snapshot/event lifecycle while retaining its own associated types and selecting a `PhysicsWorld` implementation.
- Build the FPS-specific rules in the FPS game crate: weapons, damage, enemy decisions, modes, snapshots and events. Reuse shared mechanics from `game-logic` where appropriate.
- Reuse focused libraries for established technical problems. Rapier 3D is the planned physics implementation, hidden behind our own physics interface.
- Do not adopt a full game engine or a separate game-loop library for the initial headless core. The host paces the simulation and calls one fixed-step `tick()` at a time.

## Rust style

- Use ordinary Rust composition: structs hold related data, enums represent a known set of states, and functions implement behavior.
- Keep early code beginner-readable: use descriptive names, explicit intermediate values, and straightforward control flow. Do not make code shorter or more optimized at the cost of clarity.
- Prefer named operations over operator overloading or dense method chains when the named form makes the behavior easier to understand. Explain unfamiliar Rust features where they are introduced.
- Use traits where they define a real boundary, especially the replaceable physics interface. Avoid object-oriented inheritance and avoid forcing a purely functional style.
- Start with plain structs and collections, not a full ECS. Add a larger framework only when real project needs justify it.
- Keep common game types independent of Rapier and rendering technology.

## Time, threads, and processes

- Begin with a fixed 60-tick-per-second simulation and one simulation thread. This is the starting design, not a performance guarantee.
- Basic bot decisions run in the normal update loop; they do not need their own thread. AI may update less often than physics if the behavior and tests call for it.
- Add parallel work only after profiling identifies a bottleneck and tests can protect the behavior. Concurrency adds coordination and can make results harder to reproduce.
- Multiple processes and multiplayer networking are not Stage 1 work. A host may later run the simulation on a worker thread to keep its UI responsive; that does not change the core's game rules.

## Implementation order

Follow the milestones in the architecture document: repository foundation; physics and a moving player; `game-api` and the FPS adapter implementation, reusable `game-logic`, then FPS-owned actions, combos, runtime and snapshots; then weapons/enemies and survival. Add WASM/native bindings and story support only at their planned milestones.
