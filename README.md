# Rust Game Core Architecture

A modular, headless, engine-independent game simulation framework in Rust.

This repository provides reusable crates for building headless, deterministic game simulations. It decouples game logic, character kinematics, and 3D physics from graphics, rendering, and platform hardware. You can use these crates to write your game's simulation once in Rust and run it anywhere: in native game engines (Godot/Unity/Unreal via C ABI), web browsers via WebAssembly (WASM), or headless servers.

---

## Core Philosophy

- **The host / renderer owns**: 3D meshes, shaders, cameras, UI/menus, reading hardware input devices, and audio playback.
- **The simulation core owns**: World state, collision resolution, character kinematics, game rules, fixed-step ticking, and snapshot generation.

---

## Workspace Architecture

```
                    Any Game Host / Platform Wrapper
               (Browser WASM, Godot/Unity C ABI, CLI)
                                  │
                   crates/game-api (GameAdapter)
                                  │
                 Your Game (FPS, RPG, Platformer)
             (Defines its own inputs, rules, snapshots)
                   ┌──────────────┴──────────────┐
                   ▼                             ▼
           crates/game-logic             crates/physics-api
       (Reusable systems, intents)       (PhysicsWorld trait)
                                                 │
                                                 ▼
                                       crates/physics-rapier
                                        (Rapier 3D backend)
```

### Crate Breakdown

| Crate | Responsibility | Dependencies |
| :--- | :--- | :--- |
| **[`game-types`](crates/game-types)** | Shared, engine-neutral primitive types (`Vec3`, `EntityId`). | None |
| **[`physics-api`](crates/physics-api)** | Trait-based contract for 3D physics worlds (`PhysicsWorld`, colliders, character controller, raycasting). | `game-types` |
| **[`physics-rapier`](crates/physics-rapier)** | High-performance 3D physics implementation wrapping Rapier 3D behind `physics-api`. | `game-types`, `physics-api`, `rapier3d` |
| **[`game-api`](crates/game-api)** | Universal host-facing lifecycle contract (`GameAdapter` trait). Uses associated types so your game keeps its own inputs and states. | `physics-api` |
| **[`game-logic`](crates/game-logic)** | Reusable gameplay systems (intent-driven character movement, kinematics, friction, gravity). | `game-types`, `physics-api` |
| **[`examples/visual-demo`](examples/visual-demo)** | Standalone 3D visual simulator showcasing shapes, phantoms, and physics interactions. | `game-types`, `physics-api`, `physics-rapier`, `macroquad` |

---

## Adding to Your Project

You can add these crates directly as dependencies in your game project's `Cargo.toml`:

```toml
[dependencies]
# Shared primitives and host integration contract
game-types = { git = "https://github.com/<owner>/<repo>", package = "game-types" }
game-api = { git = "https://github.com/<owner>/<repo>", package = "game-api" }

# Optional: Reusable character kinematics and movement math
game-logic = { git = "https://github.com/<owner>/<repo>", package = "game-logic" }

# 3D physics abstraction and Rapier implementation
physics-api = { git = "https://github.com/<owner>/<repo>", package = "physics-api" }
physics-rapier = { git = "https://github.com/<owner>/<repo>", package = "physics-rapier" }
```

You can choose whichever layers your game requires. For example, you can implement `GameAdapter` from `game-api` and use `physics-rapier` while opting in or out of `game-logic`'s kinematic movement.

---

## How to Build a Game with These Crates

### 1. Define Your Game's Vocabulary
Your game defines its own actions, configuration, snapshots, and events. There is no universal command enum imposed on your game.

```rust
use game_types::Vec3;

pub struct MyGameConfig {
    pub map_name: String,
    pub tick_rate_hz: u32,
}

pub enum MyGameInput {
    Move { direction: Vec3, sprint: bool },
    Jump,
    Interact,
}

pub struct MyGameSnapshot {
    pub player_position: Vec3,
    pub score: u32,
}

pub enum MyGameEvent {
    ItemCollected { item_id: u32 },
}
```

