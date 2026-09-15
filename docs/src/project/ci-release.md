# CI & Releases

## CI

```
fmt-and-lint            cargo fmt --check + clippy -D warnings --all-features
core-test               cargo test --workspace --all-features, matrix: ubuntu/windows/macos
determinism             hash::tests::known_vectors, matrix: ubuntu/windows/macos/
                        ARM64(×2), each at opt-level 0 and 3
bindings-unity-cs       .NET P/Invoke harness against a real built foldback_sys,
                        matrix: ubuntu/windows/macos
bindings-unity-il2cpp   a real Unity Editor + IL2CPP AOT player build/run, windows-latest
bindings-godot          a real headless Godot 4.7.2 engine, matrix: ubuntu/windows/macos
fuzz-smoke              cargo-fuzz against the session-file parser, 60s bounded run,
                        ubuntu-latest only (libFuzzer needs a real clang toolchain —
                        confirmed not to link on MSVC)
```

Triggers: `pull_request`, `push` to `main`, `workflow_dispatch`. All jobs above are required to merge (branch protection on `main`) except `bindings-unity-il2cpp`, which needs real `UNITY_EMAIL`/`UNITY_PASSWORD` secrets and so only runs meaningfully on this repo's own pushes, not arbitrary forks' PRs. The `determinism` job is treated as a release blocker if it ever fails — never a flaky-retry candidate, since a desync tool whose own hash isn't cross-platform-stable is self-defeating.

`cli-golden` tests (golden-file tests in `crates/foldback-cli/tests/golden.rs`) and `foldback-ui`'s own unit tests already run as part of `core-test`'s `cargo test --workspace --all-features` — no separate jobs needed while that stays fast. The Unreal binding has no CI leg at all (no scriptable install path for Unreal Engine the way `unity-setup`/a downloaded Godot binary provide for the other two) — verified locally against a real Unreal Engine 5.8.2 build instead; see `bindings/unreal/README.md`.

## Documentation site

This site — built with mdBook, deployed to GitHub Pages on every merge to `main` that touches `docs/**`, independent of the crate/UI release cadence.

## Repository topology

Single monorepo — one `Cargo.toml` workspace, one CI pipeline, one version-bump PR can touch core+CLI+UI+bindings together when they need to move in lockstep (e.g. a protocol version bump), rather than coordinating releases across five separate repos.

## Versioning scheme

- `foldback-core`/`foldback-sys`/`foldback-rs`/`foldback-cli` share one workspace version, bumped together — they're tightly coupled enough that independent versioning would just create confusing "which CLI version works with which core" questions.
- `foldback-ui` versions independently — a UI bugfix release shouldn't force a crates.io core bump, and vice versa.
- Each engine binding versions independently (its own package manifest — `package.json` for the Unity UPM package, `plugin.cfg` for the Godot addon) and declares a compatible core-version range.
- The `.foldback` file format and live-mode protocol get their own version number (`format_version`, `protocol_version`), independent of crate versions — two different `foldback-cli` versions might both speak `format_version: 1`.
- Pre-1.0 (`0.x`) for everything until the API has had real usage — not rushing to `1.0.0` just to look mature.
