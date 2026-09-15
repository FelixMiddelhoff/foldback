# Quickstart

This walks through the smallest real integration: hashing your simulation's state once per tick and detecting when two runs disagree. It matches [`examples/minimal-rust`](https://github.com/FelixMiddelhoff/foldback/tree/main/examples/minimal-rust) in the repo — run that if you want working code to poke at rather than read.

> Not yet published to crates.io (pre-1.0). Until then, depend on it as a git or path dependency.

```toml
[dependencies]
foldback-core = { git = "https://github.com/FelixMiddelhoff/foldback", package = "foldback-core" }
```

## 1. Start a session

```rust
use foldback_core::session::Session;

let mut session = Session::builder()
    .tick_rate_hz(60)
    .peer_count(2)   // how many peers will report hashes for comparison
    .build()?;
```

## 2. Hash each tick

Call this once per simulation tick, with whatever bytes your determinism depends on — Foldback never serializes for you, it only hashes what you give it.

```rust
let state_bytes = your_serialization_fn(&world);
session.hash_tick(tick, &state_bytes)?;
```

## 3. Exchange hashes with peers

```rust
for pending in session.take_pending_hashes() {
    // send pending.tick, pending.hash to your peers over your existing
    // netcode channel — a handful of bytes, piggyback on an existing packet.
}

// as hashes arrive from peers:
session.record_peer_hash(tick, peer_id, hash)?;
```

## 4. Check for divergence

```rust
if let Some(divergence) = session.check_divergence() {
    eprintln!("desync at tick {}", divergence.0);
}
```

That's the whole Level 1 (per-tick) integration. See the [Cookbook](../cookbook/README.md) for the CI-gate pattern (single-process double-run testing, no netcode needed), recording a `.foldback` file, and what Level 2/3 (per-entity, per-field) bisection will look like once they ship.

## Inspecting a recorded session

```bash
cargo run -p foldback-cli -- analyze session.foldback
```

See the [CLI Reference](../usage/cli.md) for the full command set.
