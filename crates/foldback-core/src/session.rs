//! The `Session` API — the library's actual integration surface, matching
//! the shape pinned down in foldback-cookbook.md recipes 1–2. Level 1
//! (per-tick) only in Phase 0; `record_peer_hash`/`check_divergence` are
//! written so Level 2/3 (entity/field) can extend this later without a
//! redesign, per foldback-plan.md §3.4's tiered bisection model.

use std::collections::{BTreeMap, HashSet};
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::PathBuf;

use crate::bisect::DivergenceTick;
use crate::format::{Frame, Header};
use crate::hash::hash_bytes;
use crate::ring_buffer::RingBuffer;
use crate::Error;

/// A tick hash produced locally, ready to exchange with peers over the
/// game's own netcode channel — a handful of bytes per tick, meant to
/// piggyback on an existing packet rather than open a new one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PendingHash {
    pub tick: u64,
    pub hash: u64,
}

pub struct SessionBuilder {
    tick_rate_hz: u32,
    peer_count: u32,
    local_peer_id: u16,
    retention: usize,
    build_id: [u8; 16],
    record_to: Option<PathBuf>,
}

impl Default for SessionBuilder {
    fn default() -> Self {
        SessionBuilder {
            tick_rate_hz: 60,
            peer_count: 1,
            local_peer_id: 0,
            retention: 600, // 10s of ticks at the default 60Hz
            build_id: [0u8; 16],
            record_to: None,
        }
    }
}

impl SessionBuilder {
    pub fn tick_rate_hz(mut self, v: u32) -> Self {
        self.tick_rate_hz = v;
        self
    }

    pub fn peer_count(mut self, v: u32) -> Self {
        self.peer_count = v;
        self
    }

    /// Which peer this local session *is*, for attributing its own
    /// `hash_tick` calls in the cross-peer comparison table. Defaults to 0.
    pub fn local_peer_id(mut self, v: u16) -> Self {
        self.local_peer_id = v;
        self
    }

    /// Retention window for the in-memory tick-hash ring buffer (ticks,
    /// not bytes). Default 600 (10s at 60Hz).
    pub fn retention(mut self, v: usize) -> Self {
        self.retention = v;
        self
    }

    pub fn build_id(mut self, v: [u8; 16]) -> Self {
        self.build_id = v;
        self
    }

    /// Record every frame to a `.foldback` file at `path` as it happens
    /// (streaming, not buffered-then-written) — the file's header is
    /// written immediately on `build()`.
    pub fn record_to<P: Into<PathBuf>>(mut self, path: P) -> Self {
        self.record_to = Some(path.into());
        self
    }

    pub fn build(self) -> Result<Session, Error> {
        let writer = match self.record_to {
            Some(path) => {
                let file = File::create(path)?;
                let mut w = BufWriter::new(file);
                Header::new(self.tick_rate_hz, self.peer_count, self.build_id).write_to(&mut w)?;
                Some(w)
            }
            None => None,
        };

        Ok(Session {
            tick_rate_hz: self.tick_rate_hz,
            peer_count: self.peer_count,
            local_peer_id: self.local_peer_id,
            ring: RingBuffer::new(self.retention),
            pending: Vec::new(),
            hashes_by_tick: BTreeMap::new(),
            checked_ticks: HashSet::new(),
            cumulative: 0,
            writer,
        })
    }
}

pub struct Session {
    #[allow(dead_code)] // read by future Level 2/3 work and CLI reporting
    tick_rate_hz: u32,
    #[allow(dead_code)]
    peer_count: u32,
    local_peer_id: u16,
    ring: RingBuffer<(u64, u64)>,
    pending: Vec<PendingHash>,
    /// tick -> (peer_id, hash) pairs reported so far, across all peers —
    /// the raw material `check_divergence` scans.
    hashes_by_tick: BTreeMap<u64, Vec<(u16, u64)>>,
    checked_ticks: HashSet<u64>,
    cumulative: u64,
    writer: Option<BufWriter<File>>,
}

impl Session {
    pub fn builder() -> SessionBuilder {
        SessionBuilder::default()
    }

    /// Hash this tick's state and record it as this session's own
    /// (`local_peer_id`'s) report for the tick. Never allocates beyond the
    /// hash itself and the bookkeeping below — the actual hash call is the
    /// validated-cheap hot path from the Week 0 performance spike.
    pub fn hash_tick(&mut self, tick: u64, state_bytes: &[u8]) -> Result<(), Error> {
        let hash = hash_bytes(state_bytes);
        self.record_peer_hash(tick, self.local_peer_id, hash)?;
        self.pending.push(PendingHash { tick, hash });
        self.cumulative = combine(self.cumulative, tick, hash);
        Ok(())
    }

    /// Record a hash reported by any peer (including this session's own,
    /// via `hash_tick`) into the cross-peer comparison table.
    pub fn record_peer_hash(&mut self, tick: u64, peer_id: u16, hash: u64) -> Result<(), Error> {
        self.ring.push((tick, hash));
        self.hashes_by_tick
            .entry(tick)
            .or_default()
            .push((peer_id, hash));
        if let Some(w) = &mut self.writer {
            Frame::TickHash {
                tick,
                peer_id,
                hash,
            }
            .write_to(w)?;
        }
        Ok(())
    }

    /// Drains and returns hashes produced locally since the last call —
    /// what the game is expected to send to its peers over its own
    /// netcode channel.
    pub fn take_pending_hashes(&mut self) -> Vec<PendingHash> {
        std::mem::take(&mut self.pending)
    }

