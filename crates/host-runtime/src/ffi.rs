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

use std::alloc::{alloc, dealloc, Layout};
use std::any::Any;
use std::cell::RefCell;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::slice;

use game_api::GameAdapter;

use crate::actions::ActionDecoder;
use crate::protocol::PROTOCOL_VERSION;
use crate::session::{HostError, HostSession, SnapshotEncoder};

// ============================================================================
// ABI Versioning & Standard Return Status Codes
// ============================================================================

/// Version of the `extern "C"` host ABI exported by `floride-host-runtime`.
pub const FLORIDE_ABI_VERSION: u32 = 1;

/// Operation completed successfully.
pub const FLORIDE_OK: i32 = 0;

/// Error: A null or invalid memory pointer was provided by the host.
pub const FLORIDE_ERR_NULL_PTR: i32 = -1;

/// Error: An action could not be decoded or submitted to the game.
pub const FLORIDE_ERR_ACTION_FAILED: i32 = -2;

/// Error: Advancing the simulation tick encountered a failure.
pub const FLORIDE_ERR_TICK_FAILED: i32 = -3;

/// Error: An invalid argument was supplied (for example, buffer capacity too small).
pub const FLORIDE_ERR_INVALID_ARGUMENT: i32 = -4;

/// Error: A Rust panic was caught at the FFI boundary and prevented from unwinding.
pub const FLORIDE_ERR_PANIC: i32 = -99;

// ============================================================================
// Thread-Local Last Error & Panic Guard Helpers
// ============================================================================

thread_local! {
    static LAST_ERROR: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
}

/// Records a UTF-8 error message for the current thread so foreign callers can inspect it.
pub fn set_last_error(message: impl AsRef<str>) {
    LAST_ERROR.with(|slot| {
        let mut buf = slot.borrow_mut();
        buf.clear();
        buf.extend_from_slice(message.as_ref().as_bytes());
    });
}

/// Clears any stored error message for the current thread.
pub fn clear_last_error() {
    LAST_ERROR.with(|slot| {
        slot.borrow_mut().clear();
    });
}

fn panic_payload_to_string(payload: Box<dyn Any + Send>) -> String {
    if let Some(msg) = payload.downcast_ref::<&str>() {
        format!("FFI panic caught: {msg}")
    } else if let Some(msg) = payload.downcast_ref::<String>() {
        format!("FFI panic caught: {msg}")
    } else {
        "FFI panic caught (non-string payload)".to_owned()
    }
}

/// Runs `f` inside `catch_unwind`, converting any Rust panic into [`FLORIDE_ERR_PANIC`]
/// and recording the panic message in the thread-local error slot.
pub fn ffi_guard_i32<F>(f: F) -> i32
where
    F: FnOnce() -> i32,
{
    match catch_unwind(AssertUnwindSafe(f)) {
        Ok(code) => code,
        Err(payload) => {
            set_last_error(panic_payload_to_string(payload));
            FLORIDE_ERR_PANIC
        }
    }
}

/// Runs a pointer-returning FFI closure inside `catch_unwind`, returning null on panic.
pub fn ffi_guard_ptr<T, F>(f: F) -> *const T
where
    F: FnOnce() -> *const T,
{
    match catch_unwind(AssertUnwindSafe(f)) {
        Ok(ptr) => ptr,
        Err(payload) => {
            set_last_error(panic_payload_to_string(payload));
            std::ptr::null()
        }
    }
}

/// Runs a mutable-pointer-returning FFI closure inside `catch_unwind`, returning null on panic.
pub fn ffi_guard_mut_ptr<T, F>(f: F) -> *mut T
where
    F: FnOnce() -> *mut T,
{
    match catch_unwind(AssertUnwindSafe(f)) {
        Ok(ptr) => ptr,
        Err(payload) => {
            set_last_error(panic_payload_to_string(payload));
            std::ptr::null_mut()
        }
    }
}

/// Runs a `usize`-returning FFI closure inside `catch_unwind`, returning `0` on panic.
pub fn ffi_guard_usize<F>(f: F) -> usize
where
    F: FnOnce() -> usize,
{
    match catch_unwind(AssertUnwindSafe(f)) {
        Ok(val) => val,
        Err(payload) => {
            set_last_error(panic_payload_to_string(payload));
            0
        }
    }
}

