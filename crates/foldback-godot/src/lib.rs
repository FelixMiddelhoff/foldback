// SPDX-License-Identifier: MIT OR Apache-2.0
//! `foldback-godot` — the GDExtension binding (cookbook recipe 9), built
//! directly on [`godot`] (the `gdext` project's Rust bindings for Godot 4)
//! rather than routing through `foldback-sys`'s C ABI: `gdext` generates
//! the GDExtension registration itself from Rust attributes, so a second,
//! hand-written C boundary (the way `bindings/unity` needs one, since C#
//! has no native GDExtension-equivalent) would only add an indirection
//! with nothing to show for it. `foldback-core` is called directly,
//! safely, from this crate.
//!
//! Deviation from cookbook recipe 9's sketch, found while actually
//! building this (the same "sketch vs reality" pattern every other
//! binding has hit): the sketch shows
//! `FoldbackSession.new({"tick_rate_hz": 60, ...})` — a Dictionary passed
//! straight to `.new()`. A GDExtension class's `.new()` in GDScript calls
//! the type's zero-argument `_init`; there is no supported way to route
//! extra constructor arguments through a native (non-script) class's
//! `.new()` in the version of `gdext` this binding targets. The real,
//! verified-necessary shape is `FoldbackSession.new()` (via
//! `#[class(init)]`, a `gdext`-idiomatic no-arg constructor) followed by
//! `.configure(dict) -> bool`. `foldback-cookbook.md` recipe 9 is updated
//! to match.
//!
//! `u64` hashes cross into GDScript as `i64` (GDScript's only integer
//! type) via an exact bit-reinterpretation, not a numeric cast — two
//! equal `u64` hashes produce equal `i64` values and vice versa, which is
//! all equality-based divergence checking needs; a hash may display as
//! negative in GDScript, which is expected and harmless.

use godot::classes::RefCounted;
use godot::prelude::*;

use foldback_core::session::Session;

struct FoldbackExtension;

#[gdextension]
unsafe impl ExtensionLibrary for FoldbackExtension {}

/// Status codes returned by fallible `FoldbackSession` methods — mirrors
/// `foldback-sys::Status` so the two bindings read the same way (protocol
/// spec §3's status-code convention, not a GDExtension-specific choice).
#[repr(i64)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Status {
    Ok = 0,
    NotConfigured = -1,
    Io = -3,
}

#[derive(GodotClass)]
#[class(base=RefCounted, init)]
struct FoldbackSession {
    base: Base<RefCounted>,
    inner: Option<Session>,
    last_error: String,
}

#[godot_api]
impl FoldbackSession {
    /// Builds the underlying session from a config dict matching cookbook
    /// recipe 9's fields: `tick_rate_hz` (int), `peer_count` (int),
    /// `local_peer_id` (int, default 0), `record_to_path` (String,
    /// optional — omit or empty to not record). Returns `false` and sets
    /// `get_last_error()` on failure (e.g. missing required key,
    /// unwritable `record_to_path`); the session stays unconfigured, and
    /// every hashing method becomes a documented no-op until `configure`
    /// succeeds.
    #[func]
    fn configure(&mut self, config: Dictionary<Variant, Variant>) -> bool {
        let Some(tick_rate_hz) = dict_get_i64(&config, "tick_rate_hz") else {
            self.last_error = "config missing required key 'tick_rate_hz'".into();
            return false;
        };
        let Some(peer_count) = dict_get_i64(&config, "peer_count") else {
            self.last_error = "config missing required key 'peer_count'".into();
            return false;
        };
        let local_peer_id = dict_get_i64(&config, "local_peer_id").unwrap_or(0);

        let mut builder = Session::builder()
            .tick_rate_hz(tick_rate_hz as u32)
            .peer_count(peer_count as u32)
            .local_peer_id(local_peer_id as u16);

        if let Some(retention) = dict_get_i64(&config, "retention") {
            if retention > 0 {
                builder = builder.retention(retention as usize);
            }
        }

        if let Some(record_to_path) = config
            .get("record_to_path")
            .and_then(|v| v.try_to::<GString>().ok())
        {
            let path = record_to_path.to_string();
            if !path.is_empty() {
                builder = builder.record_to(path);
            }
        }

        match builder.build() {
            Ok(session) => {
                self.inner = Some(session);
                self.last_error.clear();
                true
            }
            Err(e) => {
                self.last_error = format!("could not build session: {e}");
                false
            }
        }
    }

    /// The most recent error message from `configure` or a failed
    /// hashing call, empty if none.
    #[func]
    fn get_last_error(&self) -> GString {
        GString::from(self.last_error.as_str())
    }

    /// Hashes `state` and records it as this session's own report for
    /// `tick` (cookbook recipe 9's `hash_tick`).
    #[func]
    fn hash_tick(&mut self, tick: i64, state: PackedByteArray) -> i64 {
        self.with_session(|session| {
            session
                .hash_tick(tick as u64, state.as_slice())
                .map_err(|e| format!("hash_tick failed: {e}"))
        })
    }

    /// Records a hash reported by any peer (including this session's
    /// own, via `hash_tick`) into the cross-peer comparison table.
    #[func]
    fn record_peer_hash(&mut self, tick: i64, peer_id: i64, hash: i64) -> i64 {
        self.with_session(|session| {
            session
                .record_peer_hash(tick as u64, peer_id as u16, hash as u64)
                .map_err(|e| format!("record_peer_hash failed: {e}"))
        })
    }

