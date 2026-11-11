# Floride Game Engine

A modular, headless, engine-independent game simulation framework in Rust.

Floride separates authoritative game simulation from rendering, UI, audio, and platform hardware. Build your game's rules once in Rust using [`GameAdapter`](crates/game-api/src/adapter.rs) and deploy the exact same simulation across:
- **React / Web browsers** via WebAssembly (`floride-host-wasm` + `wasm-pack`)
- **React Native (iOS / Android)** via panic-safe C-ABI + C++ JSI (`floride-host-runtime`)
- **Native C/C++ applications & game engines** (Godot, Unity, raylib) via `.so` / `.a` / `.dylib` + `cbindgen` headers
- **Headless Rust CLIs & automated test suites** with zero graphics dependencies

---

## Core Philosophy

- **The host / renderer owns:** 3D meshes, shaders, cameras, UI/menus, hardware input polling, and audio playback.
- **The simulation core owns:** World state, collision resolution, character kinematics, game rules, fixed-rate stepping, transient events, and binary snapshot serialization.
- **Conventions:** Right-handed 3D coordinates (`+Y` up), meters for distance, seconds for time, `f32` positions/velocities, normalized quaternions (`[x, y, z, w]`), and a default 60 Hz fixed simulation rate.

---

## Workspace Architecture

```text
                   External Hosts & Renderers
       ┌──────────────────────┬───────────────────────┐
       ▼                      ▼                       ▼
  React / Web           React Native / JSI      C/C++ / Godot / Unity
(wasm-bindgen)         (C++ JSI HostObject)       (Static / Shared Lib)
       │                      │                       │
       ▼                      └───────────┬───────────┘
 crates/floride-host-wasm                 │
  (FlorideWasmSession)                    ▼
       └────────────────────► crates/host-runtime
                         (HostSession, TLV, C-ABI)
                                          │
                                          ▼
                             crates/game-api (GameAdapter)
                                          │
                           Your Game Crate (e.g. FPS / RPG)
                       (Owns actions, rules, snapshots, events)
                           ┌──────────────┴──────────────┐
                           ▼                             ▼
                   crates/game-logic             crates/physics-api
               (Movement, level, vitals)       (PhysicsWorld trait)
                                                         │
                                                         ▼
                                               crates/physics-rapier
                                                (Rapier 3D backend)
```

### Workspace Crates

| Crate | Responsibility | Key Dependencies |
| :--- | :--- | :--- |
| **[`game-types`](crates/game-types)** | Engine-neutral primitives (`Vec3`, `EntityId`). | None |
| **[`physics-api`](crates/physics-api)** | Replaceable 3D physics trait (`PhysicsWorld`, colliders, phantoms, character controller, raycasting, `PrimitiveShape`, `PhysicsTransform`). | `game-types`, `serde` |
| **[`physics-rapier`](crates/physics-rapier)** | Concrete Rapier 3D implementation (`RapierWorld`) hidden behind `physics-api`. | `game-types`, `physics-api`, `rapier3d` |
| **[`game-api`](crates/game-api)** | Host-facing lifecycle trait (`GameAdapter`) with associated types so every game keeps its own inputs and snapshots. | `physics-api` |
| **[`game-logic`](crates/game-logic)** | Reusable gameplay building blocks: character kinematics (`step_character_movement`), `LevelWorldSpec` + `spawn_level_world` + heightfields, `HealthState`, and spatial helpers (`has_line_of_sight`, `apply_radial_impulse`, `steer_toward`). | `game-types`, `physics-api`, `serde` |
| **[`floride-host-runtime`](crates/host-runtime)** | Generic host bridge: `ActionPacket` / `ActionDecoder`, TLV snapshot protocol (`TlvWriter` / `TlvReader`), `HostSession`, and panic-safe `extern "C"` FFI + C++ JSI template. | `game-api`, `game-types` |
| **[`floride-host-wasm`](crates/floride-host-wasm)** | Generic `wasm-bindgen` wrapper (`FlorideWasmSession`) exposing `HostSession` to browser JS/TS and Web Workers. | `floride-host-runtime`, `game-api`, `wasm-bindgen` |
| **[`floride-visual-demo`](examples/visual-demo)** | Development-only 3D visual showcase window (excluded from WASM/mobile builds). | `game-types`, `physics-api`, `physics-rapier`, `macroquad` |

---

## Building a Game in Rust

### 1. Add Engine Dependencies

In your game crate's `Cargo.toml` (for example, `my-game/Cargo.toml`):

