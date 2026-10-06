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
	var ai_root := Node.new()
	ai_root.name = "AI"
	map.add_child(ai_root)
	var enemies := Node.new()
	enemies.name = "Enemies"
	ai_root.add_child(enemies)
	var nomads := Node.new()
	nomads.name = "Nomads"
	ai_root.add_child(nomads)

	var fire_stream: AudioStream = AudioStreamGenerator.new()
	var explosion_stream: AudioStream = AudioStreamGenerator.new()
	var fire_event: Resource = load("res://Scripts/MockFireEvent.gd").new()
	fire_event.audioClips.append(fire_stream)
	var explosion_event: Resource = load("res://Scripts/MockFireEvent.gd").new()
	explosion_event.audioClips.append(explosion_stream)
	var weapon_data: Resource = load("res://Scripts/MockWeaponData.gd").new()
	weapon_data.fireSemi = fire_event
	var agent: Node3D = load("res://Scripts/MockShotAI.gd").new()
	agent.name = "Enemy"
	agent.position = Vector3(12.0, 0.0, 0.0)
	agent.variant = load("res://Scripts/MockVariant.gd").new()
	agent.weaponData = weapon_data
	enemies.add_child(agent)

	var no_loot := OS.get_cmdline_user_args().has("--no-loot")
	var legacy_disabled := OS.get_cmdline_user_args().has("--legacy-disabled")
	var preference := FileAccess.open("user://rtv-telemetry.cfg", FileAccess.WRITE)
	if preference == null:
		_fail("Could not create mock radar preference")
		return
	preference.store_string("[radar_lite]\nloot=%s\n%s" % [str(not no_loot).to_lower(), "overlay=false\ncontrols=false\n" if legacy_disabled else ""])
	preference.close()
	var receiver := PacketPeerUDP.new()
	if receiver.bind(0, "127.0.0.1") != OK:
		_fail("UDP bind failed")
		return
	var bridge: Node = load("res://RtVRadarShotBridge.gd").new()
	root.add_child(bridge)
	bridge.get("_udp").set_dest_address("127.0.0.1", receiver.get_local_port())
	bridge.set_process(false)
	bridge.call("_process", 0.21)
	var overlay: Control = bridge.get("_radar")
	if overlay == null or overlay.get("_map_id") != "Map" or not overlay.get("_shots").is_empty():
		_fail("Candidate HUD did not start in the mock map")
		return
	if int(overlay.get("_layer_mode")) != 0 or overlay.call("get_mode_name") != "SHOT ALERTS" or not overlay.visible or not is_equal_approx(overlay.modulate.a, 1.0):
		_fail("First mode must be the visible, shot-only radar without any F7 presses")
		return
	if bool(overlay.get("_loot_enabled")) == no_loot:
		_fail("Mock loot preference was not applied to the other F7 modes")
		return
	print("SHOT SMOKE OK: Shot Alerts is the first visible mode by default")
	_drain(receiver)

	var sound: AudioStreamPlayer3D = load("res://Scripts/AudioInstance3D.gd").new()
	root.add_child(sound)
	sound.global_position = agent.muzzle.global_position
	sound.PlayInstance(fire_event, 50.0, 400.0)
	await process_frame
	await process_frame
	var shot := _receive_shot(receiver)
	if shot.get("type") != "gunshot" or shot.get("map_id") != "Map" or shot.get("shooter_id") != agent.get_instance_id() or shot.get("position") != [12.0, 0.0, 0.0]:
		_fail("Actual AI fire sound did not create exactly one fixed-position packet: %s" % shot)
		return
	if overlay.get("_shots").size() != 1:
		_fail("AI gunshot not drawn on the candidate HUD")
		return
	print("SHOT SMOKE OK: AI fire sound observed once at its muzzle")

	for config in [{"group": nomads, "faction": 0, "x": -8.0}, {"group": enemies, "faction": 4, "x": 24.0}]:
		var other: Node3D = load("res://Scripts/MockShotAI.gd").new()
		other.name = "Nomad" if config["faction"] == 0 else "Boss"
		other.position = Vector3(config["x"], 0.0, 0.0)
		other.variant = load("res://Scripts/MockVariant.gd").new()
		other.variant.faction = config["faction"]
		other.weaponData = weapon_data
		config["group"].add_child(other)
		var other_sound: AudioStreamPlayer3D = load("res://Scripts/AudioInstance3D.gd").new()
		root.add_child(other_sound)
		other_sound.global_position = other.muzzle.global_position
		other_sound.PlayInstance(fire_event, 50.0, 400.0)
		await process_frame
		await process_frame
		var other_shot := _receive_shot(receiver)
		if other_shot.get("shooter_id") != other.get_instance_id() or other_shot.get("position") != [config["x"], 0.0, 0.0]:
			_fail("Nomad or boss shot misclassified: %s" % other_shot)
			return
	if overlay.get("_shots").size() != 3:
		_fail("Expected enemy, Nomad and boss markers")
		return
	print("SHOT SMOKE OK: enemy, Nomad and boss AI included")

	var explosion: AudioStreamPlayer3D = load("res://Scripts/AudioInstance3D.gd").new()
	root.add_child(explosion)
	explosion.global_position = agent.muzzle.global_position
	explosion.PlayInstance(explosion_event, 50.0, 400.0)
	var tail: AudioStreamPlayer3D = load("res://Scripts/AudioInstance3D.gd").new()
	root.add_child(tail)
	tail.global_position = agent.muzzle.global_position
	tail.PlayInstance(fire_event, 100.0, 400.0)
	var distant: AudioStreamPlayer3D = load("res://Scripts/AudioInstance3D.gd").new()
	root.add_child(distant)
	distant.global_position = agent.muzzle.global_position + Vector3(10, 0, 0)
	distant.PlayInstance(fire_event, 50.0, 400.0)
	var vehicle := Node3D.new()
	map.add_child(vehicle)
	var vehicle_audio: AudioStreamPlayer3D = load("res://Scripts/AudioInstance3D.gd").new()
	vehicle.add_child(vehicle_audio)
	vehicle_audio.PlayInstance(fire_event, 50.0, 400.0)
	var player_audio := AudioStreamPlayer2D.new()
	root.add_child(player_audio)
	player_audio.stream = fire_stream
	await process_frame
	await process_frame
	if not _receive_shot(receiver).is_empty() or overlay.get("_shots").size() != 3:
		_fail("Explosion, tail, distant or non-AI sound produced a false AI marker")
		return
	print("SHOT SMOKE OK: explosion, tail, distant, player/vehicle sounds rejected")

	var added: int = overlay.get("_shots")[0]["time_ms"]
	if not is_equal_approx(overlay.call("_shot_opacity", added, added), 1.0) or not is_equal_approx(overlay.call("_shot_opacity", added, added + 2500), 0.5) or not is_equal_approx(overlay.call("_shot_opacity", added, added + 5000), 0.0):
		_fail("HUD opacity did not fade smoothly in five seconds")
		return
	var newest: int = overlay.get("_shots")[-1]["time_ms"]
	overlay.call("_prune_shots", newest + 5000)
	if not overlay.get("_shots").is_empty():
		_fail("Expired shot was not removed")
		return
	overlay.call("add_gunshot", [12.0, 0.0, 0.0])
	overlay.call("update_snapshot", {"map": {"id": "MapNew", "name": "Village"}, "player": {"position": [0.0, 0.0, 0.0]}})
	if not overlay.get("_shots").is_empty():
		_fail("Old map shot leaked across a scene transition")
		return
	print("SHOT SMOKE OK: five-second fade and scene-local expiration")

	var mode_names := ["AI+TRAILS+SHOTS" if no_loot else "ALL+SHOTS", "AI ONLY", "TRAILS ONLY"]
	if not no_loot:
		mode_names.append("LOOT ONLY")
	mode_names.append("SHOT ALERTS")
	for expected in mode_names:
		bridge.call("_poll_radar_keys", true, false)
		bridge.call("_poll_radar_keys", false, false)
		if overlay.call("get_mode_name") != expected or not overlay.visible or not is_equal_approx(overlay.modulate.a, 1.0):
			_fail("F7 mode %s changed or unexpectedly hid the radar" % expected)
			return
	var hint: Label = bridge.get("_mode_hint")
	if hint == null or not hint.visible or hint.text != "RADAR: SHOT ALERTS" or hint.mouse_filter != Control.MOUSE_FILTER_IGNORE:
		_fail("F7 shot-alert choice is not confirmed by a separate, non-interactive on-screen hint")
		return
	bridge.call("_update_mode_hint", Time.get_ticks_msec() + 1801)
	if hint.visible:
		_fail("F7 selection hint did not disappear after 1.8 seconds")
		return
	if int(overlay.get("_layer_mode")) != 0 or overlay.call("get_mode_name") != "SHOT ALERTS" or not overlay.visible or not is_equal_approx(overlay.modulate.a, 1.0):
		_fail("A quiet Shot Alerts radar must stay visible")
		return
	overlay.call("add_gunshot", [500.0, 0.0, 0.0])
	if not overlay.visible or overlay.call("_project", Vector2(500.0, 0.0), [0.0, 0.0, 0.0], 0.0) != null:
		_fail("Off-range fire must not draw a marker or hide the radar")
		return
	overlay.call("add_gunshot", [12.0, 0.0, 0.0])
	if not overlay.visible or not is_equal_approx(overlay.modulate.a, 1.0):
		_fail("Shot-only radar vanished when nearby AI fired")
		return
	var alert_time: int = overlay.get("_shots")[-1]["time_ms"]
	if not is_equal_approx(overlay.call("_shot_opacity", alert_time, alert_time + 2500), 0.5):
		_fail("Shot marker did not fade smoothly inside the persistent radar")
		return
	bridge.call("_poll_radar_keys", false, true)
	if overlay.visible:
		_fail("F8 did not override shot alert")
		return
	overlay.call("_process", 0.016)
	if overlay.visible:
		_fail("Shot alert bypassed the player's F8 hide choice")
		return
	bridge.call("_poll_radar_keys", false, false)
	bridge.call("_poll_radar_keys", false, true)
	if not overlay.visible:
		_fail("F8 did not restore shot alert")
		return
	overlay.call("_prune_shots", alert_time + 5000)
	if not overlay.get("_shots").is_empty() or not overlay.visible or not is_equal_approx(overlay.modulate.a, 1.0):
		_fail("Shot-only radar should remain visible after its markers expire")
		return
	overlay.call("cycle_layers")
	if int(overlay.get("_layer_mode")) != 1 or overlay.call("get_mode_name") != ("AI+TRAILS+SHOTS" if no_loot else "ALL+SHOTS") or not overlay.visible or not is_equal_approx(overlay.modulate.a, 1.0):
		_fail("F7 did not restore the unchanged all-layers radar after shot alerts")
		return
	print("SHOT SMOKE OK: F7 cycles from default Shot Alerts through all existing layers, F8 and fade")
	quit()


func _drain(receiver: PacketPeerUDP) -> void:
	while receiver.get_available_packet_count() > 0:
		receiver.get_packet()


func _receive_shot(receiver: PacketPeerUDP) -> Dictionary:
	for attempt in 40:
		while receiver.get_available_packet_count() > 0:
			var value: Variant = JSON.parse_string(receiver.get_packet().get_string_from_utf8())
			if value is Dictionary and value.get("type") == "gunshot":
				return value
		OS.delay_msec(5)
	return {}


func _fail(message: String) -> void:
	push_error(message)
	quit(1)
