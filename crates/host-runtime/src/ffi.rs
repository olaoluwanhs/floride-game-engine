//! Universal C-ABI Foreign Function Interface (FFI) for WebAssembly and React Native.
//!
//! # What is this file for?
//! This module exports standard C-compatible functions (`extern "C"`) so that
//! outside environments—such as a Web Browser running WebAssembly (WASM) or a
//! mobile application running React Native (via JSI)—can control the game simulation.
//!
//! # Why do we use "Raw Pointers" and "Opaque Handles"?
//! Outside languages like JavaScript do not understand Rust types, traits, or generics.
//! Instead of exposing Rust structs directly, Rust allocates the session in memory and
//! hands the outside world a "coat-check ticket"—a raw memory address (`*mut FlorideSessionHandle`).
//!
//! Whenever the outside world wants to step the game or dispatch a player input, it
//! passes that ticket back into these functions. Rust safely casts the pointer back into
//! the live session and executes the command.

use std::slice;

use game_api::GameAdapter;

use crate::actions::ActionDecoder;
use crate::session::{HostError, HostSession, SnapshotEncoder};

// ============================================================================
// Standard Return Status Codes
// ============================================================================

/// Operation completed successfully.
pub const FLORIDE_OK: i32 = 0;

/// Error: A null or invalid memory pointer was provided by the host.
pub const FLORIDE_ERR_NULL_PTR: i32 = -1;

/// Error: An action could not be decoded or submitted to the game.
pub const FLORIDE_ERR_ACTION_FAILED: i32 = -2;

/// Error: Advancing the simulation tick encountered a failure.
pub const FLORIDE_ERR_TICK_FAILED: i32 = -3;

// ============================================================================
// Type Erasure: The AnySession Trait
// ============================================================================

/// An abstract interface allowing C functions to interact with any game session
/// without needing to know the game's specific generic parameters at compile time.
pub trait AnySession {
    /// Dispatches an action packet into the underlying game.
    fn dispatch_action(&mut self, action_id: u16, payload: &[u8]) -> Result<(), HostError>;

    /// Advances the simulation by one fixed tick and refreshes the snapshot buffer.
    fn tick(&mut self) -> Result<(), HostError>;

    /// Returns a direct byte slice of the serialized TLV snapshot buffer.
    fn snapshot_slice(&self) -> &[u8];

    /// Returns a raw memory pointer to the start of the snapshot buffer.
    fn snapshot_ptr(&self) -> *const u8;

    /// Returns the length in bytes of the current snapshot buffer.
    fn snapshot_len(&self) -> usize;
}

// Automatically implement `AnySession` for any concrete `HostSession`.
impl<G, D, E> AnySession for HostSession<G, D, E>
where
    G: GameAdapter,
    G::Error: std::fmt::Display,
    D: ActionDecoder<G::Input>,
    E: SnapshotEncoder<G::Snapshot, G::Event>,
{
    fn dispatch_action(&mut self, action_id: u16, payload: &[u8]) -> Result<(), HostError> {
        self.dispatch_action(action_id, payload)
    }

    fn tick(&mut self) -> Result<(), HostError> {
        self.tick()
    }

    fn snapshot_slice(&self) -> &[u8] {
        self.snapshot_slice()
    }

    fn snapshot_ptr(&self) -> *const u8 {
        self.snapshot_ptr()
    }

    fn snapshot_len(&self) -> usize {
        self.snapshot_len()
    }
}

/// An opaque wrapper around a boxed game session.
///
/// Outside hosts (JavaScript / C++) only see this as a pointer: `*mut FlorideSessionHandle`.
pub struct FlorideSessionHandle {
    inner: Box<dyn AnySession>,
}

impl FlorideSessionHandle {
    /// Wraps any game session into an opaque handle ready for foreign hosts.
    pub fn new<S: AnySession + 'static>(session: S) -> Self {
        Self {
            inner: Box::new(session),
        }
    }
}

// ============================================================================
// Exported C Functions (The Foreign Function Interface)
// ============================================================================

