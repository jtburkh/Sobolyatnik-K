extends SceneTree

func _initialize() -> void:
	call_deferred("_run")


func _run() -> void:
	var map := Node3D.new()
	map.name = "Map"
	root.add_child(map)
	current_scene = map
	var player := Node3D.new()
	player.name = "Player"
	player.add_to_group("Player")
	map.add_child(player)
	var no_loot := OS.get_cmdline_user_args().has("--no-loot")
	var config := FileAccess.open("user://rtv-telemetry.cfg", FileAccess.WRITE)
	if config == null:
		_fail("Could not create mock radar preference")
		return
	var legacy_disabled := OS.get_cmdline_user_args().has("--legacy-disabled")
	config.store_string("[radar_lite]\nloot=%s\n%s" % [str(not no_loot).to_lower(), "overlay=false\ncontrols=false\n" if legacy_disabled else ""])
	config.close()
	var bridge: Node = load("res://RtVRadarShotBridge.gd").new()
	root.add_child(bridge)
	bridge.set_process(false)
	bridge.call("_process", 0.21)
	var overlay: Control = bridge.get("_radar")
	if overlay == null or not overlay.visible or overlay.call("get_mode_name") != "SHOT ALERTS":
		_fail("Default Shot Alerts radar changed")
		return
	if not is_equal_approx(overlay.call("get_range_metres"), 100.0):
		_fail("Default detection radius should be 100m")
		return
	var persistent_hint: Label = bridge.get("_range_hint")
	if persistent_hint == null or not persistent_hint.visible or persistent_hint.text != "F9 TOGGLE DISTANCE  100m" or persistent_hint.mouse_filter != Control.MOUSE_FILTER_IGNORE:
		_fail("HUD must always show a readable F9 control and selected distance")
		return
	var origin := [0.0, 0.0, 0.0]
	if overlay.call("_project", Vector2(100.0, 0.0), origin, 0.0) == null or overlay.call("_project", Vector2(100.01, 0.0), origin, 0.0) != null:
		_fail("100m display cutoff did not include the boundary")
		return
	for expected in [200.0, 400.0, 50.0, 100.0]:
		bridge.call("_poll_range_key", true)
		if not is_equal_approx(overlay.call("get_range_metres"), expected) or persistent_hint.text != "F9 TOGGLE DISTANCE  %dm" % int(expected):
			_fail("F9 did not cycle or visibly confirm %dm" % int(expected))
			return
		bridge.call("_poll_range_key", true)
		if not is_equal_approx(overlay.call("get_range_metres"), expected):
			_fail("Holding F9 skipped a distance")
			return
		if overlay.call("_project", Vector2(expected, 0.0), origin, 0.0) == null or overlay.call("_project", Vector2(expected + 0.01, 0.0), origin, 0.0) != null:
			_fail("Cutoff did not track %dm range" % int(expected))
			return
		bridge.call("_poll_range_key", false)
	var hint: Label = bridge.get("_mode_hint")
	if hint == null or not hint.visible or hint.text != "RADAR RANGE: 100m" or hint.mouse_filter != Control.MOUSE_FILTER_IGNORE:
		_fail("F9 did not visibly confirm the selected range without capturing input")
		return
	bridge.call("_update_mode_hint", Time.get_ticks_msec() + 1801)
	if hint.visible:
		_fail("Range hint should disappear after 1.8 seconds; header should retain 100m")
		return
	print("RANGE SMOKE OK: default 100m, 50/100/200/400m F9 cycle, cutoff and UI hint")
	# A selected radius changes which shot markers are visible, not their fixed
	# positions, expiry, or the persistent Shot Alerts panel itself.
	overlay.call("add_gunshot", [150.0, 0.0, 0.0])
	var recorded: Array = overlay.get("_shots")
	var time_ms: int = recorded[0]["time_ms"]
	if overlay.call("_project", Vector2(150.0, 0.0), origin, 0.0) != null:
		_fail("Out-of-range shot appeared at 100m")
		return
	bridge.call("_poll_range_key", true)
	bridge.call("_poll_range_key", false)
	if overlay.call("_project", Vector2(150.0, 0.0), origin, 0.0) == null or overlay.get("_shots")[0]["position"] != [150.0, 0.0, 0.0] or not is_equal_approx(overlay.call("_shot_opacity", time_ms, time_ms + 2500), 0.5):
		_fail("Shot position or fade changed when range widened")
		return
	bridge.call("_poll_radar_keys", false, true)
	if overlay.visible or persistent_hint.visible:
		_fail("F8 did not hide the radar and persistent control hint")
		return
	bridge.call("_poll_range_key", true)
	bridge.call("_poll_range_key", false)
	if overlay.visible or hint.visible or not is_equal_approx(overlay.call("get_range_metres"), 400.0):
		_fail("Changing range while F8 hidden must not reveal the radar")
		return
	bridge.call("_poll_radar_keys", false, false)
	bridge.call("_poll_radar_keys", false, true)
	if not overlay.visible or not persistent_hint.visible or int(overlay.get("_layer_mode")) != 0 or not is_equal_approx(overlay.call("get_range_metres"), 400.0):
		_fail("F8 did not restore Shot Alerts and the selected range")
		return
	bridge.call("_poll_radar_keys", false, false)
	bridge.call("_poll_radar_keys", true, false)
	if overlay.call("get_mode_name") != ("AI+TRAILS+SHOTS" if no_loot else "ALL+SHOTS"):
		_fail("F7 mode cycle changed when F9 was added")
		return
	print("RANGE SMOKE OK: fixed shots, five-second fade, F7/F8 and both loot configurations")
	# Exercise the real Godot event queue and _process polling, not just a direct
	# call to _poll_range_key. Events are delivered on the next process frame.
	bridge.set_process(true)
	var down := InputEventKey.new()
	down.keycode = KEY_F9
	down.pressed = true
	Input.parse_input_event(down)
	await process_frame
	await process_frame
	if not is_equal_approx(overlay.call("get_range_metres"), 50.0) or persistent_hint.text != "F9 TOGGLE DISTANCE  50m":
		_fail("Real queued F9 input did not cycle or visibly confirm range")
		return
	var up := InputEventKey.new()
	up.keycode = KEY_F9
	up.pressed = false
	Input.parse_input_event(up)
	await process_frame
	await process_frame
	bridge.set_process(false)
	print("RANGE SMOKE OK: real queued Godot F9 input and persistent HUD label")
	quit()


func _fail(message: String) -> void:
	push_error(message)
	quit(1)