```toml
[package]
name = "my-game"
version = "0.1.0"
edition = "2024"

# Emit rlib (Rust tests/apps), cdylib (.wasm / .so / .dylib), and staticlib (.a for iOS / C++)
[lib]
crate-type = ["rlib", "cdylib", "staticlib"]

[dependencies]
game-types = { git = "https://github.com/olaoluwanhs/floride-game-engine.git", package = "game-types" }
game-api = { git = "https://github.com/olaoluwanhs/floride-game-engine.git", package = "game-api" }
game-logic = { git = "https://github.com/olaoluwanhs/floride-game-engine.git", package = "game-logic" }
physics-api = { git = "https://github.com/olaoluwanhs/floride-game-engine.git", package = "physics-api" }
physics-rapier = { git = "https://github.com/olaoluwanhs/floride-game-engine.git", package = "physics-rapier" }
floride-host-runtime = { git = "https://github.com/olaoluwanhs/floride-game-engine.git", package = "floride-host-runtime" }

# Only compiled when targeting WebAssembly
[target.'cfg(target_arch = "wasm32")'.dependencies]
floride-host-wasm = { git = "https://github.com/olaoluwanhs/floride-game-engine.git", package = "floride-host-wasm" }
wasm-bindgen = "0.2"
```

### 2. Define Your Game Types & Implement `GameAdapter`

Each game owns its configuration, inputs, snapshots, and events—`game-types` and `game-logic` never force a fixed command enum on your game.

```rust
use game_api::GameAdapter;
use game_logic::{MovementConfig, MovementIntent, MovementState, step_character_movement};
use game_types::{EntityId, Vec3};
use physics_api::{CharacterDesc, PhysicsHandle, PhysicsWorld, PrimitiveShape};

pub struct MyGameConfig {
    pub tick_rate_hz: u32,
}

pub enum MyGameInput {
    Move { direction: Vec3, sprint: bool },
    Jump,
}

pub struct MyGameSnapshot {
    pub player_position: Vec3,
    pub score: u32,
}

pub enum MyGameEvent {
    PlayerJumped,
}

pub struct MyGame<P: PhysicsWorld> {
    physics: P,
    player_handle: PhysicsHandle,
    movement_config: MovementConfig,
    movement_state: MovementState,
    pending_intent: MovementIntent,
    fixed_dt: f32,
    score: u32,
    events: Vec<MyGameEvent>,
}

impl<P: PhysicsWorld> GameAdapter for MyGame<P> {
    type Physics = P;
    type Config = MyGameConfig;
    type Input = MyGameInput;
    type Snapshot = MyGameSnapshot;
    type Event = MyGameEvent;
    type Error = String;

    fn create(config: Self::Config, mut physics: Self::Physics) -> Result<Self, Self::Error> {
        let player_handle = physics
            .create_character(&CharacterDesc {
                entity_id: EntityId::new(1),
                position: Vec3::new(0.0, 1.0, 0.0),
                shape: PrimitiveShape::Capsule {
                    half_height: 0.5,
                    radius: 0.4,
                },
                max_slope_angle_radians: 0.785,
                step_height: 0.3,
            })
            .map_err(|e| format!("Failed to create player: {e}"))?;

        let tick_rate = config.tick_rate_hz.max(1) as f32;
        Ok(Self {
            physics,
            player_handle,
            movement_config: MovementConfig::default(),
            movement_state: MovementState::default(),
            pending_intent: MovementIntent::default(),
            fixed_dt: 1.0 / tick_rate,
            score: 0,
            events: Vec::new(),
        })
    }

    fn submit_input(&mut self, input: Self::Input) -> Result<(), Self::Error> {
        match input {
            MyGameInput::Move { direction, sprint } => {
                self.pending_intent.desired_direction = direction;
                self.pending_intent.sprint = sprint;
            }
            MyGameInput::Jump => {
                self.pending_intent.jump = true;
            }
        }
        Ok(())
    }

    fn tick(&mut self) -> Result<(), Self::Error> {
        step_character_movement(
            &mut self.physics,
            self.player_handle,
            &self.movement_config,
            &mut self.movement_state,
            &self.pending_intent,
            self.fixed_dt,
        )
        .map_err(|e| format!("Movement error: {e}"))?;

        self.physics
            .step(self.fixed_dt)
            .map_err(|e| format!("Physics step error: {e}"))?;

        if self.pending_intent.jump && self.movement_state.is_grounded {
            self.events.push(MyGameEvent::PlayerJumped);
        }
        self.pending_intent.jump = false;
        Ok(())
    }

    fn snapshot(&self) -> Self::Snapshot {
        let transform = self
            .physics
            .body_transform(self.player_handle)
            .unwrap_or_default();

        MyGameSnapshot {
            player_position: transform.position,
            score: self.score,
        }
    }

    fn drain_events(&mut self) -> Vec<Self::Event> {
        std::mem::take(&mut self.events)
    }
}
```

