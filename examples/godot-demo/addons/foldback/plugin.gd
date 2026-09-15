## SPDX-License-Identifier: MIT OR Apache-2.0
## Registers the Foldback reflective-hashing visibility dock
## (foldback-reflective-hashing.md §4) — the Godot counterpart to
## `examples/bevy-editor-demo`'s `bevy_egui` panel and the Unity binding's
## `FoldbackReflectionWindow`.
##
## Deliberately not a live-hashing view the way the Bevy/Unity docks are:
## Godot's editor and a running (`F5`) game are separate OS processes by
## default, with no shared memory an editor dock could read a live
## `hash_reflected` call from the way Unity's single-process Editor+Play
## Mode allows. What *is* buildable, and is what this dock does instead,
## is the same "did you mean to include this one too" static check as
## `list_tracked` already gives GDScript callers — just live, against
## whatever node is currently selected in the editor, no Play Mode or
## session required.
@tool
extends EditorPlugin

const FoldbackDock = preload("res://addons/foldback/foldback_dock.gd")

var dock: Control


func _enter_tree() -> void:
	dock = FoldbackDock.new()
	add_control_to_dock(DOCK_SLOT_RIGHT_UL, dock)


func _exit_tree() -> void:
	remove_control_from_docks(dock)
	dock.free()
