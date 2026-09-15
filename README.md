# Foldback

[![CI](https://github.com/FelixMiddelhoff/foldback/actions/workflows/ci.yml/badge.svg)](https://github.com/FelixMiddelhoff/foldback/actions/workflows/ci.yml)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#license)

![Foldback UI opening a real recorded session, scrubbing between a clean tick and the exact tick two peers diverged](assets/foldback-ui-demo.gif)

*The UI opening a real `.foldback` file recorded by `examples/ggrs-demo` (an actual GGRS rollback session with an injected desync) — scrubbing from a clean tick to the exact tick it diverged.*

RimWorld Multiplayer, O3DE, and GGRS's `SyncTestSession` have all solved this exact problem, each locked inside one project. Foldback is the version you don't have to rebuild yourself: hash simulation state on a schedule, compare hashes across peers, and bisect down to the tick — and, with opt-in field hashing, the exact field — where two clients' simulations diverged. Reflective (auto) hashing and a real in-editor visibility dock are available for Bevy, Unity, and Godot. Not yet published to crates.io — depend on it as a git dependency for now (see the quickstart below).

## 60-second quickstart

The entire integration cost for Level-1 (tick-only) divergence detection: one call per tick.

```toml
[dependencies]
foldback-core = { git = "https://github.com/FelixMiddelhoff/foldback", package = "foldback-core" }
```

```rust
use foldback_core::session::Session;

let mut session = Session::builder()
    .tick_rate_hz(60)
    .peer_count(2)
    .build()?;

// each simulation tick:
let state_bytes = bincode::serialize(&world.deterministic_state())?;
session.hash_tick(tick, &state_bytes)?;

// exchange session.take_pending_hashes() with peers over your existing
// netcode channel — a handful of bytes per tick.
```

If a peer's hash for a tick doesn't match, `session.check_divergence()` returns `Some(DivergenceTick(tick))` the moment enough peers have reported that tick. See the [Cookbook](https://felixmiddelhoff.github.io/foldback/cookbook/README.html) for the full recipe set, including Unity/Godot/Unreal.

## Pick your engine

| Engine | What you get |
|---|---|
| [Rust / GGRS / Bevy](https://felixmiddelhoff.github.io/foldback/integrations/rust-ggrs.html) | Core API, a GGRS bridge, explicit + reflective hashing, an in-editor `bevy_egui` dock |
| [Unity](https://felixmiddelhoff.github.io/foldback/integrations/unity.html) | Explicit + reflective hashing, an `EditorWindow` dock, IL2CPP AOT support |
| [Godot](https://felixmiddelhoff.github.io/foldback/integrations/godot.html) | Explicit + reflective hashing, an in-editor dock plugin |
| [Unreal Engine](https://felixmiddelhoff.github.io/foldback/integrations/unreal.html) | Explicit binding, reflective hashing, a Mass Entity integration |

Docs: **[felixmiddelhoff.github.io/foldback](https://felixmiddelhoff.github.io/foldback/)** — start with [Quickstart](https://felixmiddelhoff.github.io/foldback/getting-started/quickstart.html).

## License

Licensed under either of

* Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
* MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.
