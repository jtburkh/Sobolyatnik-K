extends Node

const PREFIX := "[RTV_TELEMETRY_PROOF]"
const PROTOCOL_VERSION := 1
const SNAPSHOT_INTERVAL_SECONDS := 0.1
const LOG_INTERVAL_SECONDS := 1.0
const TELEMETRY_HOST := "127.0.0.1"
const TELEMETRY_PORT := 47777
const TELEMETRY_CONFIG_PATH := "user://rtv-telemetry.cfg"
const GAME_DATA_PATH := "res://Resources/GameData.tres"
const AI_SPAWNER_PATH := "/root/Map/AI"
const AI_SCRIPT := "res://Scripts/AI.gd"
const AI_FACTIONS := ["Nomad", "Bandit", "Guard", "Military", "Boss"]
const LOOT_CONTAINER_SCRIPT := "res://Scripts/LootContainer.gd"
const LOOT_SCAN_INTERVAL_SECONDS := 1.0
const LOOT_MAX_DISTANCE := 425.0
const COMMAND_PATH := "user://rtv-toolkit-command.cfg"
const COMMAND_RESULT_PATH := "user://rtv-toolkit-command-result.cfg"
const COMMAND_POLL_INTERVAL_SECONDS := 0.25
const EVENT_SYSTEM_SCRIPT := "res://Scripts/EventSystem.gd"
const AI_SPAWNER_SCRIPT := "res://Scripts/AISpawner.gd"
const RADAR_OVERLAY_SCRIPT := preload("res://RtVTelemetryProof/RadarOverlay.gd")
const CEREBRAL_MULTIPLIER_SCENE := preload("res://RtVTelemetryProof/CerebralMultiplier.tscn")
const PROGRESSION_PATH := "user://rtv-toolkit-progression.cfg"
const ARTIFACT_SCAN_INTERVAL_SECONDS := 1.0
# Crash isolation: no combat-node reads, drawing, artifact or commands by default.
# Each additional mode must be enabled deliberately in user://rtv-telemetry.cfg.
const DIAGNOSTIC_PROFILES := ["player_only", "overlay_only", "ai_only", "loot_only", "overlay_ai"]
const TRACE_LIMIT_BYTES := 2 * 1024 * 1024
const TRACE_PREFIX := "rtv-telemetry-trace-"

var _game_data: Resource
var _udp := PacketPeerUDP.new()
var _snapshot_elapsed := 0.0
var _log_elapsed := 0.0
var _waiting_elapsed := 0.0
var _send_error_logged := false
var _verbose_logging := false
var _diagnostic_profile := "player_only"
var _trace_enabled := true
var _trace_mouse := true
var _trace_file: FileAccess
var _trace_path := ""
var _trace_bytes := 0
var _trace_sequence := 0
var _telemetry_host := TELEMETRY_HOST
var _telemetry_port := TELEMETRY_PORT
var _loot_scan_elapsed := 0.0
var _loot_scan_initialized := false
var _loot_cache: Array[Dictionary] = []
var _property_cache: Dictionary = {}
var _command_elapsed := 0.0
var _last_command_id := ""
var _radar_overlay: Control
var _overlay_enabled := true
var _overlay_range := 200.0
var _overlay_size := 230.0
var _overlay_toggle_key: Key = KEY_F8
var _overlay_layers := {
	"ai": true,
	"bosses": true,
	"trails": true,
	"loot": true,
	"gunshots": true,
}
var _access_unlocked := false
var _artifact: Node3D
var _artifact_scan_elapsed := 0.0


func _ready() -> void:
	_game_data = load(GAME_DATA_PATH)
	if _game_data == null:
		push_warning("%s could not load %s; telemetry will remain idle" % [PREFIX, GAME_DATA_PATH])
		return
	_load_network_config()
	_open_trace()
	_trace("ready.begin profile=%s" % _diagnostic_profile)
	if _diagnostic_profile in ["overlay_only", "overlay_ai"]:
		_create_radar_overlay()
	var destination_error := _udp.set_dest_address(_telemetry_host, _telemetry_port)
	if destination_error != OK:
		push_warning("%s could not configure UDP destination: error %d" % [PREFIX, destination_error])
		return
	print(
		"%s loaded; UDP v%d -> %s:%d; diagnostic profile=%s; waiting for a playable scene" % [
			PREFIX,
			PROTOCOL_VERSION,
			_telemetry_host,
			_telemetry_port,
			_diagnostic_profile,
		]
	)
	# Build 2: do not declare or register AI.gd hooks. Metro Mod Loader 3.2.1
	# rewrites AI.gd using removed `boss` fields and breaks NPC animation.
	_trace("ready.done")