/// Safely frees and deallocates a host session.
///
/// # Parameters
/// - `handle`: The raw session pointer originally returned to the host.
///
/// # Safety
/// - If `handle` is non-null, it must point to a valid, live [`FlorideSessionHandle`]
///   previously allocated by Rust and not yet destroyed.
/// - Passing an already-freed pointer is undefined behavior (double-free).
///
/// # Explanation for Learners
/// In Rust, memory is automatically freed when a variable goes out of scope.
/// However, when we give a pointer to JavaScript, Rust temporarily gives up
/// ownership so the memory isn't deleted while JavaScript is using it.
///
/// When the game screen closes, JavaScript calls this function. `Box::from_raw`
/// takes ownership back, and Rust immediately deletes the session, preventing memory leaks.
#[no_mangle]
pub unsafe extern "C" fn floride_session_destroy(handle: *mut FlorideSessionHandle) {
    if handle.is_null() {
        return; // Guard against null pointers; do nothing if already empty.
    }
    // Taking ownership back into a Box will automatically deallocate it at the end of the block.
    drop(Box::from_raw(handle));
}

/// Dispatches an action packet (such as a movement or button press) into the game.
///
/// # Parameters
/// - `handle`: The pointer to the active game session.
/// - `action_id`: The numeric action identifier (e.g. `0x0001` for camera, `0x0010` for fire).
/// - `payload_ptr`: Pointer to the raw bytes of parameter data.
/// - `payload_len`: The number of bytes in the payload.
///
/// # Safety
/// - `handle` must either be null or point to a valid [`FlorideSessionHandle`].
/// - If `payload_len > 0`, `payload_ptr` must point to at least `payload_len` initialized, readable bytes.
///
/// # Returns
/// - `0` (`FLORIDE_OK`) if the action was accepted.
/// - Negative integer on failure (e.g. `FLORIDE_ERR_NULL_PTR` or `FLORIDE_ERR_ACTION_FAILED`).
#[no_mangle]
pub unsafe extern "C" fn floride_session_dispatch_action(
    handle: *mut FlorideSessionHandle,
    action_id: u16,
    payload_ptr: *const u8,
    payload_len: usize,
) -> i32 {
    // 1. Guard against null session pointer
    let session_ref = match handle.as_mut() {
        Some(s) => s,
        None => return FLORIDE_ERR_NULL_PTR,
    };

    // 2. Safely reconstruct the payload slice from pointer and length
    let payload_slice = if payload_len == 0 {
        &[][..]
    } else {
        if payload_ptr.is_null() {
            return FLORIDE_ERR_NULL_PTR;
        }
        slice::from_raw_parts(payload_ptr, payload_len)
    };

    // 3. Dispatch into the session
    match session_ref.inner.dispatch_action(action_id, payload_slice) {
        Ok(()) => FLORIDE_OK,
        Err(_) => FLORIDE_ERR_ACTION_FAILED,
    }
}

/// Advances the simulation by exactly one fixed time step (tick).
///
/// # Parameters
/// - `handle`: The pointer to the active game session.
///
/// # Safety
/// - `handle` must either be null or point to a valid [`FlorideSessionHandle`].
///
/// # Returns
/// - `0` (`FLORIDE_OK`) if the simulation stepped successfully.
/// - Negative integer on failure.
#[no_mangle]
pub unsafe extern "C" fn floride_session_tick(handle: *mut FlorideSessionHandle) -> i32 {
    let session_ref = match handle.as_mut() {
        Some(s) => s,
        None => return FLORIDE_ERR_NULL_PTR,
    };

    match session_ref.inner.tick() {
        Ok(()) => FLORIDE_OK,
        Err(_) => FLORIDE_ERR_TICK_FAILED,
    }
}

/// Returns a raw memory pointer to the beginning of the serialized TLV snapshot buffer.
///
/// # Safety
/// - `handle` must either be null or point to a valid [`FlorideSessionHandle`].
/// - The returned pointer is valid until the next call to [`floride_session_tick`] or [`floride_session_destroy`].
///
/// # Explanation for Learners
/// This enables **zero-copy reading**. Instead of copying coordinates and health values
/// across the boundary, JavaScript uses this pointer to create a direct view into Rust's
/// memory (`new DataView(wasm.memory.buffer, ptr, len)`).
#[no_mangle]
pub unsafe extern "C" fn floride_session_snapshot_ptr(
    handle: *const FlorideSessionHandle,
) -> *const u8 {
    match handle.as_ref() {
        Some(s) => s.inner.snapshot_ptr(),
        None => std::ptr::null(),
    }
}

/// Returns the total byte length of the current snapshot buffer.
///
/// # Safety
/// - `handle` must either be null or point to a valid [`FlorideSessionHandle`].
#[no_mangle]
pub unsafe extern "C" fn floride_session_snapshot_len(
    handle: *const FlorideSessionHandle,
) -> usize {
    match handle.as_ref() {
        Some(s) => s.inner.snapshot_len(),
        None => 0,
    }
}
