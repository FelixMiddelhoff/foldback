# foldback-godot — GDExtension binding

Wraps `foldback-core` directly (see `crates/foldback-godot`'s own doc comment for why this binding skips `foldback-sys`'s C ABI, unlike Unity/Unreal) and exposes a GDScript-native `FoldbackSession` class, per cookbook recipe 9.

## Using it in your own Godot project

1. Build the native library: `cargo build -p foldback-godot --release` (or `--profile dev` while iterating).
2. Copy this `addons/foldback` folder into your project's `addons/`.
3. Copy the built library (`target/release/foldback_godot.dll` / `libfoldback_godot.so` / `libfoldback_godot.dylib`) into `addons/foldback/bin/` next to `foldback.gdextension`.
4. Godot picks up the extension automatically on next project load.
5. Optional: enable the **Foldback** editor plugin (Project Settings → Plugins) for a reflective-hashing visibility dock (foldback-reflective-hashing.md §4) — shows the `foldback_`-prefixed (tracked) vs untracked properties on whatever node is currently selected in the editor, live, no Play Mode or session needed. Deliberately *not* a live-hashing feed the way the Bevy/Unity docks are: the editor and a running (`F5`) game are separate OS processes by default, so there's no shared memory a dock could read a live `hash_reflected` call from the way Unity's single-process Editor+Play Mode allows — `list_tracked`'s static tracked/untracked split is what's actually buildable here, so that's what the dock shows.

```gdscript
var session := FoldbackSession.new()
var ok := session.configure({"tick_rate_hz": 60, "peer_count": 2})
if not ok:
    push_error(session.get_last_error())

func _physics_process(_delta):
    var state := serialize_deterministic_state()
    session.hash_tick(Engine.get_physics_frames(), state)
```

Deviation from cookbook recipe 9's original sketch (`FoldbackSession.new({...})` with the config passed straight to `.new()`): see the doc comment at the top of `crates/foldback-godot/src/lib.rs` for why a two-step `.new()` + `.configure(dict)` is the real, verified shape instead.

See `examples/godot-demo` for a real, runnable end-to-end example (headless-testable via `godot --headless --script res://test.gd`).