func _exit_tree() -> void:
	if _trace_file != null:
		_trace("exit")
		_trace_file.close()
		_trace_file = null


func _open_trace() -> void:
	if not _trace_enabled:
		return
	var timestamp := Time.get_datetime_string_from_system(true, true).replace(":", "-")
	_trace_path = "user://%s%s-%d.log" % [TRACE_PREFIX, timestamp, Time.get_ticks_usec()]
	_trace_file = FileAccess.open(_trace_path, FileAccess.WRITE)
	if _trace_file == null:
		push_warning("%s cannot create diagnostic trace (%d)" % [PREFIX, FileAccess.get_open_error()])
		return
	print("%s trace: %s (limit %d bytes)" % [PREFIX, _trace_path, TRACE_LIMIT_BYTES])


func _trace(phase: String) -> void:
	if _trace_file == null or _trace_bytes >= TRACE_LIMIT_BYTES:
		return
	_trace_sequence += 1
	var line := "%d\t%d\t%s\n" % [Time.get_ticks_usec(), _trace_sequence, phase]
	if _trace_bytes + line.to_utf8_buffer().size() > TRACE_LIMIT_BYTES:
		_trace_file.store_line("%d\t%d\ttrace.limit_reached" % [Time.get_ticks_usec(), _trace_sequence])
		_trace_file.flush()
		_trace_bytes = TRACE_LIMIT_BYTES
		return
	_trace_file.store_string(line)
	_trace_file.flush() # Preserve the last phase even if RTV.exe exits abruptly.
	_trace_bytes += line.to_utf8_buffer().size()


func _load_network_config() -> void:
	var config := ConfigFile.new()
	var load_error := config.load(TELEMETRY_CONFIG_PATH)
	if load_error == ERR_FILE_NOT_FOUND:
		return
	if load_error != OK:
		push_warning("%s ignored invalid %s (error %d)" % [PREFIX, TELEMETRY_CONFIG_PATH, load_error])
		return
	_verbose_logging = bool(config.get_value("debug", "verbose_logging", false))
	_trace_enabled = bool(config.get_value("diagnostic", "trace", true))
	_trace_mouse = bool(config.get_value("diagnostic", "trace_mouse", true))
	var profile := str(config.get_value("diagnostic", "profile", "player_only"))
	if profile in DIAGNOSTIC_PROFILES:
		_diagnostic_profile = profile
	else:
		push_warning("%s unknown diagnostic profile %s; using player_only" % [PREFIX, profile])
	_telemetry_host = str(config.get_value("network", "host", TELEMETRY_HOST))
	_telemetry_port = int(config.get_value("network", "port", TELEMETRY_PORT))
	_overlay_enabled = bool(config.get_value("overlay", "enabled", true))
	_overlay_range = clampf(float(config.get_value("overlay", "range", 200.0)), 50.0, 400.0)
	_overlay_size = clampf(float(config.get_value("overlay", "size", 230.0)), 180.0, 320.0)
	_overlay_layers = {
		"ai": bool(config.get_value("overlay", "show_ai", true)),
		"bosses": bool(config.get_value("overlay", "show_bosses", true)),
		"trails": bool(config.get_value("overlay", "show_trails", true)),
		"loot": bool(config.get_value("overlay", "show_loot", true)),
		"gunshots": bool(config.get_value("overlay", "show_gunshots", true)),
	}
	var configured_key := str(config.get_value("overlay", "toggle_key", "F8"))
	var parsed_key := OS.find_keycode_from_string(configured_key)
	if parsed_key != 0:
		_overlay_toggle_key = parsed_key
	else:
		push_warning("%s ignored invalid overlay toggle key: %s" % [PREFIX, configured_key])
	if _telemetry_host.is_empty() or _telemetry_port < 1 or _telemetry_port > 65535:
		push_warning("%s ignored invalid network configuration" % PREFIX)
		_telemetry_host = TELEMETRY_HOST
		_telemetry_port = TELEMETRY_PORT