    /// Level 2: hashes one entity's state and records it as this
    /// session's own report for `tick`. Recording-only, matching
    /// `foldback_core::Session::hash_entity`'s own documented scope.
    #[func]
    fn hash_entity(&mut self, tick: i64, entity_id: i64, state: PackedByteArray) -> i64 {
        self.with_session(|session| {
            session
                .hash_entity(tick as u64, entity_id as u64, state.as_slice())
                .map_err(|e| format!("hash_entity failed: {e}"))
        })
    }

    #[func]
    fn record_peer_entity_hash(
        &mut self,
        tick: i64,
        peer_id: i64,
        entity_id: i64,
        hash: i64,
    ) -> i64 {
        self.with_session(|session| {
            session
                .record_peer_entity_hash(tick as u64, peer_id as u16, entity_id as u64, hash as u64)
                .map_err(|e| format!("record_peer_entity_hash failed: {e}"))
        })
    }

    /// Level 3: hashes one field's value and records it, keeping the raw
    /// `value` bytes too — what lets the UI show the actual differing
    /// values, not just unequal hashes.
    #[func]
    fn hash_field(
        &mut self,
        tick: i64,
        entity_id: i64,
        field_name: GString,
        value: PackedByteArray,
    ) -> i64 {
        self.with_session(|session| {
            session
                .hash_field(
                    tick as u64,
                    entity_id as u64,
                    &field_name.to_string(),
                    value.as_slice(),
                )
                .map_err(|e| format!("hash_field failed: {e}"))
        })
    }

    #[func]
    fn record_peer_field_hash(
        &mut self,
        tick: i64,
        peer_id: i64,
        entity_id: i64,
        field_name: GString,
        hash: i64,
        value: PackedByteArray,
    ) -> i64 {
        self.with_session(|session| {
            session
                .record_peer_field_hash(
                    tick as u64,
                    peer_id as u16,
                    entity_id as u64,
                    &field_name.to_string(),
                    hash as u64,
                    value.as_slice(),
                )
                .map_err(|e| format!("record_peer_field_hash failed: {e}"))
        })
    }

    /// The number of hashes currently pending, without draining them.
    #[func]
    fn pending_hash_count(&self) -> i64 {
        match &self.inner {
            Some(session) => session.pending_hash_count() as i64,
            None => 0,
        }
    }

    /// Drains up to `max` pending hashes, oldest first, as an Array of
    /// `{"tick": int, "hash": int}` dicts.
    #[func]
    fn take_pending_hashes(&mut self, max: i64) -> Array<Dictionary<Variant, Variant>> {
        let Some(session) = self.inner.as_mut() else {
            return Array::new();
        };
        let drained = session.take_pending_hashes_up_to(max.max(0) as usize);
        let mut out = Array::new();
        for ph in drained {
            let mut d = Dictionary::new();
            d.set("tick", ph.tick as i64);
            d.set("hash", ph.hash as i64);
            out.push(&d);
        }
        out
    }

    /// Checks for a new cross-peer divergence. Returns
    /// `{"found": bool, "tick": int}` — `tick` is meaningless when
    /// `found` is false.
    #[func]
    fn check_divergence(&mut self) -> Dictionary<Variant, Variant> {
        let mut out = Dictionary::new();
        match self.inner.as_mut().and_then(|s| s.check_divergence()) {
            Some(d) => {
                out.set("found", true);
                out.set("tick", d.0 as i64);
            }
            None => {
                out.set("found", false);
                out.set("tick", 0i64);
            }
        }
        out
    }

    /// The combined hash over every tick this session has locally hashed
    /// (cookbook's CI-gate pattern). 0 if unconfigured.
    #[func]
    fn finish(&self) -> i64 {
        match &self.inner {
            Some(session) => session.finish() as i64,
            None => 0,
        }
    }

    /// Writes the closing frame if recording to a file — call before the
    /// session is freed when `record_to_path` was set.
    #[func]
    fn finish_recording(&mut self) -> i64 {
        self.with_session(|session| {
            session
                .finish_recording()
                .map_err(|e| format!("finish_recording failed: {e}"))
        })
    }

    #[func]
    fn tick_rate_hz(&self) -> i64 {
        match &self.inner {
            Some(session) => session.tick_rate_hz() as i64,
            None => 0,
        }
    }

    #[func]
    fn peer_count(&self) -> i64 {
        match &self.inner {
            Some(session) => session.peer_count() as i64,
            None => 0,
        }
    }

    fn with_session(&mut self, f: impl FnOnce(&mut Session) -> Result<(), String>) -> i64 {
        let Some(session) = self.inner.as_mut() else {
            self.last_error = "session not configured — call configure() first".into();
            return Status::NotConfigured as i64;
        };
        match f(session) {
            Ok(()) => {
                self.last_error.clear();
                Status::Ok as i64
            }
            Err(msg) => {
                self.last_error = msg;
                Status::Io as i64
            }
        }
    }
}

fn dict_get_i64(dict: &Dictionary<Variant, Variant>, key: &str) -> Option<i64> {
    dict.get(key).and_then(|v| v.try_to::<i64>().ok())
}
