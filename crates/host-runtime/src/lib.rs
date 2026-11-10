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
    floride_session_destroy, floride_session_dispatch_action, floride_session_snapshot_len,
    floride_session_snapshot_ptr, floride_session_tick, AnySession, FlorideSessionHandle,
    FLORIDE_ERR_ACTION_FAILED, FLORIDE_ERR_NULL_PTR, FLORIDE_ERR_TICK_FAILED, FLORIDE_OK,
};
pub use protocol::{
    ChunkIterator, ChunkView, ProtocolError, TlvReader, TlvWriter, CHUNK_COMBAT, CHUNK_EVENTS,
    CHUNK_TRANSFORMS, CHUNK_WEAPONS, MAGIC, PROTOCOL_VERSION,
};
pub use session::{HostError, HostSession, SnapshotEncoder};
