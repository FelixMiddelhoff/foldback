// SPDX-License-Identifier: MIT OR Apache-2.0
//! `foldback-sys` — the C ABI surface (foldback-protocol-spec.md §3), for
//! every non-Rust binding (Unity/C#, Godot/GDExtension, Unreal/C++) to
//! build on. Level 1 (per-tick), Level 2 (per-entity), and Level 3
//! (per-field) hashing, matching `foldback_core::session::Session`'s own
//! three tiers — Level 2/3 landed after Level 1, the same way Phase 0
//! shipped Level 1 before Level 2/3 followed as their own slice at the
//! `foldback_core` layer.
//!
//! Every entry point is wrapped in `catch_unwind`: a Rust panic
//! unwinding across an `extern "C"` boundary into C/C++/C# calling code
//! is undefined behavior (protocol spec §3's hard rule), so panics are
//! caught here and turned into an error status instead.
//!
//! Deviation from the protocol spec's original code sketch, found while
//! actually building this (the same pattern every other Phase 2 slice
//! hit): the sketch showed `foldback_hash_tick` returning the computed
//! `uint64_t` hash directly, which conflicts with the spec's own prose
//! two lines later — "all fallible calls return a status code, never
//! throw/panic across the FFI boundary" — since a hash of exactly 0 is a
//! valid hash, not distinguishable from "no hash, call failed" if the
//! return value doubles as both. Every fallible call here returns a
//! status code instead, uniformly; `foldback.h`/the protocol spec are
//! the updated source of truth, not the original sketch.

use std::cell::RefCell;
use std::ffi::CStr;
use std::os::raw::c_char;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::PathBuf;
use std::ptr;

use foldback_core::session::Session;

thread_local! {
    /// The calling thread's most recent error message, if any. A game
    /// integration drives one `Handle` from one thread in every
    /// documented usage pattern (cookbook recipes 1-2, 8), so a
    /// thread-local — not a per-session field — is the simpler, still
    /// correct design; `foldback_last_error` below takes no session
    /// parameter for the same reason (a deviation from the protocol
    /// spec's original sketch, which the spec is updated to match).
    static LAST_ERROR: RefCell<Option<String>> = const { RefCell::new(None) };
}

fn set_last_error(msg: impl Into<String>) {
    LAST_ERROR.with(|e| *e.borrow_mut() = Some(msg.into()));
}

fn clear_last_error() {
    LAST_ERROR.with(|e| *e.borrow_mut() = None);
}

/// Status codes returned by every fallible `foldback_*` call.
#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Ok = 0,
    NullPointer = -1,
    InvalidUtf8 = -2,
    Io = -3,
    Panic = -4,
}

/// Configuration for [`foldback_session_create`], matching
/// [`foldback_core::session::SessionBuilder`]'s own fields exactly (one
/// struct in, no separate setter calls needed across the FFI boundary).
#[repr(C)]
pub struct Config {
    pub tick_rate_hz: u32,
    pub peer_count: u32,
    pub local_peer_id: u16,
    /// Ticks of ring-buffer retention. 0 means "use the library default"
    /// (600 — see `SessionBuilder::default`), since 0 itself is never a
    /// usable retention window.
    pub retention: u32,
    pub build_id: [u8; 16],
    /// Nullable, NUL-terminated UTF-8 path. Null means "don't record to
    /// a file" — matching `SessionBuilder::record_to` being optional.
    pub record_to_path: *const c_char,
}

/// Opaque handle — the C side never sees `Session`'s real layout, only
/// this pointer, so `Session`'s internal fields can change freely
/// without an ABI break.
pub struct Handle(Session);

