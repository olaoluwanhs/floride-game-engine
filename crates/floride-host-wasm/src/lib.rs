//! Generic WebAssembly (`wasm-bindgen`) host wrapper for any [`game_api::GameAdapter`]
//! bridged through [`floride_host_runtime::HostSession`].

use floride_host_runtime::ffi::{
    FLORIDE_ABI_VERSION, FLORIDE_ERR_ACTION_FAILED, FLORIDE_ERR_TICK_FAILED,
};
use floride_host_runtime::{
    ActionDecoder, AnySession, HostError, HostSession, SnapshotEncoder, PROTOCOL_VERSION,
};
use game_api::GameAdapter;
use wasm_bindgen::prelude::*;

/// Formats a [`HostError`] for browser/worker callers without losing context.
pub fn format_host_error(err: &HostError) -> String {
    err.to_string()
}

/// Generic WebAssembly session holding any type-erased [`AnySession`].
///
/// Game crates (such as `soldiers-of-fortune`) construct a concrete [`HostSession`],
/// wrap it with [`FlorideWasmSession::from_host_session`], and forward methods
/// to JavaScript / Web Workers.
#[wasm_bindgen]
pub struct FlorideWasmSession {
    inner: Box<dyn AnySession>,
}

impl FlorideWasmSession {
    /// Wraps any [`AnySession`] implementation into a WASM-compatible session.
    pub fn from_any_session<S: AnySession + 'static>(session: S) -> Self {
        Self {
            inner: Box::new(session),
        }
    }

    /// Wraps a concrete [`HostSession`] into a WASM-compatible session.
    pub fn from_host_session<G, D, E>(session: HostSession<G, D, E>) -> Self
    where
        G: GameAdapter + 'static,
        G::Error: std::fmt::Display,
        D: ActionDecoder<G::Input> + 'static,
        E: SnapshotEncoder<G::Snapshot, G::Event> + 'static,
    {
        Self::from_any_session(session)
    }

    /// Dispatches an action packet without creating a [`JsValue`] (unit-testable on native hosts).
    pub fn dispatch_action_raw(&mut self, action_id: u16, payload: &[u8]) -> Result<(), String> {
        self.inner
            .dispatch_action(action_id, payload)
            .map_err(|err| {
                format!(
                    "[code {FLORIDE_ERR_ACTION_FAILED}] {}",
                    format_host_error(&err)
                )
            })
    }

    /// Steps one simulation tick without creating a [`JsValue`] (unit-testable on native hosts).
    pub fn tick_raw(&mut self) -> Result<(), String> {
        self.inner.tick().map_err(|err| {
            format!(
                "[code {FLORIDE_ERR_TICK_FAILED}] {}",
                format_host_error(&err)
            )
        })
    }

    /// Returns a read-only Rust slice of the current serialized TLV snapshot buffer.
    pub fn snapshot_slice(&self) -> &[u8] {
        self.inner.snapshot_slice()
    }
}

#[wasm_bindgen]
impl FlorideWasmSession {
    /// Dispatches a binary action packet (`action_id` + raw payload bytes) into the game.
    #[wasm_bindgen(js_name = dispatchAction)]
    pub fn dispatch_action(&mut self, action_id: u16, payload: &[u8]) -> Result<(), JsValue> {
        self.dispatch_action_raw(action_id, payload)
            .map_err(|msg| JsValue::from_str(&msg))
    }

    /// Advances the simulation by one fixed simulation tick and updates the TLV snapshot buffer.
    pub fn tick(&mut self) -> Result<(), JsValue> {
        self.tick_raw().map_err(|msg| JsValue::from_str(&msg))
    }

    /// Returns the byte offset inside WASM linear memory pointing to the serialized TLV snapshot.
    ///
    /// Construct a temporary view via `new Uint8Array(wasm.memory.buffer, ptr, len)`.
    /// Always re-create the view after calling `tick()` or any WASM allocation that may grow memory.
    #[wasm_bindgen(js_name = snapshotPtr)]
    pub fn snapshot_ptr(&self) -> *const u8 {
        self.inner.snapshot_ptr()
    }

    /// Returns the byte length of the serialized TLV snapshot buffer.
    #[wasm_bindgen(js_name = snapshotLen)]
    pub fn snapshot_len(&self) -> usize {
        self.inner.snapshot_len()
    }

    /// Returns an owned copy of the serialized TLV snapshot buffer as a `Uint8Array`.
    ///
    /// Prefer [`snapshot_ptr`](Self::snapshot_ptr) + [`snapshot_len`](Self::snapshot_len)
    /// inside tight 60 Hz render loops to avoid extra allocations.
    #[wasm_bindgen(js_name = copySnapshot)]
    pub fn copy_snapshot(&self) -> Vec<u8> {
        self.inner.snapshot_slice().to_vec()
    }

    /// Returns the Floride host ABI version (`1`).
    #[wasm_bindgen(js_name = abiVersion)]
    pub fn abi_version() -> u32 {
        FLORIDE_ABI_VERSION
    }

