## A `class_name`-declared (not inner-class) reflected type, used only by
## the schema-drift check in test.gd — `Script.get_global_name()` (the
## schema-drift type-name source, `crates/foldback-godot/src/reflection.rs`
## `schema_type_name`) is only populated for a class declared this way, not
## for `test.gd`'s own inner classes (`ReflectUnit` etc.), which have no
## `class_name` and so fall back to `get_class()` — see that check for both
## cases side by side.
class_name FoldbackDemoUnit
extends RefCounted

var foldback_hp: int = 0
var debug_label: String = ""
