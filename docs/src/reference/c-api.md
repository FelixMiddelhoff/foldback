# C API

**Shipped.** `foldback-sys` is a full C ABI over `foldback-core` — session lifecycle, Level 1/2/3 hashing (`foldback_hash_tick`/`foldback_hash_entity`/`foldback_hash_field` and their peer-recording counterparts), schema-drift's `foldback_record_schema`, pending-hash draining, divergence checking, and error reporting. It's what `bindings/unity` and `bindings/unreal` are both built on (Godot calls `foldback-core` directly through `gdext` instead — see [Architecture](architecture.md)).

The generated header is the actual reference, not this page — see `crates/foldback-sys/include/foldback.h` (`cbindgen`-generated, never hand-edited — see [RFC-0002](../project/rfcs/0002-c-abi-surface.md) for why generation is the source of truth), per the project's "link out, don't duplicate" rule for generated references. Regenerate it after any change to the crate's `#[no_mangle] extern "C"` surface: `cbindgen --config crates/foldback-sys/cbindgen.toml --crate foldback-sys --output crates/foldback-sys/include/foldback.h`.

Wire format the C ABI's data ultimately feeds into: [Protocol Spec §1](protocol-spec.md#1-foldback-session-file-format). Live-mode transport (also C-ABI-adjacent, since it streams the same frame types): [Protocol Spec §2](protocol-spec.md#2-live-mode-transport).