func _load_progression() -> void:
	var config := ConfigFile.new()
	var load_error := config.load(PROGRESSION_PATH)
	if load_error == ERR_FILE_NOT_FOUND:
		return
	if load_error != OK:
		push_warning("%s ignored invalid progression file (error %d)" % [PREFIX, load_error])
		return
	_access_unlocked = bool(config.get_value("progression", "cerebral_multiplier", false))


func unlock_cerebral_multiplier() -> bool:
	if _access_unlocked:
		return true
	var config := ConfigFile.new()
	config.set_value("progression", "cerebral_multiplier", true)
	var save_error := config.save(PROGRESSION_PATH)
	if save_error != OK:
		push_warning("%s could not persist CM-7 unlock (error %d)" % [PREFIX, save_error])
		if is_instance_valid(_radar_overlay):
			_radar_overlay.call("show_summon_result", "CM-7 UNLOCK FAILED", false)
		return false
	_access_unlocked = true
	if is_instance_valid(_radar_overlay):
		_radar_overlay.call("set_access_unlocked", true)
		_radar_overlay.call("show_summon_result", "CM-7 LINK ESTABLISHED", true)
	print("%s CM-7 Cerebral Multiplier recovered; interface permanently unlocked" % PREFIX)
	return true


func _ensure_cerebral_multiplier() -> void:
	if _access_unlocked or is_instance_valid(_artifact):
		return
	var player := get_tree().get_first_node_in_group("Player")
	var current_scene := get_tree().current_scene
	if player == null or current_scene == null:
		return
	var player_position: Vector3 = _property_or(_game_data, &"playerPosition", Vector3.ZERO)
	var candidates: Array[Node3D] = []
	for group_name in [&"AI_SP", &"AI_WP"]:
		for candidate in get_tree().get_nodes_in_group(group_name):
			if candidate is Node3D and current_scene.is_ancestor_of(candidate):
				candidates.append(candidate)
		if not candidates.is_empty():
			break
	if candidates.is_empty():
		return
	var selected := candidates[0]
	var selected_distance := selected.global_position.distance_to(player_position)
	for candidate in candidates:
		var distance := candidate.global_position.distance_to(player_position)
		if distance >= 25.0 and (selected_distance < 25.0 or distance < selected_distance):
			selected = candidate
			selected_distance = distance
	_artifact = CEREBRAL_MULTIPLIER_SCENE.instantiate()
	current_scene.add_child(_artifact, true)
	_artifact.global_position = selected.global_position + Vector3(0.0, 0.18, 0.0)
	_artifact.rotation.y = deg_to_rad(fmod(float(_artifact.get_instance_id()), 360.0))
	if is_instance_valid(_radar_overlay):
		_radar_overlay.call("set_artifact_position", [
			_artifact.global_position.x,
			_artifact.global_position.y,
			_artifact.global_position.z,
		])
	print("%s CM-7 artifact placed at game waypoint %.1f m from player" % [PREFIX, selected_distance])


func _create_radar_overlay() -> void:
	var canvas := CanvasLayer.new()
	canvas.name = "RtVTacticalRadarLayer"
	canvas.layer = 100
	add_child(canvas)
	_radar_overlay = RADAR_OVERLAY_SCRIPT.new()
	_radar_overlay.name = "RtVTacticalRadar"
	canvas.add_child(_radar_overlay)
	_radar_overlay.call(
		"configure",
		_overlay_range,
		_overlay_size,
		OS.get_keycode_string(_overlay_toggle_key),
		_overlay_layers
	)
	_radar_overlay.call("set_diagnostic_mode", true)
	_radar_overlay.call("set_access_unlocked", false)
	_radar_overlay.call("set_overlay_enabled", _overlay_enabled)
	_radar_overlay.connect(&"summon_requested", Callable(self, "_on_overlay_summon_requested"))


