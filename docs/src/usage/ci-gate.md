# CI Integration

## Today: check an already-recorded file

```bash
foldback ci-check session.foldback
```

Exit codes: `0` clean, `1` divergence found (report on stderr), `2` couldn't read the file. See the full contract in [CLI Reference](cli.md#foldback-ci-check-file).

### Example GitHub Actions step

```yaml
- name: Check for desync
  run: |
    cargo run --release --bin my-sim -- --replay fixtures/match.log --record out.foldback
    foldback ci-check out.foldback
```

## The in-process pattern (no CLI needed)

If your test harness already runs the simulation twice in-process (the `GGRS::SyncTestSession` pattern, generalized), skip the file entirely and compare `finish()` directly:

```rust
let mut a = Session::builder().peer_count(1).build()?;
let mut b = Session::builder().peer_count(1).build()?;

for tick in 0..NUM_TICKS {
    run_one_tick(&mut world_a, tick);
    run_one_tick(&mut world_b, tick);
    a.hash_tick(tick, &serialize(&world_a))?;
    b.hash_tick(tick, &serialize(&world_b))?;
}

assert_eq!(a.finish(), b.finish());
```

This is a plain Rust `assert_eq!` — wire it into whatever test runner you already use (`cargo test`, a custom harness), no `foldback-cli` involved. See [Sessions, Files, and Live Mode](../guide/sessions.md#in-process-ci-gate-pattern).

## Planned: one-command replay orchestration

```bash
foldback ci-check --replay inputs.log --sim-binary ./target/release/my_sim
```

Not implemented yet — would spawn the given sim binary twice against a fixed input log and diff automatically, skipping the need to hand-write the double-run loop above. See the [Cookbook](../cookbook/README.md#2-ci-gate) for the target shape.
