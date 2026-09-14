// SPDX-License-Identifier: MIT OR Apache-2.0
//! `examples/ggrs-demo` — proves Foldback layered on a real GGRS rollback
//! session, not just a bare tick loop (see `examples/minimal-rust` for
//! that). Two independent local `SyncTestSession`s stand in for two
//! network peers running the same GGRS-driven simulation; each tick's
//! confirmed state is hashed and recorded into one shared, two-peer
//! `.foldback` file — including one injected divergence, plus Level 2/3
//! (per-entity, per-field) hashes in a window around it, so the recorded
//! file is real evidence for the Foldback UI (Master Sequence §4) rather
//! than a synthetic fixture — and its bisection drill-down panel has real
//! data to show, not just the honest "nothing recorded" placeholder.
//!
//! This uses `Session::record_peer_hash`/`hash_entity`/`hash_field`
//! directly (Level 1/2/3 producer API), the same as `minimal-rust` for
//! Level 1. `foldback-rs`'s GGRS bridge (`checksum`/`record_desync`,
//! cookbook recipe 3) is for a real networked `P2pSession`'s own
//! `DesyncDetected` event — `SyncTestSession` here catches mismatches
//! internally, so there's no such event to bridge.
//!
//! Run: `cargo run -p ggrs-demo`

use bytemuck::{Pod, Zeroable};
use foldback_core::hash::hash_bytes;
use foldback_core::session::Session as FoldbackSession;
use ggrs::{Config, GgrsRequest, PredictRepeatLast, SessionBuilder};

const NUM_TICKS: u64 = 120;
const DIVERGE_AT_TICK: u64 = 75;
/// Level 2/3 (entity/field) hashing is recorded only in a window around
/// the known divergence — matches how it's actually meant to be used
/// (cookbook recipes 4/5): once Level 1 has found *which tick*, finer
/// detail is worth the extra bytes; recording it for every tick of every
/// session by default isn't.
const ENTITY_FIELD_WINDOW: std::ops::Range<u64> = (DIVERGE_AT_TICK - 5)..(DIVERGE_AT_TICK + 5);

#[repr(C)]
#[derive(
    Copy, Clone, PartialEq, Eq, Pod, Zeroable, Debug, Default, serde::Serialize, serde::Deserialize,
)]
struct Input(u8);

const UP: u8 = 1 << 0;
const DOWN: u8 = 1 << 1;
const LEFT: u8 = 1 << 2;
const RIGHT: u8 = 1 << 3;

/// Deterministic scripted input — no real players, so both sim instances
/// (and any faithful re-simulation GGRS does internally) see identical
/// input for a given frame.
fn scripted_input(frame: u64, player: usize) -> Input {
    let phase = (frame + player as u64 * 7) % 8;
    let mut bits = 0u8;
    if phase < 2 {
        bits |= UP;
    } else if phase < 4 {
        bits |= RIGHT;
    } else if phase < 6 {
        bits |= DOWN;
    } else {
        bits |= LEFT;
    }
    Input(bits)
}

struct GgrsConfig;
impl Config for GgrsConfig {
    type Input = Input;
    type InputPredictor = PredictRepeatLast;
    type State = GameState;
    type Address = std::net::SocketAddr;
}

#[derive(Clone)]
struct GameState {
    players: [(i32, i32); 2],
}

impl GameState {
    fn new() -> Self {
        GameState {
            players: [(0, 0), (0, 0)],
        }
    }

    fn advance(&mut self, inputs: &[Input; 2]) {
        for (p, inp) in self.players.iter_mut().zip(inputs.iter()) {
            if inp.0 & UP != 0 {
                p.1 += 1;
            }
            if inp.0 & DOWN != 0 {
                p.1 -= 1;
            }
            if inp.0 & LEFT != 0 {
                p.0 -= 1;
            }
            if inp.0 & RIGHT != 0 {
                p.0 += 1;
            }
        }
    }

    fn serialize(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(16);
        for (x, y) in self.players {
            buf.extend_from_slice(&x.to_le_bytes());
            buf.extend_from_slice(&y.to_le_bytes());
        }
        buf
    }
}

/// Decodes per-player (x, y) back out of `GameState::serialize`'s byte
/// layout — used for Level 2/3 hashing, reading from the (possibly
/// tick-75-corrupted) bytes actually recorded rather than re-deriving
/// from `GameState`, so entity/field hashes reflect the same reality the
/// tick hash does.
fn decode_players(bytes: &[u8]) -> [(i32, i32); 2] {
    let at = |i: usize| i32::from_le_bytes(bytes[i..i + 4].try_into().unwrap());
    [(at(0), at(4)), (at(8), at(12))]
}