func _input(event: InputEvent) -> void:
	if _trace_mouse and event is InputEventMouseButton and event.button_index == MOUSE_BUTTON_LEFT and event.pressed:
		# Observe modifiers only; never press/release actions or consume mouse input.
		var equip := InputMap.has_action("item_equip") and Input.is_action_pressed("item_equip")
		var transfer := InputMap.has_action("item_transfer") and Input.is_action_pressed("item_transfer")
		var drop := InputMap.has_action("item_drop") and Input.is_action_pressed("item_drop")
		_trace("input.left equip=%s transfer=%s drop=%s" % [equip, transfer, drop])
	if _diagnostic_profile not in ["overlay_only", "overlay_ai"]:
		return
	if event is InputEventKey and event.pressed and not event.echo and event.keycode == _overlay_toggle_key:
		_overlay_enabled = not _overlay_enabled
		if is_instance_valid(_radar_overlay):
			_radar_overlay.call("set_overlay_enabled", _overlay_enabled)
		print("%s in-game radar %s" % [PREFIX, "shown" if _overlay_enabled else "hidden"])
		get_viewport().set_input_as_handled()


func _process(delta: float) -> void:
	# No file-based commands or artifact placement in any diagnostic profile.
	if _game_data == null:
		return
	_snapshot_elapsed += delta
	if _verbose_logging:
		_log_elapsed += delta
	if _diagnostic_profile == "loot_only":
		_loot_scan_elapsed += delta
	if _snapshot_elapsed < SNAPSHOT_INTERVAL_SECONDS:
		return
	_snapshot_elapsed = fmod(_snapshot_elapsed, SNAPSHOT_INTERVAL_SECONDS)

	var snapshot := _capture_snapshot()
	if snapshot.is_empty():
		if is_instance_valid(_radar_overlay):
			_radar_overlay.call("clear_snapshot")
		_waiting_elapsed += SNAPSHOT_INTERVAL_SECONDS
		if _waiting_elapsed >= 5.0:
			_waiting_elapsed = 0.0
			print("%s waiting: no Player-group node in the current scene" % PREFIX)
		return
	_waiting_elapsed = 0.0
	if is_instance_valid(_radar_overlay):
		_trace("overlay.enter")
		_radar_overlay.call("update_snapshot", snapshot)
		_trace("overlay.done")
	_trace("udp.enter")
	_send_snapshot(snapshot)
	_trace("udp.done")

	if _verbose_logging and _log_elapsed >= LOG_INTERVAL_SECONDS:
		_log_elapsed = fmod(_log_elapsed, LOG_INTERVAL_SECONDS)
		_log_snapshot(snapshot)


func _on_overlay_summon_requested(_action: String) -> void:
	_show_overlay_result("SUMMON DISABLED DURING DIAGNOSTICS", false)


func _summon_airdrop_from_overlay(current_scene: Node) -> void:
	var event_system := _find_node_with_script(current_scene, EVENT_SYSTEM_SCRIPT)
	if event_system == null or not event_system.has_method("Airdrop"):
		_show_overlay_result("CASA EVENT UNAVAILABLE", false)
		return
	event_system.call_deferred("Airdrop")
	_show_overlay_result("CASA AIRDROP QUEUED", true)
	print("%s in-game summon: native EventSystem.Airdrop queued" % PREFIX)


func _summon_boss_from_overlay(current_scene: Node) -> void:
	var ai_spawner := _find_node_with_script(current_scene, AI_SPAWNER_SCRIPT)
	if ai_spawner == null or not ai_spawner.has_method("SpawnBoss"):
		_show_overlay_result("BOSS SPAWNER UNAVAILABLE", false)
		return
	var boss_pool := ai_spawner.get_node_or_null("B_Pool")
	if boss_pool == null or boss_pool.get_node_or_null("AI_Punisher") == null:
		_show_overlay_result("PUNISHER UNAVAILABLE OR CONSUMED", false)
		return
	var waypoints: Array[Node3D] = []
	for point in get_tree().get_nodes_in_group("AI_WP"):
		if point is Node3D and current_scene.is_ancestor_of(point):
			waypoints.append(point)
	if waypoints.is_empty():
		_show_overlay_result("NO AI WAYPOINTS ON THIS MAP", false)
		return
	var player_position: Vector3 = _property_or(_game_data, &"playerPosition", Vector3.ZERO)
	var spawn_distance := float(_property_or(ai_spawner, &"spawnDistance", 100.0))
	var valid_points: Array[Node3D] = []
	for candidate in get_tree().get_nodes_in_group("AI_SP"):
		if candidate is Node3D and current_scene.is_ancestor_of(candidate) and candidate.global_position.distance_to(player_position) > spawn_distance:
			valid_points.append(candidate)
	if valid_points.is_empty():
		_show_overlay_result("NO SAFE BOSS SPAWN POINT", false)
		return
	var spawn_point: Node3D = valid_points.pick_random()
	ai_spawner.call_deferred("SpawnBoss", "Punisher", "Attack", spawn_point.global_position, waypoints.pick_random())
	_show_overlay_result("PUNISHER BOSS QUEUED", true)
	print("%s in-game summon: native AISpawner.SpawnBoss queued" % PREFIX)


