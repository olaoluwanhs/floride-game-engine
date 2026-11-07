# Rust Game Core: Architecture and Implementation Plan

**Document type:** Architecture specification and implementation guide  
**Project:** Portable first-person shooter (FPS) game simulation library  
**Status:** Proposed architecture for Stage 1 (single-player)  
**Last updated:** 2026-11-07  
**Primary readers:** Developer and GitHub Copilot coding/steering agents

---

## 1. Purpose

Build an **independent Rust library** containing the logic and simulation for a first-person shooter. The library must not be tied to any particular game engine or user interface. We should be able to run the same core from:

- A browser using Rust compiled to WebAssembly (WASM).
- A native mobile application (Android/iOS), through a native integration layer.
- A C or C++ application, including a renderer built using raylib.
- A Unity application, through a C-compatible native library and a C# wrapper.
- A Godot application, through an appropriate native extension/wrapper.
- A Rust command-line test program, with no graphics at all.

**The goal is to reuse gameplay rules and simulation, not to reuse the renderer.** Every application is responsible for drawing the world, reading hardware input, playing audio, and providing menus. The Rust core decides what happens in the game.

### Scope of this document

This document specifies **only the Rust core repository**. It describes its modules, internal contracts, physics adapter, update loop, state API, packaging, tests, implementation sequence and engineering rules. It does not specify the folder structure or implementation of a complete Next.js, React Native, Unity, or Godot application.

### Project stages

- **Stage 1: Offline single-player.** Build a solid simulation for a survival mode and later a story mode. No matchmaking, dedicated servers or online accounts are required.
- **Stage 2: Multiplayer (future).** Reuse as much of the simulation as possible in an authoritative server. Do not build networking now, but make early choices that do not prevent it later.

---

## 2. Non-negotiable design principles

1. **The core is independent.** No React, Three.js, React Native, Unity, Godot, or raylib imports anywhere in gameplay or physics contracts.
2. **Rust owns game truth.** Position, health, ammunition, enemy state, damage, scoring, survival waves and missions are decided by Rust, not by a renderer.
3. **Physics is replaceable.** Define a small Rust physics interface; implement it initially with Rapier 3D. Do not expose Rapier types in the game's public API.
4. **Use existing libraries rather than reinventing them.** Rapier handles collision detection and physics; our code handles game behavior and coordination.
5. **The core is headless.** All gameplay must be testable without rendering or audio.
6. **Use commands in and snapshots/events out.** An application sends actions; the Rust core advances the world; the application reads the result.
7. **Fixed update rate.** Simulation runs at a stable fixed interval; applications may draw at another rate.
8. **Build for today, leave room for tomorrow.** Do not implement multiplayer, a huge entity framework, or half a dozen engine bindings before a playable local prototype exists.
9. **Prefer clear, testable Rust over clever abstractions.** Add interfaces only where they solve a real boundary, such as physics or host integration.
10. **Make failures explicit.** Validate external input, handle invalid handles and corrupted save files, and never let an FFI panic cross into another language.

---

## 3. What belongs in the core, and what does not

| The Rust core owns | The application / renderer owns |
|---|---|
| World simulation and entities | 3D drawing, shaders and lighting |
| Collision and character movement via physics adapter | Loading meshes and textures onto GPU |
| Health, damage and death rules | First-person camera presentation and visual effects |
| Weapons, magazines, reload timing, projectiles and hitscan rules | Weapon models, muzzle flashes, bullet tracers |
| Enemy decision-making and combat behavior | Skeletal animation playback and blending |
| Survival waves, scores and difficulty rules | HUD, settings screens and menus |
| Story missions, triggers and objective progress | Dialog windows, subtitles and cutscene presentation |
| Inventory, ammunition and gameplay pickups | Touch, controller, keyboard and mouse collection |
| Simulation events (`ShotFired`, `EnemyKilled`, etc.) | Playing sounds, vibrations and particle effects |
| Save/load game data and version checks | File dialogs, platform storage permissions and cloud saves |
| Game-state snapshots for applications | Interpolating and displaying those snapshots |

**Important boundary:** The Rust core can expose a logical aim direction and character orientation. A renderer decides exactly how the camera moves, whether it shakes, and which animation looks best. The core must never read a browser DOM event or mobile touch event directly.

---

## 4. High-level architecture

```text
                Any game application / host
     (Browser, React Native, Unity, Godot, C++, raylib)
                           |
              Normalized game commands
                           |
                 Platform-specific wrapper
                  /                 \
              WASM API           Native C ABI
                  \                 /
                   Public Core API
                           |
                     Game Runtime
              (clock, commands, world, modes)
                  /        |        \
            Gameplay    Enemy AI    Missions
                  \        |        /
                     Physics API
                           |
                    Rapier Adapter
                           |
                      Rapier 3D
                           |
                  Snapshot + Events
                           |
                    Host / renderer
```

