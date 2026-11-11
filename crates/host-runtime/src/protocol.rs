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

/// Extended chunk type: Multi-entity 3D transforms with full rotation quaternions.
///
/// Payload layout:
/// - `count: u32` (little-endian)
/// - Repeated `count` records (33 bytes each):
///   - `entity_id: u32`
///   - `entity_kind: u8`
///   - `pos_x: f32, pos_y: f32, pos_z: f32`
///   - `rot_x: f32, rot_y: f32, rot_z: f32, rot_w: f32`
pub const CHUNK_WORLD_TRANSFORMS: u16 = 0x0005;

/// Standard chunk type: Entity identity & asset/model metadata table (`kind`, `id`, `name`).
///
/// Payload layout:
/// - `count: u32` (little-endian)
/// - Repeated `count` records:
///   - `entity_id: u32`
///   - `entity_kind: u8`
///   - `name_len: u16`
///   - `name_utf8: [u8; name_len]`
pub const CHUNK_ENTITY_METADATA: u16 = 0x0006;

/// Extended chunk type: Active AI enemy combat status, weapon, and aim telegraph rays.
pub const CHUNK_ENEMIES: u16 = 0x0007;

/// Extended chunk type: In-flight physical projectiles.
pub const CHUNK_PROJECTILES: u16 = 0x0008;

/// Extended chunk type: Armed timed explosives (grenades, charges) counting down.
pub const CHUNK_EXPLOSIVES: u16 = 0x0009;

/// Extended chunk type: Temporary surface impact blemishes (bullet holes / scorch marks).
pub const CHUNK_BLEMISHES: u16 = 0x000A;

/// Extended chunk type: Game mode progression, waves, score, timer, and Story objectives.
pub const CHUNK_MODE: u16 = 0x000B;

/// Extended chunk type: Collectible health and ammunition pickups.
pub const CHUNK_PICKUPS: u16 = 0x000C;

/// Enemy behavior state tag in [`CHUNK_ENEMIES`].
pub const ENEMY_BEHAVIOR_PATROL: u8 = 0;
/// Enemy behavior state tag in [`CHUNK_ENEMIES`].
pub const ENEMY_BEHAVIOR_CHASE: u8 = 1;
/// Enemy behavior state tag in [`CHUNK_ENEMIES`].
pub const ENEMY_BEHAVIOR_ATTACK: u8 = 2;
/// Enemy behavior state tag in [`CHUNK_ENEMIES`].
pub const ENEMY_BEHAVIOR_RELOADING: u8 = 3;
/// Enemy behavior state tag in [`CHUNK_ENEMIES`].
pub const ENEMY_BEHAVIOR_DEAD: u8 = 4;

/// Match progression status tag in [`CHUNK_MODE`].
pub const MATCH_STATUS_ACTIVE: u8 = 0;
/// Match progression status tag in [`CHUNK_MODE`].
pub const MATCH_STATUS_VICTORY: u8 = 1;
/// Match progression status tag in [`CHUNK_MODE`].
pub const MATCH_STATUS_DEFEAT: u8 = 2;

/// Pickup kind tag in [`CHUNK_PICKUPS`]: Health restore item.
pub const PICKUP_KIND_HEALTH: u8 = 1;
/// Pickup kind tag in [`CHUNK_PICKUPS`]: Ammunition reserve item.
pub const PICKUP_KIND_AMMO: u8 = 2;