fn run_catching<T>(default: T, f: impl FnOnce() -> Result<T, String>) -> T {
    match catch_unwind(AssertUnwindSafe(f)) {
        Ok(Ok(v)) => {
            clear_last_error();
            v
        }
        Ok(Err(msg)) => {
            set_last_error(msg);
            default
        }
        Err(payload) => {
            let msg = payload
                .downcast_ref::<&str>()
                .map(|s| s.to_string())
                .or_else(|| payload.downcast_ref::<String>().cloned())
                .unwrap_or_else(|| "panic in foldback-sys".to_string());
            set_last_error(msg);
            default
        }
    }
}

/// Creates a new session. Returns null on failure — check
/// [`foldback_last_error`] for why (a bad `record_to_path`, an I/O
/// error opening it, or invalid UTF-8 in the path).
///
/// # Safety
/// `config` must be a valid, non-null pointer to a fully-initialized
/// `Config`. `config->record_to_path`, if non-null, must be a
/// valid NUL-terminated C string.
#[no_mangle]
pub unsafe extern "C" fn foldback_session_create(config: *const Config) -> *mut Handle {
    run_catching(ptr::null_mut(), || {
        if config.is_null() {
            return Err("config was null".to_string());
        }
        let config = &*config;

        let record_to: Option<PathBuf> = if config.record_to_path.is_null() {
            None
        } else {
            let s = CStr::from_ptr(config.record_to_path)
                .to_str()
                .map_err(|e| format!("record_to_path was not valid UTF-8: {e}"))?;
            Some(PathBuf::from(s))
        };

        let mut builder = Session::builder()
            .tick_rate_hz(config.tick_rate_hz)
            .peer_count(config.peer_count)
            .local_peer_id(config.local_peer_id)
            .build_id(config.build_id);
        if config.retention != 0 {
            builder = builder.retention(config.retention as usize);
        }
        if let Some(path) = record_to {
            builder = builder.record_to(path);
        }

        let session = builder
            .build()
            .map_err(|e| format!("could not build session: {e}"))?;
        Ok(Box::into_raw(Box::new(Handle(session))))
    })
}

/// Destroys a session created by [`foldback_session_create`]. A no-op
/// (not an error) if `session` is null.
///
/// # Safety
/// `session` must either be null or a pointer previously returned by
/// [`foldback_session_create`] and not already destroyed.
#[no_mangle]
pub unsafe extern "C" fn foldback_session_destroy(session: *mut Handle) {
    let _ = catch_unwind(AssertUnwindSafe(|| {
        if !session.is_null() {
            drop(Box::from_raw(session));
        }
    }));
}

/// Hashes `state` and records it as this session's own report for
/// `tick` (cookbook recipe 1/8's `HashTick`).
///
/// # Safety
/// `session` must be a valid pointer from [`foldback_session_create`].
/// `state` must point to at least `len` readable bytes (or be null iff
/// `len` is 0).
#[no_mangle]
pub unsafe extern "C" fn foldback_hash_tick(
    session: *mut Handle,
    tick: u64,
    state: *const u8,
    len: usize,
) -> Status {
    run_catching(Status::Panic, || {
        let session = as_session_mut(session)?;
        let bytes = bytes_from_raw(state, len)?;
        session
            .0
            .hash_tick(tick, bytes)
            .map_err(|e| format!("hash_tick failed: {e}"))?;
        Ok(Status::Ok)
    })
}

/// Records a hash reported by any peer (including this session's own,
/// via [`foldback_hash_tick`]) into the cross-peer comparison table.
///
/// # Safety
/// `session` must be a valid pointer from [`foldback_session_create`].
#[no_mangle]
pub unsafe extern "C" fn foldback_record_peer_hash(
    session: *mut Handle,
    tick: u64,
    peer_id: u16,
    hash: u64,
) -> Status {
    run_catching(Status::Panic, || {
        let session = as_session_mut(session)?;
        session
            .0
            .record_peer_hash(tick, peer_id, hash)
            .map_err(|e| format!("record_peer_hash failed: {e}"))?;
        Ok(Status::Ok)
    })
}

