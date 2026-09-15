# Testing Strategy

Every crate and binding carries real tests, not just compiled code: unit/proptest coverage in `foldback-core`, golden-file/integration tests in `foldback-cli`, FFI-boundary tests in `foldback-sys` (run against a real built native library), and each engine binding verified against its own real toolchain in CI — a .NET P/Invoke harness *and* a real IL2CPP AOT build for Unity, a real headless Godot engine for Godot, a real locally-built Unreal Engine for Unreal (no hosted CI leg — see `bindings/unreal/README.md` for why). All green on every push (`fmt`, `clippy`, cross-OS `test`, `determinism`, and `fuzz-smoke`) — exact test counts drift too fast to keep accurate here, check CI for the current numbers.

## 1. Core library (`foldback-core`)

**Unit tests** — hash function known-vectors, ring buffer wraparound/retention/capacity edge cases, snapshot compression round-trips (empty/tiny/large, corrupted-data handling), session file format round-trips for every frame type plus truncated-file recovery, bisection engine given a synthetic hash tree with a known injected divergence.

**Property-based tests** (`proptest`) — session file round-trip over arbitrary frame sequences, bisection engine over arbitrary hash trees with a randomly-placed single injected divergence, ring buffer retention invariant over arbitrary push sequences.

**Fuzzing** (`cargo-fuzz`) — the session file parser is the highest-value target: it's fed real files from the wild (users attaching `.foldback` files to bug reports), including corrupted/truncated/adversarial ones. `crates/foldback-core/fuzz/fuzz_targets/parse_session_file.rs` feeds arbitrary bytes through `Header::read_from` + `FrameReader`, checking only that the format's own contract holds (protocol-spec.md §1.2: a malformed/truncated file returns `Err` or yields a valid prefix, never panics or hangs) — CI's `fuzz-smoke` job runs it for 60s on every push, ubuntu-latest only (cargo-fuzz/libFuzzer needs a real clang toolchain for its sanitizer-coverage runtime, which doesn't link on MSVC). A hand-picked adversarial-byte-sequence battery in `crates/foldback-core/tests/fuzz_smoke_sanity.rs` covers the same code path via plain `cargo test`, runnable on any platform.

**Cross-platform determinism tests** — the one category that isn't "does the code work" but "does the *hash* mean what it claims to mean." The CI `determinism` job: `hash::tests::known_vectors` runs across Linux/Windows/macOS × x86_64/ARM64, asserting a fixed input hashes to a pinned constant on every platform, plus the same test at `-O0` and `-O3` to catch compiler-introduced non-determinism independent of the game's own code.

## 2. CLI (`foldback-cli`)

Golden-file tests against checked-in fixtures (`crates/foldback-cli/tests/fixtures/`: clean, diverging, and truncated sessions) assert exact output. The `ci-check` exit-code contract (0 clean, non-zero on divergence) is tested explicitly, since external CI pipelines script against it directly. Argument-parsing edge cases (missing file, malformed file, missing argument) assert a clear error message and correct exit code, never a panic.

## 3. Engine bindings

Two tiers per binding: binding-level unit/FFI-marshaling tests, and an integration demo doubling as a live test — run two instances headless, inject a deliberate divergence, assert Foldback detects and bisects to the injected point. See each binding's own [integration page](../integrations/rust-ggrs.md) for what that looks like concretely (a .NET console harness for Unity, a headless Godot script run, an Unreal Automation Test).

## 4. UI (`foldback-ui`)

Component/session-loading unit tests run as part of `cargo test --workspace`. Visual regression tests (Playwright screenshot diffing against baseline images) and an E2E smoke test (drag a fixture file onto the window, assert the timeline renders and the drill-down panel shows the expected diff) aren't set up yet.

## 5. CI pipeline shape

See [CI & Releases](ci-release.md) for the full job graph.

## 6. What NOT to test

Not chasing 100% line coverage as a target — the property-based and fuzz tests on the session-file/bisection core are worth far more than incidental coverage of CLI argument-parsing branches. Not testing Tauri's own window-chrome plumbing, or third-party component internals — test Foldback's usage of them, not their own correctness.