/// Entity kind tag: Local player character (`CHUNK_WORLD_TRANSFORMS` / `CHUNK_ENTITY_METADATA`).
pub const ENTITY_KIND_PLAYER: u8 = 1;
/// Entity kind tag: AI enemy actor.
pub const ENTITY_KIND_ENEMY: u8 = 2;
/// Entity kind tag: Dynamic movable rigid body prop.
pub const ENTITY_KIND_DYNAMIC: u8 = 3;
/// Entity kind tag: Static scenery / cover collider.
pub const ENTITY_KIND_STATIC: u8 = 4;
/// Entity kind tag: Shootable combat target dummy.
pub const ENTITY_KIND_TARGET: u8 = 5;
/// Entity kind tag: Collectible health or ammo pickup.
pub const ENTITY_KIND_PICKUP: u8 = 6;
/// Entity kind tag: Non-blocking trigger zone volume.
pub const ENTITY_KIND_TRIGGER: u8 = 7;

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
        self.buffer
            .extend_from_slice(&PROTOCOL_VERSION.to_le_bytes());
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

        let magic = u32::from_le_bytes([buffer[0], buffer[1], buffer[2], buffer[3]]);
        if magic != MAGIC {
            return Err(ProtocolError::InvalidMagic(magic));
        }

        let version = u16::from_le_bytes([buffer[4], buffer[5]]);
        if version != PROTOCOL_VERSION {
            return Err(ProtocolError::UnsupportedVersion(version));
        }

        let chunk_count = u16::from_le_bytes([buffer[6], buffer[7]]);
        let tick_index = u64::from_le_bytes([
            buffer[8], buffer[9], buffer[10], buffer[11], buffer[12], buffer[13], buffer[14],
            buffer[15],
        ]);

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

        let chunk_type = u16::from_le_bytes([self.slice[0], self.slice[1]]);
        let chunk_flags = u16::from_le_bytes([self.slice[2], self.slice[3]]);
        let chunk_len =
            u32::from_le_bytes([self.slice[4], self.slice[5], self.slice[6], self.slice[7]])
                as usize;

        let total_chunk_size = 8usize.saturating_add(chunk_len);
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

/// Full 3D transform entry encoded in [`CHUNK_WORLD_TRANSFORMS`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WorldTransformRecord {
    /// Application-assigned numeric ID (paired with `entity_kind`).
    pub entity_id: u32,
    /// Kind tag such as [`ENTITY_KIND_PLAYER`] or [`ENTITY_KIND_DYNAMIC`].
    pub entity_kind: u8,
    /// World-space `[x, y, z]` coordinates in meters.
    pub position: [f32; 3],
    /// Unit orientation quaternion `[x, y, z, w]`.
    pub rotation: [f32; 4],
}

/// Entity identity record encoded in [`CHUNK_ENTITY_METADATA`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntityMetadataRecord {
    /// Application-assigned numeric ID (paired with `entity_kind`).
    pub entity_id: u32,
    /// Kind tag matching the corresponding [`WorldTransformRecord`].
    pub entity_kind: u8,
    /// Authored object/model identifier string (e.g., `"crate_wooden"`, `"boss_guard"`).
    pub name: String,
}

/// Serializes `records` into a [`CHUNK_WORLD_TRANSFORMS`] binary payload.
pub fn encode_world_transforms(records: &[WorldTransformRecord]) -> Vec<u8> {
    let mut buf = Vec::with_capacity(4 + records.len() * 33);
    buf.extend_from_slice(&(records.len() as u32).to_le_bytes());
    for rec in records {
        buf.extend_from_slice(&rec.entity_id.to_le_bytes());
        buf.push(rec.entity_kind);
        for v in rec.position {
            buf.extend_from_slice(&v.to_le_bytes());
        }
        for q in rec.rotation {
            buf.extend_from_slice(&q.to_le_bytes());
        }
    }
    buf
}

