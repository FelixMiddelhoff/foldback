// SPDX-License-Identifier: MIT OR Apache-2.0
//! `examples/minimal-rust` — the smallest possible Foldback integration,
//! no engine. Proves the Level-1 API end to end (foldback-plan.md §6,
//! Phase 0 exit criterion) against a toy deterministic simulation: no
//! floats, no external RNG, no engine dependency — just enough state to
//! demonstrate hashing, the CI-gate double-run pattern, divergence
//! detection, and recording a real `.foldback` file.
//!
//! Run: `cargo run -p minimal-rust`

use foldback_core::session::Session;

const NUM_ENTITIES: usize = 8;
const NUM_TICKS: u64 = 120;

#[derive(Clone)]
struct World {
    // Fixed-point-ish integer state — deliberately no floats, so this
    // example can't accidentally demonstrate a float-determinism bug
    // instead of the API it's meant to showcase.
    entities: Vec<(i64, i64, i64, i64)>, // (x, y, vx, vy)
}

impl World {
    fn new() -> Self {
        let entities = (0..NUM_ENTITIES)
            .map(|i| (0i64, 0i64, (i as i64 % 3) - 1, (i as i64 % 5) - 2))
            .collect();
        World { entities }
    }

    fn tick(&mut self) {
        for e in &mut self.entities {
            e.0 += e.2;
            e.1 += e.3;
            if e.0.abs() > 100 {
                e.2 = -e.2;
            }
            if e.1.abs() > 100 {
                e.3 = -e.3;
            }
        }
    }

    /// Manual serialization — deliberately not pulling in bincode, to
    /// keep this "no engine, no framework" example's own dependency
    /// footprint at zero beyond foldback-core itself.
    fn serialize(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(self.entities.len() * 32);
        for (x, y, vx, vy) in &self.entities {
            buf.extend_from_slice(&x.to_le_bytes());
            buf.extend_from_slice(&y.to_le_bytes());
            buf.extend_from_slice(&vx.to_le_bytes());
            buf.extend_from_slice(&vy.to_le_bytes());
        }
        buf
    }
}

fn run(world: &mut World, session: &mut Session, corrupt_at_tick: Option<u64>) {
    for tick in 0..NUM_TICKS {
        world.tick();
        let mut bytes = world.serialize();
        if Some(tick) == corrupt_at_tick {
            // Simulate the exact bug class this tool exists to catch: one
            // peer's simulation silently produces different state.
            bytes[0] ^= 0xFF;
        }
        session.hash_tick(tick, &bytes).unwrap();
    }
}

fn main() {
    println!("=== Foldback minimal-rust example ===\n");

    // --- 1. CI-gate pattern (cookbook recipe 2): two clean, independent
    //     runs of the same deterministic sim must produce equal finish(). ---
    let mut world_a = World::new();
    let mut world_b = World::new();
    let mut session_a = Session::builder()
        .tick_rate_hz(60)
        .peer_count(1)
        .build()
        .unwrap();
    let mut session_b = Session::builder()
        .tick_rate_hz(60)
        .peer_count(1)
        .build()
        .unwrap();
    run(&mut world_a, &mut session_a, None);
    run(&mut world_b, &mut session_b, None);

    println!("1. Two clean runs, {NUM_TICKS} ticks each:");
    println!("   run A finish() = {:016x}", session_a.finish());
    println!("   run B finish() = {:016x}", session_b.finish());
    assert_eq!(
        session_a.finish(),
        session_b.finish(),
        "clean runs must match"
    );
    println!("   MATCH — deterministic across two independent runs.\n");

    // --- 2. The failure case: an injected divergence, caught exactly. ---
    let mut world_c = World::new();
    let mut session_c = Session::builder()
        .tick_rate_hz(60)
        .peer_count(1)
        .build()
        .unwrap();
    run(&mut world_c, &mut session_c, Some(75));

    println!("2. A third run with a divergence injected at tick 75:");
    println!("   run C finish() = {:016x}", session_c.finish());
    assert_ne!(
        session_a.finish(),
        session_c.finish(),
        "corrupted run must not match"
    );
    println!("   MISMATCH detected via finish() — as expected.\n");

    // --- 3. Two-peer live comparison via record_peer_hash/check_divergence
    //     (cookbook recipe 1's actual peer-exchange shape), pinpointing
    //     the exact divergent tick rather than just "something's wrong." ---
    let mut compare = Session::builder()
        .tick_rate_hz(60)
        .peer_count(2)
        .build()
        .unwrap();
    for (tick, hash) in replay_hashes(&session_a) {
        compare.record_peer_hash(tick, 0, hash).unwrap();
    }
    for (tick, hash) in replay_hashes(&session_c) {
        compare.record_peer_hash(tick, 1, hash).unwrap();
    }
    let divergence = compare.check_divergence();
    println!("3. Cross-peer comparison (clean session vs. corrupted session):");
    match divergence {
        Some(d) => println!(
            "   check_divergence() -> tick {} — exactly the injected tick.",
            d.0
        ),
        None => println!("   check_divergence() -> none (unexpected!)"),
    }
    assert_eq!(divergence.map(|d| d.0), Some(75));
    println!();

    // --- 4. Record a real .foldback file, analyzable with the CLI. ---
    let out_path = std::env::temp_dir().join("foldback-minimal-rust-example.foldback");
    let mut world_d = World::new();
    let mut recording = Session::builder()
        .tick_rate_hz(60)
        .peer_count(1)
        .record_to(&out_path)
        .build()
        .unwrap();
    run(&mut world_d, &mut recording, None);
    recording.finish_recording().unwrap();

    println!("4. Recorded a real .foldback session to:");
    println!("   {}", out_path.display());
    println!("   Inspect it with:");
    println!(
        "   cargo run -p foldback-cli -- analyze {}",
        out_path.display()
    );
}

/// Re-derive (tick, hash) pairs from a session's retained ring buffer —
/// standing in for "what the game would have received from a peer over
/// the network," for this single-process demo.
fn replay_hashes(session: &Session) -> Vec<(u64, u64)> {
    session.retained_ticks().copied().collect()
}