    /// Returns the Floride TLV snapshot protocol version (`1`).
    #[wasm_bindgen(js_name = protocolVersion)]
    pub fn protocol_version() -> u16 {
        PROTOCOL_VERSION
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use floride_host_runtime::{ActionError, ActionPacket, TlvReader, TlvWriter, CHUNK_TRANSFORMS};
    use game_types::Vec3;
    use physics_api::{
        CharacterDesc, CharacterMoveResult, DynamicBodyDesc, PhantomColliderDesc, PhantomOverlap,
        PhantomRayHit, PhysicsError, PhysicsHandle, PhysicsTransform, PhysicsWorld, RayHit,
        RayQuery, StaticColliderDesc,
    };

    struct StubPhysics;
    impl PhysicsWorld for StubPhysics {
        fn create_static_collider(
            &mut self,
            _: &StaticColliderDesc,
        ) -> Result<PhysicsHandle, PhysicsError> {
            Ok(PhysicsHandle::new(1))
        }
        fn remove_static_collider(&mut self, _: PhysicsHandle) -> Result<(), PhysicsError> {
            Ok(())
        }
        fn create_phantom_collider(
            &mut self,
            _: &PhantomColliderDesc,
        ) -> Result<PhysicsHandle, PhysicsError> {
            Ok(PhysicsHandle::new(2))
        }
        fn set_phantom_transform(
            &mut self,
            _: PhysicsHandle,
            _: &PhysicsTransform,
        ) -> Result<(), PhysicsError> {
            Ok(())
        }
        fn remove_phantom_collider(&mut self, _: PhysicsHandle) -> Result<(), PhysicsError> {
            Ok(())
        }
        fn phantom_overlaps(&self, _: PhysicsHandle) -> Result<Vec<PhantomOverlap>, PhysicsError> {
            Ok(Vec::new())
        }
        fn create_dynamic_body(
            &mut self,
            _: &DynamicBodyDesc,
        ) -> Result<PhysicsHandle, PhysicsError> {
            Ok(PhysicsHandle::new(3))
        }
        fn remove_dynamic_body(&mut self, _: PhysicsHandle) -> Result<(), PhysicsError> {
            Ok(())
        }
        fn apply_impulse(&mut self, _: PhysicsHandle, _: &Vec3) -> Result<(), PhysicsError> {
            Ok(())
        }
        fn apply_impulse_at_point(
            &mut self,
            _: PhysicsHandle,
            _: &Vec3,
            _: &Vec3,
        ) -> Result<(), PhysicsError> {
            Ok(())
        }
        fn create_character(&mut self, _: &CharacterDesc) -> Result<PhysicsHandle, PhysicsError> {
            Ok(PhysicsHandle::new(4))
        }
        fn remove_character(&mut self, _: PhysicsHandle) -> Result<(), PhysicsError> {
            Ok(())
        }
        fn move_character(
            &mut self,
            _: PhysicsHandle,
            _: &Vec3,
        ) -> Result<CharacterMoveResult, PhysicsError> {
            Ok(CharacterMoveResult {
                applied_displacement: Vec3::ZERO,
                is_grounded: true,
                collided: false,
            })
        }
        fn is_grounded(&self, _: PhysicsHandle) -> Result<bool, PhysicsError> {
            Ok(true)
        }
        fn raycast(&self, _: &RayQuery) -> Result<Option<RayHit>, PhysicsError> {
            Ok(None)
        }
        fn raycast_phantoms(&self, _: &RayQuery) -> Result<Vec<PhantomRayHit>, PhysicsError> {
            Ok(Vec::new())
        }
        fn step(&mut self, _: f32) -> Result<(), PhysicsError> {
            Ok(())
        }
        fn body_transform(&self, _: PhysicsHandle) -> Result<PhysicsTransform, PhysicsError> {
            Ok(PhysicsTransform::IDENTITY)
        }
    }

    struct StubGame(u32);
    impl GameAdapter for StubGame {
        type Physics = StubPhysics;
        type Config = ();
        type Input = u32;
        type Snapshot = u32;
        type Event = ();
        type Error = String;

        fn create(_: (), _: StubPhysics) -> Result<Self, String> {
            Ok(Self(0))
        }
        fn submit_input(&mut self, v: u32) -> Result<(), String> {
            self.0 += v;
            Ok(())
        }
        fn tick(&mut self) -> Result<(), String> {
            self.0 += 1;
            Ok(())
        }
        fn snapshot(&self) -> u32 {
            self.0
        }
        fn drain_events(&mut self) -> Vec<()> {
            Vec::new()
        }
    }

    struct StubDecoder;
    impl ActionDecoder<u32> for StubDecoder {
        fn decode_action(&self, pkt: ActionPacket<'_>) -> Result<u32, ActionError> {
            match pkt.action_id {
                1 => Ok(10),
                id => Err(ActionError::UnknownActionId(id)),
            }
        }
    }

    struct StubEncoder;
    impl SnapshotEncoder<u32, ()> for StubEncoder {
        fn encode_snapshot(&mut self, snap: &u32, _: &[()], writer: &mut TlvWriter) {
            writer.write_chunk(CHUNK_TRANSFORMS, 0, &snap.to_le_bytes());
        }
    }

    #[test]
    fn wasm_session_wraps_host_session_and_exposes_tlv() {
        let game = StubGame::create((), StubPhysics).unwrap();
        let host = HostSession::new(game, StubDecoder, StubEncoder);
        let mut session = FlorideWasmSession::from_host_session(host);

        assert!(session.dispatch_action_raw(99, &[]).is_err());
        session.dispatch_action_raw(1, &[]).unwrap();
        session.tick_raw().unwrap();

        let bytes = session.copy_snapshot();
        assert_eq!(bytes.len(), session.snapshot_len());
        assert!(!session.snapshot_ptr().is_null());
        let reader = TlvReader::new(&bytes).unwrap();
        assert_eq!(reader.tick_index(), 1);
        let chunk = reader.find_chunk(CHUNK_TRANSFORMS).unwrap().unwrap();
        assert_eq!(u32::from_le_bytes(chunk.payload.try_into().unwrap()), 11);
    }
}