/// Decodes a [`CHUNK_WORLD_TRANSFORMS`] payload safely without panicking on truncated input.
pub fn decode_world_transforms(payload: &[u8]) -> Result<Vec<WorldTransformRecord>, ProtocolError> {
    if payload.len() < 4 {
        return Err(ProtocolError::CorruptedChunk {
            chunk_type: CHUNK_WORLD_TRANSFORMS,
            expected_bytes: 4,
            remaining_bytes: payload.len(),
        });
    }
    let count = u32::from_le_bytes([payload[0], payload[1], payload[2], payload[3]]) as usize;
    let record_bytes = 33usize;
    let needed = 4usize.saturating_add(count.saturating_mul(record_bytes));
    if payload.len() < needed {
        return Err(ProtocolError::CorruptedChunk {
            chunk_type: CHUNK_WORLD_TRANSFORMS,
            expected_bytes: needed,
            remaining_bytes: payload.len(),
        });
    }

    let mut out = Vec::with_capacity(count);
    let mut offset = 4;
    for _ in 0..count {
        let entity_id = u32::from_le_bytes([
            payload[offset],
            payload[offset + 1],
            payload[offset + 2],
            payload[offset + 3],
        ]);
        let entity_kind = payload[offset + 4];
        let read_f32 = |pos: usize| {
            f32::from_le_bytes([
                payload[pos],
                payload[pos + 1],
                payload[pos + 2],
                payload[pos + 3],
            ])
        };
        let position = [
            read_f32(offset + 5),
            read_f32(offset + 9),
            read_f32(offset + 13),
        ];
        let rotation = [
            read_f32(offset + 17),
            read_f32(offset + 21),
            read_f32(offset + 25),
            read_f32(offset + 29),
        ];
        out.push(WorldTransformRecord {
            entity_id,
            entity_kind,
            position,
            rotation,
        });
        offset += record_bytes;
    }
    Ok(out)
}

/// Serializes `records` into a [`CHUNK_ENTITY_METADATA`] binary payload.
pub fn encode_entity_metadata(records: &[EntityMetadataRecord]) -> Vec<u8> {
    let mut buf = Vec::with_capacity(4 + records.len() * 16);
    buf.extend_from_slice(&(records.len() as u32).to_le_bytes());
    for rec in records {
        buf.extend_from_slice(&rec.entity_id.to_le_bytes());
        buf.push(rec.entity_kind);
        let bytes = rec.name.as_bytes();
        let len = bytes.len().min(u16::MAX as usize);
        buf.extend_from_slice(&(len as u16).to_le_bytes());
        buf.extend_from_slice(&bytes[..len]);
    }
    buf
}

/// Decodes a [`CHUNK_ENTITY_METADATA`] payload safely without panicking on truncated input.
pub fn decode_entity_metadata(payload: &[u8]) -> Result<Vec<EntityMetadataRecord>, ProtocolError> {
    if payload.len() < 4 {
        return Err(ProtocolError::CorruptedChunk {
            chunk_type: CHUNK_ENTITY_METADATA,
            expected_bytes: 4,
            remaining_bytes: payload.len(),
        });
    }
    let count = u32::from_le_bytes([payload[0], payload[1], payload[2], payload[3]]) as usize;
    let mut offset = 4usize;
    let mut out = Vec::with_capacity(count.min(payload.len().saturating_sub(4) / 7));

    for _ in 0..count {
        if payload.len() < offset + 7 {
            return Err(ProtocolError::CorruptedChunk {
                chunk_type: CHUNK_ENTITY_METADATA,
                expected_bytes: offset + 7,
                remaining_bytes: payload.len(),
            });
        }
        let entity_id = u32::from_le_bytes([
            payload[offset],
            payload[offset + 1],
            payload[offset + 2],
            payload[offset + 3],
        ]);
        let entity_kind = payload[offset + 4];
        let name_len = u16::from_le_bytes([payload[offset + 5], payload[offset + 6]]) as usize;
        offset += 7;
        if payload.len() < offset + name_len {
            return Err(ProtocolError::CorruptedChunk {
                chunk_type: CHUNK_ENTITY_METADATA,
                expected_bytes: offset + name_len,
                remaining_bytes: payload.len(),
            });
        }
        let name = std::str::from_utf8(&payload[offset..offset + name_len])
            .map_err(|_| ProtocolError::CorruptedChunk {
                chunk_type: CHUNK_ENTITY_METADATA,
                expected_bytes: offset + name_len,
                remaining_bytes: payload.len(),
            })?
            .to_owned();
        offset += name_len;
        out.push(EntityMetadataRecord {
            entity_id,
            entity_kind,
            name,
        });
    }
    Ok(out)
}

