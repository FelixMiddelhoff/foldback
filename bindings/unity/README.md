# Foldback — Unity binding

C# UPM package wrapping [`foldback-sys`](../../crates/foldback-sys)'s C ABI. See [docs/integrations/unity.md](../../docs/src/integrations/unity.md) for status, usage, and known gaps.

## Layout

- `Runtime/` — the package itself: `FoldbackSession`/`FoldbackConfig` (the public API, cookbook recipe 8), `FoldbackNative` (the raw P/Invoke declarations, internal), `FoldbackReflection` (reflective hashing, foldback-reflective-hashing.md §2.2). `noEngineReferences: true` in its `.asmdef` — pure C#, no `UnityEngine` dependency, which is what lets `Tests~/FoldbackSys.Tests` run it outside Unity entirely.
- `Editor/` — `FoldbackReflectionWindow` (`Window > Foldback > Reflection Inspector`): a real in-editor dock for reflective hashing's visibility tooling (§4), the Unity counterpart to `examples/bevy-editor-demo`. A **Live** tab lists every `FoldbackReflection.HashReflected` call observed (subscribes to its `Recorded` event) with drill-down into the captured field paths/hashes; an **Inspect** tab shows `ListTracked`'s tracked/untracked split for any dragged-in object, no Play Mode or session needed. Its own `.asmdef` restricts it to `includePlatforms: ["Editor"]` so it never ships in a player build.
- `Tests~/FoldbackSys.Tests/` — a plain `dotnet` console harness (not a Unity project) that P/Invokes a real built `foldback_sys` native library and asserts real behavior. The trailing `~` is Unity's convention for "don't import this folder as an asset" — it doesn't ship as part of the package.

## Installing the package into a Unity project

Not yet published anywhere installable — add via UPM's "install from git URL" pointing at this subtree once a release tag exists (see `foldback-ci-release-plan.md` §5.3), or copy `Runtime/` (and `Editor/`, for the dock) into your project's `Packages/` manually in the meantime. You'll also need to place a built `foldback_sys` native library (`foldback_sys.dll` / `libfoldback_sys.dylib` / `libfoldback_sys.so`) in your project's per-platform Plugins folder — building and publishing those isn't automated yet.

## Running the verification harness

```bash
cargo build -p foldback-sys
dotnet build -c Release bindings/unity/Tests~/FoldbackSys.Tests
```

Copy the built `foldback_sys` native library (from `target/debug/` or `target/release/`) next to the harness's output `.dll` before running it — `dotnet <path-to>/FoldbackSys.Tests.dll` — since P/Invoke resolves the native library relative to the running assembly.
