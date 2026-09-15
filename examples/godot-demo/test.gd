extends SceneTree
## Headless end-to-end verification for the foldback-godot binding, run via
## `godot --headless --script res://test.gd`. Not a GDScript unit-test
## framework — a straight-line script printing PASS/FAIL per check and
## exiting non-zero if anything failed, so CI can gate on the exit code.


class ReflectPos extends RefCounted:
	var foldback_x: float = 0.0
	var foldback_y: float = 0.0


class ReflectUnit extends RefCounted:
	var foldback_pos: ReflectPos
	var foldback_hp: int = 0
	var debug_label: String = ""


class CycleNode extends RefCounted:
	var foldback_next: RefCounted
	var foldback_value: int = 0


class DeepNode extends RefCounted:
	var foldback_inner: RefCounted
	var foldback_value: int = 0

const FoldbackDemoUnitScript := preload("res://FoldbackDemoUnit.gd")


## Reads a `.foldback` file's `Metadata` frames (type `0x20`) whose key
## starts with `"foldback.schema."` — schema-drift detection
## (foldback-reflective-hashing.md §7). Parses the raw wire format
## directly (protocol-spec.md §1) rather than shelling out to
## `foldback-cli`, so this check has no dependency beyond the file itself.
func read_schema_metadata(path: String) -> Array:
	var f := FileAccess.open(path, FileAccess.READ)
	if f == null:
		return []
	f.seek(32)  # fixed 32-byte header (protocol-spec.md §1.1)
	var out: Array = []
	while f.get_position() < f.get_length():
		var frame_type := f.get_8()
		var payload_len := f.get_32()
		var payload := f.get_buffer(payload_len)
		if frame_type == 0x20:  # Metadata
			var stream := StreamPeerBuffer.new()
			stream.data_array = payload
			var key_len := stream.get_u32()
			var key := stream.get_utf8_string(key_len)
			var value_len := stream.get_u32()
			var value := stream.get_utf8_string(value_len)
			if key.begins_with("foldback.schema."):
				out.append({"key": key, "value": value})
	return out