/// Safely copies `src` bytes into a caller-owned C buffer, always writing the required length
/// into `out_written` if non-null.
///
/// # Safety
/// - `out_written` must be null or point to writable `usize` memory.
/// - If `src.len() > 0` and `capacity >= src.len()`, `out_ptr` must be valid for `src.len()` writes.
pub unsafe fn copy_bytes_into_raw(
    src: &[u8],
    out_ptr: *mut u8,
    capacity: usize,
    out_written: *mut usize,
) -> i32 {
    if !out_written.is_null() {
        *out_written = src.len();
    }
    if src.is_empty() {
        return FLORIDE_OK;
    }
    if out_ptr.is_null() {
        set_last_error("output buffer pointer is null");
        return FLORIDE_ERR_NULL_PTR;
    }
    if capacity < src.len() {
        set_last_error(format!(
            "output buffer capacity ({capacity} bytes) smaller than required {} bytes",
            src.len()
        ));
        return FLORIDE_ERR_INVALID_ARGUMENT;
    }
    std::ptr::copy_nonoverlapping(src.as_ptr(), out_ptr, src.len());
    FLORIDE_OK
}

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

/// Returns the version of the Floride host C-ABI.
#[no_mangle]
pub extern "C" fn floride_abi_version() -> u32 {
    FLORIDE_ABI_VERSION
}

/// Returns the TLV snapshot protocol version produced by this runtime.
#[no_mangle]
pub extern "C" fn floride_protocol_version() -> u16 {
    PROTOCOL_VERSION
}

/// Returns a pointer to the last UTF-8 error message recorded on the calling thread,
/// or null if no error is stored.
///
/// The pointer remains valid until the next FFI call on the same thread.
#[no_mangle]
pub extern "C" fn floride_last_error_ptr() -> *const u8 {
    ffi_guard_ptr(|| {
        LAST_ERROR.with(|slot| {
            let buf = slot.borrow();
            if buf.is_empty() {
                std::ptr::null()
            } else {
                buf.as_ptr()
            }
        })
    })
}

/// Returns the byte length of the last UTF-8 error message recorded on the calling thread.
#[no_mangle]
pub extern "C" fn floride_last_error_len() -> usize {
    ffi_guard_usize(|| LAST_ERROR.with(|slot| slot.borrow().len()))
}

/// Copies the last UTF-8 error message into a caller-provided buffer.
///
/// # Safety
/// - `out_written` must be null or point to writable `usize` memory.
/// - If `out_ptr` is non-null, it must be writable for `capacity` bytes.
#[no_mangle]
pub unsafe extern "C" fn floride_copy_last_error(
    out_ptr: *mut u8,
    capacity: usize,
    out_written: *mut usize,
) -> i32 {
    ffi_guard_i32(|| {
        let msg = LAST_ERROR.with(|slot| slot.borrow().clone());
        copy_bytes_into_raw(&msg, out_ptr, capacity, out_written)
    })
}

/// Allocates `len` bytes inside Rust/WASM guest memory with 1-byte alignment.
///
/// Returns null if `len == 0` or allocation fails. Call [`floride_dealloc`] with the same
/// pointer and `len` once finished.
#[no_mangle]
pub extern "C" fn floride_alloc(len: usize) -> *mut u8 {
    ffi_guard_mut_ptr(|| {
        if len == 0 {
            return std::ptr::null_mut();
        }
        let Ok(layout) = Layout::from_size_align(len, 1) else {
            set_last_error("invalid allocation layout");
            return std::ptr::null_mut();
        };
        // Safety: `len > 0` and alignment is 1, satisfying `GlobalAlloc::alloc`.
        let ptr = unsafe { alloc(layout) };
        if ptr.is_null() {
            set_last_error("out of memory in floride_alloc");
        }
        ptr
    })
}

