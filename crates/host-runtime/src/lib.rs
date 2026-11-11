//! Extensible zero-copy host runtime adapter for Floride game engine.
//!
//! Connects WebAssembly (WASM), React Native (JSI), and Native C hosts to any Rust
//! game implementing [`game_api::GameAdapter`].
//!
//! # Architecture
//! - [`actions`]: Extensible action packet envelope (`action_id: u16`, `payload: &[u8]`).
//! - [`protocol`]: Tagged Chunk (TLV) binary snapshot serializer and reader.
//! - [`session`]: Generic host session orchestrator holding the game and memory buffers.
//! - [`ffi`]: Universal C-ABI functions for foreign hosts (WASM / React Native JSI).

pub mod actions;
pub mod ffi;
pub mod protocol;
pub mod session;

pub use actions::{ActionDecoder, ActionError, ActionPacket};
pub use ffi::{
    clear_last_error, copy_bytes_into_raw, ffi_guard_i32, ffi_guard_mut_ptr, ffi_guard_ptr,
    ffi_guard_usize, floride_abi_version, floride_alloc, floride_copy_last_error, floride_dealloc,
    floride_last_error_len, floride_last_error_ptr, floride_protocol_version,
    floride_session_copy_snapshot, floride_session_destroy, floride_session_dispatch_action,
    floride_session_snapshot_len, floride_session_snapshot_ptr, floride_session_tick,
    set_last_error, AnySession, FlorideSessionHandle, FLORIDE_ABI_VERSION,
    FLORIDE_ERR_ACTION_FAILED, FLORIDE_ERR_INVALID_ARGUMENT, FLORIDE_ERR_NULL_PTR,
    FLORIDE_ERR_PANIC, FLORIDE_ERR_TICK_FAILED, FLORIDE_OK,
};
pub use protocol::{
    decode_blemishes, decode_enemies, decode_entity_metadata, decode_explosives, decode_mode,
    decode_pickups, decode_projectiles, decode_world_transforms, encode_blemishes, encode_enemies,
    encode_entity_metadata, encode_explosives, encode_mode, encode_pickups, encode_projectiles,
    encode_world_transforms, BlemishRecord, ChunkIterator, ChunkView, EnemyAimRayRecord,
    EnemyCombatRecord, EntityMetadataRecord, ExplosiveRecord, ModeRecord, ObjectiveRecord,
    PickupRecord, ProjectileRecord, ProtocolError, TlvReader, TlvWriter, WorldTransformRecord,
    CHUNK_BLEMISHES, CHUNK_COMBAT, CHUNK_ENEMIES, CHUNK_ENTITY_METADATA, CHUNK_EVENTS,
    CHUNK_EXPLOSIVES, CHUNK_MODE, CHUNK_PICKUPS, CHUNK_PROJECTILES, CHUNK_TRANSFORMS,
    CHUNK_WEAPONS, CHUNK_WORLD_TRANSFORMS, ENEMY_BEHAVIOR_ATTACK, ENEMY_BEHAVIOR_CHASE,
    ENEMY_BEHAVIOR_DEAD, ENEMY_BEHAVIOR_PATROL, ENEMY_BEHAVIOR_RELOADING, ENTITY_KIND_DYNAMIC,
    ENTITY_KIND_ENEMY, ENTITY_KIND_PICKUP, ENTITY_KIND_PLAYER, ENTITY_KIND_STATIC,
    ENTITY_KIND_TARGET, ENTITY_KIND_TRIGGER, MAGIC, MATCH_STATUS_ACTIVE, MATCH_STATUS_DEFEAT,
    MATCH_STATUS_VICTORY, PICKUP_KIND_AMMO, PICKUP_KIND_HEALTH, PROTOCOL_VERSION,
};
pub use session::{HostError, HostSession, SnapshotEncoder};
