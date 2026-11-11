# Floride TLV Snapshot Binary Format (`PROTOCOL_VERSION = 1`)

All integers and IEEE-754 floats (`f32`) are **little-endian**. All UTF-8 strings use a `u16` byte-length prefix followed by raw UTF-8 bytes (truncated cleanly to `u16::MAX` if needed).

---

## 1. Global Header (16 bytes)

| Offset | Size | Type | Name | Description |
| :--- | :--- | :--- | :--- | :--- |
| `0x00` | 4 | `u32` / `[u8; 4]` | `magic` | `"FLOR"` (`0x464C4F52` big-endian, `0x524F4C46` little-endian) |
| `0x04` | 2 | `u16` | `protocol_ver` | `1` |
| `0x06` | 2 | `u16` | `chunk_count` | Number of contiguous TLV chunks that follow |
| `0x08` | 8 | `u64` | `tick_index` | Authoritative fixed simulation tick counter |

---

## 2. Chunk Envelope (8 bytes + `chunk_len` payload)

Repeated `chunk_count` times immediately starting at byte `16`:

| Offset | Size | Type | Name | Description |
| :--- | :--- | :--- | :--- | :--- |
| `0x00` | 2 | `u16` | `chunk_type` | Chunk type identifier (`0x0001` .. `0x000C`) |
| `0x02` | 2 | `u16` | `chunk_flags` | Reserved flags (`0` in v1) |
| `0x04` | 4 | `u32` | `chunk_len` | Payload length in bytes |
| `0x08` | `chunk_len` | `[u8]` | `chunk_data` | Packed payload |

---

## 3. Standard Engine Chunk Types (`crates/host-runtime/src/protocol.rs`)

### `0x0005` — `CHUNK_WORLD_TRANSFORMS`
- `count: u32`
- Repeated `count` fixed **33-byte** records:
  - `entity_id: u32`
  - `entity_kind: u8` (`1=Player`, `2=Enemy`, `3=Dynamic`, `4=Static`, `5=Target`, `6=Pickup`, `7=Trigger`)
  - `position: [f32; 3]` (`x, y, z` meters, Y-up right-handed)
  - `rotation: [f32; 4]` (`x, y, z, w` unit quaternion)

> Note: Dead enemies/targets and unavailable pickups are omitted from `CHUNK_WORLD_TRANSFORMS` while remaining in `CHUNK_ENTITY_METADATA`. Renderers should hide meshes whose `(entity_id, entity_kind)` is absent from `CHUNK_WORLD_TRANSFORMS`.

### `0x0006` — `CHUNK_ENTITY_METADATA`
- `count: u32`
- Repeated `count` records:
  - `entity_id: u32`
  - `entity_kind: u8`
  - `name_len: u16` + `name_utf8: [u8; name_len]`

### `0x0007` — `CHUNK_ENEMIES`
- `count: u32`
- Repeated `count` records:
  - `entity_id: u32`
  - `health: f32`, `max_health: f32`
  - `is_alive: u8` (`0` or `1`)
  - `behavior: u8` (`0=Patrol`, `1=Chase`, `2=Attack`, `3=Reloading`, `4=Dead`)
  - `capsule_radius: f32`, `capsule_height: f32`
  - `active_weapon_id: u32`, `ammo: u32`, `reserve_ammo: u32`, `is_reloading: u8`
  - `weapon_name_len: u16` + `weapon_name_utf8`
  - `has_aim_ray: u8` (`0` or `1`)
  - If `has_aim_ray != 0`: `origin: [f32; 3]`, `end_point: [f32; 3]`, `direction: [f32; 3]`, `hits_player: u8`, `is_firing_soon: u8` (38 bytes)

### `0x0008` — `CHUNK_PROJECTILES`
- `count: u32`
- Repeated `count` fixed **37-byte** records:
  - `id: u64`, `position: [f32; 3]`, `velocity: [f32; 3]`, `radius: f32`, `is_alive: u8`

### `0x0009` — `CHUNK_EXPLOSIVES`
- `count: u32`
- Repeated `count` fixed **36-byte** records:
  - `id: u64`, `entity_id: u32`, `position: [f32; 3]`, `radius: f32`, `fuse_ticks_remaining: u32`, `max_fuse_ticks: u32`

### `0x000A` — `CHUNK_BLEMISHES`
- `count: u32`
- Repeated `count` fixed **48-byte** records:
  - `id: u64`, `entity_id: u32` (`0` if anonymous scenery), `position: [f32; 3]`, `normal: [f32; 3]`, `radius: f32`, `remaining_ticks: u32`, `max_ticks: u32`

### `0x000B` — `CHUNK_MODE`
- `mode_name: string` (`u16` len + UTF-8)
- `status: u8` (`0=Active`, `1=Victory`, `2=Defeat`)
- `has_outcome_reason: u8` + (`outcome_reason: string` if `!= 0`)
- `current_wave: u32`, `total_waves: u32`, `breather_ticks_remaining: u32`
- `score: u32`, `enemies_eliminated: u32`
- `has_time_remaining: u8` + (`time_remaining_ticks: u64` if `!= 0`)
- `objective_count: u32` + repeated (`id: string`, `description: string`, `completed: u8`, `progress: u32`, `target: u32`)

### `0x000C` — `CHUNK_PICKUPS`
- `count: u32`
- Repeated `count` fixed **42-byte** records:
  - `id: u64`, `position: [f32; 3]`, `yaw: f32`, `radius: f32`, `is_available: u8`, `kind_tag: u8` (`1=Health`, `2=Ammo`), `health_amount: f32`, `weapon_id: u32`, `reserve_rounds: u32`

---

## 4. WASM & Native Buffer Lifetime Rules

- **WebAssembly (Zero-Copy View):**
  ```ts
  const ptr = session.snapshotPtr();
  const len = session.snapshotLen();
  const bytes = new Uint8Array(wasm.memory.buffer, ptr, len);
  const view = new DataView(wasm.memory.buffer, ptr, len);
  ```
  **Do not cache `Uint8Array` or `DataView` across `tick()`, `dispatchAction()`, `free()`, or any WASM allocation.** WebAssembly `memory.grow()` detaches old `ArrayBuffer` views, and `tick()` overwrites the internal buffer. Re-create the view on each frame or use `session.copySnapshot()` when transferring across Web Worker `postMessage` boundaries.
- **Native C-ABI / React Native JSI:**
  - `*_snapshot_ptr` is valid on the owning thread until the next `*_tick` or `*_destroy`.
  - When bridging across threads (e.g. a background simulation thread + JS thread), serialize access with a mutex and copy bytes out via `*_copy_snapshot` (see `crates/host-runtime/cpp/FlorideJsiHostObject.hpp`).
