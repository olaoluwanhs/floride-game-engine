use floride_host_runtime::{
    decode_blemishes, decode_enemies, decode_entity_metadata, decode_explosives, decode_mode,
    decode_pickups, decode_projectiles, decode_world_transforms, encode_blemishes, encode_enemies,
    encode_entity_metadata, encode_explosives, encode_mode, encode_pickups, encode_projectiles,
    encode_world_transforms, ActionDecoder, ActionError, ActionPacket, BlemishRecord,
    EnemyAimRayRecord, EnemyCombatRecord, EntityMetadataRecord, ExplosiveRecord, HostSession,
    ModeRecord, ObjectiveRecord, PickupRecord, ProjectileRecord, ProtocolError, SnapshotEncoder,
    TlvReader, TlvWriter, WorldTransformRecord, CHUNK_BLEMISHES, CHUNK_COMBAT, CHUNK_ENEMIES,
    CHUNK_ENTITY_METADATA, CHUNK_EVENTS, CHUNK_EXPLOSIVES, CHUNK_MODE, CHUNK_PICKUPS,
    CHUNK_PROJECTILES, CHUNK_TRANSFORMS, CHUNK_WORLD_TRANSFORMS, ENEMY_BEHAVIOR_ATTACK,
    ENTITY_KIND_DYNAMIC, ENTITY_KIND_ENEMY, ENTITY_KIND_STATIC, MAGIC, MATCH_STATUS_ACTIVE,
    PICKUP_KIND_AMMO, PICKUP_KIND_HEALTH,
};
use game_api::GameAdapter;
use game_types::Vec3;
use physics_api::{
    CharacterDesc, CharacterMoveResult, DynamicBodyDesc, PhantomColliderDesc, PhantomOverlap,
    PhantomRayHit, PhysicsError, PhysicsHandle, PhysicsTransform, PhysicsWorld, RayHit, RayQuery,
    StaticColliderDesc,
};

// ============================================================================
// 1. TLV Snapshot Protocol Tests
// ============================================================================

#[test]
fn test_header_serialization_and_validation() {
    let mut writer = TlvWriter::new();
    writer.start_snapshot(42);

    assert_eq!(writer.len(), 16);
    let slice = writer.as_slice();

    // Check magic bytes (0x464C4F52 = 'F', 'L', 'O', 'R')
    assert_eq!(&slice[0..4], b"FLOR");

    let reader = TlvReader::new(slice).expect("valid header must parse");
    assert_eq!(reader.tick_index(), 42);
    assert_eq!(reader.chunk_count(), 0);
}

#[test]
fn test_multi_chunk_composition_and_discovery() {
    let mut writer = TlvWriter::new();
    writer.start_snapshot(100);

    // Chunk 1: Transforms (entity 1 at (10.0, 20.0, 30.0))
    let mut transform_data = Vec::new();
    transform_data.extend_from_slice(&1u32.to_le_bytes()); // entity_id
    transform_data.extend_from_slice(&10.0f32.to_le_bytes()); // x
    transform_data.extend_from_slice(&20.0f32.to_le_bytes()); // y
    transform_data.extend_from_slice(&30.0f32.to_le_bytes()); // z
    writer.write_chunk(CHUNK_TRANSFORMS, 0, &transform_data);

    // Chunk 2: Combat (entity 1: hp 75.0, max_hp 100.0)
    let mut combat_data = Vec::new();
    combat_data.extend_from_slice(&1u32.to_le_bytes());
    combat_data.extend_from_slice(&75.0f32.to_le_bytes());
    combat_data.extend_from_slice(&100.0f32.to_le_bytes());
    writer.write_chunk(CHUNK_COMBAT, 0, &combat_data);

    let buffer = writer.as_slice();
    let reader = TlvReader::new(buffer).expect("valid stream");
    assert_eq!(reader.tick_index(), 100);
    assert_eq!(reader.chunk_count(), 2);

    // Query chunks
    let transforms = reader
        .find_chunk(CHUNK_TRANSFORMS)
        .expect("transforms chunk must exist")
        .expect("must parse cleanly");
    assert_eq!(transforms.chunk_type, CHUNK_TRANSFORMS);
    assert_eq!(transforms.payload, &transform_data[..]);

    let combat = reader
        .find_chunk(CHUNK_COMBAT)
        .expect("combat chunk must exist")
        .expect("must parse cleanly");
    assert_eq!(combat.chunk_type, CHUNK_COMBAT);
    assert_eq!(combat.payload, &combat_data[..]);

    // Query non-existent chunk
    assert!(reader.find_chunk(CHUNK_EVENTS).is_none());
}

