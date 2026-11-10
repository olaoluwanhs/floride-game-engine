# Floride Game Engine: Standardized React & React Native Host Runtime

**Document Type:** Engine Specification & Architecture Proposal  
**Target Runtimes:** React (Web / WebAssembly) and React Native (iOS / Android / JSI)  
**Status:** Approved Architecture Draft  
**Related Documents:** [Rust_Game_Core_Architecture.md](Rust_Game_Core_Architecture.md)  

---

## 1. Overview & Goal

The Floride Game Engine is a headless, simulation-first game engine written in Rust. While the engine's core simulation loop (`game-api::GameAdapter`) is strictly platform- and renderer-agnostic, integrating a simulation into web and mobile applications traditionally introduces significant boilerplate:
- Setting up WebAssembly linear memory bindings.
- Managing Web Worker threads to avoid blocking UI rendering.
- Implementing fixed-timestep accumulation loops (e.g. 60 Hz simulation vs 120 Hz display refresh).
- Writing native C++ JSI / TurboModule bridges for React Native.
- Serializing and deserializing state snapshots at high frame rates without garbage collection (GC) stutter.

**The Goal of this specification:**  
Standardize the host runtime directly within **`floride-game-engine`** so that **any game** implementing `GameAdapter` can be embedded into React (Web) and React Native (Mobile) using a single, unified TypeScript SDK (`@floride/react`).

Game authors focus solely on game rules, mechanics, and 3D rendering. The Floride engine manages execution, threading, memory transfer, and timing.

---

## 2. High-Level Architecture

```text
               Consumer Game (e.g. Soldiers of Fortune)
              ┌──────────────────────────────────────────────┐
              │  impl GameAdapter for FpsRuntime {           │
              │      type Input = FpsInput;                  │
              │      type Snapshot = FpsSnapshot;            │
              │      // Authoritative game rules in Rust     │
              │  }                                           │
              └──────────────────────┬───────────────────────┘
                                     │
      ═══════════════════════════════╪════════════════════════════════
                        floride-game-engine
      ───────────────────────────────┼────────────────────────────────
      1. Rust Host Crates:           ▼
         • floride-host-wasm   ── Generic WASM session & linear memory export
         • floride-host-native ── Generic C-ABI / JSI native library (.so / .a)
                                     │
      ───────────────────────────────┼────────────────────────────────
      2. Engine TypeScript Package (@floride/react):
         • Fixed-step accumulator (60 Hz game ticks + interpolation factor)
         • Web Worker orchestration (keeps physics off the UI thread)
         • Universal React Hook: useFlorideGame()
      ═══════════════════════════════╪════════════════════════════════
                                     ▼
                      Frontend Rendering Layer
         ┌───────────────────────────┴───────────────────────────┐
         ▼                                                       ▼
  React Web (Three.js / R3F)                   React Native (Three / Skia)
```

---

## 3. Rust Engine Host Crates

To support both environments without polluting core simulation crates, `floride-game-engine` provides two host bridge crates:

### 3.1 `floride-host-wasm` (Browser / WebAssembly)

* **Location:** `crates/floride-host-wasm`
* **Dependencies:** `wasm-bindgen`, `game-api`, `js-sys`, `web-sys`
* **Responsibilities:**
  1. Wraps any type `G: GameAdapter` into a WASM-exported session class (`FlorideWasmSession`).
  2. Exposes zero-copy memory access to the current snapshot via WASM linear memory (`Uint8Array` / `Float32Array`).
  3. Provides lifecycle methods: `create(config_bytes)`, `submit_input(input_bytes)`, `tick()`, `snapshot_ptr()`, `snapshot_len()`, and `drain_events()`.

```rust
#[wasm_bindgen]
pub struct FlorideWasmSession {
    adapter: Box<dyn DynamicGameAdapter>,
    snapshot_buffer: Vec<u8>,
}

#[wasm_bindgen]
impl FlorideWasmSession {
    pub fn tick(&mut self) -> Result<(), JsValue> { ... }
    pub fn snapshot_ptr(&self) -> *const u8 { ... }
    pub fn snapshot_len(&self) -> usize { ... }
    pub fn submit_input(&mut self, input_bytes: &[u8]) -> Result<(), JsValue> { ... }
}
```

### 3.2 `floride-host-native` (React Native / C-ABI)

