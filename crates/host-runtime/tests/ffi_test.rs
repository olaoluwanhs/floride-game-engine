use floride_host_runtime::{
    floride_session_destroy, floride_session_dispatch_action, floride_session_snapshot_len,
    floride_session_snapshot_ptr, floride_session_tick, ActionDecoder, ActionError, ActionPacket,
    FlorideSessionHandle, HostSession, SnapshotEncoder, TlvReader, TlvWriter, CHUNK_TRANSFORMS,
    FLORIDE_ERR_ACTION_FAILED, FLORIDE_ERR_NULL_PTR, FLORIDE_OK,
};
use game_api::GameAdapter;
use game_types::Vec3;
use physics_api::{
    CharacterDesc, CharacterMoveResult, DynamicBodyDesc, PhantomColliderDesc, PhantomOverlap,
    PhantomRayHit, PhysicsError, PhysicsHandle, PhysicsTransform, PhysicsWorld, RayHit, RayQuery,
    StaticColliderDesc,
};

// ============================================================================
// Minimal Mock Setup for FFI Testing
// ============================================================================

struct StubPhysics;
impl PhysicsWorld for StubPhysics {
    fn create_static_collider(&mut self, _desc: &StaticColliderDesc) -> Result<PhysicsHandle, PhysicsError> {
        Ok(PhysicsHandle::new(1))
    }
    fn remove_static_collider(&mut self, _handle: PhysicsHandle) -> Result<(), PhysicsError> {
        Ok(())
    }
    fn create_phantom_collider(&mut self, _desc: &PhantomColliderDesc) -> Result<PhysicsHandle, PhysicsError> {
        Ok(PhysicsHandle::new(2))
    }
    fn set_phantom_transform(&mut self, _handle: PhysicsHandle, _transform: &PhysicsTransform) -> Result<(), PhysicsError> {
        Ok(())
    }
    fn remove_phantom_collider(&mut self, _handle: PhysicsHandle) -> Result<(), PhysicsError> {
        Ok(())
    }
    fn phantom_overlaps(&self, _handle: PhysicsHandle) -> Result<Vec<PhantomOverlap>, PhysicsError> {
        Ok(Vec::new())
    }
    fn create_dynamic_body(&mut self, _desc: &DynamicBodyDesc) -> Result<PhysicsHandle, PhysicsError> {
        Ok(PhysicsHandle::new(3))
    }
    fn remove_dynamic_body(&mut self, _handle: PhysicsHandle) -> Result<(), PhysicsError> {
        Ok(())
    }
    fn apply_impulse(&mut self, _handle: PhysicsHandle, _impulse: &Vec3) -> Result<(), PhysicsError> {
        Ok(())
    }
    fn apply_impulse_at_point(&mut self, _handle: PhysicsHandle, _impulse: &Vec3, _point: &Vec3) -> Result<(), PhysicsError> {
        Ok(())
    }
    fn create_character(&mut self, _desc: &CharacterDesc) -> Result<PhysicsHandle, PhysicsError> {
        Ok(PhysicsHandle::new(4))
    }
    fn remove_character(&mut self, _handle: PhysicsHandle) -> Result<(), PhysicsError> {
        Ok(())
    }
    fn move_character(&mut self, _handle: PhysicsHandle, _displacement: &Vec3) -> Result<CharacterMoveResult, PhysicsError> {
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
}

struct MockGame {
    yaw: f32,
    pitch: f32,
    pending_inputs: Vec<MockInput>,
}

impl GameAdapter for MockGame {
    type Physics = StubPhysics;
    type Config = ();
    type Input = MockInput;
    type Snapshot = (f32, f32);
    type Event = ();
    type Error = String;

    fn create(_config: Self::Config, _physics: Self::Physics) -> Result<Self, Self::Error> {
        Ok(Self {
            yaw: 0.0,
            pitch: 0.0,
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
            }
        }
        Ok(())
    }

    fn snapshot(&self) -> Self::Snapshot {
        (self.yaw, self.pitch)
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
            id => Err(ActionError::UnknownActionId(id)),
        }
    }
}