#[test]
fn test_malformed_protocol_buffers() {
    // Buffer too short for header
    assert_eq!(
        TlvReader::new(&[0u8; 10]),
        Err(ProtocolError::BufferTooShort)
    );

    // Bad magic
    let bad_magic = [0u8; 16];
    assert_eq!(
        TlvReader::new(&bad_magic),
        Err(ProtocolError::InvalidMagic(0))
    );

    // Unsupported version
    let mut bad_version = [0u8; 16];
    bad_version[0..4].copy_from_slice(&MAGIC.to_le_bytes());
    bad_version[4..6].copy_from_slice(&99u16.to_le_bytes());
    assert_eq!(
        TlvReader::new(&bad_version),
        Err(ProtocolError::UnsupportedVersion(99))
    );

    // Chunk length exceeds remaining buffer
    let mut writer = TlvWriter::new();
    writer.start_snapshot(1);
    writer.write_chunk(CHUNK_TRANSFORMS, 0, &[1, 2, 3, 4]);
    let mut truncated = writer.as_slice().to_vec();
    truncated.pop(); // Remove 1 byte

    let reader = TlvReader::new(&truncated).unwrap();
    let mut iter = reader.iter_chunks();
    assert!(matches!(
        iter.next(),
        Some(Err(ProtocolError::CorruptedChunk { .. }))
    ));
}

// ============================================================================
// 2. Action Packet Envelope Tests
// ============================================================================

#[test]
fn test_action_packet_parsing() {
    let payload = [0xAA, 0xBB, 0xCC];
    let mut raw = Vec::new();
    raw.extend_from_slice(&0x0001u16.to_le_bytes()); // action_id
    raw.extend_from_slice(&(payload.len() as u16).to_le_bytes()); // payload_len
    raw.extend_from_slice(&payload);

    let packet = ActionPacket::from_bytes(&raw).expect("valid action");
    assert_eq!(packet.action_id, 0x0001);
    assert_eq!(packet.payload, &payload);

    // Buffer too short
    assert_eq!(
        ActionPacket::from_bytes(&[0x01, 0x00]),
        Err(ActionError::BufferTooShort)
    );

    // Length mismatch
    let mut truncated = raw.clone();
    truncated.pop();
    assert!(matches!(
        ActionPacket::from_bytes(&truncated),
        Err(ActionError::PayloadLengthMismatch { .. })
    ));
}

// ============================================================================
// 3. Full HostSession Integration with Mock GameAdapter
// ============================================================================

struct StubPhysics;

impl PhysicsWorld for StubPhysics {
    fn create_static_collider(
        &mut self,
        _desc: &StaticColliderDesc,
    ) -> Result<PhysicsHandle, PhysicsError> {
        Ok(PhysicsHandle::new(1))
    }
    fn remove_static_collider(&mut self, _handle: PhysicsHandle) -> Result<(), PhysicsError> {
        Ok(())
    }
    fn create_phantom_collider(
        &mut self,
        _desc: &PhantomColliderDesc,
    ) -> Result<PhysicsHandle, PhysicsError> {
        Ok(PhysicsHandle::new(2))
    }
    fn set_phantom_transform(
        &mut self,
        _handle: PhysicsHandle,
        _transform: &PhysicsTransform,
    ) -> Result<(), PhysicsError> {
        Ok(())
    }
    fn remove_phantom_collider(&mut self, _handle: PhysicsHandle) -> Result<(), PhysicsError> {
        Ok(())
    }
    fn phantom_overlaps(
        &self,
        _handle: PhysicsHandle,
    ) -> Result<Vec<PhantomOverlap>, PhysicsError> {
        Ok(Vec::new())
    }
    fn create_dynamic_body(
        &mut self,
        _desc: &DynamicBodyDesc,
    ) -> Result<PhysicsHandle, PhysicsError> {
        Ok(PhysicsHandle::new(3))
    }
    fn remove_dynamic_body(&mut self, _handle: PhysicsHandle) -> Result<(), PhysicsError> {
        Ok(())
    }
    fn apply_impulse(
        &mut self,
        _handle: PhysicsHandle,
        _impulse: &Vec3,
    ) -> Result<(), PhysicsError> {
        Ok(())
    }
    fn apply_impulse_at_point(
        &mut self,
        _handle: PhysicsHandle,
        _impulse: &Vec3,
        _point: &Vec3,
    ) -> Result<(), PhysicsError> {
        Ok(())
    }
    fn create_character(&mut self, _desc: &CharacterDesc) -> Result<PhysicsHandle, PhysicsError> {
        Ok(PhysicsHandle::new(4))
    }
    fn remove_character(&mut self, _handle: PhysicsHandle) -> Result<(), PhysicsError> {
        Ok(())
    }
    fn move_character(
        &mut self,
        _handle: PhysicsHandle,
        _displacement: &Vec3,
    ) -> Result<CharacterMoveResult, PhysicsError> {
        Ok(CharacterMoveResult {
            applied_displacement: Vec3::ZERO,
            is_grounded: true,
            collided: false,
        })
    }
    fn is_grounded(&self, _handle: PhysicsHandle) -> Result<bool, PhysicsError> {
        Ok(true)
    }
    fn raycast(&self, _query: &RayQuery) -> Result<Option<RayHit>, PhysicsError> {
        Ok(None)
    }
    fn raycast_phantoms(&self, _query: &RayQuery) -> Result<Vec<PhantomRayHit>, PhysicsError> {
        Ok(Vec::new())
    }
    fn step(&mut self, _dt: f32) -> Result<(), PhysicsError> {
        Ok(())
    }
    fn body_transform(&self, _handle: PhysicsHandle) -> Result<PhysicsTransform, PhysicsError> {
        Ok(PhysicsTransform::IDENTITY)
    }
}

