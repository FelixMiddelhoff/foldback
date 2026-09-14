// SPDX-License-Identifier: MIT OR Apache-2.0
//! Bridges Foldback into GGRS's own checksum mechanism (cookbook recipe
//! 3), rather than requiring a second, duplicate `hash_tick` call per
//! frame.
//!
//! GGRS has no hookable "checksum callback" to wrap — a game computes its
//! own checksum and hands it to GGRS via `GameStateCell::save(frame,
//! state, checksum)`, and for a real networked `P2pSession`, GGRS emits
//! `GgrsEvent::DesyncDetected { local_checksum, remote_checksum, .. }`
//! when two peers' checksums for a frame disagree. The integration point
//! is therefore two small helpers, not a session-wrapping extension
//! trait: [`checksum`] so a game's own checksum *is* Foldback's hash (one
//! computation, not two), and [`record_desync`] to feed GGRS's own event
//! into a [`foldback_core::session::Session`] for bisection/recording —
//! reusing GGRS's already-working checksum exchange over the network
//! instead of Foldback re-implementing peer-to-peer hash exchange itself.
//!
//! `SyncTestSession` (single-process, re-simulation-based — see
//! `examples/ggrs-demo`) doesn't need this: it catches a checksum
//! mismatch internally and returns `Err(GgrsError::MismatchedChecksum)`
//! directly, with no cross-peer network exchange to bridge.

use foldback_core::hash::hash_bytes;
use foldback_core::session::Session;
use foldback_core::Error;

/// A game state's checksum, suitable for `GameStateCell::save`'s
/// `checksum: Option<u128>` parameter. Widens Foldback's 64-bit hash into
/// the low 64 bits of GGRS's 128-bit checksum (high bits zero) — lossless,
/// and [`narrow`] recovers the exact original hash for recording.
pub fn checksum(state_bytes: &[u8]) -> u128 {
    hash_bytes(state_bytes) as u128
}

/// Recovers the original Foldback hash from a checksum produced by
/// [`checksum`] — including one that arrived over the network as a GGRS
/// `DesyncDetected` event's `local_checksum`/`remote_checksum`, which are
/// only meaningful if both peers actually produced theirs via
/// [`checksum`] in the first place.
pub fn narrow(ggrs_checksum: u128) -> u64 {
    ggrs_checksum as u64
}

/// Feeds a GGRS `DesyncDetected` event into a Foldback [`Session`] so the
/// desync GGRS already found gets Foldback's own recording/UI treatment
/// (a `.foldback` file to open, the timeline, peer comparison) instead of
/// living only as a one-off log line. `tick` is GGRS's `Frame` (`i32`,
/// non-negative for a real `DesyncDetected` event) cast to `u64` by the
/// caller — same convention `Session::hash_tick`/`record_peer_hash`
/// already use, no extra validation invented here.
pub fn record_desync(
    session: &mut Session,
    tick: u64,
    local_peer_id: u16,
    remote_peer_id: u16,
    local_checksum: u128,
    remote_checksum: u128,
) -> Result<(), Error> {
    session.record_peer_hash(tick, local_peer_id, narrow(local_checksum))?;
    session.record_peer_hash(tick, remote_peer_id, narrow(remote_checksum))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checksum_narrows_back_to_the_original_hash() {
        let bytes = b"some game state";
        let original = hash_bytes(bytes);
        let widened = checksum(bytes);
        assert_eq!(narrow(widened), original);
    }

    #[test]
    fn matching_checksums_produce_no_divergence() {
        let mut session = Session::builder().peer_count(2).build().unwrap();
        let cs = checksum(b"agreed state");
        record_desync(&mut session, 10, 0, 1, cs, cs).unwrap();
        assert_eq!(session.check_divergence(), None);
    }

    #[test]
    fn mismatched_checksums_produce_a_divergence_at_the_reported_frame() {
        let mut session = Session::builder().peer_count(2).build().unwrap();
        let local = checksum(b"peer 0's state");
        let remote = checksum(b"peer 1's drifted state");
        record_desync(&mut session, 42, 0, 1, local, remote).unwrap();
        let divergence = session.check_divergence();
        assert_eq!(divergence.map(|d| d.0), Some(42));
    }
}
