# Determinism Pitfalls

Foldback tells you *that* and *where* your simulation diverged — it doesn't fix non-determinism in your own code. These are the bug classes that most commonly cause the "divergence" Foldback will report, worth knowing before you go looking.

## Floating point

The single most common source of cross-platform/cross-compiler divergence:
- **FMA (fused multiply-add) contraction**: the compiler may fuse `a * b + c` into a single instruction with different rounding than separate multiply-then-add, and whether it does depends on target CPU, optimization level, and compiler flags — identical source, different binary output, different result.
- **Denormal handling**: some platforms flush denormals to zero by default, others don't; a value that should be a tiny denormal can silently become exactly `0.0` on one platform and not another.
- **Transcendental functions** (`sin`, `cos`, `sqrt`, etc.): not guaranteed bit-identical across libm implementations, even for the same input.

This isn't hypothetical — see `blog-bitexact-fma-hunt.md` in the Poncelet project (Foldback's sibling determinism-focused project) for a real investigation into exactly this bug class.

## Hash map/set iteration order

Iterating a `HashMap`/`HashSet` (or equivalents in other languages) is not guaranteed to visit entries in a consistent order across runs, processes, or platforms — if your serialization walks one of these directly, two otherwise-identical states can serialize to different byte sequences and hash differently. Use an ordered structure (`BTreeMap`, a sorted `Vec`) for anything that feeds a hash.

## Uninitialized memory / padding bytes

Struct padding bytes are not guaranteed zeroed, and hashing a `#[repr(C)]` struct's raw bytes (rather than its logical fields) can pick up garbage that varies run to run without any logical state actually differing. Serialize logical fields explicitly rather than hashing raw memory when this matters.

## Timestamps, random IDs, debug-only fields

The exact failure mode [RFC-0003](../project/rfcs/0003-field-hashing-opt-in.md) is designed around: a field that's genuinely non-deterministic by design (a wall-clock timestamp, a debug label) gets included in what's hashed, and now every run "diverges" for a reason that has nothing to do with your actual simulation bug. Foldback's opt-in field hashing exists specifically so this can't happen silently — but if you're hashing a whole serialized blob at Level 1, the same discipline is on you.

## Compiler/optimization-level differences

Two builds of the same source at different optimization levels (or on different compilers) can produce different floating-point codegen even with identical semantics on paper — this is exactly what the project's own `determinism` CI job checks for (running the same test at `-O0` and `-O3` and asserting identical hashes), and worth doing in your own CI too if float determinism matters to your simulation.
