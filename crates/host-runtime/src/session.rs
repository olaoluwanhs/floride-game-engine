//! Generic host session wrapping any [`game_api::GameAdapter`].

use game_api::GameAdapter;

use crate::actions::{ActionDecoder, ActionError, ActionPacket};
use crate::protocol::TlvWriter;

/// Errors that can occur during host runtime session operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostError {
    /// An error occurred while decoding or submitting an input action.
    Action(ActionError),
    /// The underlying game simulation returned an error.
    Game(String),
}

impl From<ActionError> for HostError {
    fn from(err: ActionError) -> Self {
        Self::Action(err)
    }
}

/// Trait implemented by game adapters or serializers to serialize game snapshots
/// and transient events into TLV snapshot chunks.
pub trait SnapshotEncoder<Snapshot, Event> {
    /// Populates `writer` with the appropriate TLV chunks for this tick.
    fn encode_snapshot(&mut self, snapshot: &Snapshot, events: &[Event], writer: &mut TlvWriter);
}

/// Authoritative host session wrapping a game simulation, an action decoder,
/// and a persistent TLV snapshot buffer.
pub struct HostSession<G, D, E>
where
    G: GameAdapter,
    D: ActionDecoder<G::Input>,
    E: SnapshotEncoder<G::Snapshot, G::Event>,
{
    game: G,
    decoder: D,
    encoder: E,
    writer: TlvWriter,
    tick_index: u64,
}

impl<G, D, E> HostSession<G, D, E>
where
    G: GameAdapter,
    G::Error: std::fmt::Display,
    D: ActionDecoder<G::Input>,
    E: SnapshotEncoder<G::Snapshot, G::Event>,
{
    /// Creates a new `HostSession` from an initialized game simulation, decoder, and encoder.
    pub fn new(game: G, decoder: D, encoder: E) -> Self {
        let mut session = Self {
            game,
            decoder,
            encoder,
            writer: TlvWriter::new(),
            tick_index: 0,
        };
        // Generate initial tick 0 snapshot
        session.update_snapshot();
        session
    }

    /// Dispatches an action packet from raw parts into the game simulation.
    pub fn dispatch_action(&mut self, action_id: u16, payload: &[u8]) -> Result<(), HostError> {
        let packet = ActionPacket::new(action_id, payload);
        let input = self.decoder.decode_action(packet)?;
        self.game
            .submit_input(input)
            .map_err(|e| HostError::Game(e.to_string()))?;
        Ok(())
    }

    /// Advances the simulation by one tick, drains events, and refreshes the snapshot buffer.
    pub fn tick(&mut self) -> Result<(), HostError> {
        self.game
            .tick()
            .map_err(|e| HostError::Game(e.to_string()))?;
        self.tick_index = self.tick_index.wrapping_add(1);
        self.update_snapshot();
        Ok(())
    }

    /// Internal helper to pull snapshot & events from the game and write TLV chunks.
    fn update_snapshot(&mut self) {
        let snapshot = self.game.snapshot();
        let events = self.game.drain_events();
        self.writer.start_snapshot(self.tick_index);
        self.encoder
            .encode_snapshot(&snapshot, &events, &mut self.writer);
    }

    /// Returns a direct read-only byte slice of the serialized TLV snapshot buffer.
    pub fn snapshot_slice(&self) -> &[u8] {
        self.writer.as_slice()
    }

    /// Returns a raw pointer to the start of the snapshot buffer for C / WASM consumers.
    pub fn snapshot_ptr(&self) -> *const u8 {
        self.writer.as_ptr()
    }

    /// Returns the total byte length of the serialized snapshot buffer.
    pub fn snapshot_len(&self) -> usize {
        self.writer.len()
    }

    /// Provides immutable access to the underlying game adapter.
    pub fn game(&self) -> &G {
        &self.game
    }

    /// Provides mutable access to the underlying game adapter.
    pub fn game_mut(&mut self) -> &mut G {
        &mut self.game
    }
}
