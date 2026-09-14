# Performance

## The constraint

Foldback's hot path runs *inside someone else's frame budget* — a 60Hz game has 16.6ms per frame total, and hashing is competing with everything else the game does that frame. The design rule: treat under ~5% of that budget (≈830µs) as the ceiling for combined hashing cost at a given entity count, and a low single-digit percentage as the actual target — a debug-adjacent tool asking for more than that is a hard sell regardless of the exact number.

## Week 0 spike result (validated, not a starting guess)

Before any code was written, a dedicated benchmark spike answered the load-bearing question directly: does the hashing budget hold at realistic RTS-scale entity counts (grounded in real numbers — 5,000–50,000 simulated entities in a busy late-game battle, not a round guess)? Full methodology and all raw numbers: the spike report (this page's first real content, per that report's own plan).

**Result: the budget holds with large margin, confirmed on two tiers (a full-power dev machine and a throttled stand-in) with both mean and p99 tail latency measured.**

| Entities | Full hot path (hash + ring-buffer handoff), worst case (p99, throttled tier) | % of 16.6ms frame |
|---|---|---|
| 100 | 1.48 µs | 0.009% |
| 1,000 | 2.88 µs | 0.017% |
| 5,000 | 15.45 µs | 0.093% |
| 10,000 | 24.85 µs | 0.150% |
| 50,000 | 102.02 µs | 0.615% |
| 100,000 | 193.19 µs | **1.164%** |

Real per-entity marginal cost: **~1.5–1.6 ns/entity**, roughly **31–33x cheaper** than the original unvalidated 50ns/entity starting target from before the spike ran. Scaling is clean and linear throughout — no cache-locality or allocation knee found in Foldback's own code path at any tested entity count, on either tier. Tail latency is tight everywhere measured (p99/p50 ratio ≤1.07) — the specific failure mode of "a spike is worse than consistent slowness" did not occur.

**No mitigation was required** to ship the Level 1 (`hash_tick`) API as originally designed. One mitigation pulled forward anyway: a batched Level 2 API (`hash_entities_batch`), motivated by a flat ~1.7x per-call tax found when hashing per-entity in a loop vs. one combined blob — cheap to design in now, expensive to retrofit as a breaking change later.

## Design rules the hot path follows

- Zero-alloc on the `hash_tick` call itself — the caller serializes, Foldback only hashes bytes it's handed.
- No syscalls on the hot path — file/socket I/O happens on a background thread via a bounded SPSC channel, validated as part of the spike's "full pipeline" measurement above.
- Snapshot compression (zstd) is explicitly off the hot path — it's real cost (single-digit milliseconds at 100,000 entities), confirmed too slow for per-tick use, and only runs at the configured snapshot cadence.

## What's not benchmarked yet

A second tier that's genuinely different hardware (the spike's "tier 2" was the same CPU under core-affinity/priority throttling, not a different machine) and peak memory of the ring buffer + pending-frame queue. Neither blocks the current conclusion given the ~4x+ headroom found, but worth closing before the numbers above go into public marketing copy.

## Benchmark harness

The spike used a standalone `criterion.rs` crate (kept, not thrown away, but not yet merged into this repo — see the spike report for its location). Moving it into `foldback-core` as the permanent regression-benchmark suite, now that real hashing code exists here to benchmark, is still open work, not done yet.