### 2. Implement `GameAdapter`
Implement the [`GameAdapter`](crates/game-api) trait from `game-api`. This tells any host how to initialize, feed inputs to, tick, and read state from your game:

```rust
use game_api::GameAdapter;
use game_logic::{MovementConfig, MovementIntent, MovementState, step_character_movement};
use physics_api::{CharacterDesc, PhysicsHandle, PhysicsWorld, PrimitiveShape};

pub struct MyGame<P: PhysicsWorld> {
    physics: P,
    player_handle: PhysicsHandle,
    movement_config: MovementConfig,
    movement_state: MovementState,
    pending_intent: MovementIntent,
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
                shape: PrimitiveShape::Capsule { half_height: 0.9, radius: 0.4 },
                initial_position: Vec3::new(0.0, 1.0, 0.0),
                ..Default::default()
            })
            .map_err(|e| format!("Failed to create player: {e:?}"))?;

        Ok(Self {
            physics,
            player_handle,
            movement_config: MovementConfig::default(),
            movement_state: MovementState::default(),
            pending_intent: MovementIntent::default(),
            score: 0,
            events: Vec::new(),
        })
    }

    fn submit_input(&mut self, input: Self::Input) -> Result<(), Self::Error> {
        // Map your game-specific inputs into generic movement intents
        match input {
            MyGameInput::Move { direction, sprint } => {
                self.pending_intent.desired_direction = direction;
                self.pending_intent.sprint = sprint;
            }
            MyGameInput::Jump => {
                self.pending_intent.jump = true;
            }
            MyGameInput::Interact => {
                // Handle interactions
            }
        }
        Ok(())
    }

    fn tick(&mut self) -> Result<(), Self::Error> {
        const DT: f32 = 1.0 / 60.0;

        // 1. Step character movement using reusable kinematics from game-logic
        step_character_movement(
            &mut self.physics,
            self.player_handle,
            &self.movement_config,
            &mut self.movement_state,
            &self.pending_intent,
            DT,
        ).map_err(|e| format!("Movement error: {e:?}"))?;

        // 2. Advance the physics simulation world
        self.physics.step(DT).map_err(|e| format!("Physics step error: {e:?}"))?;

        // 3. Reset one-shot inputs
        self.pending_intent.jump = false;

        Ok(())
    }

    fn snapshot(&self) -> Self::Snapshot {
        let transform = self.physics.body_transform(self.player_handle)
            .unwrap_or_default();

        MyGameSnapshot {
            player_position: transform.translation,
            score: self.score,
        }
    }

    fn drain_events(&mut self) -> Vec<Self::Event> {
        std::mem::take(&mut self.events)
    }
}
```

### 3. Drive the Game from Any Host
Any host application can now drive your game completely generically:

```rust
use game_api::GameAdapter;
use physics_rapier::RapierPhysicsWorld;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let physics = RapierPhysicsWorld::new(Vec3::new(0.0, -9.81, 0.0));
    let config = MyGameConfig { map_name: "level_01".into(), tick_rate_hz: 60 };

    let mut game = MyGame::create(config, physics)?;

    // Standard fixed-rate simulation loop
    for _ in 0..60 {
        game.submit_input(MyGameInput::Move {
            direction: Vec3::new(0.0, 0.0, 1.0),
            sprint: false,
        })?;

        game.tick()?;

        let snapshot = game.snapshot();
        println!("Player is at: {:?}", snapshot.player_position);

        let events = game.drain_events();
        for event in events {
            // Forward events to host audio or UI systems
        }
    }

    Ok(())
}
```

---

## Verification & Development

To build and run tests across all crates in this repository:

```bash
# Run all unit and integration tests
cargo test --workspace

# Run Clippy lints across the entire workspace
cargo clippy --workspace --all-targets

# Run the 3D visual showcase demo
cargo run -p visual-demo
```