enum MockInput {
    Look { yaw: f32, pitch: f32 },
    Fire,
}

struct MockGame {
    yaw: f32,
    pitch: f32,
    shots_fired: u32,
    pending_inputs: Vec<MockInput>,
}

impl GameAdapter for MockGame {
    type Physics = StubPhysics;
    type Config = ();
    type Input = MockInput;
    type Snapshot = (f32, f32, u32);
    type Event = String;
    type Error = String;

    fn create(_config: Self::Config, _physics: Self::Physics) -> Result<Self, Self::Error> {
        Ok(Self {
            yaw: 0.0,
            pitch: 0.0,
            shots_fired: 0,
            pending_inputs: Vec::new(),
        })
    }

    fn submit_input(&mut self, input: Self::Input) -> Result<(), Self::Error> {
        self.pending_inputs.push(input);
        Ok(())
    }

    fn tick(&mut self) -> Result<(), Self::Error> {
        for input in self.pending_inputs.drain(..) {
            match input {
                MockInput::Look { yaw, pitch } => {
                    self.yaw = yaw;
                    self.pitch = pitch;
                }
                MockInput::Fire => {
                    self.shots_fired += 1;
                }
            }
        }
        Ok(())
    }

    fn snapshot(&self) -> Self::Snapshot {
        (self.yaw, self.pitch, self.shots_fired)
    }

    fn drain_events(&mut self) -> Vec<Self::Event> {
        Vec::new()
    }
}

struct MockDecoder;
impl ActionDecoder<MockInput> for MockDecoder {
    fn decode_action(&self, packet: ActionPacket<'_>) -> Result<MockInput, ActionError> {
        match packet.action_id {
            0x0001 => {
                if packet.payload.len() != 8 {
                    return Err(ActionError::InvalidPayload("expected 8 bytes".into()));
                }
                let yaw = f32::from_le_bytes(packet.payload[0..4].try_into().unwrap());
                let pitch = f32::from_le_bytes(packet.payload[4..8].try_into().unwrap());
                Ok(MockInput::Look { yaw, pitch })
            }
            0x0010 => Ok(MockInput::Fire),
            id => Err(ActionError::UnknownActionId(id)),
        }
    }
}

struct MockEncoder;
impl SnapshotEncoder<(f32, f32, u32), String> for MockEncoder {
    fn encode_snapshot(
        &mut self,
        snapshot: &(f32, f32, u32),
        _events: &[String],
        writer: &mut TlvWriter,
    ) {
        let (yaw, pitch, shots) = *snapshot;
        let mut buf = Vec::new();
        buf.extend_from_slice(&yaw.to_le_bytes());
        buf.extend_from_slice(&pitch.to_le_bytes());
        buf.extend_from_slice(&shots.to_le_bytes());
        writer.write_chunk(CHUNK_TRANSFORMS, 0, &buf);
    }
}