// ============================================================================
// Shared Checked Payload Reader / Writer Helpers
// ============================================================================

fn push_len_prefixed_str(buf: &mut Vec<u8>, value: &str) {
    let bytes = value.as_bytes();
    let len = bytes.len().min(u16::MAX as usize);
    buf.extend_from_slice(&(len as u16).to_le_bytes());
    buf.extend_from_slice(&bytes[..len]);
}

fn push_vec3(buf: &mut Vec<u8>, v: [f32; 3]) {
    for component in v {
        buf.extend_from_slice(&component.to_le_bytes());
    }
}

struct PayloadCursor<'a> {
    chunk_type: u16,
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> PayloadCursor<'a> {
    fn new(chunk_type: u16, bytes: &'a [u8]) -> Self {
        Self {
            chunk_type,
            bytes,
            offset: 0,
        }
    }

    fn take(&mut self, len: usize) -> Result<&'a [u8], ProtocolError> {
        let end = self.offset.saturating_add(len);
        if end > self.bytes.len() {
            return Err(ProtocolError::CorruptedChunk {
                chunk_type: self.chunk_type,
                expected_bytes: end,
                remaining_bytes: self.bytes.len(),
            });
        }
        let slice = &self.bytes[self.offset..end];
        self.offset = end;
        Ok(slice)
    }

    fn read_u8(&mut self) -> Result<u8, ProtocolError> {
        Ok(self.take(1)?[0])
    }

    fn read_u16(&mut self) -> Result<u16, ProtocolError> {
        let s = self.take(2)?;
        Ok(u16::from_le_bytes([s[0], s[1]]))
    }

    fn read_u32(&mut self) -> Result<u32, ProtocolError> {
        let s = self.take(4)?;
        Ok(u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
    }

    fn read_u64(&mut self) -> Result<u64, ProtocolError> {
        let s = self.take(8)?;
        Ok(u64::from_le_bytes([
            s[0], s[1], s[2], s[3], s[4], s[5], s[6], s[7],
        ]))
    }

    fn read_f32(&mut self) -> Result<f32, ProtocolError> {
        let s = self.take(4)?;
        Ok(f32::from_le_bytes([s[0], s[1], s[2], s[3]]))
    }

    fn read_vec3(&mut self) -> Result<[f32; 3], ProtocolError> {
        Ok([self.read_f32()?, self.read_f32()?, self.read_f32()?])
    }

    fn read_string(&mut self) -> Result<String, ProtocolError> {
        let len = self.read_u16()? as usize;
        let start = self.offset;
        let bytes = self.take(len)?;
        std::str::from_utf8(bytes)
            .map(str::to_owned)
            .map_err(|_| ProtocolError::CorruptedChunk {
                chunk_type: self.chunk_type,
                expected_bytes: start.saturating_add(len),
                remaining_bytes: self.bytes.len(),
            })
    }
}

// ============================================================================
// Extended Snapshot Records & Codecs (Chunks 0x0007 .. 0x000C)
// ============================================================================

/// Visible enemy aim/telegraph ray encoded inside [`CHUNK_ENEMIES`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EnemyAimRayRecord {
    pub origin: [f32; 3],
    pub end_point: [f32; 3],
    pub direction: [f32; 3],
    pub hits_player: bool,
    pub is_firing_soon: bool,
}

