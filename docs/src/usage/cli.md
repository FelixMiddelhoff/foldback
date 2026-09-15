# CLI Reference

The `foldback` binary, built from `crates/foldback-cli`.

```bash
cargo install --path crates/foldback-cli   # until published to crates.io
```

## `foldback analyze <file>`

Prints a human-readable report of a `.foldback` session file: header fields, frame counts by type, whether the file ended cleanly (has an `EndOfStream` frame — its absence means the recording was cut short, e.g. a crash mid-recording), and the first divergence tick found, if any.

```
$ foldback analyze session.foldback
format_version : 1
tick_rate_hz   : 60
peer_count     : 2
build_id       : 00000000000000000000000000000000
ended cleanly  : true

tick hashes    : 20
entity hashes  : 0
field hashes   : 0
snapshots      : 0
metadata       : 0

divergence     : tick 6
```

Exit codes: `0` on success (a report was printed — this does *not* mean the session was clean, check the `divergence` line), `2` if the file couldn't be opened or parsed.

## `foldback ci-check <file>`

Same analysis, but exits with a CI-friendly contract instead of a human report:

- **Exit 0**: no divergence found. Prints a one-line summary to stdout.
- **Exit 1**: divergence found. Prints `DIVERGENCE DETECTED at tick N` and a short report to stderr.
- **Exit 2**: the file couldn't be opened or parsed.

```bash
foldback ci-check session.foldback || exit 1
```

This exit-code contract is tested explicitly (`crates/foldback-cli/tests/golden.rs`) since external CI pipelines script against it directly — see [CI Integration](ci-gate.md) for a full workflow example.

## `foldback schema-diff <before> <after>`

Schema-drift detection for reflective hashing ([Reflective/Auto Hashing](../integrations/reflective-hashing.md)): compares two `.foldback` files' recorded schemas — each type a reflective walker hashed, and the sorted set of `#[foldback(hash)]`-tagged fields it saw for that type (recorded once per type per session via `Session::record_schema`) — and reports any type present in both whose tagged-field set changed. Catches "someone added a field mid-development" before it's mistaken for a real divergence.

```
$ foldback schema-diff old-build.foldback new-build.foldback
SCHEMA DRIFT: my_game::Unit
  before : hp,pos
  after  : hp,pos,shield
```

Exit codes: `0` if no drift (also the result if a file has no schema metadata at all — e.g. it only used explicit hashing, never reflective), `1` if drift is found, `2` if a file couldn't be opened or parsed.

## `foldback record` — not yet implemented

Deferred: its natural mechanism (either a live-mode WS connection, or a `--sim-binary` replay-and-diff orchestrator) isn't designed yet, and live mode itself is Phase 2. Use `Session::builder().record_to(path)` at the library level today (see [Sessions, Files, and Live Mode](../guide/sessions.md)) — `examples/minimal-rust` demonstrates it end to end.
