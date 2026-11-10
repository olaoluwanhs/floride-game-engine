//! Tagged Chunk (TLV - Type-Length-Value) Binary Protocol for zero-copy snapshots.
//!
//! # Protocol Layout
//!
//! ```text
//! === GLOBAL BUFFER HEADER (16 bytes) ===
//! Offset  Type    Name            Description
//! 0x00    u32     magic           0x464C4F52 ("FLOR")
//! 0x04    u16     protocol_ver    Protocol version (1)
//! 0x06    u16     chunk_count     Total component chunks in this snapshot
//! 0x08    u64     tick_index      Authoritative simulation tick number
//!
//! === CONTIGUOUS CHUNKS (TLV Stream) ===
//! Offset  Type    Name            Description
//! 0x00    u16     chunk_type      Component Type ID (e.g. 0x0001 = Transforms)
//! 0x02    u16     chunk_flags     Flags (e.g. compressed, delta-frame)
//! 0x04    u32     chunk_len       Length in bytes of payload data (M)
//! 0x08    [u8; M] chunk_data      Packed raw array data
//! ```

/// Protocol magic bytes: "FLOR" ASCII represented as little-endian u32 (0x524F_4C46).
pub const MAGIC: u32 = u32::from_le_bytes(*b"FLOR");

/// Current supported protocol specification version.
pub const PROTOCOL_VERSION: u16 = 1;

/// Standard chunk type: Entity 3D Transforms (ID, position, orientation quaternion).
pub const CHUNK_TRANSFORMS: u16 = 0x0001;

/// Standard chunk type: Combat & Vitals (ID, current HP, max HP).
pub const CHUNK_COMBAT: u16 = 0x0002;

/// Standard chunk type: Weapon & Ammo state.
pub const CHUNK_WEAPONS: u16 = 0x0003;

/// Standard chunk type: Transient gameplay events ring buffer.
pub const CHUNK_EVENTS: u16 = 0x0004;

/// Errors that can occur when reading or parsing a TLV snapshot stream.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProtocolError {
    /// The buffer was smaller than the required 16-byte global header.
    BufferTooShort,
    /// The buffer did not start with the "FLOR" magic signature.
    InvalidMagic(u32),
    /// The protocol version specified in the header is not supported.
    UnsupportedVersion(u16),
    /// A chunk's declared length exceeds the remaining bytes in the buffer.
    CorruptedChunk {
        chunk_type: u16,
        expected_bytes: usize,
        remaining_bytes: usize,
    },
}

/// Writer for serializing zero-copy snapshot streams into a reusable memory buffer.
#[derive(Default, Debug)]
pub struct TlvWriter {
    buffer: Vec<u8>,
    chunk_count: u16,
}

impl TlvWriter {
    /// Creates a new `TlvWriter` with pre-allocated memory.
    pub fn new() -> Self {
        Self {
            buffer: Vec::with_capacity(1024),
            chunk_count: 0,
        }
    }

    /// Resets the buffer and writes the initial 16-byte global header.
    pub fn start_snapshot(&mut self, tick_index: u64) {
        self.buffer.clear();
        self.chunk_count = 0;

        // 0x00: magic (4 bytes)
        self.buffer.extend_from_slice(&MAGIC.to_le_bytes());
        // 0x04: protocol_ver (2 bytes)
        self.buffer.extend_from_slice(&PROTOCOL_VERSION.to_le_bytes());
        // 0x06: chunk_count (2 bytes, starts at 0)
        self.buffer.extend_from_slice(&0u16.to_le_bytes());
        // 0x08: tick_index (8 bytes)
        self.buffer.extend_from_slice(&tick_index.to_le_bytes());
    }

    /// Appends a new self-describing component chunk to the stream.
    pub fn write_chunk(&mut self, chunk_type: u16, chunk_flags: u16, payload: &[u8]) {
        // Chunk Header: 8 bytes
        // 0x00: chunk_type (u16)
        self.buffer.extend_from_slice(&chunk_type.to_le_bytes());
        // 0x02: chunk_flags (u16)
        self.buffer.extend_from_slice(&chunk_flags.to_le_bytes());
        // 0x04: chunk_len (u32)
        let chunk_len = payload.len() as u32;
        self.buffer.extend_from_slice(&chunk_len.to_le_bytes());
        // 0x08: chunk_data
        self.buffer.extend_from_slice(payload);

        // Increment chunk count and update offset 0x06 in global header
        self.chunk_count += 1;
        let count_bytes = self.chunk_count.to_le_bytes();
        self.buffer[6] = count_bytes[0];
        self.buffer[7] = count_bytes[1];
    }