/// Enemy combat vitals, weapon, and telegraph state encoded in [`CHUNK_ENEMIES`].
#[derive(Debug, Clone, PartialEq)]
pub struct EnemyCombatRecord {
    pub entity_id: u32,
    pub health: f32,
    pub max_health: f32,
    pub is_alive: bool,
    pub behavior: u8,
    pub capsule_radius: f32,
    pub capsule_height: f32,
    pub active_weapon_id: u32,
    pub ammo: u32,
    pub reserve_ammo: u32,
    pub is_reloading: bool,
    pub active_weapon_name: String,
    pub aim_ray: Option<EnemyAimRayRecord>,
}

/// Serializes `records` into a [`CHUNK_ENEMIES`] payload.
pub fn encode_enemies(records: &[EnemyCombatRecord]) -> Vec<u8> {
    let mut buf = Vec::with_capacity(4 + records.len() * 88);
    buf.extend_from_slice(&(records.len() as u32).to_le_bytes());
    for rec in records {
        buf.extend_from_slice(&rec.entity_id.to_le_bytes());
        buf.extend_from_slice(&rec.health.to_le_bytes());
        buf.extend_from_slice(&rec.max_health.to_le_bytes());
        buf.push(u8::from(rec.is_alive));
        buf.push(rec.behavior);
        buf.extend_from_slice(&rec.capsule_radius.to_le_bytes());
        buf.extend_from_slice(&rec.capsule_height.to_le_bytes());
        buf.extend_from_slice(&rec.active_weapon_id.to_le_bytes());
        buf.extend_from_slice(&rec.ammo.to_le_bytes());
        buf.extend_from_slice(&rec.reserve_ammo.to_le_bytes());
        buf.push(u8::from(rec.is_reloading));
        push_len_prefixed_str(&mut buf, &rec.active_weapon_name);
        match &rec.aim_ray {
            Some(ray) => {
                buf.push(1);
                push_vec3(&mut buf, ray.origin);
                push_vec3(&mut buf, ray.end_point);
                push_vec3(&mut buf, ray.direction);
                buf.push(u8::from(ray.hits_player));
                buf.push(u8::from(ray.is_firing_soon));
            }
            None => buf.push(0),
        }
    }
    buf
}

/// Decodes a [`CHUNK_ENEMIES`] payload safely without panicking on truncated input.
pub fn decode_enemies(payload: &[u8]) -> Result<Vec<EnemyCombatRecord>, ProtocolError> {
    let mut cursor = PayloadCursor::new(CHUNK_ENEMIES, payload);
    let count = cursor.read_u32()? as usize;
    let mut out = Vec::with_capacity(count.min(payload.len() / 38));
    for _ in 0..count {
        let entity_id = cursor.read_u32()?;
        let health = cursor.read_f32()?;
        let max_health = cursor.read_f32()?;
        let is_alive = cursor.read_u8()? != 0;
        let behavior = cursor.read_u8()?;
        let capsule_radius = cursor.read_f32()?;
        let capsule_height = cursor.read_f32()?;
        let active_weapon_id = cursor.read_u32()?;
        let ammo = cursor.read_u32()?;
        let reserve_ammo = cursor.read_u32()?;
        let is_reloading = cursor.read_u8()? != 0;
        let active_weapon_name = cursor.read_string()?;
        let has_aim_ray = cursor.read_u8()? != 0;
        let aim_ray = if has_aim_ray {
            Some(EnemyAimRayRecord {
                origin: cursor.read_vec3()?,
                end_point: cursor.read_vec3()?,
                direction: cursor.read_vec3()?,
                hits_player: cursor.read_u8()? != 0,
                is_firing_soon: cursor.read_u8()? != 0,
            })
        } else {
            None
        };
        out.push(EnemyCombatRecord {
            entity_id,
            health,
            max_health,
            is_alive,
            behavior,
            capsule_radius,
            capsule_height,
            active_weapon_id,
            ammo,
            reserve_ammo,
            is_reloading,
            active_weapon_name,
            aim_ray,
        });
    }
    Ok(out)
}

