# Foldback

A desync/divergence debugger for lockstep/rollback multiplayer games: hash simulation state on a schedule, compare hashes across peers, and bisect down to the tick and (with opt-in field hashing) the field where two clients' simulations diverged.

Status: Phase 0 done. `foldback-core` (hashing, session file format, Level 1 bisection) and part of `foldback-cli` (`analyze`, `ci-check`) are implemented and tested. No UI, live mode, or engine binding beyond a raw Rust example yet.

Docs: **[felixmiddelhoff.github.io/foldback](https://felixmiddelhoff.github.io/foldback/)** — start with [Quickstart](https://felixmiddelhoff.github.io/foldback/getting-started/quickstart.html).

## License

Licensed under either of

* Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
* MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.