### 3. Reuse Shared World & Level Helpers (`game-logic`)

For levels, terrain, health, and AI utilities, `game-logic` also exports:
- **World loading:** `LevelWorldSpec`, `LevelStaticObject`, `LevelDynamicObject`, `LevelTriggerVolume`, `LevelMarker`, `spawn_level_world`, and procedural/heightmap terrain (`generate_heightfield_floor`, `sample_heightfield_y`).
- **Combat & AI:** `HealthState`, `DamageOutcome`, `apply_radial_impulse`, `linear_falloff`, `has_line_of_sight`, and `steer_toward`.

---

## Deploying Games Across Environments

To run your game outside Rust (browser, mobile, C/C++, Godot, Unity), implement two small traits from `floride-host-runtime`:
1. **`ActionDecoder<YourInput>`** — decodes binary `(action_id: u16, payload: &[u8])` packets into your game's `Input` enum (validating byte lengths and finite floats).
2. **`SnapshotEncoder<YourSnapshot, YourEvent>`** — writes your game's state and transient events into a `TlvWriter` stream using standard chunk IDs (`CHUNK_TRANSFORMS`, `CHUNK_COMBAT`, `CHUNK_WEAPONS`, `CHUNK_EVENTS`, `CHUNK_WORLD_TRANSFORMS`, `CHUNK_ENTITY_METADATA`, `CHUNK_ENEMIES`, `CHUNK_PROJECTILES`, `CHUNK_EXPLOSIVES`, `CHUNK_BLEMISHES`, `CHUNK_MODE`, `CHUNK_PICKUPS`).

Wrap them together in `HostSession::new(game, decoder, encoder)`.

> **Important Rust linking rule:** `floride-host-runtime` is game-agnostic and does not depend on your game crate. To export a single `.wasm`, `.so`, or `.a` binary containing your game, export your concrete constructors (and forward or re-export the session calls) directly from your game crate.

---

### Environment 1: Pure Rust / Headless CLI & Tests

Drive `GameAdapter` directly with `RapierWorld::new()`:

```rust
use game_api::GameAdapter;
use physics_rapier::RapierWorld;

fn run_headless() -> Result<(), String> {
    let mut game = MyGame::create(MyGameConfig { tick_rate_hz: 60 }, RapierWorld::new())?;
    game.submit_input(MyGameInput::Jump)?;
    game.tick()?;
    let _snapshot = game.snapshot();
    let _events = game.drain_events();
    Ok(())
}
```

---

### Environment 2: Web / React (WebAssembly via `floride-host-wasm`)

In your game crate (gated behind `#[cfg(target_arch = "wasm32")]`), wrap your `HostSession` in `FlorideWasmSession` and expose a `#[wasm_bindgen]` constructor:

```rust
use floride_host_runtime::HostSession;
use floride_host_wasm::FlorideWasmSession;
use physics_rapier::RapierWorld;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub struct MyWasmGame(FlorideWasmSession);

#[wasm_bindgen]
impl MyWasmGame {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Result<MyWasmGame, JsValue> {
        let game = MyGame::create(MyGameConfig { tick_rate_hz: 60 }, RapierWorld::new())
            .map_err(|e| JsValue::from_str(&e))?;
        let session = HostSession::new(game, MyActionDecoder, MySnapshotEncoder);
        Ok(Self(FlorideWasmSession::from_host_session(session)))
    }

    #[wasm_bindgen(js_name = dispatchAction)]
    pub fn dispatch_action(&mut self, action_id: u16, payload: &[u8]) -> Result<(), JsValue> {
        self.0.dispatch_action(action_id, payload)
    }

    pub fn tick(&mut self) -> Result<(), JsValue> {
        self.0.tick()
    }

    #[wasm_bindgen(js_name = snapshotPtr)]
    pub fn snapshot_ptr(&self) -> *const u8 {
        self.0.snapshot_ptr()
    }

    #[wasm_bindgen(js_name = snapshotLen)]
    pub fn snapshot_len(&self) -> usize {
        self.0.snapshot_len()
    }

    #[wasm_bindgen(js_name = copySnapshot)]
    pub fn copy_snapshot(&self) -> Vec<u8> {
        self.0.copy_snapshot()
    }
}
```

**Build the browser package:**

```bash
rustup target add wasm32-unknown-unknown
cargo install wasm-pack
wasm-pack build --target web --out-dir pkg
```

**Consume in React / Web Workers (TypeScript):**