* **Location:** `crates/floride-host-native`
* **Dependencies:** `game-api`
* **Responsibilities:**
  1. Compiles to static (`.a`) and dynamic (`.so` / `.dylib`) libraries for iOS, Android, and desktop hosts.
  2. Exposes an extern `#[repr(C)]` ABI with opaque handles:
     ```c
     typedef struct FlorideSession FlorideSession;

     FlorideSession* floride_create(const uint8_t* config_data, size_t config_len);
     int32_t         floride_submit_input(FlorideSession* session, const uint8_t* data, size_t len);
     int32_t         floride_tick(FlorideSession* session);
     const uint8_t*  floride_get_snapshot_ptr(const FlorideSession* session);
     size_t          floride_get_snapshot_len(const FlorideSession* session);
     void            floride_destroy(FlorideSession* session);
     ```
  3. Integrates with React Native via modern **C++ JSI (JavaScript Interface)** or **Nitro Modules**, eliminating the legacy JSON asynchronous bridge.

---

## 4. The Shared TypeScript Package (`@floride/react`)

The engine provides an official npm package `@floride/react` (and a framework-agnostic core `@floride/client`) that standardizes runtime execution across Web and Native.

### 4.1 Driver Abstraction

The TypeScript SDK communicates through a simple driver interface:

```typescript
export interface FlorideDriver {
  init(config: Uint8Array): Promise<void>;
  submitInput(input: Uint8Array): void;
  tick(): void;
  getSnapshotBuffer(): Uint8Array;
  drainEvents(): Uint8Array[];
  destroy(): void;
}
```

* **`WasmDriver`:** Uses `WebAssembly.instantiate` and automatically runs inside a Web Worker.
* **`JsiDriver`:** Direct synchronous C++ calls via React Native JSI into the compiled Rust binary.

### 4.2 The Fixed-Step Time Accumulator

To prevent physics inconsistencies and frame-rate degradation on high-refresh displays (90 Hz, 120 Hz, 144 Hz), `@floride/react` embeds a standard time accumulator:

```typescript
const FIXED_STEP = 1 / 60; // 60 simulation ticks per second
let accumulator = 0.0;
let lastTime = performance.now();

function gameLoop(currentTime: number) {
  const delta = (currentTime - lastTime) / 1000;
  lastTime = currentTime;

  // Clamp max delta to prevent spiral of death on lag spikes
  accumulator += Math.min(delta, 0.25);

  while (accumulator >= FIXED_STEP) {
    driver.tick();
    accumulator -= FIXED_STEP;
  }

  // Alpha factor (0.0 to 1.0) used by renderers to interpolate visual poses
  const alpha = accumulator / FIXED_STEP;
  notifySubscribers(driver.getSnapshotBuffer(), alpha);

  requestAnimationFrame(gameLoop);
}
```

### 4.3 Universal React Hook (`useFlorideGame`)

Frontend applications consume the simulation cleanly using React idiomatic patterns:

```tsx
import React, { useRef } from 'react';
import { useFlorideGame, WasmDriver } from '@floride/react';
import { Canvas } from '@react-three/fiber';

export function GameView() {
  const { snapshot, submitInput, alpha } = useFlorideGame({
    driver: new WasmDriver('/game_core.wasm'),
    tickRate: 60,
  });

  if (!snapshot) return <div>Loading simulation...</div>;

  return (
    <Canvas>
      {/* Three.js / React Three Fiber scene */}
      <ambientLight />
      <PlayerMesh 
        position={snapshot.player.position} 
        interpolation={alpha} 
      />
    </Canvas>
  );
}
```

---

## 5. Binary Protocol & Memory Performance

To achieve 60+ FPS on mobile devices and browsers without GC thrashing:
1. **Zero-Copy Snapshot Reading:**
   - In WASM: Snapshots are read directly via `new Float32Array(wasmMemory.buffer, ptr, len)`.
   - In React Native JSI: Snapshots are exposed as an `ArrayBuffer` directly over native memory.
2. **Binary Serializers:**
   - Game inputs and snapshots can use fixed-struct binary representations, Protocol Buffers, or compact `bincode` / `bitcode`.
3. **Threading Isolation:**
   - Web: The game loop and physics run exclusively in a dedicated Web Worker. The React main thread only receives decoded snapshots during animation frames.
   - Mobile: The simulation runs on a dedicated native background thread, updating a thread-safe atomic snapshot buffer read by JSI.

---

## 6. Development & Implementation Roadmap

| Phase | Milestone | Deliverable |
| :--- | :--- | :--- |
| **Phase 1** | **Host WASM Crate** | Scaffolding `crates/floride-host-wasm` in `floride-game-engine`. Exposes generic `FlorideWasmSession`. |
| **Phase 2** | **Host Native C-ABI Crate** | Scaffolding `crates/floride-host-native`. Exposes standard `#[repr(C)]` functions. |
| **Phase 3** | **TypeScript SDK (`@floride/react`)** | Implement the fixed-step loop, worker runner, and React hook in an npm package within the repo. |
| **Phase 4** | **Verification with Soldiers of Fortune** | Embed `soldiers-of-fortune` into a sample Vite/React web runner and Expo/React Native mobile runner. |