func _show_overlay_result(message: String, success: bool) -> void:
	if is_instance_valid(_radar_overlay):
		_radar_overlay.call("show_summon_result", message, success)


func _poll_control_request() -> void:
	if not FileAccess.file_exists(COMMAND_PATH):
		return
	var config := ConfigFile.new()
	var load_error := config.load(COMMAND_PATH)
	if load_error != OK:
		push_warning("%s ignored malformed runtime command (error %d)" % [PREFIX, load_error])
		return
	var command_id := str(config.get_value("command", "id", ""))
	var action := str(config.get_value("command", "action", ""))
	if command_id.is_empty() or action not in ["spawn_airdrop", "spawn_boss"]:
		_reject_control_request(command_id, "unsupported or malformed command")
		return
	if command_id == _last_command_id:
		_remove_control_request()
		return
	var player := get_tree().get_first_node_in_group("Player")
	var current_scene := get_tree().current_scene
	if player == null or current_scene == null:
		return
	if action == "spawn_airdrop":
		_poll_airdrop_command(command_id, current_scene)
	else:
		_poll_boss_command(command_id, current_scene)


func _poll_airdrop_command(command_id: String, current_scene: Node) -> void:
	var event_system := _find_node_with_script(current_scene, EVENT_SYSTEM_SCRIPT)
	if event_system == null or not event_system.has_method("Airdrop"):
		return
	if not _consume_control_request(command_id, "airdrop"):
		return
	event_system.call_deferred("Airdrop")
	_write_command_result(command_id, "accepted", "native EventSystem.Airdrop queued")
	print("%s command %s: native EventSystem.Airdrop queued" % [PREFIX, command_id])


func _poll_boss_command(command_id: String, current_scene: Node) -> void:
	var ai_spawner := _find_node_with_script(current_scene, AI_SPAWNER_SCRIPT)
	if ai_spawner == null or not ai_spawner.has_method("SpawnBoss"):
		return
	var boss_pool := ai_spawner.get_node_or_null("B_Pool")
	if boss_pool == null:
		return
	if boss_pool.get_node_or_null("AI_Punisher") == null:
		_reject_control_request(command_id, "native Punisher is unavailable or already consumed")
		return
	var waypoints: Array[Node3D] = []
	for point in get_tree().get_nodes_in_group("AI_WP"):
		if point is Node3D and current_scene.is_ancestor_of(point):
			waypoints.append(point)
	if waypoints.is_empty():
		_reject_control_request(command_id, "current map has no AI waypoints")
		return
	var player_position: Vector3 = _property_or(_game_data, &"playerPosition", Vector3.ZERO)
	var spawn_distance := float(_property_or(ai_spawner, &"spawnDistance", 100.0))
	var valid_points: Array[Node3D] = []
	for candidate in get_tree().get_nodes_in_group("AI_SP"):
		if candidate is Node3D and current_scene.is_ancestor_of(candidate) and candidate.global_position.distance_to(player_position) > spawn_distance:
			valid_points.append(candidate)
	if valid_points.is_empty():
		_reject_control_request(command_id, "current map has no safe boss spawn point")
		return
	var spawn_point: Node3D = valid_points.pick_random()
	if not _consume_control_request(command_id, "boss"):
		return
	ai_spawner.call_deferred("SpawnBoss", "Punisher", "Attack", spawn_point.global_position, waypoints.pick_random())
	_write_command_result(command_id, "accepted", "native Punisher queued")
	print("%s command %s: native AISpawner.SpawnBoss queued" % [PREFIX, command_id])


func _consume_control_request(command_id: String, action_name: String) -> bool:
	# Consume before execution for at-most-once behavior across crashes/restarts.
	if not _remove_control_request():
		push_warning("%s could not consume %s request; command not executed" % [PREFIX, action_name])
		return false
	_last_command_id = command_id
	return true


