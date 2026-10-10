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
	var event_system: Node3D = load("res://Scripts/EventSystem.gd").new()
	map.add_child(event_system)
	var spawner: Node3D = load("res://Scripts/AISpawner.gd").new()
	spawner.name = "AI"
	map.add_child(spawner)
	var pool := Node3D.new()
	pool.name = "B_Pool"
	spawner.add_child(pool)
	for name in ["Punisher", "Bogeyman"]:
		var boss := Node3D.new()
		boss.name = "AI_" + name
		pool.add_child(boss)
	for group in ["AI_SP", "AI_WP", "AI_LP"]:
		var point := Node3D.new()
		point.name = group
		point.position = Vector3(200.0, 0, 0)
		point.add_to_group(group)
		map.add_child(point)
	var bridge: Node = load("res://RtVRadarSummonBridge.gd").new()
	root.add_child(bridge)
	bridge.set_process(false)
	bridge.call("_process", 0.25)
	var overlay: Control = bridge.get("_radar")
	if overlay == null or not overlay.visible or overlay.call("get_mode_name") != "SHOT ALERTS" or not is_equal_approx(overlay.call("get_range_metres"), 100.0):
		_fail("Candidate changed the default HUD/range")
		return
	var now := Time.get_ticks_msec()
	event_system.map = null
	if bridge.call("_try_summon", "spawn_airdrop", map, now) != "CASA EVENT NOT READY" or event_system.calls != 0:
		_fail("Uninitialized native airdrop event should not execute")
		return
	event_system.map = map
	bridge.call("_poll_summon_keys", true, false, now) # F10 opens Airdrop.
	bridge.call("_poll_summon_keys", false, false, now)
	if bridge.get("_summon_menu") != 0 or not str(bridge.get("_summon_hint").text).contains("CASA AIRDROP"):
		_fail("F10 did not select airdrop")
		return
	if event_system.calls != 0:
		_fail("Selection alone spawned an event")
		return
	bridge.call("_poll_summon_keys", false, true, now) # F11 confirms once.
	await process_frame
	if event_system.calls != 1:
		_fail("Confirmed in-game airdrop was not called exactly once")
		return
	bridge.call("_poll_summon_keys", false, true, now)
	await process_frame
	if event_system.calls != 1:
		_fail("Holding F11 caused a duplicate")
		return
	bridge.call("_poll_summon_keys", false, false, now)
	bridge.set("_summon_cooldown_until_ms", 0)
	var command_id := "123-%d" % int(Time.get_unix_time_from_system() * 1000000000.0)
	var request := ConfigFile.new()
	request.set_value("command", "id", command_id)
	request.set_value("command", "action", "spawn_airdrop")
	if request.save("user://rtv-toolkit-command.cfg") != OK:
		_fail("Could not create fixture request")
		return
	bridge.call("_poll_control_request")
	await process_frame
	var result := ConfigFile.new()
	if FileAccess.file_exists("user://rtv-toolkit-command.cfg") or result.load("user://rtv-toolkit-command-result.cfg") != OK or result.get_value("result", "status", "") != "accepted" or event_system.calls != 2:
		_fail("Toolkit request was not consumed and acknowledged exactly once")
		return
	request.save("user://rtv-toolkit-command.cfg")
	bridge.call("_poll_control_request")
	await process_frame
	if event_system.calls != 2 or result.load("user://rtv-toolkit-command-result.cfg") != OK or result.get_value("result", "status", "") != "rejected":
		_fail("Duplicate Toolkit request was not rejected")
		return
	bridge.set("_summon_cooldown_until_ms", 0)
	request.set_value("command", "id", "123-1")
	request.save("user://rtv-toolkit-command.cfg")
	bridge.call("_poll_control_request")
	if event_system.calls != 2 or FileAccess.file_exists("user://rtv-toolkit-command.cfg"):
		_fail("Expired request should be removed without execution")
		return
	bridge.call("_poll_summon_keys", true, false, now)
	bridge.call("_poll_summon_keys", false, false, now)
	bridge.call("_poll_summon_keys", true, false, now)
	bridge.call("_poll_summon_keys", false, true, now)
	await process_frame
	if spawner.calls.size() != 1 or spawner.calls[0][0] != "Punisher" or spawner.calls[0][1] != "Attack":
		_fail("Punisher must use native Attack and current-map points")
		return
	bridge.set("_summon_cooldown_until_ms", 0)
	bridge.call("_poll_summon_keys", false, false, now)
	for _i in 3:
		bridge.call("_poll_summon_keys", true, false, now)
		bridge.call("_poll_summon_keys", false, false, now)
	bridge.call("_poll_summon_keys", false, true, now)
	await process_frame
	if spawner.calls.size() != 2 or spawner.calls[1][0] != "Bogeyman" or spawner.calls[1][1] != "Lurk" or spawner.calls[1][3] == null or not spawner.calls[1][3].is_in_group("AI_LP"):
		_fail("Bogeyman must use native Lurk at a current-map lurk point")
		return
	bridge.set("_summon_cooldown_until_ms", 0)
	if bridge.call("_try_summon", "spawn_punisher", map, Time.get_ticks_msec()) != "PUNISHER UNAVAILABLE":
		_fail("Consumed native boss pool should not be recreated")
		return
	var new_bogeyman := Node3D.new()
	new_bogeyman.name = "AI_Bogeyman"
	pool.add_child(new_bogeyman)
	for point in get_nodes_in_group("AI_LP"):
		point.remove_from_group("AI_LP")
	if bridge.call("_try_summon", "spawn_bogeyman", map, Time.get_ticks_msec()) != "NO SAFE BOGEYMAN WAYPOINT" or spawner.calls.size() != 2:
		_fail("Bogeyman without a current-map lurk point should not execute")
		return
	print("SUMMON SMOKE OK: F10/F11, paired Toolkit airdrop, unready/stale/duplicate rejection, pooled bosses and missing waypoint")
	quit()


func _fail(message: String) -> void:
	push_error(message)
	quit(1)