**Direction of dependencies:** The game depends on common types and the **physics interface**; the Rapier adapter implements that interface. The WASM and native wrappers depend on the game. The game never depends on wrappers.

---

## 5. Recommended standalone Git repository

Repository name may be `game-core` for now; rename it after the project has a proper name.

```text
game-core/
├── Cargo.toml                  # Rust workspace definition
├── Cargo.lock                  # Commit for reproducible applications/builds
├── rust-toolchain.toml         # Pinned Rust toolchain
├── README.md
├── LICENSE                     # Decide before publishing publicly
├── .gitignore
├── .github/
│   └── workflows/
│       └── ci.yml
├── crates/
│   ├── game-types/             # Shared, engine-neutral types and commands
│   │   └── src/lib.rs
│   ├── physics-api/            # Physics trait + shared physics data types
│   │   └── src/lib.rs
│   ├── physics-rapier/         # Rapier 3D implementation of physics-api
│   │   └── src/lib.rs
│   ├── game/                   # Entire playable single-player simulation
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── runtime.rs
│   │       ├── world.rs
│   │       ├── entities.rs
│   │       ├── player.rs
│   │       ├── movement.rs
│   │       ├── weapons.rs
│   │       ├── combat.rs
│   │       ├── enemies.rs
│   │       ├── ai.rs
│   │       ├── inventory.rs
│   │       ├── modes/
│   │       │   ├── mod.rs
│   │       │   ├── survival.rs
│   │       │   └── story.rs
│   │       ├── missions.rs
│   │       ├── events.rs
│   │       ├── snapshot.rs
│   │       └── save.rs
│   ├── bindings-wasm/          # Browser entry points using wasm-bindgen
│   │   └── src/lib.rs
│   └── bindings-native/        # Stable C-compatible native entry points
│       ├── src/lib.rs
│       └── include/game_core.h # Generated or checked C header
├── examples/
│   └── headless-demo/          # Small CLI for exercising the game
├── tests/                      # Cross-crate behavior/compatibility tests
├── benches/                    # Performance benchmarks when needed
├── docs/
│   ├── architecture.md
│   ├── coordinate-system.md
│   ├── public-api.md
│   ├── snapshot-format.md
│   ├── save-format.md
│   └── decisions/
└── tools/
    └── generate-header/       # FFI header generation tooling
```

### Why these crates are separate

- **`game-types`:** Common definitions (`Vec3`, entity IDs, inputs, configuration, events). Other crates can refer to these without needing Rapier.
- **`physics-api`:** A contract describing what the game needs from physics.
- **`physics-rapier`:** The only place where Rapier-specific structures should appear. Translate between our types and Rapier types here.
- **`game`:** Implements the actual gameplay. It is not specific to an engine or rendering technology.
- **`bindings-wasm`:** Converts browser-facing calls and buffers into calls to `game`.
- **`bindings-native`:** Exposes a simple native interface usable by C/C++, C#, JSI or engine extensions.

**Keep the initial setup manageable:** It is acceptable to start with `game-types`, `physics-api`, `physics-rapier`, `game`, and `examples/headless-demo`. Add WASM and native bindings **after** the headless simulation runs. Do not create dozens of empty modules just to match the eventual tree.

### Dependency rule

```text
game-types                 (no dependencies on our other crates)
physics-api  ------------> game-types
physics-rapier -----------> physics-api + game-types + rapier3d
game ---------------------> game-types + physics-api
bindings-wasm ------------> game (+ physics-rapier to construct the runtime)
bindings-native ----------> game (+ physics-rapier to construct the runtime)
headless-demo ------------> game (+ physics-rapier)
```

If factory creation becomes repetitive, introduce a small `game-defaults` crate later to assemble the game with Rapier. **Do not make the `game` crate import Rapier just for convenience.**

---

## 6. Shared conventions: coordinates, units and identity

Choose conventions before building movement or exporting levels:

- **3D coordinate system:** right-handed; **Y is up**.
- **Unit of distance:** meters.
- **Unit of time:** seconds.
- **Position/velocity:** 32-bit floats initially, with validation for NaN and infinity.
- **Rotation:** normalized quaternion for 3D orientation in snapshots; yaw/pitch may be used internally for player aiming.
- **Simulation rate:** start at **60 ticks per second** (`dt = 1.0 / 60.0`). Treat this as a configurable, benchmarkable choice rather than a performance guarantee.
- **Entities:** stable application-level `EntityId` values. Do not expose Rapier's internal collider/body handles to applications.
- **IDs:** do not reuse a deleted entity's ID in a way that could confuse clients; use generation counters or monotonic IDs.
- **Gravity:** configurable; an Earth-like default is approximately -9.81 m/s² on Y.
- **Input axes:** define a consistent convention (for example, local X = strafe and local Z = forward). Document whether forward means negative Z, and test it.

The renderer must translate these conventions if an engine uses different coordinates or units. **The core's conventions never change just because we add an engine.**

---

## 7. The public game API

