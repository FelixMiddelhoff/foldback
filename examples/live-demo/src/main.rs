// SPDX-License-Identifier: MIT OR Apache-2.0
//! `examples/live-demo` — proves live mode end to end (protocol spec §2):
//! a game embeds `LiveServer`, streams real frames as they happen, and
//! the Foldback UI's "Connect live…" renders them arriving in real time,
//! including an injected divergence — the same bug class
//! `examples/minimal-rust`/`ggrs-demo` demonstrate for the offline-file
//! path, proven here for the live path instead.
//!
//! Two simulated peers from one process, same simplification
//! `minimal-rust`/`ggrs-demo` already use — a real integration streams
//! its own local peer's hashes and receives the other peer's over the
//! game's own netcode (`Session::take_pending_hashes`/`record_peer_hash`,
//! cookbook recipe 1), not both from a single `LiveServer`.
//!
//! Run: `cargo run -p live-demo`, then in another terminal `cargo run -p
//! foldback-ui` and click "Connect live…" (default `ws://127.0.0.1:9871`
//! matches this example's port) within the 5-second connect window.

use std::thread::sleep;
use std::time::Duration;

use foldback_core::format::Frame;
use foldback_core::hash::hash_bytes;
use foldback_core::live::LiveServer;

const NUM_TICKS: u64 = 90;
const DIVERGE_AT_TICK: u64 = 45;
const TICK_RATE_HZ: u64 = 20; // slower than a real 60Hz so it's watchable live

#[derive(Clone)]
struct World {
    entities: Vec<(i64, i64, i64, i64)>, // (x, y, vx, vy) — see minimal-rust for why integer, not float
}

impl World {
    fn new() -> Self {
        let entities = (0..8)
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

fn main() {
    println!("=== Foldback live-demo ===\n");

    let live = LiveServer::bind("127.0.0.1:9871", TICK_RATE_HZ as u32, 2, [0u8; 16])
        .expect("failed to bind ws://127.0.0.1:9871 — is it already in use?");
    println!("Live server listening on ws://127.0.0.1:9871");
    println!(r#"In another terminal: cargo run -p foldback-ui, then click "Connect live…""#);
    println!("(accepts the default URL as-is)\n");
    println!("Streaming starts in 5 seconds — connect before then to see it from tick 0.");
    sleep(Duration::from_secs(5));

    let mut world_a = World::new();
    let mut world_b = World::new();
    println!("Streaming {NUM_TICKS} ticks at {TICK_RATE_HZ}Hz, divergence injected at tick {DIVERGE_AT_TICK}...");

    for tick in 0..NUM_TICKS {
        world_a.tick();
        world_b.tick();
        let bytes_a = world_a.serialize();
        let mut bytes_b = world_b.serialize();
        if tick == DIVERGE_AT_TICK {
            // The exact bug class Foldback exists to catch: one peer's
            // simulation silently drifts from the other.
            bytes_b[0] ^= 0xFF;
        }
        live.send_frame(Frame::TickHash {
            tick,
            peer_id: 0,
            hash: hash_bytes(&bytes_a),
        });
        live.send_frame(Frame::TickHash {
            tick,
            peer_id: 1,
            hash: hash_bytes(&bytes_b),
        });
        sleep(Duration::from_millis(1000 / TICK_RATE_HZ));
    }

    live.send_frame(Frame::EndOfStream);
    println!("\nDone streaming — the UI's view stays live-connected and inspectable.");
    sleep(Duration::from_secs(3));
}
