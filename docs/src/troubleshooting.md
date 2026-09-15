# Troubleshooting

This page grows from real reported problems.

## Foldback reports a divergence I can't explain in my own simulation logic

**Read this section before filing a bug against Foldback.** Foldback can only be as deterministic as what's fed into it — if a divergence is real but the cause isn't in your gameplay code, it's almost always one of these:

- **A compiler optimization flag on your own build**, most commonly `-ffast-math` or aggressive FMA (fused multiply-add) contraction. These can change floating-point results in ways that are still IEEE-754-*legal* per compiler but not bit-identical across platforms/builds — Foldback will correctly and faithfully report the resulting divergence, because it *is* a real difference in the hashed state, just not a bug in your simulation. If you're optimizing for performance, check your build flags first before debugging gameplay logic. This is a well-documented, real class of bug — not a hypothetical.
- **A non-deterministic field accidentally included in what you hash** — a wall-clock timestamp, a `Instant::now()`-derived value, an uninitialized-memory read, a hash-map iteration order (if you're hashing a container yourself outside Foldback's own field-hashing helpers, which already sort `HashMap`/`HashSet` for you). Double-check exactly which fields `#[foldback(hash)]` (or your reflective walker's tag) actually covers — the opt-in design (cookbook §5) exists specifically to make this list explicit and reviewable, not to eliminate the mistake entirely.
- **A race condition in your own state capture** — reading state for hashing before all of a tick's writes have landed (a threading issue in your game's own update loop, not in Foldback).

None of the above are bugs in Foldback: the whole point of the divergence detector is to report differences faithfully, even when the cause is upstream of it. If you've ruled out all three and still see an unexplained divergence, that's worth a real issue report — see below.

## `foldback analyze`/`ci-check` errors with "session file magic bytes do not match FBK1"

The file isn't a valid `.foldback` file — either it's a different file entirely, or it's been corrupted/truncated before any valid header was written. Check the file was actually produced by `Session::builder().record_to(...)`.

## `ended cleanly: false` in an `analyze` report

Expected for a session where `finish_recording()` was never called — most commonly, the process crashed or exited before recording finished. This is by design: the format is truncation-tolerant, so everything recorded up to the cut is still valid and analyzable, but the report tells you honestly that it's an incomplete recording rather than pretending otherwise.

## `divergence: none found` but I know something's wrong

`check_divergence`/`find_first_divergence` only compares ticks where 2 or more peers have reported a hash — a tick with only one peer's data isn't flagged as clean, it's just not comparable yet. Make sure every peer is actually calling `record_peer_hash` (or `hash_tick` for the local peer) for every tick.

Also: Level 1 (per-tick) hashing only tells you *which tick* diverged, nothing finer. If you need to know which entity or field, that's Level 2/3 — not built yet, see [Hashing Levels](guide/hashing-levels.md).

## Filing a real issue

If your problem isn't covered here, [open an issue](https://github.com/FelixMiddelhoff/foldback/issues) with a minimal reproduction — a `.foldback` file or a small repro project, per the bug report template.