func _reject_control_request(command_id: String, reason: String) -> void:
	if not _remove_control_request():
		return
	_write_command_result(command_id, "rejected", reason)
	push_warning("%s rejected runtime command %s: %s" % [PREFIX, command_id, reason])


func _remove_control_request() -> bool:
	var absolute_path := ProjectSettings.globalize_path(COMMAND_PATH)
	return DirAccess.remove_absolute(absolute_path) == OK


func _write_command_result(command_id: String, status: String, message: String) -> void:
	var result := ConfigFile.new()
	result.set_value("result", "id", command_id)
	result.set_value("result", "status", status)
	result.set_value("result", "message", message)
	var save_error := result.save(COMMAND_RESULT_PATH)
	if save_error != OK:
		push_warning("%s could not write command result (error %d)" % [PREFIX, save_error])


func _find_node_with_script(root: Node, script_path: String) -> Node:
	var pending: Array[Node] = [root]
	while not pending.is_empty():
		var node: Node = pending.pop_back()
		var script: Variant = node.get_script()
		if script is Script and (script as Script).resource_path == script_path:
			return node
		for child in node.get_children():
			pending.append(child)
	return null


func _capture_snapshot() -> Dictionary:
	var player := get_tree().get_first_node_in_group("Player")
	if player == null:
		return {}

	_trace("capture.enter")
	var position: Vector3 = _property_or(_game_data, &"playerPosition", Vector3.ZERO)
	var forward: Vector3 = _property_or(_game_data, &"playerVector", Vector3.ZERO)
	_trace("capture.player")
	var ai_snapshots: Array[Dictionary] = []
	if _diagnostic_profile in ["ai_only", "overlay_ai"]:
		_trace("capture.ai.enter")
		ai_snapshots = _capture_ai_snapshots()
		_trace("capture.ai.done count=%d" % ai_snapshots.size())
	var loot_snapshots: Array[Dictionary] = []
	if _diagnostic_profile == "loot_only":
		_trace("capture.loot.enter")
		loot_snapshots = _active_loot_containers(position)
		_trace("capture.loot.done count=%d" % loot_snapshots.size())
	_trace("capture.map.enter")
	var map_identity := _current_map_identity()
	_trace("capture.map.done")
	var result := {
		"version": PROTOCOL_VERSION,
		"type": "snapshot",
		"diagnostic_profile": _diagnostic_profile,
		"timestamp_ms": Time.get_ticks_msec(),
		"map": map_identity,
		"player": {
			"id": player.get_instance_id(),
			"position": [position.x, position.y, position.z],
			"heading": _heading_degrees(forward),
		},
		"ai": ai_snapshots,
		"loot": loot_snapshots,
	}
	_trace("capture.done")
	return result


func _capture_ai_snapshots() -> Array[Dictionary]:
	var ai_snapshots: Array[Dictionary] = []
	for ai in _active_ai_nodes():
		if not is_instance_valid(ai) or not (ai is Node3D):
			continue
		var ai_position := (ai as Node3D).global_position
		var ai_forward := _ai_view_direction(ai)
		var sensor: Variant = _property_or(ai, &"sensor", null)
		var faction := _ai_faction(ai)
		var priority: Variant = _property_or(sensor, &"priority", null) if sensor is Object else null
		var player_target: bool = priority is Node and is_instance_valid(priority) and priority.is_in_group("Player")
		ai_snapshots.append({
			"id": ai.get_instance_id(),
			"position": [ai_position.x, ai_position.y, ai_position.z],
			"alive": not bool(_property_or(ai, &"dead", false)),
			"heading": _heading_degrees(ai_forward),
			"vision_range": _ai_vision_range(ai),
			"vision_half_angle": _ai_vision_half_angle(sensor),
			"player_visible": player_target and bool(_property_or(sensor, &"PVisible", false)),
			"boss": faction == "Boss",
			"faction": faction,
			"friendly": faction == "Nomad" and float(_property_or(_game_data, &"reputation", 0.0)) >= 50.0,
		})

	return ai_snapshots


