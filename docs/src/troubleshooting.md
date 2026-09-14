# Troubleshooting

This page grows from real reported problems — short for now since the project is Phase 0 and has no external users yet.

## `foldback analyze`/`ci-check` errors with "session file magic bytes do not match FBK1"

The file isn't a valid `.foldback` file — either it's a different file entirely, or it's been corrupted/truncated before any valid header was written. Check the file was actually produced by `Session::builder().record_to(...)`.

## `ended cleanly: false` in an `analyze` report

Expected for a session where `finish_recording()` was never called — most commonly, the process crashed or exited before recording finished. This is by design: the format is truncation-tolerant, so everything recorded up to the cut is still valid and analyzable, but the report tells you honestly that it's an incomplete recording rather than pretending otherwise.

## `divergence: none found` but I know something's wrong

`check_divergence`/`find_first_divergence` only compares ticks where 2 or more peers have reported a hash — a tick with only one peer's data isn't flagged as clean, it's just not comparable yet. Make sure every peer is actually calling `record_peer_hash` (or `hash_tick` for the local peer) for every tick.

Also: Level 1 (per-tick) hashing only tells you *which tick* diverged, nothing finer. If you need to know which entity or field, that's Level 2/3 — not built yet, see [Hashing Levels](guide/hashing-levels.md).

## Filing a real issue

If your problem isn't covered here, [open an issue](https://github.com/FelixMiddelhoff/foldback/issues) with a minimal reproduction — a `.foldback` file or a small repro project, per the bug report template.