Applications should interact with one conceptual object, `GameRuntime` (or `GameSession`). It should hide internal storage, physics handles and AI details.

### Main responsibilities

1. Create a game from configuration and a level description.
2. Accept normalized player commands.
3. Run exactly one fixed simulation step on request.
4. Provide read-only state snapshots.
5. Provide gameplay events produced during simulation.
6. Save or restore supported game data.
7. Reset or close the game cleanly.

### Suggested Rust-facing API (illustrative, not final compilable code)

```rust
pub struct GameRuntime<P: PhysicsWorld> {
    // world, physics, mode, clock and event queues
    physics: P,
}

impl<P: PhysicsWorld> GameRuntime<P> {
    pub fn new(config: GameConfig, level: LevelDefinition, physics: P)
        -> Result<Self, GameError>;

    pub fn submit(&mut self, command: GameCommand) -> Result<(), GameError>;
    pub fn tick(&mut self) -> Result<(), GameError>; // one fixed step
    pub fn snapshot(&self) -> GameSnapshot;
    pub fn drain_events(&mut self) -> Vec<GameEvent>;
}
```

An internal generic `GameRuntime<P>` is fine in Rust. The public FFI boundary should **hide the generic** behind a concrete constructed runtime and opaque handle. Another acceptable design is `Box<dyn PhysicsWorld>` where the performance and API trade-offs are measured.

### Commands sent **into** the core

```text
Move(forward, strafe)
Look(yaw_delta, pitch_delta)
Jump
StartFire / StopFire    (or timestamped FirePressed for semi-auto)
StartAim / StopAim
Reload
SwitchWeapon(slot)
Interact(target)
UseItem(slot)
Pause / Resume
StartMode(Survival | Story)
```

All values are **game actions**, not browser key names, touch coordinates or controller button codes. External adapters convert hardware input into the same commands.

### Data returned **from** the core

- **Snapshot:** current state required to draw the world: entity ID, transform, visual/animation state identifiers, health, weapon/ammo, player information, world time and mode state.
- **Events:** one-off occurrences: shot fired, reload started/completed, hit registered, entity killed, pickup collected, wave started, objective completed and game over.

Events and snapshots have different jobs. A snapshot tells the renderer **what is true now**; an event tells it **what just happened** so it can play a sound, show a notification or create a temporary effect.

### One authority rule

Only the core may decide that an enemy received damage, a bullet hit, a reload finished, or a mission objective completed. The renderer can show an optimistic visual effect, but must not permanently change game state on its own.

---

## 8. World and entity design

Do **not** build a full ECS framework on day one. Start with straightforward Rust structs and collections. Introduce an ECS only if the project grows enough to justify it.

### Suggested conceptual model

```text
GameRuntime
  GameWorld
    Player
    Enemies
    InteractiveObjects
    Pickups
    Projectiles (only when needed)
    LevelCollision
    MissionTriggers
  GameMode
  PhysicsWorld
  GameClock
  EventQueue
```

### Entity composition

Each logical entity has a stable ID and some subset of:

- Transform (position and orientation)
- Health and damage status
- Collider/physics relationship
- Movement parameters
- Weapon/inventory
- AI state
- Game tags (enemy, pickup, door, mission target)
- Render presentation key (for example, `enemy_soldier_a`, not a mesh file path)

A **presentation key** tells the host which model/animation to use. The host maps that key to its own 3D resources. The Rust core must not load `.blend`, `.glb`, textures or audio itself.

### Level data contract

The core needs logical level information: spawn positions, walls/floor collision, interactive items, enemy spawn points and trigger volumes. The host may import authored level data from Blender or other tools, but the core consumes a **validated, engine-neutral level description**.

Example logical values: `spawn_point`, `static_box_collider`, `enemy_spawn_region`, `interactable_door`, `mission_trigger`.

Do not assume the visible mesh is the collider. Rendering geometry and collision geometry are separate resources. Prefer simple colliders for performance and consistency.

---

## 9. Physics adapter: central requirement

### Why an adapter exists

We want to start with **Rapier 3D** but preserve the option to experiment with Bullet or another physics system. All physics-related behavior must go through the physics API. Game code must not call Rapier directly.

### What the physics interface should support initially

- Create and remove static world colliders.
- Create and remove player/enemy collision bodies.
- Move a character while respecting obstacles and ground slopes.
- Query whether a character is grounded.
- Perform raycasts for shooting and line-of-sight checks.
- Return collision hits using **our** `EntityId` and common vector types.
- Advance the physics world by one fixed timestep.
- Read relevant body transforms.

Add rigid-body impulses, joints, vehicles, ragdolls and complex triggers **only when gameplay needs them**. The interface should describe game needs, not expose every Rapier function.

### Example interface shape (conceptual)

