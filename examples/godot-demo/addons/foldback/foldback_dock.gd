## SPDX-License-Identifier: MIT OR Apache-2.0
## The Foldback reflective-hashing visibility dock's actual UI — see
## `plugin.gd` for why this is a selection-driven static view (tracked vs
## untracked property names) rather than a live-hashing feed. Shows
## `FoldbackSession.list_tracked`'s tracked/untracked split for whichever
## node is currently selected in the editor, refreshing on every
## selection change.
@tool
extends VBoxContainer

var _session: FoldbackSession
var _target_label: Label
var _tracked_list: ItemList
var _untracked_list: ItemList
var _selection: EditorSelection


func _init() -> void:
	name = "Foldback"
	_session = FoldbackSession.new()

	var title := Label.new()
	title.text = "Foldback — Reflective Hashing"
	add_child(title)

	_target_label = Label.new()
	_target_label.text = "(no selection)"
	add_child(_target_label)

	var tracked_header := Label.new()
	tracked_header.text = "Tracked (foldback_-prefixed):"
	add_child(tracked_header)
	_tracked_list = ItemList.new()
	_tracked_list.custom_minimum_size = Vector2(0, 120)
	add_child(_tracked_list)

	var untracked_header := Label.new()
	untracked_header.text = "Untracked:"
	add_child(untracked_header)
	_untracked_list = ItemList.new()
	_untracked_list.custom_minimum_size = Vector2(0, 120)
	add_child(_untracked_list)


func _ready() -> void:
	_selection = EditorInterface.get_selection()
	_selection.selection_changed.connect(_on_selection_changed)
	_on_selection_changed()


func _on_selection_changed() -> void:
	var nodes := _selection.get_selected_nodes()
	if nodes.is_empty():
		_target_label.text = "(no selection)"
		_tracked_list.clear()
		_untracked_list.clear()
		return

	var target: Node = nodes[0]
	_target_label.text = "%s (%s)" % [target.name, target.get_class()]

	var result: Dictionary = _session.list_tracked(target)
	_tracked_list.clear()
	for prop_name in result.get("tracked", []):
		_tracked_list.add_item(prop_name)

	_untracked_list.clear()
	for prop_name in result.get("untracked", []):
		_untracked_list.add_item(prop_name)
