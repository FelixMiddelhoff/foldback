# Unity

## Status

**Not started.** Planned for Phase 3, after `foldback-sys`'s C ABI is stabilized (currently an empty crate skeleton with a `cbindgen.toml` stub — no real FFI surface yet). Phase 3 needs: `foldback-sys` C ABI stabilized, a C# P/Invoke wrapper, and a Unity editor window.

## Who this will be for

Unity projects using a lockstep or rollback netcode solution, including ones targeting IL2CPP (AOT compilation) — the C ABI is being designed with IL2CPP's marshaling constraints in mind from the start (see [RFC-0002](../project/rfcs/0002-c-abi-surface.md)), not retrofitted later.

## In the meantime

The target API shape is sketched in [Cookbook recipe 8](../cookbook/README.md#8-unity-integration) — subject to change before it's actually implemented, per the cookbook's own "proposal to validate" framing.

## Planning detail

The IL2CPP AOT marshaling risk and its mitigation plan: see the project's risk register (P1, platform/FFI risks) — full detail once `project/*.md` pages beyond testing/performance/ci-release land.