```rust
pub trait PhysicsWorld {
    fn create_static_collider(&mut self, collider: ColliderDesc)
        -> Result<PhysicsHandle, PhysicsError>;

    fn create_character(&mut self, desc: CharacterDesc)
        -> Result<PhysicsHandle, PhysicsError>;

    fn move_character(&mut self, handle: PhysicsHandle, desired: Vec3)
        -> Result<CharacterMoveResult, PhysicsError>;

    fn raycast(&self, query: RayQuery) -> Result<Option<RayHit>, PhysicsError>;

    fn step(&mut self, fixed_dt: f32) -> Result<(), PhysicsError>;

    fn body_transform(&self, handle: PhysicsHandle)
        -> Result<Transform, PhysicsError>;
}
```

The exact signatures may change when implemented. In particular, character movement should define clearly **when** it occurs relative to physics stepping, what collision filters apply, and whether the character controller consumes velocity or desired displacement.

### Rapier implementation

`physics-rapier` owns Rapier's world, collision data, body/collider handles and query pipeline. It converts between `game-types` and Rapier structures. It stores an internal mapping from our entity IDs/handles to Rapier handles.

For FPS player movement, start by evaluating Rapier's **kinematic character controller** (capsule collider; walk, slide, stairs and slopes). Note that its movement calculations do not automatically implement all game behavior: our game code still owns speed, jumping, gravity integration, sprinting, crouching and aim direction. The character controller handles collision-aware movement, not gameplay decisions.

### Rules for physics swapping

- No `rapier3d::*` types in `game` public signatures.
- No Rapier handle serialized in save games or snapshots.
- Separate unit tests for physics behavior (wall blocking, jumping, grounding, ray hits).
- Expect physics tuning differences after swapping engines. The adapter makes a swap manageable, **not free or perfectly identical**.
- Record the versions and configuration used for reproducible behavior.

---

## 10. Fixed-step game loop

The host/application can run at different frame rates. The Rust simulation advances in **fixed ticks**.

```text
Host gathers input
    ↓
Host translates to GameCommands
    ↓
Host or its worker accumulates elapsed time
    ↓
For each pending fixed tick:
    accept commands for the tick
    update player intentions
    update AI intentions
    apply character movement / physics operations
    advance physics simulation
    resolve combat, contacts and interactions
    update inventory, modes and missions
    record events and new state
    ↓
Return latest snapshots/events
    ↓
Host renders smoothly between states
```

**The exact ordering becomes a tested part of the game rules.** For example, if a shot occurs in a tick, specify whether the hit query sees movement before or after that tick's physics update.

### Fixed-step policy

- Begin at **60 Hz**.
- Do not use real-world frame delta directly in every physics update.
- The host controls wall-clock pacing; the core's `tick()` always advances one configured fixed interval.
- Limit catch-up ticks to prevent a slow device from spiraling into more and more work; define how to handle overload.
- The host may interpolate between previous/current snapshots for smooth rendering.
- Use a seedable random-number generator for enemy spawns, spread and procedural decisions, where appropriate.
- Do not promise perfect cross-platform determinism until it is explicitly tested. Physics build options, floating-point behavior and ordering can affect repeatability.

### Input policy

- Normalize input outside the core.
- Define whether commands are held states (movement axes, aiming, firing) or one-time actions (jump, reload).
- Attach tick IDs or sequence numbers internally where needed so an action is not accidentally applied twice.
- Test input ordering and repeated commands.

---

## 11. Player controller

The first playable milestone is **one controllable player inside a simple test level**.

### Initial behaviors

- Walk forward/backward and strafe.
- Rotate horizontally (yaw) and aim vertically (pitch).
- Gravity, grounding, jumping, collision and wall sliding.
- Basic sprinting after normal movement feels correct.
- Stop and restart without corrupting state.

### Later behaviors (do not implement immediately)

- Crouching, sliding and climbing.
- Fall damage and stamina.
- Different movement states from equipment or injury.
- Specialized camera and weapon sway effects (presentation-only aspects remain outside Rust).

The player is represented physically using a simple capsule or similarly appropriate collider, not the detailed 3D character mesh. The Rust game controls valid movement, while the renderer follows the returned player transform.

---

## 12. Weapons and combat

Start with **one hitscan weapon**: a shot instantly casts a ray from an agreed gameplay aim origin to a maximum range. Later add projectile weapons if needed.

### Suggested weapon data

```text
WeaponDefinition
  weapon_id
  fire_mode             (single, automatic, burst later)
  damage
  maximum_range
  rounds_per_magazine
  fire_interval_ticks
  reload_duration_ticks
  spread_parameters
  recoil_gameplay_parameters
```

### Required weapon rules

1. Check the current equipped weapon and ammunition.
2. Check whether firing is allowed (reload state, cooldown, alive player).
3. Determine a shot direction from player aim and any gameplay spread.
4. Raycast via `PhysicsWorld`.
5. Resolve the hit target and damage in Rust.
6. Update ammunition, cooldown and target health.
7. Emit events (`ShotFired`, `Hit`, `Killed`, `ReloadStarted`, etc.).

