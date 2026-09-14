# CI & Releases

## Current CI (implemented)

```
fmt-and-lint    cargo fmt --check + clippy -D warnings
core-test       cargo test --workspace, matrix: ubuntu/windows/macos
determinism     hash::tests::known_vectors, matrix: ubuntu/windows/macos/
                ARM64(×2), each at opt-level 0 and 3
```

Triggers: `pull_request`, `push` to `main`, `workflow_dispatch`. All three jobs above are required to merge (branch protection on `main`). The `determinism` job is treated as a release blocker if it ever fails — never a flaky-retry candidate, since a desync tool whose own hash isn't cross-platform-stable is self-defeating.

`cli-golden` tests (golden-file tests in `crates/foldback-cli/tests/golden.rs`) already run as part of `core-test`'s `cargo test --workspace` — no separate job needed while that stays fast. `ui-unit` will be added once `foldback-ui` exists.

## Planned nightly/slow tier (not built yet)

```
fuzz-full        cargo-fuzz, longer duration than a PR-time smoke run
ui-visual        Playwright screenshot diff
ui-e2e           Playwright against a built Tauri app
bindings-unity   Unity batch-mode divergence-injection integration test
bindings-godot   Godot --headless divergence-injection integration test
bindings-unreal  Unreal automation commandlet (Phase 5)
```

## Documentation site

This site — built with mdBook, deployed to GitHub Pages on every merge to `main` that touches `docs/**`, independent of the crate/UI release cadence.

## Repository topology

Single monorepo — one `Cargo.toml` workspace, one CI pipeline, one version-bump PR can touch core+CLI+UI+bindings together when they need to move in lockstep (e.g. a protocol version bump), rather than coordinating releases across five separate repos.

## Versioning scheme

- `foldback-core`/`foldback-sys`/`foldback-rs`/`foldback-cli` share one workspace version, bumped together — they're tightly coupled enough that independent versioning would just create confusing "which CLI version works with which core" questions.
- `foldback-ui` will version independently once it exists — a UI bugfix release shouldn't force a crates.io core bump, and vice versa.
- Engine bindings will each version independently, but declare a compatible core-version range, once they exist.
- The `.foldback` file format and live-mode protocol get their own version number (`format_version`, `protocol_version`), independent of crate versions — two different `foldback-cli` versions might both speak `format_version: 1`.
- Pre-1.0 (`0.x`) for everything until the API has had real usage — not rushing to `1.0.0` just to look mature.

## Definition of "ready to bootstrap" (historical — already passed)

Confirmed before repo bootstrap actually ran: name locked (Foldback), license locked (MIT/Apache-2.0 dual), the opt-in derive-macro default and `.foldback` frame layout stable enough that Phase 0 wasn't building against a moving target. All three held, and did — the format and the derive-macro decision haven't needed to change since.