/// Active projectile record encoded in [`CHUNK_PROJECTILES`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ProjectileRecord {
    pub id: u64,
    pub position: [f32; 3],
    pub velocity: [f32; 3],
    pub radius: f32,
    pub is_alive: bool,
}

/// Serializes `records` into a [`CHUNK_PROJECTILES`] payload.
pub fn encode_projectiles(records: &[ProjectileRecord]) -> Vec<u8> {
    let mut buf = Vec::with_capacity(4 + records.len() * 37);
    buf.extend_from_slice(&(records.len() as u32).to_le_bytes());
    for rec in records {
        buf.extend_from_slice(&rec.id.to_le_bytes());
        push_vec3(&mut buf, rec.position);
        push_vec3(&mut buf, rec.velocity);
        buf.extend_from_slice(&rec.radius.to_le_bytes());
        buf.push(u8::from(rec.is_alive));
    }
    buf
}

/// Decodes a [`CHUNK_PROJECTILES`] payload safely without panicking on truncated input.
pub fn decode_projectiles(payload: &[u8]) -> Result<Vec<ProjectileRecord>, ProtocolError> {
    let mut cursor = PayloadCursor::new(CHUNK_PROJECTILES, payload);
    let count = cursor.read_u32()? as usize;
    let mut out = Vec::with_capacity(count.min(payload.len() / 37));
    for _ in 0..count {
        out.push(ProjectileRecord {
            id: cursor.read_u64()?,
            position: cursor.read_vec3()?,
            velocity: cursor.read_vec3()?,
            radius: cursor.read_f32()?,
            is_alive: cursor.read_u8()? != 0,
        });
    }
    Ok(out)
}

/// Armed timed explosive record encoded in [`CHUNK_EXPLOSIVES`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ExplosiveRecord {
    pub id: u64,
    pub entity_id: u32,
    pub position: [f32; 3],
    pub radius: f32,
    pub fuse_ticks_remaining: u32,
    pub max_fuse_ticks: u32,
}

/// Serializes `records` into a [`CHUNK_EXPLOSIVES`] payload.
pub fn encode_explosives(records: &[ExplosiveRecord]) -> Vec<u8> {
    let mut buf = Vec::with_capacity(4 + records.len() * 36);
    buf.extend_from_slice(&(records.len() as u32).to_le_bytes());
    for rec in records {
        buf.extend_from_slice(&rec.id.to_le_bytes());
        buf.extend_from_slice(&rec.entity_id.to_le_bytes());
        push_vec3(&mut buf, rec.position);
        buf.extend_from_slice(&rec.radius.to_le_bytes());
        buf.extend_from_slice(&rec.fuse_ticks_remaining.to_le_bytes());
        buf.extend_from_slice(&rec.max_fuse_ticks.to_le_bytes());
    }
    buf
}

/// Decodes a [`CHUNK_EXPLOSIVES`] payload safely without panicking on truncated input.
pub fn decode_explosives(payload: &[u8]) -> Result<Vec<ExplosiveRecord>, ProtocolError> {
    let mut cursor = PayloadCursor::new(CHUNK_EXPLOSIVES, payload);
    let count = cursor.read_u32()? as usize;
    let mut out = Vec::with_capacity(count.min(payload.len() / 36));
    for _ in 0..count {
        out.push(ExplosiveRecord {
            id: cursor.read_u64()?,
            entity_id: cursor.read_u32()?,
            position: cursor.read_vec3()?,
            radius: cursor.read_f32()?,
            fuse_ticks_remaining: cursor.read_u32()?,
            max_fuse_ticks: cursor.read_u32()?,
        });
    }
    Ok(out)
}

/// Temporary surface impact blemish record encoded in [`CHUNK_BLEMISHES`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BlemishRecord {
    pub id: u64,
    /// Struck entity ID, or `0` for anonymous scenery.
    pub entity_id: u32,
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub radius: f32,
    pub remaining_ticks: u32,
    pub max_ticks: u32,
}