No damage should be determined by an application-specific raycast in Three.js, Unity or Godot. Their raycasts may be used for **visual-only interactions**, but the authoritative gameplay hit comes from the core.

### Combat validation tests

- Cannot fire an empty weapon.
- Cannot bypass the firing cooldown.
- Reload consumes the correct time and reserve ammo.
- Walls block hits when expected.
- Health never becomes invalid; death is emitted once.
- Repeating an action does not duplicate its effect.

---

## 13. Enemy AI

For Stage 1, enemy behavior should be simple but convincing. Use a **finite-state machine**, not an LLM or a complicated AI framework.

```text
Idle / Patrol
     ↓ (player detected)
Investigate / Chase
     ↓ (close enough + line of sight)
Attack
     ↓ (lose sight)
Search
     ↓ (give up)
Patrol

Any active state → Dead (when health reaches zero)
```

### AI responsibilities

- Select a behavior based on distance, vision and game events.
- Request movement through the same movement/physics rules as other characters.
- Select targets and attack using the same combat system as the player.
- Use a basic navigation strategy initially; introduce pathfinding/navmesh support when level complexity requires it.
- Handle dying, despawning and loot drops.

**Important:** Collision handling is not pathfinding. Rapier can keep enemies from walking through a wall, but it does not, by itself, plan a route around a building. Navigation should eventually have its own interface or service, separate from physics.

AI decisions may update less frequently than physics ticks if that improves performance; record and test the behavior.

---

## 14. Game modes

Survival and story modes share the same player, weapons, physics, AI, health and inventory systems. A game mode changes the rules for **objectives, enemies and progression**, not the underlying simulation.

### Survival mode (build first)

- Spawn enemies in waves.
- Increase difficulty in a controlled way.
- Track score, wave number and elapsed survival time.
- Support health/ammo pickups.
- End when the player dies, then permit restart.

### Story mode (build after survival is playable)

- Define missions, objectives, checkpoints and triggers as **data**, not hard-coded branches inside the main loop.
- Examples: reach an area, eliminate targets, collect an item, activate a switch, protect a location.
- Track which objectives are locked, active or complete.
- Emit events that the host can use to show dialogue, UI or cutscenes.
- Support saving mission progress.

### Suggested interface (conceptual)

```rust
pub trait GameMode {
    fn on_start(&mut self, world: &mut GameWorld);
    fn on_event(&mut self, event: &GameEvent, world: &mut GameWorld);
    fn on_tick(&mut self, world: &mut GameWorld);
    fn status(&self) -> ModeStatus;
}
```

This is a design sketch. If Rust borrowing becomes awkward, use a mode input/context object rather than fighting the borrow checker. Avoid creating an enormous game-mode abstraction before two real modes exist.

---

## 15. Inventory, pickups and interaction

Start small:

- Inventory contains held weapon slots, ammunition reserves and basic consumables.
- Pickups have a logical identity, type, position and interaction range.
- The player sends an `Interact` command; Rust checks distance/line-of-sight/requirements and applies the effect.
- Gameplay values are stored in common, engine-neutral structures.
- UI labels and item icons are looked up by the host using item IDs.

Future extensions might include armor, crafting, equipment or loot tables. None is required for the first playable milestone.

---

## 16. Snapshots, events and memory layout

The runtime should use **normal, readable Rust structures internally**. The browser/native wrappers are responsible for presenting state in an efficient external format.

### Start with two formats

1. **Structured Rust snapshot:** easy to inspect and unit test.
2. **Versioned binary snapshot:** compact format generated by the platform wrapper for high-frequency renderer updates.

Do not force all internal game logic to operate directly on raw byte arrays. That would make development unnecessarily difficult.

### Suggested snapshot categories

- Header: format version, simulation tick, number of entities.
- Entity transforms: stable ID, position, rotation.
- Visual state: model/presentation ID, animation state, flags.
- Player HUD data: health, armor, ammo, weapon slot, score.
- Mode state: wave, objective IDs, timers.

### TypedArray policy for the browser

- `bindings-wasm` exposes a compact buffer or writes into caller-managed WASM memory.
- The browser converts the data to suitable TypedArray views and sends snapshots from its worker to the renderer using transferable `ArrayBuffer`s where practical.
- Define endianness, field widths, alignment and offsets in `docs/snapshot-format.md`.
- Avoid silently assuming every value can be packed as `f32`: large entity IDs and tick counters need appropriately defined integer fields.
- Do not keep a JavaScript view across WASM memory growth without refreshing it.
- Do not implement `SharedArrayBuffer`/Atomics initially; that is a later optimization only if measurements justify it.

### Native policy

- `bindings-native` exposes a safe copy-into-caller-buffer operation at first.
- Later a JSI/other host adapter may offer lower-copy access after profiling.
- **Zero-copy is not guaranteed** by calling it a TypedArray or a JSI bridge. Memory ownership and lifetime must be explicit.

### Event delivery

