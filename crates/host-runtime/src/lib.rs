//! Extensible zero-copy host runtime adapter for Floride game engine.
//!
//! Connects WebAssembly (WASM), React Native (JSI), and Native C hosts to any Rust
//! game implementing [`game_api::GameAdapter`].
//!
//! # Architecture
//! - [`actions`]: Extensible action packet envelope (`action_id: u16`, `payload: &[u8]`).
//! - [`protocol`]: Tagged Chunk (TLV) binary snapshot serializer and reader.
//! - [`session`]: Generic host session orchestrator holding the game and memory buffers.

pub mod actions;
pub mod protocol;
pub mod session;

pub use actions::{ActionDecoder, ActionError, ActionPacket};
pub use protocol::{
    ChunkIterator, ChunkView, ProtocolError, TlvReader, TlvWriter, CHUNK_COMBAT, CHUNK_EVENTS,
    CHUNK_TRANSFORMS, CHUNK_WEAPONS, MAGIC, PROTOCOL_VERSION,
};
pub use session::{HostError, HostSession, SnapshotEncoder};