struct MockEncoder;
impl SnapshotEncoder<(f32, f32), ()> for MockEncoder {
    fn encode_snapshot(&mut self, snapshot: &(f32, f32), _events: &[()], writer: &mut TlvWriter) {
        let (yaw, pitch) = *snapshot;
        let mut buf = Vec::new();
        buf.extend_from_slice(&yaw.to_le_bytes());
        buf.extend_from_slice(&pitch.to_le_bytes());
        writer.write_chunk(CHUNK_TRANSFORMS, 0, &buf);
    }
}

fn create_raw_test_session() -> *mut FlorideSessionHandle {
    let game = MockGame::create((), StubPhysics).unwrap();
    let session = HostSession::new(game, MockDecoder, MockEncoder);
    let handle = FlorideSessionHandle::new(session);
    Box::into_raw(Box::new(handle))
}

// ============================================================================
// C-ABI Tests
// ============================================================================

#[test]
fn test_c_abi_null_pointer_safety() {
    unsafe {
        // Null handle checks
        assert_eq!(
            floride_session_dispatch_action(std::ptr::null_mut(), 1, std::ptr::null(), 0),
            FLORIDE_ERR_NULL_PTR
        );
        assert_eq!(
            floride_session_tick(std::ptr::null_mut()),
            FLORIDE_ERR_NULL_PTR
        );
        assert_eq!(floride_session_snapshot_ptr(std::ptr::null()), std::ptr::null());
        assert_eq!(floride_session_snapshot_len(std::ptr::null()), 0);

        // Destroying null must not panic
        floride_session_destroy(std::ptr::null_mut());
    }
}

#[test]
fn test_c_abi_full_lifecycle_and_zero_copy_read() {
    unsafe {
        let handle = create_raw_test_session();

        // Check initial snapshot pointer and length
        let ptr = floride_session_snapshot_ptr(handle);
        let len = floride_session_snapshot_len(handle);
        assert!(!ptr.is_null());
        assert!(len >= 16);

        // Read snapshot directly from the raw pointer (zero-copy)
        let initial_slice = std::slice::from_raw_parts(ptr, len);
        let reader = TlvReader::new(initial_slice).expect("valid snapshot");
        assert_eq!(reader.tick_index(), 0);

        // Dispatch camera look action via C FFI
        let mut look_data = Vec::new();
        look_data.extend_from_slice(&3.14159f32.to_le_bytes()); // yaw
        look_data.extend_from_slice(&0.785f32.to_le_bytes()); // pitch
        let status = floride_session_dispatch_action(
            handle,
            0x0001,
            look_data.as_ptr(),
            look_data.len(),
        );
        assert_eq!(status, FLORIDE_OK);

        // Advance simulation tick via C FFI
        let tick_status = floride_session_tick(handle);
        assert_eq!(tick_status, FLORIDE_OK);

        // Re-read snapshot buffer after tick
        let updated_ptr = floride_session_snapshot_ptr(handle);
        let updated_len = floride_session_snapshot_len(handle);
        let updated_slice = std::slice::from_raw_parts(updated_ptr, updated_len);
        let updated_reader = TlvReader::new(updated_slice).expect("valid updated snapshot");
        assert_eq!(updated_reader.tick_index(), 1);

        let chunk = updated_reader
            .find_chunk(CHUNK_TRANSFORMS)
            .expect("transforms chunk must exist")
            .expect("valid chunk");
        let yaw = f32::from_le_bytes(chunk.payload[0..4].try_into().unwrap());
        let pitch = f32::from_le_bytes(chunk.payload[4..8].try_into().unwrap());
        assert!((yaw - 3.14159).abs() < 1e-5);
        assert!((pitch - 0.785).abs() < 1e-5);

        // Clean up session via C FFI
        floride_session_destroy(handle);
    }
}

#[test]
fn test_c_abi_error_on_invalid_action() {
    unsafe {
        let handle = create_raw_test_session();

        // Dispatch unknown action ID 0x9999
        let status = floride_session_dispatch_action(handle, 0x9999, std::ptr::null(), 0);
        assert_eq!(status, FLORIDE_ERR_ACTION_FAILED);

        // Dispatch known action but with invalid payload length (expected 8, provided 4)
        let bad_payload = [0u8; 4];
        let bad_len_status = floride_session_dispatch_action(
            handle,
            0x0001,
            bad_payload.as_ptr(),
            bad_payload.len(),
        );
        assert_eq!(bad_len_status, FLORIDE_ERR_ACTION_FAILED);

        floride_session_destroy(handle);
    }
}
