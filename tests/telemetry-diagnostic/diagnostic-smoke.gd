extends SceneTree

func _initialize() -> void:
	call_deferred("_run")


func _run() -> void:
	var player := Node3D.new()
	player.name = "MockPlayer"
	player.add_to_group("Player")
	root.add_child(player)
	var map := Node3D.new()
	map.name = "Map"
	root.add_child(map)
	var ai_spawner := Node3D.new()
	ai_spawner.name = "AI"
	map.add_child(ai_spawner)
	var enemies := Node3D.new()
	enemies.name = "Enemies"
	ai_spawner.add_child(enemies)
	var nomads := Node3D.new()
	nomads.name = "Nomads"
	ai_spawner.add_child(nomads)
	var nomad: Node3D = load("res://Scripts/AI.gd").new()
	var variant: Resource = load("res://Scripts/MockVariant.gd").new()
	nomad.set("variant", variant)
	var sensor: Node3D = load("res://Scripts/MockSensor.gd").new()
	nomad.add_child(sensor)
	nomad.set("sensor", sensor)
	sensor.set("priority", player)
	nomads.add_child(nomad)
	var container: Node3D = load("res://Scripts/LootContainer.gd").new()
	map.add_child(container)
	var area := Area3D.new()
	container.add_child(area)
	var proxy := CollisionShape3D.new()
	proxy.shape = SphereShape3D.new()
	proxy.add_to_group("Interactable")
	area.add_child(proxy)
	var bridge: Node = load("res://RtVTelemetryProof/Main.gd").new()
	bridge.process_mode = Node.PROCESS_MODE_DISABLED
	root.add_child(bridge)
	if bridge.get("_game_data") == null:
		push_error("Mock GameData was not loaded")
		quit(1)
		return
	bridge.call("_process", 0.11)
	var click := InputEventMouseButton.new()
	click.button_index = MOUSE_BUTTON_LEFT
	click.pressed = true
	bridge.call("_input", click)
	var trace_path: String = bridge.get("_trace_path")
	var trace := FileAccess.get_file_as_string(trace_path)
	if trace_path.is_empty() or not trace.contains("capture.enter") or not trace.contains("capture.done") or not trace.contains("udp.done") or not trace.contains("input.left equip=false"):
		push_error("Trace missing flushed phases: %s" % trace)
		quit(1)
		return
	print("SMOKE OK: trace flushed snapshot and input phases")
	for profile in ["player_only", "overlay_only", "ai_only", "loot_only", "overlay_ai"]:
		bridge.set("_diagnostic_profile", profile)
		var snap: Dictionary = bridge.call("_capture_snapshot")
		if snap.is_empty() or snap.get("diagnostic_profile") != profile or snap["ai"].size() != (1 if profile in ["ai_only", "overlay_ai"] else 0) or snap["loot"].size() != (1 if profile == "loot_only" else 0):
			push_error("Bad diagnostics snapshot for %s: %s" % [profile, JSON.stringify(snap)])
			quit(1)
			return
		print("SMOKE OK: %s ai=%d loot=%d" % [profile, snap["ai"].size(), snap["loot"].size()])
	bridge.set("_diagnostic_profile", "overlay_ai")
	bridge.call("_create_radar_overlay")
	var overlay: Control = bridge.get("_radar_overlay")
	overlay.call("update_snapshot", bridge.call("_capture_snapshot"))
	await process_frame
	if not overlay.visible or not bool(overlay.get("_diagnostic_mode")):
		push_error("Diagnostic overlay missing")
		quit(1)
		return
	bridge.call("_on_overlay_summon_requested", "spawn_boss")
	print("SMOKE OK: diagnostic overlay and summon guard")
	quit()