/// Runs one local GGRS `SyncTestSession` for `NUM_TICKS` frames, calling
/// `on_confirmed_tick(tick, state_bytes)` once per frame as it's
/// confirmed. `check_distance(0)` keeps this a clean one-request-per-tick
/// demo of hooking a real GGRS session rather than also exercising
/// GGRS's own rollback re-simulation bookkeeping.
fn run_sim(corrupt_at: Option<u64>, mut on_confirmed_tick: impl FnMut(u64, Vec<u8>)) {
    let mut sess = SessionBuilder::<GgrsConfig>::new()
        .with_num_players(2)
        .unwrap()
        .with_check_distance(0)
        .start_synctest_session()
        .unwrap();

    let mut state = GameState::new();
    let mut tick: u64 = 0;

    for frame in 0..NUM_TICKS {
        for player in 0..2 {
            sess.add_local_input(player, scripted_input(frame, player))
                .unwrap();
        }
        let requests = sess.advance_frame().unwrap();
        for req in requests {
            match req {
                GgrsRequest::SaveGameState { cell, .. } => {
                    cell.save(frame as i32, Some(state.clone()), None);
                }
                GgrsRequest::LoadGameState { cell, .. } => {
                    state = cell.load().expect("saved state must exist");
                }
                GgrsRequest::AdvanceFrame { inputs } => {
                    let a = inputs[0].0;
                    let b = inputs[1].0;
                    state.advance(&[a, b]);
                    let mut bytes = state.serialize();
                    if Some(tick) == corrupt_at {
                        // The exact bug class Foldback exists to catch: one
                        // peer's simulation silently drifts from the other.
                        bytes[0] ^= 0xFF;
                    }
                    on_confirmed_tick(tick, bytes);
                    tick += 1;
                }
            }
        }
    }
}

/// Records Level 2 (per-entity) and Level 3 (per-field) hashes for both
/// players at `tick`, into `recording` as `peer_id`'s report — see
/// `Session::hash_entity`/`hash_field` for why these two exist separately
/// from `record_peer_hash` and don't participate in `check_divergence`.
fn record_entity_and_field_hashes(
    recording: &mut FoldbackSession,
    tick: u64,
    peer_id: u16,
    bytes: &[u8],
) {
    for (entity_id, (x, y)) in decode_players(bytes).into_iter().enumerate() {
        let entity_id = entity_id as u64;
        let entity_bytes = [x.to_le_bytes(), y.to_le_bytes()].concat();
        recording
            .record_peer_entity_hash(tick, peer_id, entity_id, hash_bytes(&entity_bytes))
            .unwrap();
        for (field_name, value) in [("position.x", x), ("position.y", y)] {
            let value_bytes = value.to_le_bytes();
            recording
                .record_peer_field_hash(
                    tick,
                    peer_id,
                    entity_id,
                    field_name,
                    hash_bytes(&value_bytes),
                    &value_bytes,
                )
                .unwrap();
        }
    }
}

fn main() {
    println!("=== Foldback ggrs-demo ===\n");

    let out_path = std::env::temp_dir().join("foldback-ggrs-demo.foldback");
    let mut recording = FoldbackSession::builder()
        .tick_rate_hz(60)
        .peer_count(2)
        .record_to(&out_path)
        .build()
        .unwrap();

    println!("Running peer 0's GGRS session ({NUM_TICKS} ticks, clean)...");
    run_sim(None, |tick, bytes| {
        let hash = hash_bytes(&bytes);
        recording.record_peer_hash(tick, 0, hash).unwrap();
        if ENTITY_FIELD_WINDOW.contains(&tick) {
            record_entity_and_field_hashes(&mut recording, tick, 0, &bytes);
        }
    });

    println!(
        "Running peer 1's GGRS session ({NUM_TICKS} ticks, divergence injected at tick {DIVERGE_AT_TICK})..."
    );
    run_sim(Some(DIVERGE_AT_TICK), |tick, bytes| {
        let hash = hash_bytes(&bytes);
        recording.record_peer_hash(tick, 1, hash).unwrap();
        if ENTITY_FIELD_WINDOW.contains(&tick) {
            record_entity_and_field_hashes(&mut recording, tick, 1, &bytes);
        }
    });

    let divergence = recording.check_divergence();
    match divergence {
        Some(d) => println!(
            "\ncheck_divergence() -> tick {} (expected {DIVERGE_AT_TICK})",
            d.0
        ),
        None => println!("\ncheck_divergence() -> none (unexpected!)"),
    }
    assert_eq!(divergence.map(|d| d.0), Some(DIVERGE_AT_TICK));

    recording.finish_recording().unwrap();
    println!("\nRecorded a real two-peer .foldback session to:");
    println!("  {}", out_path.display());
    println!("Open it in the Foldback UI (cargo run -p foldback-ui), or inspect it with:");
    println!(
        "  cargo run -p foldback-cli -- analyze {}",
        out_path.display()
    );
}