/// Level 2: hashes one entity's state and records it as this session's
/// own report for `tick` (cookbook recipe 4). Recording-only — like
/// `foldback_core::Session::hash_entity` itself, this is a no-op beyond
/// the hash call unless the session was created with `record_to_path`
/// set; there's no in-memory Level 2 divergence tracking yet (matches
/// `Session::hash_entity`'s own documented scope).
///
/// # Safety
/// `session` must be a valid pointer from [`foldback_session_create`].
/// `state` must point to at least `len` readable bytes (or be null iff
/// `len` is 0).
#[no_mangle]
pub unsafe extern "C" fn foldback_hash_entity(
    session: *mut Handle,
    tick: u64,
    entity_id: u64,
    state: *const u8,
    len: usize,
) -> Status {
    run_catching(Status::Panic, || {
        let session = as_session_mut(session)?;
        let bytes = bytes_from_raw(state, len)?;
        session
            .0
            .hash_entity(tick, entity_id, bytes)
            .map_err(|e| format!("hash_entity failed: {e}"))?;
        Ok(Status::Ok)
    })
}

/// Records an entity hash reported by any peer (including this
/// session's own, via [`foldback_hash_entity`]) — the
/// `foldback_record_peer_hash` counterpart for Level 2.
///
/// # Safety
/// `session` must be a valid pointer from [`foldback_session_create`].
#[no_mangle]
pub unsafe extern "C" fn foldback_record_peer_entity_hash(
    session: *mut Handle,
    tick: u64,
    peer_id: u16,
    entity_id: u64,
    hash: u64,
) -> Status {
    run_catching(Status::Panic, || {
        let session = as_session_mut(session)?;
        session
            .0
            .record_peer_entity_hash(tick, peer_id, entity_id, hash)
            .map_err(|e| format!("record_peer_entity_hash failed: {e}"))?;
        Ok(Status::Ok)
    })
}

/// Level 3: hashes one field's value and records it as this session's
/// own report for `tick`, keeping the raw `value` bytes too (cookbook
/// recipe 5) — this is what lets the UI show "3.14159 vs 3.14158"
/// instead of just two unequal hashes. Recording-only, same caveat as
/// [`foldback_hash_entity`].
///
/// # Safety
/// `session` must be a valid pointer from [`foldback_session_create`].
/// `field_name` must be a valid NUL-terminated UTF-8 C string. `value`
/// must point to at least `value_len` readable bytes (or be null iff
/// `value_len` is 0).
#[no_mangle]
pub unsafe extern "C" fn foldback_hash_field(
    session: *mut Handle,
    tick: u64,
    entity_id: u64,
    field_name: *const c_char,
    value: *const u8,
    value_len: usize,
) -> Status {
    run_catching(Status::Panic, || {
        let session = as_session_mut(session)?;
        let field_name = str_from_raw(field_name, "field_name")?;
        let bytes = bytes_from_raw(value, value_len)?;
        session
            .0
            .hash_field(tick, entity_id, field_name, bytes)
            .map_err(|e| format!("hash_field failed: {e}"))?;
        Ok(Status::Ok)
    })
}

/// Records a field hash reported by any peer — the
/// `foldback_record_peer_hash` counterpart for Level 3.
///
/// # Safety
/// `session` must be a valid pointer from [`foldback_session_create`].
/// `field_name` must be a valid NUL-terminated UTF-8 C string. `value`
/// must point to at least `value_len` readable bytes (or be null iff
/// `value_len` is 0).
#[no_mangle]
pub unsafe extern "C" fn foldback_record_peer_field_hash(
    session: *mut Handle,
    tick: u64,
    peer_id: u16,
    entity_id: u64,
    field_name: *const c_char,
    hash: u64,
    value: *const u8,
    value_len: usize,
) -> Status {
    run_catching(Status::Panic, || {
        let session = as_session_mut(session)?;
        let field_name = str_from_raw(field_name, "field_name")?;
        let bytes = bytes_from_raw(value, value_len)?;
        session
            .0
            .record_peer_field_hash(tick, peer_id, entity_id, field_name, hash, bytes)
            .map_err(|e| format!("record_peer_field_hash failed: {e}"))?;
        Ok(Status::Ok)
    })
}