#[test]
fn test_host_session_full_lifecycle() {
    let game = MockGame::create((), StubPhysics).unwrap();
    let mut session = HostSession::new(game, MockDecoder, MockEncoder);

    // Initial snapshot at tick 0
    assert_eq!(session.snapshot_slice().len(), 16 + 8 + 12);
    let reader = TlvReader::new(session.snapshot_slice()).unwrap();
    assert_eq!(reader.tick_index(), 0);

    // Dispatch look action (yaw: 1.57, pitch: -0.5)
    let mut look_payload = Vec::new();
    look_payload.extend_from_slice(&1.57f32.to_le_bytes());
    look_payload.extend_from_slice(&(-0.5f32).to_le_bytes());
    session
        .dispatch_action(0x0001, &look_payload)
        .expect("dispatch look");

    // Dispatch fire action
    session.dispatch_action(0x0010, &[]).expect("dispatch fire");

    // Tick simulation
    session.tick().expect("simulation tick");

    // Validate updated snapshot
    let reader = TlvReader::new(session.snapshot_slice()).unwrap();
    assert_eq!(reader.tick_index(), 1);
    let chunk = reader.find_chunk(CHUNK_TRANSFORMS).unwrap().unwrap();
    let yaw = f32::from_le_bytes(chunk.payload[0..4].try_into().unwrap());
    let pitch = f32::from_le_bytes(chunk.payload[4..8].try_into().unwrap());
    let shots = u32::from_le_bytes(chunk.payload[8..12].try_into().unwrap());

    assert!((yaw - 1.57).abs() < 1e-5);
    assert!((pitch - (-0.5)).abs() < 1e-5);
    assert_eq!(shots, 1);
}

#[test]
fn test_world_transforms_and_entity_metadata_chunks_roundtrip() {
    let transforms = vec![
        WorldTransformRecord {
            entity_id: 200,
            entity_kind: ENTITY_KIND_DYNAMIC,
            position: [1.5, 2.0, -3.5],
            rotation: [0.0, 0.38268343, 0.0, 0.9238795],
        },
        WorldTransformRecord {
            entity_id: 500,
            entity_kind: ENTITY_KIND_ENEMY,
            position: [-4.0, 1.0, 8.0],
            rotation: [0.0, 1.0, 0.0, 0.0],
        },
    ];
    let metadata = vec![
        EntityMetadataRecord {
            entity_id: 200,
            entity_kind: ENTITY_KIND_DYNAMIC,
            name: "wood_crate_custom".to_owned(),
        },
        EntityMetadataRecord {
            entity_id: 100,
            entity_kind: ENTITY_KIND_STATIC,
            name: "bunker_wall".to_owned(),
        },
    ];

    let mut writer = TlvWriter::new();
    writer.start_snapshot(77);
    writer.write_chunk(
        CHUNK_WORLD_TRANSFORMS,
        0,
        &encode_world_transforms(&transforms),
    );
    writer.write_chunk(CHUNK_ENTITY_METADATA, 0, &encode_entity_metadata(&metadata));

    let reader = TlvReader::new(writer.as_slice()).unwrap();
    let tf_chunk = reader.find_chunk(CHUNK_WORLD_TRANSFORMS).unwrap().unwrap();
    let md_chunk = reader.find_chunk(CHUNK_ENTITY_METADATA).unwrap().unwrap();
    assert_eq!(
        decode_world_transforms(tf_chunk.payload).unwrap(),
        transforms
    );
    assert_eq!(decode_entity_metadata(md_chunk.payload).unwrap(), metadata);

    // Truncated buffers must return CorruptedChunk error cleanly.
    assert!(decode_world_transforms(&[1, 0]).is_err());
    assert!(decode_entity_metadata(&tf_chunk.payload[..2]).is_err());
}