    /// Returns the finalized read-only byte slice of the snapshot buffer.
    pub fn as_slice(&self) -> &[u8] {
        &self.buffer
    }

    /// Returns the raw pointer to the start of the buffer.
    pub fn as_ptr(&self) -> *const u8 {
        self.buffer.as_ptr()
    }

    /// Returns the total byte length of the serialized snapshot buffer.
    pub fn len(&self) -> usize {
        self.buffer.len()
    }

    /// Returns whether the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.buffer.is_empty()
    }
}

/// A parsed view of a single component chunk inside a snapshot buffer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChunkView<'a> {
    pub chunk_type: u16,
    pub chunk_flags: u16,
    pub payload: &'a [u8],
}

/// Zero-copy reader for inspecting and validating serialized TLV snapshot buffers.
#[derive(Debug, PartialEq, Eq)]
pub struct TlvReader<'a> {
    buffer: &'a [u8],
    tick_index: u64,
    chunk_count: u16,
}

impl<'a> TlvReader<'a> {
    /// Validates and parses the 16-byte global header from the provided buffer.
    pub fn new(buffer: &'a [u8]) -> Result<Self, ProtocolError> {
        if buffer.len() < 16 {
            return Err(ProtocolError::BufferTooShort);
        }

        let magic = u32::from_le_bytes(buffer[0..4].try_into().unwrap());
        if magic != MAGIC {
            return Err(ProtocolError::InvalidMagic(magic));
        }

        let version = u16::from_le_bytes(buffer[4..6].try_into().unwrap());
        if version != PROTOCOL_VERSION {
            return Err(ProtocolError::UnsupportedVersion(version));
        }

        let chunk_count = u16::from_le_bytes(buffer[6..8].try_into().unwrap());
        let tick_index = u64::from_le_bytes(buffer[8..16].try_into().unwrap());

        Ok(Self {
            buffer,
            tick_index,
            chunk_count,
        })
    }

    /// Returns the simulation tick index recorded in the header.
    pub fn tick_index(&self) -> u64 {
        self.tick_index
    }

    /// Returns the total number of chunks declared in the header.
    pub fn chunk_count(&self) -> u16 {
        self.chunk_count
    }

    /// Returns an iterator over all contiguous component chunks in the snapshot.
    pub fn iter_chunks(&self) -> ChunkIterator<'a> {
        ChunkIterator {
            slice: &self.buffer[16..],
        }
    }

    /// Searches the stream for the first chunk matching `chunk_type`.
    pub fn find_chunk(&self, chunk_type: u16) -> Option<Result<ChunkView<'a>, ProtocolError>> {
        for chunk in self.iter_chunks() {
            match chunk {
                Ok(view) if view.chunk_type == chunk_type => return Some(Ok(view)),
                Err(err) => return Some(Err(err)),
                _ => continue,
            }
        }
        None
    }
}

/// Iterator that yields parsed [`ChunkView`] instances from a raw chunk stream.
pub struct ChunkIterator<'a> {
    slice: &'a [u8],
}

impl<'a> Iterator for ChunkIterator<'a> {
    type Item = Result<ChunkView<'a>, ProtocolError>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.slice.is_empty() {
            return None;
        }

        if self.slice.len() < 8 {
            return Some(Err(ProtocolError::CorruptedChunk {
                chunk_type: 0,
                expected_bytes: 8,
                remaining_bytes: self.slice.len(),
            }));
        }

        let chunk_type = u16::from_le_bytes(self.slice[0..2].try_into().unwrap());
        let chunk_flags = u16::from_le_bytes(self.slice[2..4].try_into().unwrap());
        let chunk_len = u32::from_le_bytes(self.slice[4..8].try_into().unwrap()) as usize;

        let total_chunk_size = 8 + chunk_len;
        if self.slice.len() < total_chunk_size {
            let remaining = self.slice.len() - 8;
            self.slice = &[];
            return Some(Err(ProtocolError::CorruptedChunk {
                chunk_type,
                expected_bytes: chunk_len,
                remaining_bytes: remaining,
            }));
        }

        let payload = &self.slice[8..total_chunk_size];
        self.slice = &self.slice[total_chunk_size..];

        Some(Ok(ChunkView {
            chunk_type,
            chunk_flags,
            payload,
        }))
    }
}