Events should have a clear lifetime (e.g., consumed/drained after a tick). Include IDs or tick references where useful to avoid replaying one-time effects. Keep a separate reliable, readable event representation from transform snapshots where appropriate.

---

## 17. WASM and native interfaces

Both wrappers must expose the **same game behaviors** but do **not** need identical low-level signatures.

### Browser WASM (`bindings-wasm`)

Use `wasm-bindgen` to expose entry points that create/destroy a game, submit commands, advance fixed ticks, and retrieve snapshots/events. The web app may place WASM inside a worker to keep simulation off the render thread.

A separate browser client is responsible for React, R3F, user input and animation. **None of those dependencies belong in this repository.**

### Native C-compatible interface (`bindings-native`)

Use a C-compatible ABI to make the library accessible to C/C++, Unity/C#, Godot and mobile native wrappers. A small C API should expose opaque game handles, not Rust structs such as `Vec`, `String` or trait objects.

Illustrative operations:

```c
GameHandle* game_create(const GameConfigC* config);
GameStatus game_submit_input(GameHandle* game, const GameInputC* input);
GameStatus game_tick(GameHandle* game);
size_t game_snapshot_size(const GameHandle* game);
GameStatus game_write_snapshot(const GameHandle* game,
                               uint8_t* out_buffer,
                               size_t capacity,
                               size_t* bytes_written);
void game_destroy(GameHandle* game);
```

This is an **interface sketch** and should be refined before implementation.

### Native safety requirements

- Opaque handles; document who owns them and when they become invalid.
- Validate pointers, lengths and null arguments at every public entry point.
- Mark ABI data structures `#[repr(C)]` and use fixed-width integer fields.
- Catch or prevent unwinding across the C boundary; return an error status instead.
- Never let the caller free Rust-owned memory using a foreign allocator.
- Prefer caller-owned output buffers to simplify memory lifetime.
- Define thread ownership; do not assume the same game handle is safe to mutate from multiple threads.
- Version the public native API before distributing builds.
- Generate the C header with a reproducible build step (e.g., `cbindgen`) and test it against a small C/C++ consumer.
- For Unity or Godot, write a small engine-side wrapper around this API. These integrations are **future adapters**, not part of Stage 1.

---

## 18. Persistence and configuration

The Rust library should understand **what** must be saved; the host decides **where** bytes are stored.

### Save data should include

- Save schema version.
- Current level and game mode.
- Player stats, equipment, inventory and position.
- Enemies and interactive world changes that must persist.
- Current survival or story progress.
- Random seed/state if required for restoring gameplay.

### Save rules

- Provide `export_save()` and `import_save(bytes)`-style APIs.
- Never serialize raw pointers, Rapier handles, native handles or engine-specific objects.
- Validate input and provide actionable errors for incompatible versions.
- Begin with a readable format for debugging if convenient; optimize later.
- A save format is **not** automatically a complete deterministic physics snapshot. Decide separately how precise restoration must be.

### Configuration

Use explicit, validated data for movement speeds, jump strength, weapons, enemy stats, spawn waves and missions. Do not hard-code balance values throughout unrelated files.

The host may read JSON/RON/TOML or other authored data and pass it to Rust. The internal game should operate on validated typed structures.

---

## 19. Performance principles

Our first target is correctness and a playable feel. Only optimize based on measurements.

- Start with a 60 Hz simulation target; measure the time each tick actually takes on the target device.
- Keep physics collision shapes simple even when visual assets are moderately detailed.
- Do not allocate a new large object graph every tick without a reason.
- Avoid sending every internal field to the renderer.
- Reuse buffers where practical.
- Use stable IDs for mapping game entities to renderer objects.
- Profile physics, AI, snapshot generation and host integration separately.
- For mobile, test on **real Android/iOS hardware** rather than relying only on desktop benchmarks.
- Improve AI update frequency, spatial queries and data layout only after profiling.
- Be cautious with threading and cross-platform determinism; make it an explicit, tested optimization.

The Rust core is **not** responsible for GPU draw calls. Optimizing 3D models, textures, shaders and animation remains the responsibility of the host/game project.

---

## 20. Testing strategy

### Unit tests (inside individual crates)

- Vector and coordinate conversions.
- Player movement constraints and state transitions.
- Weapon cooldown, ammo, reload and damage rules.
- AI state transitions.
- Survival wave progression.
- Mission completion conditions.
- Save-format validation.

### Physics adapter tests

- Falling character lands on ground.
- Player cannot pass through a solid wall.
- Player slides along a wall where expected.
- Steep slope and step behavior is consistent with design.
- A raycast hits the nearest valid collider.
- Collision filters correctly ignore the shooter where required.

### Headless integration tests

Run whole scenarios without any graphics:

1. Create a level, player and enemy.
2. Move the player for a known number of ticks.
3. Aim and shoot the enemy.
4. Check ammo, hit events, health and enemy death.
5. Start survival mode, clear a wave and verify progression.
6. Save and restore supported state.