/// Serializes `records` into a [`CHUNK_BLEMISHES`] payload.
pub fn encode_blemishes(records: &[BlemishRecord]) -> Vec<u8> {
    let mut buf = Vec::with_capacity(4 + records.len() * 48);
    buf.extend_from_slice(&(records.len() as u32).to_le_bytes());
    for rec in records {
        buf.extend_from_slice(&rec.id.to_le_bytes());
        buf.extend_from_slice(&rec.entity_id.to_le_bytes());
        push_vec3(&mut buf, rec.position);
        push_vec3(&mut buf, rec.normal);
        buf.extend_from_slice(&rec.radius.to_le_bytes());
        buf.extend_from_slice(&rec.remaining_ticks.to_le_bytes());
        buf.extend_from_slice(&rec.max_ticks.to_le_bytes());
    }
    buf
}

/// Decodes a [`CHUNK_BLEMISHES`] payload safely without panicking on truncated input.
pub fn decode_blemishes(payload: &[u8]) -> Result<Vec<BlemishRecord>, ProtocolError> {
    let mut cursor = PayloadCursor::new(CHUNK_BLEMISHES, payload);
    let count = cursor.read_u32()? as usize;
    let mut out = Vec::with_capacity(count.min(payload.len() / 48));
    for _ in 0..count {
        out.push(BlemishRecord {
            id: cursor.read_u64()?,
            entity_id: cursor.read_u32()?,
            position: cursor.read_vec3()?,
            normal: cursor.read_vec3()?,
            radius: cursor.read_f32()?,
            remaining_ticks: cursor.read_u32()?,
            max_ticks: cursor.read_u32()?,
        });
    }
    Ok(out)
}

/// Story/mode objective progress record encoded inside [`CHUNK_MODE`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectiveRecord {
    pub id: String,
    pub description: String,
    pub completed: bool,
    pub progress: u32,
    pub target: u32,
}

/// Match & mode HUD state encoded in [`CHUNK_MODE`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModeRecord {
    pub mode_name: String,
    pub status: u8,
    pub outcome_reason: Option<String>,
    pub current_wave: u32,
    pub total_waves: u32,
    pub breather_ticks_remaining: u32,
    pub score: u32,
    pub enemies_eliminated: u32,
    pub time_remaining_ticks: Option<u64>,
    pub objectives: Vec<ObjectiveRecord>,
}

/// Serializes `record` into a [`CHUNK_MODE`] payload.
pub fn encode_mode(record: &ModeRecord) -> Vec<u8> {
    let mut buf = Vec::with_capacity(64 + record.objectives.len() * 32);
    push_len_prefixed_str(&mut buf, &record.mode_name);
    buf.push(record.status);
    match &record.outcome_reason {
        Some(reason) => {
            buf.push(1);
            push_len_prefixed_str(&mut buf, reason);
        }
        None => buf.push(0),
    }
    buf.extend_from_slice(&record.current_wave.to_le_bytes());
    buf.extend_from_slice(&record.total_waves.to_le_bytes());
    buf.extend_from_slice(&record.breather_ticks_remaining.to_le_bytes());
    buf.extend_from_slice(&record.score.to_le_bytes());
    buf.extend_from_slice(&record.enemies_eliminated.to_le_bytes());
    match record.time_remaining_ticks {
        Some(ticks) => {
            buf.push(1);
            buf.extend_from_slice(&ticks.to_le_bytes());
        }
        None => buf.push(0),
    }
    buf.extend_from_slice(&(record.objectives.len() as u32).to_le_bytes());
    for obj in &record.objectives {
        push_len_prefixed_str(&mut buf, &obj.id);
        push_len_prefixed_str(&mut buf, &obj.description);
        buf.push(u8::from(obj.completed));
        buf.extend_from_slice(&obj.progress.to_le_bytes());
        buf.extend_from_slice(&obj.target.to_le_bytes());
    }
    buf
}

