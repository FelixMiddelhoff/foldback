extends SceneTree
## Headless end-to-end verification for the foldback-godot binding, run via
## `godot --headless --script res://test.gd`. Not a GDScript unit-test
## framework — a straight-line script printing PASS/FAIL per check and
## exiting non-zero if anything failed, so CI can gate on the exit code.


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

	if all_ok:
		print("ALL PASS")
		quit(0)
	else:
		print("SOME FAILED")
		quit(1)
