---
rfc: 0005
title: Bisection granularity model and its snapshot-restore limitation
status: accepted
created: 2026-09-14
supersedes:
---

# Summary

Ratifies the tick→entity→field bisection model as Foldback's central mechanism, and formally documents a real limitation: bisection can only narrow to whatever granularity of hash data was actually recorded (Level 1/2/3) — narrowing below that requires re-simulating from a snapshot with finer instrumentation enabled, which requires the game to have deterministic snapshot-restore capability.

# Motivation

The bisection model (main plan §3.4) is the product's actual premise. It has a real, previously-undocumented limitation (`foldback-risk-plan.md` T1) worth ratifying as an explicit, accepted-with-eyes-open design constraint rather than leaving it as a footnote discovered later by a confused user. The RFC format is the right place to make a limitation this central official rather than incidental.

# Design

Bisection reads whatever granularity of hash data was actually recorded at capture time (Level 1 combined-blob, Level 2 per-entity, or Level 3 per-field, per the cookbook). Narrowing *below* the recorded granularity requires re-simulating from a snapshot with finer instrumentation enabled. This requires the game itself to support deterministic snapshot-restore — rollback-style games have this by construction; a pure lockstep-without-rollback game may not.

# Drawbacks

This is a real gap in the "bisect to find exactly what diverged" pitch for one class of integration (lockstep-without-rollback games lacking snapshot-restore). Accepted as an honest, documented limitation — stated plainly in the guide, per the risk plan's mitigation — rather than something to silently under-promise around or discover via a support issue.

# Alternatives considered

Always recording at the finest granularity (Level 3, per-field) by default was considered and rejected — the perf-spike work shows headroom exists, but always-maximal instrumentation still isn't free, and it would remove the explicit level-selection tooling exists to provide (per cookbook recipes 1–4, users choose granularity deliberately based on their integration's needs).

# Prior art

The general shape — coarse continuous monitoring with the option to re-run at finer instrumentation once a problem window is identified — mirrors how sampling profilers and most production tracing tools handle the same fidelity-vs-overhead tradeoff.

# Unresolved questions

Whether `foldback-cli` should grow a first-class `--resimulate <sim-binary>` mode that calls back into a game's own binary to re-run from a snapshot generically — flagged in the risk plan as needing a spike against the real GGRS demo before deciding. Not decided here.

# History

- 2026-09-14: retroactively drafted and accepted during repo bootstrap, decision made during the original planning pass (see `foldback-plan.md` §3.4 and `foldback-risk-plan.md` T1).