func _current_map_identity() -> Dictionary:
	var current_scene := get_tree().current_scene
	if current_scene == null:
		return {}
	var scene_path := current_scene.scene_file_path
	var map_name := str(current_scene.name)
	if not scene_path.is_empty():
		map_name = scene_path.get_file().get_basename().capitalize()
	return {
		"id": scene_path if not scene_path.is_empty() else str(current_scene.name),
		"name": map_name,
	}


func _ai_faction(ai: Node) -> String:
	var variant: Variant = _property_or(ai, &"variant", null)
	if not variant is Object:
		return "Unknown"
	var index := int(_property_or(variant, &"faction", -1))
	if index < 0 or index >= AI_FACTIONS.size():
		return "Unknown"
	return AI_FACTIONS[index]


func _ai_view_direction(ai: Node) -> Vector3:
	var sensor: Variant = _property_or(ai, &"sensor", null)
	if sensor is Node3D and is_instance_valid(sensor):
		# Sensor.Priority() uses +Z as the center of its 150-degree view.
		return (sensor as Node3D).global_transform.basis.z.normalized()
	return (ai as Node3D).global_transform.basis.z.normalized()


func _ai_vision_range(ai: Node) -> float:
	var sensor: Variant = _property_or(ai, &"sensor", null)
	return float(_property_or(sensor, &"viewDistance", 150.0)) if sensor is Object else 150.0


func _ai_vision_half_angle(sensor: Variant) -> Variant:
	if not sensor is Object or bool(_property_or(sensor, &"force", false)):
		# Forced sensor acquisition does not use the angular cone.
		return null
	return float(_property_or(sensor, &"viewAngle", 150.0)) / 2.0


func _active_loot_containers(player_position: Vector3) -> Array[Dictionary]:
	if not _loot_scan_initialized or _loot_scan_elapsed >= LOOT_SCAN_INTERVAL_SECONDS:
		_loot_scan_elapsed = 0.0
		_loot_scan_initialized = true
		_loot_cache = _scan_loot_containers()
	var active: Array[Dictionary] = []
	for entry in _loot_cache:
		var node: Variant = entry.get("node")
		if not node is Node3D or not is_instance_valid(node):
			continue
		var container := node as Node3D
		if not container.is_visible_in_tree() or not _container_has_enabled_proxy(entry):
			continue
		if not _container_has_loot(container):
			continue
		if container.global_position.distance_to(player_position) > LOOT_MAX_DISTANCE:
			continue
		active.append({
			"id": container.get_instance_id(),
			"position": [
				container.global_position.x,
				container.global_position.y,
				container.global_position.z,
			],
			"name": str(_property_or(container, &"containerName", container.name)),
			"locked": bool(_property_or(container, &"locked", false)),
			"corpse": bool(_property_or(container, &"corpse", false)),
		})
	return active


func _container_has_loot(container: Node3D) -> bool:
	var storaged := bool(_property_or(container, &"storaged", false))
	var contents_property: StringName = &"storage" if storaged else &"loot"
	var contents: Variant = _property_or(container, contents_property, [])
	return contents is Array and not (contents as Array).is_empty()


func _scan_loot_containers() -> Array[Dictionary]:
	var found: Dictionary = {}
	for proxy in get_tree().get_nodes_in_group("Interactable"):
		if not is_instance_valid(proxy):
			continue
		var container := _find_loot_container(proxy)
		if container == null:
			continue
		var id := container.get_instance_id()
		if not found.has(id):
			found[id] = {"node": container, "proxies": []}
		# Record disabled shapes too: a living AI's corpse container becomes
		# interactable on death without adding a new node to the scene tree.
		var shapes: Array = [proxy] if proxy is CollisionShape3D else proxy.find_children(
			"*", "CollisionShape3D", true, false
		)
		found[id]["proxies"].append({"proxy": proxy, "shapes": shapes})
	var result: Array[Dictionary] = []
	for entry in found.values():
		result.append(entry)
	return result


func _find_loot_container(start: Node) -> Node3D:
	var current: Node = start
	for _depth in 8:
		if current == null:
			break
		var script: Variant = current.get_script()
		if script is Script and (script as Script).resource_path == LOOT_CONTAINER_SCRIPT:
			return current as Node3D
		current = current.get_parent()
	return null


