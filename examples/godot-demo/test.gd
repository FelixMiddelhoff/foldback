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

	# Performance: not assumed free (plan §5) — printed for visibility,
	# not asserted, matching the Bevy/Unity walkers.
	const ENTITY_COUNT := 1000
	var units: Array = []
	for i in range(ENTITY_COUNT):
		units.append(mk_unit.call(float(i), "d"))

	var perf_session := FoldbackSession.new()
	perf_session.configure({"tick_rate_hz": 60, "peer_count": 1})
	var reflective_start := Time.get_ticks_usec()
	for id in range(ENTITY_COUNT):
		perf_session.hash_reflected(0, id, "unit", units[id])
	var reflective_usec := Time.get_ticks_usec() - reflective_start

	var explicit_session := FoldbackSession.new()
	explicit_session.configure({"tick_rate_hz": 60, "peer_count": 1})
	var explicit_start := Time.get_ticks_usec()
	for id in range(ENTITY_COUNT):
		var u: ReflectUnit = units[id]
		explicit_session.hash_field(0, id, "unit.pos.x", var_to_bytes(u.foldback_pos.foldback_x))
		explicit_session.hash_field(0, id, "unit.pos.y", var_to_bytes(u.foldback_pos.foldback_y))
		explicit_session.hash_field(0, id, "unit.hp", var_to_bytes(u.foldback_hp))
	var explicit_usec := Time.get_ticks_usec() - explicit_start

	print("reflective: ", ENTITY_COUNT, " entities in ", reflective_usec / 1000.0, " ms")
	print("explicit:   ", ENTITY_COUNT, " entities in ", explicit_usec / 1000.0, " ms")

	if all_ok:
		print("ALL PASS")
		quit(0)
	else:
		print("SOME FAILED")
		quit(1)