/// Decodes a [`CHUNK_MODE`] payload safely without panicking on truncated input.
pub fn decode_mode(payload: &[u8]) -> Result<ModeRecord, ProtocolError> {
    let mut cursor = PayloadCursor::new(CHUNK_MODE, payload);
    let mode_name = cursor.read_string()?;
    let status = cursor.read_u8()?;
    let has_reason = cursor.read_u8()? != 0;
    let outcome_reason = if has_reason {
        Some(cursor.read_string()?)
    } else {
        None
    };
    let current_wave = cursor.read_u32()?;
    let total_waves = cursor.read_u32()?;
    let breather_ticks_remaining = cursor.read_u32()?;
    let score = cursor.read_u32()?;
    let enemies_eliminated = cursor.read_u32()?;
    let has_time_limit = cursor.read_u8()? != 0;
    let time_remaining_ticks = if has_time_limit {
        Some(cursor.read_u64()?)
    } else {
        None
    };
    let obj_count = cursor.read_u32()? as usize;
    let mut objectives = Vec::with_capacity(obj_count.min(payload.len() / 13));
    for _ in 0..obj_count {
        objectives.push(ObjectiveRecord {
            id: cursor.read_string()?,
            description: cursor.read_string()?,
            completed: cursor.read_u8()? != 0,
            progress: cursor.read_u32()?,
            target: cursor.read_u32()?,
        });
    }
    Ok(ModeRecord {
        mode_name,
        status,
        outcome_reason,
        current_wave,
        total_waves,
        breather_ticks_remaining,
        score,
        enemies_eliminated,
        time_remaining_ticks,
        objectives,
    })
}

/// Collectible health/ammo pickup record encoded in [`CHUNK_PICKUPS`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PickupRecord {
    pub id: u64,
    pub position: [f32; 3],
    pub yaw: f32,
    pub radius: f32,
    pub is_available: bool,
    pub kind_tag: u8,
    pub health_amount: f32,
    pub weapon_id: u32,
    pub reserve_rounds: u32,
}

/// Serializes `records` into a [`CHUNK_PICKUPS`] payload.
pub fn encode_pickups(records: &[PickupRecord]) -> Vec<u8> {
    let mut buf = Vec::with_capacity(4 + records.len() * 42);
    buf.extend_from_slice(&(records.len() as u32).to_le_bytes());
    for rec in records {
        buf.extend_from_slice(&rec.id.to_le_bytes());
        push_vec3(&mut buf, rec.position);
        buf.extend_from_slice(&rec.yaw.to_le_bytes());
        buf.extend_from_slice(&rec.radius.to_le_bytes());
        buf.push(u8::from(rec.is_available));
        buf.push(rec.kind_tag);
        buf.extend_from_slice(&rec.health_amount.to_le_bytes());
        buf.extend_from_slice(&rec.weapon_id.to_le_bytes());
        buf.extend_from_slice(&rec.reserve_rounds.to_le_bytes());
    }
    buf
}

/// Decodes a [`CHUNK_PICKUPS`] payload safely without panicking on truncated input.
pub fn decode_pickups(payload: &[u8]) -> Result<Vec<PickupRecord>, ProtocolError> {
    let mut cursor = PayloadCursor::new(CHUNK_PICKUPS, payload);
    let count = cursor.read_u32()? as usize;
    let mut out = Vec::with_capacity(count.min(payload.len() / 42));
    for _ in 0..count {
        out.push(PickupRecord {
            id: cursor.read_u64()?,
            position: cursor.read_vec3()?,
            yaw: cursor.read_f32()?,
            radius: cursor.read_f32()?,
            is_available: cursor.read_u8()? != 0,
            kind_tag: cursor.read_u8()?,
            health_amount: cursor.read_f32()?,
            weapon_id: cursor.read_u32()?,
            reserve_rounds: cursor.read_u32()?,
        });
    }
    Ok(out)
}
