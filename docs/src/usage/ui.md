# UI Guide

`foldback-ui` is a Tauri desktop app for inspecting a `.foldback` session file — offline replay only in v1 (live mode is Phase 2).

Run it from a checkout with:

```bash
cargo run -p foldback-ui
```

## Opening a session

Two ways, same result:
- **Drag a `.foldback` file onto the window** — no dialog, no setup.
- Click **Open session…** and pick a file.

## What you're looking at

- **Tick timeline**: the full recorded tick range, green where every reporting peer agreed, red from the first divergence onward. Click anywhere to scrub; the accent line marks your current tick. Accent tick marks are recorded snapshots.
- **Peers at tick N**: each peer's hash for the selected tick, with the odd-one-out highlighted red when they disagree. Below it, the last tick peers agreed on and the nearest snapshot at or before the divergence, when one exists.
- **Session**: which hashing levels this file actually has data for (`tick`, and `entity`/`field` if recorded), whether the recording ended cleanly, and counts of metadata entries and snapshots.
- **Bisection result**: entity/field-level drill-down, when the file has `EntityHash`/`FieldHash` frames for the selected tick (`Session::hash_entity`/`hash_field`, cookbook recipes 4/5 — see `examples/ggrs-demo` for a real recording that includes them). A session with only Level 1 (tick) data shows a plain note here instead of fabricated detail. When field data does exist, the UI shows the raw diverging values side by side and does not guess a root cause — check them against your own simulation code.

See the [Foldback UI mockup](https://claude.ai/code/artifact/98a136d1-d606-4d96-bc90-04e6fe74f292) for the original visual spec this was built against.
