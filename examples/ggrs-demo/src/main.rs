// SPDX-License-Identifier: MIT OR Apache-2.0
//! `examples/ggrs-demo` — proves Foldback layered on a real GGRS rollback
//! session, not just a bare tick loop (see `examples/minimal-rust` for
//! that). Two independent local `SyncTestSession`s stand in for two
//! network peers running the same GGRS-driven simulation; each tick's
//! confirmed state is hashed and recorded into one shared, two-peer
//! `.foldback` file — including one injected divergence, so the recorded
//! file is real evidence for the Foldback UI (Master Sequence §4) rather
//! than a synthetic fixture.
//!
//! `foldback-rs`'s `attach_foldback` GGRS extension (cookbook recipe 3)
//! isn't built yet — that's Phase 2. This example calls
//! `foldback_core::hash::hash_bytes` and `Session::record_peer_hash`
//! directly, the same Level-1 API `minimal-rust` uses.
//!
//! Run: `cargo run -p ggrs-demo`

use bytemuck::{Pod, Zeroable};
use foldback_core::hash::hash_bytes;
use foldback_core::session::Session as FoldbackSession;
use ggrs::{Config, GgrsRequest, PredictRepeatLast, SessionBuilder};

const NUM_TICKS: u64 = 120;
const DIVERGE_AT_TICK: u64 = 75;

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
    });

    println!(
        "Running peer 1's GGRS session ({NUM_TICKS} ticks, divergence injected at tick {DIVERGE_AT_TICK})..."
    );
    run_sim(Some(DIVERGE_AT_TICK), |tick, bytes| {
        let hash = hash_bytes(&bytes);
        recording.record_peer_hash(tick, 1, hash).unwrap();
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
