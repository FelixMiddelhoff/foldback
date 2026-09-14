# Sessions, Files, and Live Mode

A `Session` is one in-process object; how its data reaches other peers or a later analysis is up to you.

## In-process (CI-gate pattern)

Run your simulation twice (or replay a fixed input log twice) in the same process, hash both, compare:

```rust
let mut a = Session::builder().tick_rate_hz(60).peer_count(1).build()?;
let mut b = Session::builder().tick_rate_hz(60).peer_count(1).build()?;

for tick in 0..NUM_TICKS {
    run_one_tick(&mut world_a, tick);
    run_one_tick(&mut world_b, tick);
    a.hash_tick(tick, &serialize(&world_a))?;
    b.hash_tick(tick, &serialize(&world_b))?;
}

assert_eq!(a.finish(), b.finish());
```

`finish()` is a single combined hash over every tick a session has locally hashed — this is `GGRS::SyncTestSession`'s trick, generalized. See [CI Integration](../usage/ci-gate.md) for wiring this into an actual CI job.

## Peer-to-peer (real multiplayer)

Exchange `take_pending_hashes()`/`record_peer_hash()` over your existing netcode channel — a handful of bytes per tick, meant to piggyback on a packet you're already sending, not open a new connection. See the [Quickstart](../getting-started/quickstart.md).

## Recording to a `.foldback` file

```rust
let mut session = Session::builder()
    .tick_rate_hz(60)
    .peer_count(2)
    .record_to("session.foldback")
    .build()?;

// ... hash_tick / record_peer_hash as normal ...

session.finish_recording()?; // writes the closing frame
```

The file format is append-only and streaming-writable — frames are written as they happen, not buffered and flushed at the end — and tolerant of truncation: a crash mid-recording still leaves a file that parses cleanly up to the last complete frame. Full byte layout: [Protocol & File Format Spec](../reference/protocol-spec.md).

## Live mode (game side built, UI side planned — Phase 2)

A running game streams directly over a local WebSocket, using the same frame format as a `.foldback` file — so a reader has one parser for both live and offline analysis. The game-embeddable server (`foldback_core::live::LiveServer`, feature `live`) is built: bind a loopback socket, send frames as they happen, never blocks the game's own thread. See [Cookbook recipe 7](../cookbook/README.md#7-live-mode) for the API. The Foldback UI's WebSocket *client* isn't built yet — `foldback-ui` currently only opens `.foldback` files (drag-and-drop or a dialog), not a live connection. Full wire format: [RFC-0004](../project/rfcs/0004-live-mode-transport.md).