#[test]
fn test_extended_snapshot_chunks_roundtrip_and_truncation_safety() {
    let enemies = vec![
        EnemyCombatRecord {
            entity_id: 500,
            health: 75.0,
            max_health: 100.0,
            is_alive: true,
            behavior: ENEMY_BEHAVIOR_ATTACK,
            capsule_radius: 0.45,
            capsule_height: 1.8,
            active_weapon_id: 2,
            ammo: 1,
            reserve_ammo: 5,
            is_reloading: false,
            active_weapon_name: "Rocket Launcher".to_owned(),
            aim_ray: Some(EnemyAimRayRecord {
                origin: [1.0, 1.7, -10.0],
                end_point: [0.0, 1.2, 5.0],
                direction: [-0.06, -0.03, 0.99],
                hits_player: true,
                is_firing_soon: true,
            }),
        },
        EnemyCombatRecord {
            entity_id: 501,
            health: 0.0,
            max_health: 100.0,
            is_alive: false,
            behavior: 4,
            capsule_radius: 0.45,
            capsule_height: 1.8,
            active_weapon_id: 1,
            ammo: 0,
            reserve_ammo: 0,
            is_reloading: false,
            active_weapon_name: "Assault Rifle".to_owned(),
            aim_ray: None,
        },
    ];
    let projectiles = vec![ProjectileRecord {
        id: 11,
        position: [2.0, 1.5, -3.0],
        velocity: [0.0, 0.0, 30.0],
        radius: 0.15,
        is_alive: true,
    }];
    let explosives = vec![ExplosiveRecord {
        id: 22,
        entity_id: 100_022,
        position: [-1.0, 0.4, 2.0],
        radius: 0.18,
        fuse_ticks_remaining: 42,
        max_fuse_ticks: 150,
    }];
    let blemishes = vec![BlemishRecord {
        id: 33,
        entity_id: 100,
        position: [0.5, 1.1, -5.5],
        normal: [0.0, 0.0, 1.0],
        radius: 0.11,
        remaining_ticks: 240,
        max_ticks: 300,
    }];
    let mode = ModeRecord {
        mode_name: "Story".to_owned(),
        status: MATCH_STATUS_ACTIVE,
        outcome_reason: None,
        current_wave: 2,
        total_waves: 5,
        breather_ticks_remaining: 120,
        score: 450,
        enemies_eliminated: 6,
        time_remaining_ticks: Some(3600),
        objectives: vec![ObjectiveRecord {
            id: "extract".to_owned(),
            description: "Reach extraction zone".to_owned(),
            completed: false,
            progress: 0,
            target: 1,
        }],
    };
    let pickups = vec![
        PickupRecord {
            id: 1,
            position: [-6.0, 0.6, 0.0],
            yaw: 0.0,
            radius: 1.5,
            is_available: true,
            kind_tag: PICKUP_KIND_HEALTH,
            health_amount: 40.0,
            weapon_id: 0,
            reserve_rounds: 0,
        },
        PickupRecord {
            id: 2,
            position: [6.0, 0.6, 0.0],
            yaw: 1.5,
            radius: 1.5,
            is_available: false,
            kind_tag: PICKUP_KIND_AMMO,
            health_amount: 0.0,
            weapon_id: 2,
            reserve_rounds: 4,
        },
    ];

    let mut writer = TlvWriter::new();
    writer.start_snapshot(99);
    writer.write_chunk(CHUNK_ENEMIES, 0, &encode_enemies(&enemies));
    writer.write_chunk(CHUNK_PROJECTILES, 0, &encode_projectiles(&projectiles));
    writer.write_chunk(CHUNK_EXPLOSIVES, 0, &encode_explosives(&explosives));
    writer.write_chunk(CHUNK_BLEMISHES, 0, &encode_blemishes(&blemishes));
    writer.write_chunk(CHUNK_MODE, 0, &encode_mode(&mode));
    writer.write_chunk(CHUNK_PICKUPS, 0, &encode_pickups(&pickups));

    let reader = TlvReader::new(writer.as_slice()).unwrap();
    assert_eq!(reader.chunk_count(), 6);
    assert_eq!(
        decode_enemies(reader.find_chunk(CHUNK_ENEMIES).unwrap().unwrap().payload).unwrap(),
        enemies
    );
    assert_eq!(
        decode_projectiles(
            reader
                .find_chunk(CHUNK_PROJECTILES)
                .unwrap()
                .unwrap()
                .payload
        )
        .unwrap(),
        projectiles
    );
    assert_eq!(
        decode_explosives(
            reader
                .find_chunk(CHUNK_EXPLOSIVES)
                .unwrap()
                .unwrap()
                .payload
        )
        .unwrap(),
        explosives
    );
    assert_eq!(
        decode_blemishes(reader.find_chunk(CHUNK_BLEMISHES).unwrap().unwrap().payload).unwrap(),
        blemishes
    );
    assert_eq!(
        decode_mode(reader.find_chunk(CHUNK_MODE).unwrap().unwrap().payload).unwrap(),
        mode
    );
    assert_eq!(
        decode_pickups(reader.find_chunk(CHUNK_PICKUPS).unwrap().unwrap().payload).unwrap(),
        pickups
    );

    // Truncated payloads for each chunk must return Err without panicking.
    assert!(decode_enemies(&[1, 0, 0, 0, 1]).is_err());
    assert!(decode_projectiles(&[1, 0, 0, 0, 1]).is_err());
    assert!(decode_explosives(&[1, 0, 0, 0, 1]).is_err());
    assert!(decode_blemishes(&[1, 0, 0, 0, 1]).is_err());
    assert!(decode_mode(&[5, 0, b'a']).is_err());
    assert!(decode_pickups(&[1, 0, 0, 0, 1]).is_err());
}
