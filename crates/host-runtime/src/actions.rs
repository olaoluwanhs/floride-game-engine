//! Generic Polymorphic Action Envelope for extensible host-to-game input dispatching.
//!
//! # Binary Layout of an Action Packet
//!
//! ```text
//! Offset  Type    Name            Description
//! 0x00    u16     action_id       Arbitrary game action identifier
//! 0x02    u16     payload_len     Length of payload bytes N
//! 0x04    [u8; N] payload_bytes   Packed parameters (e.g. 2x f32 for yaw/pitch, 3x f32 for move)
//! ```

/// Errors that can occur when parsing or decoding input action packets.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActionError {
    /// The input buffer was smaller than the 4-byte action header.
    BufferTooShort,
    /// The declared payload length exceeded the actual available bytes.
    PayloadLengthMismatch { declared: usize, available: usize },
    /// The game or decoder does not recognize the specified action ID.
    UnknownActionId(u16),
    /// The payload bytes could not be decoded into the expected parameter types.
    InvalidPayload(String),
}

impl std::fmt::Display for ActionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BufferTooShort => write!(f, "action buffer shorter than 4-byte header"),
            Self::PayloadLengthMismatch {
                declared,
                available,
            } => write!(
                f,
                "action payload declared {declared} bytes, but only {available} available"
            ),
            Self::UnknownActionId(id) => write!(f, "unknown action id 0x{id:04X}"),
            Self::InvalidPayload(msg) => write!(f, "invalid action payload: {msg}"),
        }
    }
}

impl std::error::Error for ActionError {}

/// A zero-copy view over an action packet received from the host.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ActionPacket<'a> {
    /// Game-defined action identifier.
    pub action_id: u16,
    /// Raw unparsed payload bytes for this action.
    pub payload: &'a [u8],
}

impl<'a> ActionPacket<'a> {
    /// Creates an `ActionPacket` directly from its components.
    pub fn new(action_id: u16, payload: &'a [u8]) -> Self {
        Self { action_id, payload }
    }

    /// Parses an action packet from a binary slice adhering to the 4-byte header envelope.
    pub fn from_bytes(bytes: &'a [u8]) -> Result<Self, ActionError> {
        if bytes.len() < 4 {
            return Err(ActionError::BufferTooShort);
        }

        let action_id = u16::from_le_bytes([bytes[0], bytes[1]]);
        let payload_len = u16::from_le_bytes([bytes[2], bytes[3]]) as usize;

        if bytes.len() < 4 + payload_len {
            return Err(ActionError::PayloadLengthMismatch {
                declared: payload_len,
                available: bytes.len() - 4,
            });
        }

        let payload = &bytes[4..4 + payload_len];

        Ok(Self { action_id, payload })
    }

    /// Serializes this action packet into an output byte vector.
    pub fn serialize_into(&self, output: &mut Vec<u8>) {
        output.extend_from_slice(&self.action_id.to_le_bytes());
        let payload_len = self.payload.len() as u16;
        output.extend_from_slice(&payload_len.to_le_bytes());
        output.extend_from_slice(self.payload);
    }
}

/// Trait implemented by game-specific input decoders to transform generic
/// [`ActionPacket`]s into concrete simulation input types.
pub trait ActionDecoder<Input> {
    /// Decodes a generic action packet into a game's concrete input command.
    fn decode_action(&self, packet: ActionPacket<'_>) -> Result<Input, ActionError>;
}