/// Records `type_name`'s current tagged field set as a `Metadata` frame —
/// the schema-drift detection mechanism
/// (foldback-reflective-hashing.md §7,
/// [`foldback_core::schema`]). A reflective walker binding (Unity's
/// `FoldbackReflection`, Godot's `hash_reflected`) calls this once per
/// tracked type it walks; a no-op past the first call for a given
/// `type_name` this session, same as `foldback_core::session::Session::
/// record_schema` itself.
///
/// # Safety
/// `session` must be a valid pointer from [`foldback_session_create`].
/// `type_name` must be a valid NUL-terminated UTF-8 C string.
/// `field_names` must point to at least `field_count` readable, non-null,
/// NUL-terminated UTF-8 C string pointers (or be null iff `field_count`
/// is 0).
#[no_mangle]
pub unsafe extern "C" fn foldback_record_schema(
    session: *mut Handle,
    type_name: *const c_char,
    field_names: *const *const c_char,
    field_count: usize,
) -> Status {
    run_catching(Status::Panic, || {
        let session = as_session_mut(session)?;
        let type_name = str_from_raw(type_name, "type_name")?;

        let mut fields = Vec::with_capacity(field_count);
        if field_count > 0 {
            if field_names.is_null() {
                return Err("field_names was null but field_count was nonzero".to_string());
            }
            let ptrs = std::slice::from_raw_parts(field_names, field_count);
            for &ptr in ptrs {
                fields.push(str_from_raw(ptr, "field_names entry")?);
            }
        }

        session
            .0
            .record_schema(type_name, &fields)
            .map_err(|e| format!("record_schema failed: {e}"))?;
        Ok(Status::Ok)
    })
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct PendingHash {
    pub tick: u64,
    pub hash: u64,
}

/// The number of hashes currently pending (produced locally since the
/// last [`foldback_take_pending_hashes`] call), without draining them —
/// lets a caller size its send buffer before draining.
///
/// # Safety
/// `session` must be a valid pointer from [`foldback_session_create`].
#[no_mangle]
pub unsafe extern "C" fn foldback_pending_hash_count(session: *const Handle) -> usize {
    run_catching(0, || {
        let session = as_session_ref(session)?;
        Ok(session.0.pending_hash_count())
    })
}

/// Drains up to `out_capacity` pending hashes into `out`, oldest first,
/// removing exactly the ones written — a partial drain (more pending
/// than `out_capacity`) leaves the remainder for the next call, matching
/// how a game's own netcode may cap how much it sends per packet.
/// Returns the number of entries actually written.
///
/// # Safety
/// `session` must be a valid pointer from [`foldback_session_create`].
/// `out` must point to at least `out_capacity` writable
/// `PendingHash` slots (or be null iff `out_capacity` is 0).
#[no_mangle]
pub unsafe extern "C" fn foldback_take_pending_hashes(
    session: *mut Handle,
    out: *mut PendingHash,
    out_capacity: usize,
) -> usize {
    run_catching(0, || {
        let session = as_session_mut(session)?;
        if out_capacity > 0 && out.is_null() {
            return Err("out was null but out_capacity was nonzero".to_string());
        }
        let drained = session.0.take_pending_hashes_up_to(out_capacity);
        for (i, ph) in drained.iter().enumerate() {
            *out.add(i) = PendingHash {
                tick: ph.tick,
                hash: ph.hash,
            };
        }
        Ok(drained.len())
    })
}

/// Checks for a new cross-peer divergence (cookbook recipe 1). Returns
/// `Status::Ok` with `*out_tick` set if one was found, or a
/// distinguishable "not found" via the separate `out_found` flag — kept
/// as two out-params rather than overloading the status code, since
/// "no divergence yet" is a normal outcome, not an error.
///
/// # Safety
/// `session`, `out_found`, and `out_tick` must be valid, non-null
/// pointers (`out_tick` need not be initialized).
#[no_mangle]
pub unsafe extern "C" fn foldback_check_divergence(
    session: *mut Handle,
    out_found: *mut bool,
    out_tick: *mut u64,
) -> Status {
    run_catching(Status::Panic, || {
        let session = as_session_mut(session)?;
        if out_found.is_null() || out_tick.is_null() {
            return Err("out_found or out_tick was null".to_string());
        }
        match session.0.check_divergence() {
            Some(d) => {
                *out_found = true;
                *out_tick = d.0;
            }
            None => {
                *out_found = false;
            }
        }
        Ok(Status::Ok)
    })
}

/// The combined hash over every tick this session has locally hashed
/// (cookbook's CI-gate pattern). Infallible given a valid session.
///
/// # Safety
/// `session` must be a valid pointer from [`foldback_session_create`].
#[no_mangle]
pub unsafe extern "C" fn foldback_finish(session: *const Handle) -> u64 {
    run_catching(0, || {
        let session = as_session_ref(session)?;
        Ok(session.0.finish())
    })
}

/// Writes the closing frame if recording to a file — call before
/// [`foldback_session_destroy`] when `record_to_path` was set, so a
/// reader can tell a clean recording from a truncated one.
///
/// # Safety
/// `session` must be a valid pointer from [`foldback_session_create`].
#[no_mangle]
pub unsafe extern "C" fn foldback_finish_recording(session: *mut Handle) -> Status {
    run_catching(Status::Panic, || {
        let session = as_session_mut(session)?;
        session
            .0
            .finish_recording()
            .map_err(|e| format!("finish_recording failed: {e}"))?;
        Ok(Status::Ok)
    })
}

/// # Safety
/// `session` must be a valid pointer from [`foldback_session_create`].
#[no_mangle]
pub unsafe extern "C" fn foldback_tick_rate_hz(session: *const Handle) -> u32 {
    run_catching(0, || {
        let session = as_session_ref(session)?;
        Ok(session.0.tick_rate_hz())
    })
}

/// # Safety
/// `session` must be a valid pointer from [`foldback_session_create`].
#[no_mangle]
pub unsafe extern "C" fn foldback_peer_count(session: *const Handle) -> u32 {
    run_catching(0, || {
        let session = as_session_ref(session)?;
        Ok(session.0.peer_count())
    })
}

/// Writes the calling thread's most recent error message, NUL-terminated,
/// into `buf` (truncated to fit if necessary). Returns the full message's
/// length in bytes, not counting the NUL terminator — 0 if there is no
/// pending error. A successful call to any other `foldback_*` function
/// clears the error.
///
/// # Safety
/// `buf` must point to at least `buf_len` writable bytes, or be null iff
/// `buf_len` is 0.
#[no_mangle]
pub unsafe extern "C" fn foldback_last_error(buf: *mut c_char, buf_len: usize) -> usize {
    let msg = LAST_ERROR.with(|e| e.borrow().clone());
    let Some(msg) = msg else { return 0 };
    if buf_len > 0 && !buf.is_null() {
        let bytes = msg.as_bytes();
        let copy_len = bytes.len().min(buf_len - 1);
        ptr::copy_nonoverlapping(bytes.as_ptr(), buf as *mut u8, copy_len);
        *buf.add(copy_len) = 0;
    }
    msg.len()
}

unsafe fn as_session_mut<'a>(session: *mut Handle) -> Result<&'a mut Handle, String> {
    session
        .as_mut()
        .ok_or_else(|| "session was null".to_string())
}

unsafe fn as_session_ref<'a>(session: *const Handle) -> Result<&'a Handle, String> {
    session
        .as_ref()
        .ok_or_else(|| "session was null".to_string())
}

unsafe fn str_from_raw<'a>(ptr: *const c_char, label: &str) -> Result<&'a str, String> {
    if ptr.is_null() {
        return Err(format!("{label} was null"));
    }
    CStr::from_ptr(ptr)
        .to_str()
        .map_err(|e| format!("{label} was not valid UTF-8: {e}"))
}

unsafe fn bytes_from_raw<'a>(ptr: *const u8, len: usize) -> Result<&'a [u8], String> {
    if len == 0 {
        return Ok(&[]);
    }
    if ptr.is_null() {
        return Err("state pointer was null but len was nonzero".to_string());
    }
    Ok(std::slice::from_raw_parts(ptr, len))
}

#[cfg(test)]
mod tests {
    use std::ffi::CString;

    use super::*;

    fn make_config() -> Config {
        Config {
            tick_rate_hz: 60,
            peer_count: 2,
            local_peer_id: 0,
            retention: 0,
            build_id: [0u8; 16],
            record_to_path: ptr::null(),
        }
    }

    #[test]
    fn create_hash_and_destroy_round_trips() {
        unsafe {
            let config = make_config();
            let session = foldback_session_create(&config);
            assert!(!session.is_null());

            let state = [1u8, 2, 3, 4];
            let status = foldback_hash_tick(session, 0, state.as_ptr(), state.len());
            assert_eq!(status, Status::Ok);

            assert_eq!(foldback_pending_hash_count(session), 1);
            let mut out = [PendingHash { tick: 0, hash: 0 }; 4];
            let n = foldback_take_pending_hashes(session, out.as_mut_ptr(), out.len());
            assert_eq!(n, 1);
            assert_eq!(out[0].tick, 0);
            assert_eq!(foldback_pending_hash_count(session), 0);

            foldback_session_destroy(session);
        }
    }

    #[test]
    fn mismatched_peer_hashes_are_detected_as_divergence() {
        unsafe {
            let config = make_config();
            let session = foldback_session_create(&config);

            foldback_hash_tick(session, 5, [1u8].as_ptr(), 1);
            foldback_record_peer_hash(session, 5, 1, 0xdead_beef);

            let mut found = false;
            let mut tick = 0u64;
            let status = foldback_check_divergence(session, &mut found, &mut tick);
            assert_eq!(status, Status::Ok);
            assert!(found);
            assert_eq!(tick, 5);

            foldback_session_destroy(session);
        }
    }

    #[test]
    fn matching_peer_hashes_produce_no_divergence() {
        unsafe {
            let config = make_config();
            let session = foldback_session_create(&config);

            foldback_hash_tick(session, 5, [1u8].as_ptr(), 1);
            let hash = foldback_core::hash::hash_bytes(&[1u8]);
            foldback_record_peer_hash(session, 5, 1, hash);

            let mut found = true;
            let mut tick = 0u64;
            foldback_check_divergence(session, &mut found, &mut tick);
            assert!(!found);

            foldback_session_destroy(session);
        }
    }

    #[test]
    fn null_session_reports_an_error_instead_of_crashing() {
        unsafe {
            let status = foldback_hash_tick(ptr::null_mut(), 0, ptr::null(), 0);
            assert_eq!(status, Status::Panic);

            let mut buf = [0i8; 256];
            let len = foldback_last_error(buf.as_mut_ptr(), buf.len());
            assert!(len > 0);
            let msg = CStr::from_ptr(buf.as_ptr()).to_str().unwrap();
            assert!(msg.contains("null"));
        }
    }

    #[test]
    fn recording_to_a_file_round_trips_through_finish_recording() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.foldback");
        let path_c = CString::new(path.to_str().unwrap()).unwrap();

        unsafe {
            let mut config = make_config();
            config.record_to_path = path_c.as_ptr();
            let session = foldback_session_create(&config);
            assert!(!session.is_null());

            foldback_hash_tick(session, 0, [9u8].as_ptr(), 1);
            let status = foldback_finish_recording(session);
            assert_eq!(status, Status::Ok);
            foldback_session_destroy(session);
        }

        assert!(path.exists());
        assert!(std::fs::metadata(&path).unwrap().len() > 0);
    }

    #[test]
    fn entity_and_field_hashing_write_frames_when_recording() {
        use foldback_core::format::{Frame, FrameReader, Header};

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("l23.foldback");
        let path_c = CString::new(path.to_str().unwrap()).unwrap();
        let field_name = CString::new("position.x").unwrap();

        unsafe {
            let mut config = make_config();
            config.record_to_path = path_c.as_ptr();
            let session = foldback_session_create(&config);
            assert!(!session.is_null());

            let entity_state = [1u8, 2, 3];
            let status =
                foldback_hash_entity(session, 10, 7, entity_state.as_ptr(), entity_state.len());
            assert_eq!(status, Status::Ok);

            let field_value = 3.5f32.to_le_bytes();
            let status = foldback_hash_field(
                session,
                10,
                7,
                field_name.as_ptr(),
                field_value.as_ptr(),
                field_value.len(),
            );
            assert_eq!(status, Status::Ok);

            let status = foldback_record_peer_entity_hash(session, 10, 1, 7, 0xdead_beef);
            assert_eq!(status, Status::Ok);
            let status = foldback_record_peer_field_hash(
                session,
                10,
                1,
                7,
                field_name.as_ptr(),
                0xcafe_babe,
                field_value.as_ptr(),
                field_value.len(),
            );
            assert_eq!(status, Status::Ok);

            let status = foldback_finish_recording(session);
            assert_eq!(status, Status::Ok);
            foldback_session_destroy(session);
        }

        let mut file = std::fs::File::open(&path).unwrap();
        Header::read_from(&mut file).unwrap();
        let frames: Vec<_> = FrameReader::new(file).map(|f| f.unwrap()).collect();

        let entity_frames: Vec<_> = frames
            .iter()
            .filter(|f| matches!(f, Frame::EntityHash { .. }))
            .collect();
        assert_eq!(entity_frames.len(), 2, "one local + one peer entity hash");
        assert!(matches!(
            entity_frames[0],
            Frame::EntityHash {
                tick: 10,
                peer_id: 0,
                entity_id: 7,
                ..
            }
        ));
        assert!(matches!(
            entity_frames[1],
            Frame::EntityHash {
                tick: 10,
                peer_id: 1,
                entity_id: 7,
                hash: 0xdead_beef,
                ..
            }
        ));

        let field_frames: Vec<_> = frames
            .iter()
            .filter(|f| matches!(f, Frame::FieldHash { .. }))
            .collect();
        assert_eq!(field_frames.len(), 2, "one local + one peer field hash");
        assert!(matches!(
            field_frames[0],
            Frame::FieldHash { tick: 10, peer_id: 0, entity_id: 7, field_name, .. }
            if field_name == "position.x"
        ));
        assert!(matches!(
            field_frames[1],
            Frame::FieldHash {
                tick: 10,
                peer_id: 1,
                entity_id: 7,
                hash: 0xcafe_babe,
                ..
            }
        ));
    }

    #[test]
    fn entity_and_field_hashing_are_no_ops_without_recording() {
        unsafe {
            let config = make_config(); // no record_to_path
            let session = foldback_session_create(&config);
            assert!(!session.is_null());

            let status = foldback_hash_entity(session, 0, 0, [1u8].as_ptr(), 1);
            assert_eq!(status, Status::Ok);

            let field_name = CString::new("x").unwrap();
            let status = foldback_hash_field(session, 0, 0, field_name.as_ptr(), [1u8].as_ptr(), 1);
            assert_eq!(status, Status::Ok);

            foldback_session_destroy(session);
        }
    }

    #[test]
    fn null_field_name_reports_an_error_instead_of_crashing() {
        unsafe {
            let config = make_config();
            let session = foldback_session_create(&config);

            let status = foldback_hash_field(session, 0, 0, ptr::null(), ptr::null(), 0);
            assert_eq!(status, Status::Panic);

            let mut buf = [0i8; 256];
            let len = foldback_last_error(buf.as_mut_ptr(), buf.len());
            assert!(len > 0);
            let msg = CStr::from_ptr(buf.as_ptr()).to_str().unwrap();
            assert!(msg.contains("field_name"));

            foldback_session_destroy(session);
        }
    }

    #[test]
    fn finish_reflects_locally_hashed_ticks() {
        unsafe {
            let config_a = make_config();
            let session_a = foldback_session_create(&config_a);
            let config_b = make_config();
            let session_b = foldback_session_create(&config_b);

            for tick in 0..5u64 {
                let state = [tick as u8];
                foldback_hash_tick(session_a, tick, state.as_ptr(), 1);
                foldback_hash_tick(session_b, tick, state.as_ptr(), 1);
            }

            assert_eq!(foldback_finish(session_a), foldback_finish(session_b));

            foldback_hash_tick(session_b, 5, [0xffu8].as_ptr(), 1);
            assert_ne!(foldback_finish(session_a), foldback_finish(session_b));

            foldback_session_destroy(session_a);
            foldback_session_destroy(session_b);
        }
    }

    #[test]
    fn record_schema_writes_one_metadata_frame_per_type_not_per_call() {
        use foldback_core::format::{Frame, FrameReader, Header};

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("schema.foldback");
        let path_c = CString::new(path.to_str().unwrap()).unwrap();
        let type_name = CString::new("Unit").unwrap();
        let field_hp = CString::new("hp").unwrap();
        let field_pos = CString::new("pos").unwrap();
        let fields = [field_hp.as_ptr(), field_pos.as_ptr()];

        unsafe {
            let mut config = make_config();
            config.record_to_path = path_c.as_ptr();
            let session = foldback_session_create(&config);
            assert!(!session.is_null());

            let status =
                foldback_record_schema(session, type_name.as_ptr(), fields.as_ptr(), fields.len());
            assert_eq!(status, Status::Ok);
            // A second call for the same type this session must not
            // write a second Metadata frame.
            let status =
                foldback_record_schema(session, type_name.as_ptr(), fields.as_ptr(), fields.len());
            assert_eq!(status, Status::Ok);

            let status = foldback_finish_recording(session);
            assert_eq!(status, Status::Ok);
            foldback_session_destroy(session);
        }

        let mut file = std::fs::File::open(&path).unwrap();
        Header::read_from(&mut file).unwrap();
        let frames: Vec<_> = FrameReader::new(file).map(|f| f.unwrap()).collect();
        let schema_frames: Vec<_> = frames
            .iter()
            .filter(
                |f| matches!(f, Frame::Metadata { key, .. } if key.starts_with("foldback.schema.")),
            )
            .collect();
        assert_eq!(schema_frames.len(), 1);
        assert!(matches!(
            schema_frames[0],
            Frame::Metadata { key, value }
            if key == "foldback.schema.Unit" && value == "hp,pos"
        ));
    }

    #[test]
    fn record_schema_null_type_name_reports_an_error_instead_of_crashing() {
        unsafe {
            let config = make_config();
            let session = foldback_session_create(&config);

            let status = foldback_record_schema(session, ptr::null(), ptr::null(), 0);
            assert_eq!(status, Status::Panic);

            let mut buf = [0i8; 256];
            let len = foldback_last_error(buf.as_mut_ptr(), buf.len());
            assert!(len > 0);
            let msg = CStr::from_ptr(buf.as_ptr()).to_str().unwrap();
            assert!(msg.contains("type_name"));

            foldback_session_destroy(session);
        }
    }
}