### Cross-platform/API tests

- WASM creates a game and produces readable snapshot bytes.
- Native test application creates, updates and destroys a game.
- Snapshot format round-trips correctly.
- Invalid commands, malformed save bytes and invalid handles produce errors rather than undefined behavior.

### CI minimum

Run formatting checks, linting, Rust tests and build checks for supported targets. Pin tool versions and document installation. Add WASM/native target jobs when those wrappers are created.

Suggested commands:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo build --workspace
```

These are intended quality gates; adjust feature/target combinations if WASM-only APIs require target-specific jobs.

---

## 21. Development roadmap: small, verifiable milestones

### Milestone 0 — Repository foundation

- Create workspace and initial crates.
- Pin Rust toolchain and dependencies; set up formatting, lints and CI.
- Define common coordinate conventions, error types and `EntityId`.
- Write README explaining what the core does and does not do.

**Done when:** All crates compile, tests run, CI passes, and the dependency direction is clear.

### Milestone 1 — Physics and a moving player

- Build the minimal physics interface.
- Implement it with Rapier 3D.
- Create a level with floor and wall colliders.
- Add player position, capsule collision, grounding, gravity and movement.
- Run it in a headless command-line program.

**Done when:** The player can walk, jump, land and collide with walls, verified by automated tests.

### Milestone 2 — Game runtime and snapshots

- Add commands, fixed ticks, stable entity IDs and event queue.
- Produce structured game snapshots.
- Add a test that runs thousands of simulation ticks without a renderer.

**Done when:** External code can drive the game entirely through the public Rust API.

### Milestone 3 — First weapon and enemy

- Add player health, one hitscan weapon and reload rules.
- Add a stationary enemy target with health and collider.
- Implement shooting, hit detection, damage and death events.

**Done when:** A headless test moves a player, shoots a target and verifies the result.

### Milestone 4 — Simple active enemies and survival

- Implement enemy patrol/chase/attack states.
- Add enemy spawns, score, death and wave progression.
- Keep level/navigation simple at first.

**Done when:** An entire basic survival round can be simulated without graphics.

### Milestone 5 — WASM adapter (first real rendering integration)

- Add browser-facing wrapper and binary snapshot format.
- Make a tiny JavaScript/WASM consumer example or contract test.
- Document TypedArray layout and lifetime rules.

**Done when:** A browser client can control the simulation and display accurate transforms. The complete browser application lives in a separate repository.

### Milestone 6 — Native ABI adapter

- Add C-compatible entry points and a generated header.
- Build a tiny C/C++ test consumer.
- Document memory ownership, errors and versioning.

**Done when:** A C/C++ caller can start, tick and inspect the same simulation.

### Milestone 7 — Story support and saves

- Add mission definitions, triggers, objective state and checkpoints.
- Support save/load of documented persistent state.

**Done when:** A headless test can complete a small mission, save, reload and continue.

### Milestone 8 — Mobile and other engine adapters (external repositories)

- Validate a JSI-backed native wrapper with React Native, R3F Native and real device performance.
- Additional Unity/Godot/raylib adapters may be created later.

**Done when:** One external host can use the core without changing `game` or `physics-api` source.

### Future Stage 2 — Multiplayer

Only after single-player is solid: investigate authoritative server reuse, tick/command sequencing, networking, prediction, reconciliation and transport choices (WebSocket, WebRTC DataChannels or other appropriate options). Do not assume a local single-player simulation automatically becomes a secure network server.

---

## 22. Decisions to defer deliberately

These are not settled, and agents must **not invent a permanent answer without evidence**:

| Decision | Default for now | When to revisit |
|---|---|---|
| Entity framework | Plain Rust structs/collections | When scale/complexity warrants ECS |
| Physics engine | Rapier 3D behind an interface | After tests show an actual limitation |
| Physics precision | `f32` | If large worlds or accuracy require more |
| Runtime timestep | 60 Hz | After device profiling and gameplay testing |
| WASM exchange | TypedArray-compatible binary buffers | If snapshot transfer becomes a bottleneck |
| Mobile transport | C ABI with JSI-capable wrapper later | After native benchmark/prototype |
| Multiplayer transport | Not implemented in Stage 1 | Stage 2 network prototypes |
| Pathfinding | Simple test-level navigation | When maps become more complex |
| Game data authoring | Small validated data files | As level/story authoring grows |
| Animation | Output animation state IDs only | When a renderer has a concrete animation pipeline |

---

## 23. Instructions for GitHub Copilot / coding agents

Use this section when generating repository instructions, constitutions, steering rules or task prompts.

### Core engineering rules

1. **Read this architecture before changing project structure.** Treat its scope and dependency boundaries as the baseline; propose and document deviations rather than silently changing them.
2. **Implement only the Rust core in this repository.** Do not create a Next.js application, React Native app, Unity project or renderer without an explicit request.
3. **Never introduce renderer-specific dependencies into `game`, `game-types` or `physics-api`.**
4. **Never introduce Rapier types outside `physics-rapier` or its internal integration/wiring.** The common API uses our own types.
5. **Keep gameplay authoritative in Rust.** Frontends send commands and consume state; they do not determine hits, damage or mission completion.
6. **Prefer a small working feature over speculative infrastructure.** Do not build multiplayer, networking, distributed services or abstract plugin systems during Stage 1.
7. **Use stable entity IDs and fixed simulation ticks.** Never assume render frame rate equals simulation tick rate.
8. **Every gameplay behavior change requires tests.** For physics changes, add both direct adapter tests and a small game-level scenario.
9. **No `.unwrap()` / `.expect()` on untrusted external data or public FFI paths.** Return explicit errors. Internal test helpers may use them appropriately.
10. **Keep unsafe Rust isolated and documented.** The goal is safe internal Rust; unsafe operations should primarily occur at carefully audited native boundaries.
11. **Maintain clean ownership of resources.** All native handles, output buffers and events need clear lifetimes.
12. **Avoid premature optimization.** Measure and report performance before changing data layout, threading or memory-sharing strategy.
13. **Keep documentation synchronized.** Changing coordinate conventions, public API, binary format or saves requires updating relevant docs and examples.
14. **Make changes incrementally.** Compile and test after each milestone. Do not generate huge untested codebases in one pass.
15. **Avoid unexplained magic numbers.** Gameplay tuning belongs in explicit configuration/data definitions.
16. **Do not assert cross-platform determinism, zero-copy or engine compatibility without tests.** Describe these as goals until demonstrated.
17. **Keep versions explicit.** Verify current library APIs before writing implementation code; do not guess Rapier function signatures from older examples.
18. **When requirements are incomplete, choose a minimal reversible implementation**, record the choice and identify what remains undecided.

### Definition of done for each feature

A feature is done only when:

- The relevant Rust crates compile.
- Behavior is covered by appropriate unit/integration tests.
- Public APIs do not leak platform or Rapier internals.
- Documentation/examples reflect any changed usage.
- `cargo fmt`, `cargo clippy`, `cargo test` and build checks pass for applicable targets.
- It can run headlessly; no graphical client is required to prove basic correctness.

### First implementation prompt for Copilot

> Use `docs/architecture.md` as the architecture source of truth. Implement **Milestone 0 only** for this standalone Rust game core. Create a minimal Cargo workspace with `game-types`, `physics-api`, `physics-rapier`, `game`, and a headless example. Define the shared coordinate conventions, minimal types, errors and a small physics interface, but do not implement gameplay or platform bindings yet. Add a README, basic tests, formatting/lint setup and a short explanation of the crate dependency direction. Verify with Cargo commands and report exactly what works and what remains to be built. Do not introduce any rendering, React, Unity, Godot or multiplayer code.

---

## 24. Compatibility promise and limitations

This architecture is designed to make the **game rules and simulation portable**, but it is not a promise that every engine behaves identically without extra work.

- Unity, Godot, React Native and raylib each need a host-specific adapter.
- Some engines may prefer to run their own physics; to keep consistent gameplay we should normally let **Rust/Rapier remain the authoritative physics simulation** and treat engine physics as visual-only unless there is a deliberate redesign.
- Animation, audio, menu UI, camera effects, visual models and graphical performance are not portable simply because gameplay logic is.
- A stable public ABI and a stable serialized snapshot format are separate concerns. Version both explicitly once consumers depend on them.
- Game AI behavior, physics order and input commands should remain testable even if the rendering engine changes.

**Ultimate success test:** We should be able to build a second small application using the same Rust core (for example a raylib/C++ viewer), send it the same commands, and receive logically equivalent game-state results **without rewriting the weapons, movement, combat, AI or mode rules**.

---

## 25. Technical references

Use official/current documentation during implementation; verify APIs against the actual pinned dependency versions.

- Rapier documentation: https://rapier.rs/docs/
- Rapier Rust character controller: https://rapier.rs/docs/user_guides/rust/character_controller/
- Rapier Rust crate: https://docs.rs/rapier3d/latest/rapier3d/
- Rust FFI safety and layout: https://doc.rust-lang.org/stable/nomicon/ffi.html
- Rust WebAssembly guide: https://rustwasm.github.io/docs/wasm-bindgen/
- Rust Cargo workspaces: https://doc.rust-lang.org/book/ch14-03-cargo-workspaces.html
- cbindgen: https://github.com/mozilla/cbindgen

---

## 26. Summary

**Build the game logic once in Rust. Keep Rapier behind a replaceable physics interface. Feed the simulation normalized commands. Return clear snapshots and events. Keep rendering and device input outside the core. Deliver a small, tested, headless FPS simulation before creating platform wrappers or multiplayer infrastructure.**

The first real milestone is **not** a polished game. It is a Rust application that creates a world, moves a player, handles collisions and can prove its behavior with tests. Everything else grows from that working foundation.