```ts
import init, { MyWasmGame } from "./pkg/my_game";

const wasm = await init();
const game = new MyWasmGame();

game.dispatchAction(0x0001, new Uint8Array(8));
game.tick();

// Zero-copy view into WASM linear memory (re-create after every tick/allocation):
const bytes = new Uint8Array(wasm.memory.buffer, game.snapshotPtr(), game.snapshotLen());
// Or copy out for Web Worker postMessage transfers:
const ownedBytes = game.copySnapshot();
```

If you prefer calling raw `extern "C"` WASM exports without `wasm-bindgen`, `floride-host-runtime` also exports `floride_alloc(len)` and `floride_dealloc(ptr, len)`.

---

### Environment 3: React Native (C-ABI + C++ JSI)

1. **Export concrete C-ABI constructors in your game crate** returning `*mut FlorideSessionHandle` and forward the panic-safe functions from `floride_host_runtime::ffi`:
   - Session lifecycle: `floride_session_dispatch_action`, `floride_session_tick`, `floride_session_snapshot_ptr`, `floride_session_snapshot_len`, `floride_session_copy_snapshot`, `floride_session_destroy`
   - Versioning & error reporting: `floride_abi_version()`, `floride_protocol_version()`, `floride_last_error_len()`, `floride_last_error_ptr()`, `floride_copy_last_error(...)`
   - Memory helpers: `floride_alloc(len)`, `floride_dealloc(ptr, len)`
   - All functions catch Rust panics (`FLORIDE_ERR_PANIC = -99`) so panics never unwind across C++ / JSI.
2. **Generate a C header with `cbindgen`:**
   - See [crates/host-runtime/cbindgen.toml](crates/host-runtime/cbindgen.toml) and [crates/host-runtime/include/floride_host.h](crates/host-runtime/include/floride_host.h).
3. **Wire the C++ JSI bridge:**
   - Use [crates/host-runtime/cpp/FlorideJsiHostObject.hpp](crates/host-runtime/cpp/FlorideJsiHostObject.hpp) inside your React Native TurboModule/JSI package. Populate `floride::FlorideCAbiVTable` with your game's C-ABI functions; JS receives synchronous `dispatchAction(id, payload)`, `tick()`, `readSnapshot()` (`ArrayBuffer`), and `destroy()` protected by a mutex.
4. **Cross-compile mobile binaries:**
   - Android (`cdylib`): build `.so` libraries per ABI (`aarch64-linux-android`, `armv7-linux-androideabi`, `x86_64-linux-android`) using the Android NDK linker.
   - iOS (`staticlib`): build `.a` libraries (`aarch64-apple-ios`, `aarch64-apple-ios-sim`) using Xcode on macOS.

---

### Environment 4: Native C/C++ & Game Engines (Godot, Unity, raylib)

Link the compiled `.a` (`staticlib`) or `.so`/`.dylib`/`.dll` (`cdylib`) against the `cbindgen` C header (`floride_host.h` + your game constructor header). Call `create(...)` to get an opaque `FlorideSessionHandle*`, submit inputs via `dispatch_action`, step at 60 Hz with `tick`, read or copy the TLV snapshot (`snapshot_ptr`/`snapshot_len` or `copy_snapshot`), and free with `destroy`.

---

## Documentation & Verification

### Further Reading
- [Rust_Game_Core_Architecture.md](docs/Rust_Game_Core_Architecture.md) — Core design principles, crate boundaries, and Stage 1 milestones.
- [Standardized_React_and_React_Native_Host_Runtime.md](docs/Standardized_React_and_React_Native_Host_Runtime.md) — Web Worker fixed-step accumulator, `WasmDriver` / `JsiDriver`, and React SDK architecture.
- [snapshot-format.md](docs/snapshot-format.md) — Complete byte-level TLV snapshot specification (`MAGIC`, chunk layouts, entity kinds, and TypedArray/JSI lifetime rules).

### Useful Commands

```bash
# Run headless engine tests (excluding the windowed visual demo)
cargo test --workspace --exclude floride-visual-demo

# Check formatting and lints on the host runtime crates
cargo fmt -p floride-host-runtime -p floride-host-wasm -- --check
cargo clippy -p floride-host-runtime -p floride-host-wasm --lib -- -D warnings

# Verify WebAssembly compilation for all core library crates
cargo check -p game-types -p physics-api -p physics-rapier -p game-api -p game-logic -p floride-host-runtime -p floride-host-wasm --target wasm32-unknown-unknown

# Run the interactive 3D visual showcase demo
cargo run -p floride-visual-demo --bin visual-demo
```