func _initialize() -> void:
	var all_ok := true

	# --- Level 1: cross-peer divergence detection ---
	var session_a := FoldbackSession.new()
	var session_b := FoldbackSession.new()
	if not session_a.configure({"tick_rate_hz": 60, "peer_count": 2, "local_peer_id": 0}):
		print("FAIL: session_a configure: ", session_a.get_last_error())
		all_ok = false
	if not session_b.configure({"tick_rate_hz": 60, "peer_count": 2, "local_peer_id": 1}):
		print("FAIL: session_b configure: ", session_b.get_last_error())
		all_ok = false

	for tick in range(20):
		var state := PackedByteArray([tick])
		session_a.hash_tick(tick, state)
		session_b.hash_tick(tick, state)

	# Injected divergence at tick 10.
	session_b.hash_tick(10, PackedByteArray([255]))

	# Toy in-process hash exchange (not real networking — the point here is
	# exercising the binding's API surface, not the transport).
	for ph in session_a.take_pending_hashes(1000):
		session_b.record_peer_hash(ph["tick"], 0, ph["hash"])
	for ph in session_b.take_pending_hashes(1000):
		session_a.record_peer_hash(ph["tick"], 1, ph["hash"])

	var div := session_a.check_divergence()
	if not div["found"] or div["tick"] != 10:
		print("FAIL: expected divergence at tick 10, got ", div)
		all_ok = false
	else:
		print("PASS: divergence correctly detected at tick 10")

	# --- Level 2/3: entity + field hashing, recorded to a real .foldback file ---
	var rec_path := ProjectSettings.globalize_path("user://godot-demo.foldback")
	var rec_session := FoldbackSession.new()
	if not rec_session.configure({"tick_rate_hz": 60, "peer_count": 1, "record_to_path": rec_path}):
		print("FAIL: rec_session configure: ", rec_session.get_last_error())
		all_ok = false
	else:
		var status: int = rec_session.hash_entity(5, 7, PackedByteArray([1, 2, 3]))
		if status != 0:
			print("FAIL: hash_entity status ", status, ": ", rec_session.get_last_error())
			all_ok = false

		var field_value := PackedByteArray([0, 0, 64, 64])  # 3.0f32 little-endian
		status = rec_session.hash_field(5, 7, "position.x", field_value)
		if status != 0:
			print("FAIL: hash_field status ", status, ": ", rec_session.get_last_error())
			all_ok = false

		status = rec_session.finish_recording()
		if status != 0:
			print("FAIL: finish_recording status ", status, ": ", rec_session.get_last_error())
			all_ok = false
		elif not FileAccess.file_exists(rec_path):
			print("FAIL: recording file was not written: ", rec_path)
			all_ok = false
		else:
			print("PASS: Level 2/3 hashing recorded to ", rec_path)

	# --- finish() agreement / disagreement across independent sessions ---
	var s1 := FoldbackSession.new()
	var s2 := FoldbackSession.new()
	s1.configure({"tick_rate_hz": 60, "peer_count": 1})
	s2.configure({"tick_rate_hz": 60, "peer_count": 1})
	for tick in range(5):
		s1.hash_tick(tick, PackedByteArray([tick]))
		s2.hash_tick(tick, PackedByteArray([tick]))
	if s1.finish() != s2.finish():
		print("FAIL: finish() should agree for identical input")
		all_ok = false
	s2.hash_tick(5, PackedByteArray([255]))
	if s1.finish() == s2.finish():
		print("FAIL: finish() should disagree after a divergent extra tick")
		all_ok = false
	else:
		print("PASS: finish() agreement/disagreement correct")

	# --- error path: hashing before configure() is a clean no-crash error ---
	var unconfigured := FoldbackSession.new()
	var status: int = unconfigured.hash_tick(0, PackedByteArray([1]))
	if status == 0:
		print("FAIL: hash_tick on an unconfigured session should not return Ok")
		all_ok = false
	elif unconfigured.get_last_error() == "":
		print("FAIL: unconfigured session should set get_last_error()")
		all_ok = false
	else:
		print("PASS: unconfigured session reports a clean error, not a crash")

	# --- Reflective hashing (foldback-reflective-hashing.md §2.3) ---
	var refl_session := FoldbackSession.new()
	refl_session.configure({"tick_rate_hz": 60, "peer_count": 1})

	var unit := ReflectUnit.new()
	unit.foldback_pos = ReflectPos.new()
	unit.foldback_pos.foldback_x = 1.5
	unit.foldback_pos.foldback_y = -2.0
	unit.foldback_hp = 42
	unit.debug_label = "not hashed"

	var preview: Array = refl_session.hash_reflected(0, 7, "unit", unit)
	var paths: Array = []
	for p in preview:
		paths.append(p["path"])
	if paths.has("unit.foldback_pos.foldback_x") and paths.has("unit.foldback_pos.foldback_y") \
			and paths.has("unit.foldback_hp"):
		print("PASS: reflective walk records foldback_-prefixed fields, re-checked at each nested Object")
	else:
		print("FAIL: reflective walk missing expected paths: ", paths)
		all_ok = false

	var untracked_leaked := false
	for p in paths:
		if str(p).find("debug_label") != -1:
			untracked_leaked = true
	if untracked_leaked:
		print("FAIL: the untagged field was recorded")
		all_ok = false
	else:
		print("PASS: the untagged field is never recorded")

	# Same tracked state -> identical hashes; a differing tracked field
	# changes them; an untagged field's difference doesn't.
	var mk_unit := func(x: float, label: String) -> ReflectUnit:
		var u := ReflectUnit.new()
		u.foldback_pos = ReflectPos.new()
		u.foldback_pos.foldback_x = x
		u.foldback_pos.foldback_y = -2.0
		u.foldback_hp = 42
		u.debug_label = label
		return u

	var sum_hashes := func(prev: Array) -> int:
		var total := 0
		for p in prev:
			total ^= int(p["hash"])
		return total

	var sa := FoldbackSession.new()
	var sb := FoldbackSession.new()
	var sc := FoldbackSession.new()
	sa.configure({"tick_rate_hz": 60, "peer_count": 1})
	sb.configure({"tick_rate_hz": 60, "peer_count": 1})
	sc.configure({"tick_rate_hz": 60, "peer_count": 1})
	var ha: int = sum_hashes.call(sa.hash_reflected(0, 0, "u", mk_unit.call(1.0, "a")))
	var hb: int = sum_hashes.call(sb.hash_reflected(0, 0, "u", mk_unit.call(1.0, "b differs but untracked")))
	var hc: int = sum_hashes.call(sc.hash_reflected(0, 0, "u", mk_unit.call(1.1, "a")))
	if ha == hb and ha != hc:
		print("PASS: only tracked-field differences change the hash")
	else:
		print("FAIL: tracked/untracked hash sensitivity wrong (ha=", ha, " hb=", hb, " hc=", hc, ")")
		all_ok = false

	# list_tracked: the visibility-tooling data source.
	var lt: Dictionary = refl_session.list_tracked(unit)
	if lt["tracked"].has("foldback_pos") and lt["tracked"].has("foldback_hp") \
			and lt["untracked"].has("debug_label"):
		print("PASS: list_tracked reports the tracked/untracked split correctly")
	else:
		print("FAIL: list_tracked wrong: ", lt)
		all_ok = false

	# A genuine reference cycle is caught, loudly, rather than hanging.
	var cyc_a := CycleNode.new()
	var cyc_b := CycleNode.new()
	cyc_a.foldback_next = cyc_b
	cyc_b.foldback_next = cyc_a
	var cycle_session := FoldbackSession.new()
	cycle_session.configure({"tick_rate_hz": 60, "peer_count": 1})
	var cyc_preview: Array = cycle_session.hash_reflected(0, 0, "n", cyc_a)
	if cyc_preview.is_empty() and cycle_session.get_last_error().find("cycle") != -1:
		print("PASS: a genuine reference cycle is caught: ", cycle_session.get_last_error())
	else:
		print("FAIL: reference cycle not caught (preview size ", cyc_preview.size(), ", error '", cycle_session.get_last_error(), "')")
		all_ok = false

	# The depth guard rejects a runaway (but acyclic) chain.
	var deep: RefCounted = DeepNode.new()
	deep.foldback_value = 0
	for i in range(10):
		var next_deep := DeepNode.new()
		next_deep.foldback_value = i
		next_deep.foldback_inner = deep
		deep = next_deep
	var depth_session := FoldbackSession.new()
	depth_session.configure({"tick_rate_hz": 60, "peer_count": 1})
	var depth_preview: Array = depth_session.hash_reflected(0, 0, "d", deep)
	if depth_preview.is_empty() and depth_session.get_last_error().find("depth") != -1:
		print("PASS: a runaway nesting chain hits the depth guard: ", depth_session.get_last_error())
	else:
		print("FAIL: depth guard not triggered (preview size ", depth_preview.size(), ", error '", depth_session.get_last_error(), "')")
		all_ok = false

	# --- Schema-drift detection (foldback-reflective-hashing.md §7) ---
	var schema_path := ProjectSettings.globalize_path("user://godot-demo-schema.foldback")
	var schema_session := FoldbackSession.new()
	schema_session.configure({"tick_rate_hz": 60, "peer_count": 1, "record_to_path": schema_path})

	# A `class_name`-declared type: `Script.get_global_name()` gives a
	# real, build-stable type name.
	var named_unit: RefCounted = FoldbackDemoUnitScript.new()
	named_unit.foldback_hp = 5
	schema_session.hash_reflected(0, 0, "u", named_unit)
	schema_session.hash_reflected(1, 0, "u", named_unit)  # same type again — must not duplicate the schema frame

	# An inner class with no `class_name` (like `unit` above): falls back
	# to `get_class()`, i.e. the native `RefCounted` — the documented
	# limitation, not silently wrong.
	schema_session.hash_reflected(2, 1, "u2", unit)

	schema_session.finish_recording()
	var schema_frames := read_schema_metadata(schema_path)

	var named_frames := schema_frames.filter(func(f): return f["key"] == "foldback.schema.FoldbackDemoUnit")
	var fallback_frames := schema_frames.filter(func(f): return f["key"] == "foldback.schema.RefCounted")
	if named_frames.size() == 1 and named_frames[0]["value"] == "foldback_hp":
		print("PASS: class_name type recorded exactly one schema frame with the tagged field set")
	else:
		print("FAIL: class_name schema frames wrong: ", named_frames)
		all_ok = false
	if fallback_frames.size() == 1 and fallback_frames[0]["value"] == "foldback_hp,foldback_pos":
		print("PASS: inner-class (no class_name) falls back to get_class(), still one schema frame")
	else:
		print("FAIL: fallback schema frames wrong: ", fallback_frames)
		all_ok = false

	# Performance: not assumed free (plan §5). Printed either way, and —
	# a CI regression gate, not just visibility — also asserted against a
	# generous ratio bound. Best-of-3 timing on real work (1,000
	# entities) to keep the signal well above CI-runner jitter. Godot's
	# reflective walk measured ~13x explicit's cost locally (notably
	# higher than Bevy/Unity's ~3.5-3.7x, likely get_property_list()'s
	# per-call Dictionary-array construction — see
	# docs/src/integrations/reflective-hashing.md), so the ceiling here
	# is wider than the other two engines' — still generous enough to
	# absorb CI-runner noise, tight enough to catch an order-of-
	# magnitude regression.
	const ENTITY_COUNT := 1000
	const MAX_RATIO := 40.0
	var units: Array = []
	for i in range(ENTITY_COUNT):
		units.append(mk_unit.call(float(i), "d"))

	var time_reflective_ms := func() -> float:
		var perf_session := FoldbackSession.new()
		perf_session.configure({"tick_rate_hz": 60, "peer_count": 1})
		var start := Time.get_ticks_usec()
		for id in range(ENTITY_COUNT):
			perf_session.hash_reflected(0, id, "unit", units[id])
		return (Time.get_ticks_usec() - start) / 1000.0

	var time_explicit_ms := func() -> float:
		var explicit_session := FoldbackSession.new()
		explicit_session.configure({"tick_rate_hz": 60, "peer_count": 1})
		var start := Time.get_ticks_usec()
		for id in range(ENTITY_COUNT):
			var u: ReflectUnit = units[id]
			explicit_session.hash_field(0, id, "unit.pos.x", var_to_bytes(u.foldback_pos.foldback_x))
			explicit_session.hash_field(0, id, "unit.pos.y", var_to_bytes(u.foldback_pos.foldback_y))
			explicit_session.hash_field(0, id, "unit.hp", var_to_bytes(u.foldback_hp))
		return (Time.get_ticks_usec() - start) / 1000.0

	var reflective_ms: float = min(time_reflective_ms.call(), min(time_reflective_ms.call(), time_reflective_ms.call()))
	var explicit_ms: float = min(time_explicit_ms.call(), min(time_explicit_ms.call(), time_explicit_ms.call()))

	print("reflective: ", ENTITY_COUNT, " entities in ", reflective_ms, " ms")
	print("explicit:   ", ENTITY_COUNT, " entities in ", explicit_ms, " ms")
	var safe_explicit_ms: float = max(explicit_ms, 0.001)
	var ratio: float = reflective_ms / safe_explicit_ms
	if ratio <= MAX_RATIO:
		print("PASS: reflective hashing stays within a %.1fx budget of explicit (%.1fx measured)" % [MAX_RATIO, ratio])
	else:
		print("FAIL: reflective hashing exceeded its %.1fx budget of explicit (%.1fx measured)" % [MAX_RATIO, ratio])
		all_ok = false

	if all_ok:
		print("ALL PASS")
		quit(0)
	else:
		print("SOME FAILED")
		quit(1)
