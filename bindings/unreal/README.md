# foldback-unreal — the Unreal Engine plugin

Wraps `foldback-sys`'s C ABI (unlike Unity, which does the same, Godot goes direct through `gdext` — see `bindings/godot`'s own README for why). `UFoldbackSubsystem` is the Blueprint-facing wrapper; `FFoldbackSession` (plain C++, not a `UObject`) is the underlying RAII wrapper it delegates to, usable directly from C++ without a `GameInstance` — this plugin's own Automation Test (`examples/unreal-demo`) uses it that way.

## Building it yourself

1. Build the static lib: `cargo build -p foldback-sys --release` from the repo root.
2. Regenerate the header (only needed if `foldback-sys`'s public C API changed): `cbindgen --config crates/foldback-sys/cbindgen.toml --crate foldback-sys --output crates/foldback-sys/include/foldback.h` from `crates/foldback-sys`.
3. Stage both into this plugin's `Source/ThirdParty/Foldback/`:
   - `crates/foldback-sys/include/foldback.h` → `Source/ThirdParty/Foldback/include/foldback.h`
   - `target/release/foldback_sys.lib` (Windows) / `libfoldback_sys.a` (Linux/Mac) → `Source/ThirdParty/Foldback/Win64/` / `Linux/` / `Mac/`
4. Drop this `bindings/unreal` folder into your project's `Plugins/` (or point `AdditionalPluginDirectories` in your `.uproject` at its parent, the way `examples/unreal-demo/UnrealDemo.uproject` does — lets this repo's own example reference the plugin without copying it).

Neither the header nor the static lib is committed to this repo (see `.gitignore`) — same "regenerate, don't commit binaries" pattern as `bindings/unity` and `bindings/godot`.

## `cpp_compat` — a real header portability fix found building this

`foldback-sys/cbindgen.toml` generates a plain C header (`language = "C"`) — Unity's C# P/Invoke and Godot's `gdext` binding never compile it at all, so nobody had actually fed it through a C++ compiler until this plugin did. Doing that surfaced two real bugs, both fixed at the source (`cbindgen.toml`), not worked around here:

1. The header's first line (`header = "..."` in `cbindgen.toml`) wasn't wrapped in a comment — invalid as the very first tokens of a C/C++ translation unit. Fixed by wrapping it in `/* ... */`.
2. In C++ (unlike C), an `enum Foo { ... };` declaration already introduces `Foo` as a type name — cbindgen's plain-C fallback path then adds a conflicting `typedef int32_t FoldbackStatus;`, a genuine redefinition error under MSVC/Clang in C++ mode, not a style nit. Fixed by adding `cpp_compat = true` to `cbindgen.toml`, which guards that path with `#ifdef __cplusplus` and adds cbindgen's own `extern "C" { ... }` wrapper around every function declaration — so this plugin's own C++ files just `#include "foldback.h"` directly, no manual `extern "C"` wrapping needed.

## Verified against a real, locally-installed Unreal Engine 5.8.2

`examples/unreal-demo`'s `Foldback.Verify` Automation Test exercises the full binding surface — Level 1 divergence detection, Level 2/3 hashing round-tripped through a real `.foldback` file (independently re-verified with `foldback-cli analyze`, not just self-reported), `finish()` agreement/disagreement, and the unconfigured-session error path — run headless:

```
UnrealEditor-Cmd.exe <path-to-UnrealDemo.uproject> -ExecCmds="Automation RunTests Foldback.Verify;Quit" -unattended -nopause -nullrhi -nosplash
```

## No hosted CI leg (a real, disclosed gap)

Unlike Unity (Unity Hub has a scriptable CLI installer) and Godot (a plain, licensable-free binary download), Unreal Engine has no install path that doesn't go through the Epic Games Launcher — a GUI application requiring an Epic account login. There is no way to install it on a GitHub-hosted runner within a workflow. A real CI leg for this binding would need a **self-hosted runner with Unreal Engine pre-installed and licensed** — out of scope for this repo's current (free, hosted-runner-only) CI setup. This binding is verified locally (see above), the same honest-gap treatment the project gives every other known limitation, not silently assumed to work.
