# Testing Strategy

## Current status

Every crate and binding carries real tests, not just compiled code: unit/proptest coverage in `foldback-core`, golden-file/integration tests in `foldback-cli`, FFI-boundary tests in `foldback-sys` (run against a real built native library), and each engine binding verified against its own real toolchain in CI — a .NET P/Invoke harness *and* a real IL2CPP AOT build for Unity, a real headless Godot engine for Godot, a real locally-built Unreal Engine for Unreal (no hosted CI leg — see `bindings/unreal/README.md` for why). All green on every push (`fmt`, `clippy`, cross-OS `test`, `determinism`, and a `fuzz-smoke` job) — exact test counts drift too fast to keep accurate here, check CI for the current numbers. A `cargo-fuzz` harness for the session-file parser — the highest-value fuzz target per §1.3 below — is set up and runs a bounded smoke pass (60s) on every push; a longer nightly/scheduled campaign isn't set up yet.

## 1. Core library (`foldback-core`)

**Unit tests** — hash function known-vectors, ring buffer wraparound/retention/capacity edge cases, snapshot compression round-trips (empty/tiny/large, corrupted-data handling), session file format round-trips for every frame type plus truncated-file recovery, bisection engine given a synthetic hash tree with a known injected divergence. All implemented.

**Property-based tests** (`proptest`) — session file round-trip over arbitrary frame sequences, bisection engine over arbitrary hash trees with a randomly-placed single injected divergence, ring buffer retention invariant over arbitrary push sequences. Implemented for the ring buffer and bisection engine; the full session-file-round-trip property test isn't written yet.

**Fuzzing** (`cargo-fuzz`) — set up for the session file parser, the highest-value target: it will eventually be fed real files from the wild (users attaching `.foldback` files to bug reports), including corrupted/truncated/adversarial ones. `crates/foldback-core/fuzz/fuzz_targets/parse_session_file.rs` feeds arbitrary bytes through `Header::read_from` + `FrameReader`, checking only that the format's own contract holds (protocol-spec.md §1.2: a malformed/truncated file returns `Err` or yields a valid prefix, never panics or hangs) — CI's `fuzz-smoke` job runs it for 60s on every push, ubuntu-latest only (cargo-fuzz/libFuzzer needs a real clang toolchain for its sanitizer-coverage runtime; confirmed not to link on MSVC). A longer scheduled/nightly campaign — hours instead of seconds, actually likely to find something a 60s smoke pass won't — isn't set up yet. The C ABI surface (`foldback-sys`, shipped) is a second target, not yet fuzzed.

**Cross-platform determinism tests** — the one category that isn't "does the code work" but "does the *hash* mean what it claims to mean." Implemented as the CI `determinism` job: `hash::tests::known_vectors` runs across Linux/Windows/macOS × x86_64/ARM64, asserting a fixed input hashes to a pinned constant on every platform, plus the same test at `-O0` and `-O3` to catch compiler-introduced non-determinism independent of the game's own code.

## 2. CLI (`foldback-cli`)

Golden-file tests against checked-in fixtures (`crates/foldback-cli/tests/fixtures/`: clean, diverging, and truncated sessions) assert exact output. The `ci-check` exit-code contract (0 clean, non-zero on divergence) is tested explicitly, since external CI pipelines script against it directly. Argument-parsing edge cases (missing file, malformed file, missing argument) assert a clear error message and correct exit code, never a panic. All implemented.

## 3. Engine bindings (planned)

Per binding, two tiers once a binding exists: binding-level unit tests (does FFI marshaling round-trip correctly?) and an integration demo doubling as a live test (run two instances headless, inject a deliberate divergence, assert Foldback detects and bisects to the injected point).

## 4. UI (planned)

Component tests (Vitest), visual regression tests (Playwright screenshot diffing against baseline images, light and dark theme), an E2E smoke test (drag a fixture file onto the window, assert the timeline renders and the drill-down panel shows the expected diff), and a small manual QA checklist before each release.

## 5. CI pipeline shape

```
fmt-and-lint    cargo fmt --check + clippy -D warnings
core-test       cargo test --workspace (Linux/Windows/macOS matrix)
determinism     cross-platform + ARM64 hash-equality, opt-level 0 vs 3
```
Implemented — see [CI & Releases](ci-release.md) for the full job graph including the nightly/slow tier (fuzzing, engine-binding integration, UI visual/e2e) once those exist.

## 6. What NOT to test

Not chasing 100% line coverage as a target — the property-based and (eventual) fuzz tests on the session-file/bisection core are worth far more than incidental coverage of CLI argument-parsing branches. Not testing Tauri's own window-chrome plumbing, or third-party component internals, once the UI exists — test Foldback's usage of them, not their own correctness.