    /// Returns the earliest tick, not already reported by a previous call,
    /// where two or more reporting peers' hashes disagree. `None` if no
    /// new divergence exists yet — ticks with fewer than 2 peers reported
    /// so far simply aren't comparable yet, not flagged.
    pub fn check_divergence(&mut self) -> Option<DivergenceTick> {
        for (&tick, peer_hashes) in &self.hashes_by_tick {
            if self.checked_ticks.contains(&tick) || peer_hashes.len() < 2 {
                continue;
            }
            let first = peer_hashes[0].1;
            if peer_hashes.iter().any(|(_, h)| *h != first) {
                self.checked_ticks.insert(tick);
                return Some(DivergenceTick(tick));
            }
        }
        None
    }

    /// A single combined hash over every tick this session has locally
    /// hashed, order-dependent (both sides of a comparison must process
    /// ticks in the same order — true for two runs of the same
    /// simulation). Used for the CI-gate pattern: two independent runs'
    /// `finish()` values are equal iff every tick hashed identically,
    /// without needing per-tick bookkeeping at the call site.
    pub fn finish(&self) -> u64 {
        self.cumulative
    }

    /// Writes the closing `EndOfStream` frame if recording to a file.
    /// Its *absence* (session dropped without calling this, e.g. a crash)
    /// is exactly how a reader tells a truncated recording from a clean
    /// one — per protocol spec §1.2.
    pub fn finish_recording(&mut self) -> Result<(), Error> {
        if let Some(w) = &mut self.writer {
            Frame::EndOfStream.write_to(w)?;
            w.flush()?;
        }
        Ok(())
    }

    pub fn tick_rate_hz(&self) -> u32 {
        self.tick_rate_hz
    }

    pub fn peer_count(&self) -> u32 {
        self.peer_count
    }

    pub fn retained_ticks(&self) -> impl Iterator<Item = &(u64, u64)> {
        self.ring.iter()
    }
}

fn combine(acc: u64, tick: u64, hash: u64) -> u64 {
    let mut buf = [0u8; 24];
    buf[0..8].copy_from_slice(&acc.to_le_bytes());
    buf[8..16].copy_from_slice(&tick.to_le_bytes());
    buf[16..24].copy_from_slice(&hash.to_le_bytes());
    hash_bytes(&buf)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_tick_produces_pending_hash() {
        let mut s = Session::builder()
            .tick_rate_hz(60)
            .peer_count(1)
            .build()
            .unwrap();
        s.hash_tick(0, b"state at tick 0").unwrap();
        let pending = s.take_pending_hashes();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].tick, 0);
        // Draining again yields nothing new.
        assert!(s.take_pending_hashes().is_empty());
    }

    #[test]
    fn two_clean_runs_produce_equal_finish() {
        let mut a = Session::builder().peer_count(1).build().unwrap();
        let mut b = Session::builder().peer_count(1).build().unwrap();
        for tick in 0..50u64 {
            let state = format!("tick-{tick}-state");
            a.hash_tick(tick, state.as_bytes()).unwrap();
            b.hash_tick(tick, state.as_bytes()).unwrap();
        }
        assert_eq!(a.finish(), b.finish());
    }

    #[test]
    fn diverging_run_produces_different_finish() {
        let mut a = Session::builder().peer_count(1).build().unwrap();
        let mut b = Session::builder().peer_count(1).build().unwrap();
        for tick in 0..50u64 {
            let state_a = format!("tick-{tick}-state");
            // b silently diverges at tick 30 — the actual bug class this exists to catch.
            let state_b = if tick == 30 {
                "corrupted".to_string()
            } else {
                format!("tick-{tick}-state")
            };
            a.hash_tick(tick, state_a.as_bytes()).unwrap();
            b.hash_tick(tick, state_b.as_bytes()).unwrap();
        }
        assert_ne!(a.finish(), b.finish());
    }

    #[test]
    fn check_divergence_finds_exact_tick_and_reports_once() {
        let mut s = Session::builder().peer_count(2).build().unwrap();
        for tick in 0..10u64 {
            s.record_peer_hash(tick, 0, 100 + tick).unwrap();
            let peer1_hash = if tick == 5 { 9999 } else { 100 + tick };
            s.record_peer_hash(tick, 1, peer1_hash).unwrap();
        }
        assert_eq!(s.check_divergence(), Some(DivergenceTick(5)));
        // Already reported — must not report the same tick again.
        assert_eq!(s.check_divergence(), None);
    }

    #[test]
    fn check_divergence_none_when_clean() {
        let mut s = Session::builder().peer_count(2).build().unwrap();
        for tick in 0..10u64 {
            s.record_peer_hash(tick, 0, 100 + tick).unwrap();
            s.record_peer_hash(tick, 1, 100 + tick).unwrap();
        }
        assert_eq!(s.check_divergence(), None);
    }

    #[test]
    fn recording_to_file_round_trips_through_frame_reader() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("session.foldback");

        {
            let mut s = Session::builder()
                .tick_rate_hz(60)
                .peer_count(1)
                .record_to(&path)
                .build()
                .unwrap();
            for tick in 0..5u64 {
                s.hash_tick(tick, format!("state-{tick}").as_bytes())
                    .unwrap();
            }
            s.finish_recording().unwrap();
        }

        let mut file = File::open(&path).unwrap();
        let header = Header::read_from(&mut file).unwrap();
        assert_eq!(header.tick_rate_hz, 60);
        assert_eq!(header.peer_count, 1);

        let reader = crate::format::FrameReader::new(file);
        let frames: Result<Vec<_>, _> = reader.collect();
        let frames = frames.unwrap();
        assert_eq!(frames.len(), 6); // 5 TickHash + EndOfStream
        assert!(matches!(frames.last(), Some(Frame::EndOfStream)));
    }
}