/// Frees memory previously returned by [`floride_alloc`].
///
/// # Safety
/// - `ptr` must either be null or have been returned by [`floride_alloc`] with the same `len`.
#[no_mangle]
pub unsafe extern "C" fn floride_dealloc(ptr: *mut u8, len: usize) {
    let _ = catch_unwind(AssertUnwindSafe(|| {
        if ptr.is_null() || len == 0 {
            return;
        }
        if let Ok(layout) = Layout::from_size_align(len, 1) {
            dealloc(ptr, layout);
        }
    }));
}

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
    let _ = catch_unwind(AssertUnwindSafe(|| {
        if handle.is_null() {
            return; // Guard against null pointers; do nothing if already empty.
        }
        // Taking ownership back into a Box will automatically deallocate it at the end of the block.
        drop(Box::from_raw(handle));
    }));
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
/// - Negative integer on failure (e.g. `FLORIDE_ERR_NULL_PTR`, `FLORIDE_ERR_ACTION_FAILED`, or `FLORIDE_ERR_PANIC`).
#[no_mangle]
pub unsafe extern "C" fn floride_session_dispatch_action(
    handle: *mut FlorideSessionHandle,
    action_id: u16,
    payload_ptr: *const u8,
    payload_len: usize,
) -> i32 {
    ffi_guard_i32(|| {
        clear_last_error();

        // 1. Guard against null session pointer
        let session_ref = match handle.as_mut() {
            Some(s) => s,
            None => {
                set_last_error("session handle pointer is null");
                return FLORIDE_ERR_NULL_PTR;
            }
        };

        // 2. Safely reconstruct the payload slice from pointer and length
        let payload_slice = if payload_len == 0 {
            &[][..]
        } else {
            if payload_ptr.is_null() {
                set_last_error("action payload pointer is null while payload_len > 0");
                return FLORIDE_ERR_NULL_PTR;
            }
            slice::from_raw_parts(payload_ptr, payload_len)
        };

        // 3. Dispatch into the session
        match session_ref.inner.dispatch_action(action_id, payload_slice) {
            Ok(()) => FLORIDE_OK,
            Err(err) => {
                set_last_error(err.to_string());
                FLORIDE_ERR_ACTION_FAILED
            }
        }
    })
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
    ffi_guard_i32(|| {
        clear_last_error();

        let session_ref = match handle.as_mut() {
            Some(s) => s,
            None => {
                set_last_error("session handle pointer is null");
                return FLORIDE_ERR_NULL_PTR;
            }
        };

        match session_ref.inner.tick() {
            Ok(()) => FLORIDE_OK,
            Err(err) => {
                set_last_error(err.to_string());
                FLORIDE_ERR_TICK_FAILED
            }
        }
    })
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
    ffi_guard_ptr(|| match handle.as_ref() {
        Some(s) => s.inner.snapshot_ptr(),
        None => std::ptr::null(),
    })
}

/// Returns the total byte length of the current snapshot buffer.
///
/// # Safety
/// - `handle` must either be null or point to a valid [`FlorideSessionHandle`].
#[no_mangle]
pub unsafe extern "C" fn floride_session_snapshot_len(
    handle: *const FlorideSessionHandle,
) -> usize {
    ffi_guard_usize(|| match handle.as_ref() {
        Some(s) => s.inner.snapshot_len(),
        None => 0,
    })
}

/// Copies the serialized TLV snapshot buffer into a caller-owned output buffer.
///
/// Always writes the required byte length into `*out_written` (if non-null), even if
/// `capacity` is smaller than needed (returning [`FLORIDE_ERR_INVALID_ARGUMENT`]).
///
/// # Safety
/// - `handle` must either be null or point to a valid [`FlorideSessionHandle`].
/// - `out_written` must be null or point to writable `usize` memory.
/// - If `out_ptr` is non-null, it must be writable for `capacity` bytes.
#[no_mangle]
pub unsafe extern "C" fn floride_session_copy_snapshot(
    handle: *const FlorideSessionHandle,
    out_ptr: *mut u8,
    capacity: usize,
    out_written: *mut usize,
) -> i32 {
    ffi_guard_i32(|| {
        clear_last_error();

        let session_ref = match handle.as_ref() {
            Some(s) => s,
            None => {
                set_last_error("session handle pointer is null");
                return FLORIDE_ERR_NULL_PTR;
            }
        };

        copy_bytes_into_raw(
            session_ref.inner.snapshot_slice(),
            out_ptr,
            capacity,
            out_written,
        )
    })
}
