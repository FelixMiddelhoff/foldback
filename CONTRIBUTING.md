# Contributing to Foldback

## Building and testing

```bash
cargo build --workspace
cargo test --workspace
```

Or, with [`just`](https://github.com/casey/just) installed, run the same gate CI runs:

```bash
just check
```

This runs `cargo fmt --check`, `cargo clippy -D warnings`, and `cargo test --workspace` — matching the required CI job. Run it locally before pushing.

## Toolchain

The Rust version is pinned in `rust-toolchain.toml`. `rustup` will pick it up automatically; you don't need to install it separately.

## Determinism-sensitive code

Any change touching `foldback-core`'s hashing/serialization path, `foldback-sys`'s FFI/C ABI boundary, an engine binding's reflective-hashing walker, or `.foldback` session-file/protocol frame parsing is determinism-sensitive — a subtle bug here produces a false desync report or, worse, a silent false negative. Review such changes with extra care; contributors using Claude Code can invoke the `foldback-determinism-review` skill (bundled in `.claude/skills/`) for a checklist pass.

## Submitting a pull request

1. Open an issue first for anything non-trivial (new feature, API change, protocol change) — cheap to discuss before code exists, expensive to discuss after.
2. Keep PRs scoped to one change. A protocol version bump touching core+CLI+UI+bindings together is fine (monorepo, lockstep release); an unrelated drive-by refactor bundled into a feature PR is not.
3. Fill in the PR template's changelog-relevant-description checkbox — it feeds the changelog automation.
4. CI must pass (fmt, clippy, tests, and the `determinism` cross-platform job) before merge.

## Documentation

Docs live under `docs/src/` (mdBook), deployed to GitHub Pages on every merge to `main` touching `docs/**`. Style conventions: [`docs/STYLE.md`](docs/STYLE.md). Build locally with:

```bash
cargo install mdbook
mdbook serve docs
```

## Code of Conduct

This project follows the [Contributor Covenant](CODE_OF_CONDUCT.md).