func _container_has_enabled_proxy(entry: Dictionary) -> bool:
	for proxy_entry in entry["proxies"]:
		var proxy: Node = proxy_entry["proxy"]
		if not is_instance_valid(proxy) or not proxy.is_inside_tree():
			continue
		var shapes: Array = proxy_entry["shapes"]
		if shapes.is_empty():
			return true
		for shape in shapes:
			if is_instance_valid(shape) and not (shape as CollisionShape3D).disabled:
				return true
	return false


func _send_snapshot(snapshot: Dictionary) -> void:
	_send_message(snapshot)


func _send_message(message: Dictionary) -> void:
	var packet := JSON.stringify(message).to_utf8_buffer()
	var send_error := _udp.put_packet(packet)
	if send_error == OK:
		_send_error_logged = false
	elif not _send_error_logged:
		_send_error_logged = true
		push_warning("%s UDP send failed with error %d; sampling will continue" % [PREFIX, send_error])


func _log_snapshot(snapshot: Dictionary) -> void:
	var player: Dictionary = snapshot["player"]
	var map: Dictionary = snapshot["map"]
	var position: Array = player["position"]
	print(
		"%s Map: %s (%s); Player: id=%d x=%.3f y=%.3f z=%.3f heading=%.2f" % [
			PREFIX,
			map.get("name", "Unknown"),
			map.get("id", ""),
			player["id"],
			position[0],
			position[1],
			position[2],
			player["heading"],
		]
	)
	var ai_snapshots: Array = snapshot["ai"]
	if ai_snapshots.is_empty():
		print("%s AI: none active" % PREFIX)
		return
	for index in ai_snapshots.size():
		var ai: Dictionary = ai_snapshots[index]
		var ai_position: Array = ai["position"]
		print(
			"%s AI #%d: id=%d x=%.3f y=%.3f z=%.3f alive=%s" % [
				PREFIX,
				index + 1,
				ai["id"],
				ai_position[0],
				ai_position[1],
				ai_position[2],
				str(ai["alive"]),
			]
		)


func _active_ai_nodes() -> Array[Node]:
	var result: Array[Node] = []
	var manager := get_node_or_null(AI_SPAWNER_PATH)
	if manager != null:
		for branch in ["Enemies", "Nomads"]:
			var active_root := manager.get_node_or_null(branch)
			if active_root == null:
				continue
			for child in active_root.get_children():
				if child is Node3D and _is_ai_node(child):
					result.append(child)
		return result

	# Defensive fallback for a map with another spawner hierarchy. Both
	# factions have collision proxies in groups; the root script and active
	# flag prevent including preallocated off-map E/N/B pool members.
	var seen: Dictionary = {}
	for group_name in ["AI", "Nomad"]:
		for proxy in get_tree().get_nodes_in_group(group_name):
			var candidate: Node = proxy.owner if proxy.owner != null else proxy
			if not _is_ai_node(candidate) or not bool(_property_or(candidate, &"active", false)):
				continue
			var id := candidate.get_instance_id()
			if not seen.has(id):
				seen[id] = true
				result.append(candidate)
	return result


func _is_ai_node(candidate: Node) -> bool:
	if not candidate is Node3D or not is_instance_valid(candidate):
		return false
	var script: Variant = candidate.get_script()
	return script is Script and (script as Script).resource_path == AI_SCRIPT


func _property_or(object: Object, property: StringName, fallback: Variant) -> Variant:
	if not is_instance_valid(object):
		return fallback
	# Exported property names are stable for a given script. Cache the full
	# property set once instead of rebuilding it for every field, AI and tick.
	var script: Variant = object.get_script()
	var cache_key: Variant = script if script is Script else object.get_class()
	if not _property_cache.has(cache_key):
		var properties: Dictionary = {}
		for entry in object.get_property_list():
			properties[StringName(entry.get("name", ""))] = true
		_property_cache[cache_key] = properties
	if _property_cache[cache_key].has(property):
		return object.get(property)
	return fallback


func _heading_degrees(forward: Vector3) -> float:
	if Vector2(forward.x, forward.z).is_zero_approx():
		return 0.0
	# Road to Vostok stores camera-forward in playerVector. Godot forward is -Z,
	# so north/-Z maps to 0 degrees and +X maps to 90 degrees.
	return fposmod(rad_to_deg(atan2(forward.x, -forward.z)), 360.0)
