extends "res://RtVRadarShotBridge.gd"

# Next-release candidate. Only game-owned events, never arbitrary scenes or AI hooks.
const COMMAND_PATH := "user://rtv-toolkit-command.cfg"
const RESULT_PATH := "user://rtv-toolkit-command-result.cfg"
const EVENT_SCRIPT := "res://Scripts/EventSystem.gd"
const SPAWNER_SCRIPT := "res://Scripts/AISpawner.gd"
const COMMAND_MAX_AGE_SECONDS := 30
const SUMMON_COOLDOWN_MS := 15000
const MENU_TIMEOUT_MS := 8000
const OPTIONS := ["spawn_airdrop", "spawn_punisher", "spawn_bogeyman"]

var _summon_hint: Label
var _summon_menu := -1
var _summon_menu_until_ms := 0
var _summon_cooldown_until_ms := 0
var _f10_down := false
var _f11_down := false
var _command_elapsed := 0.0
var _last_command_id := ""


func _ready() -> void:
	super._ready()
	if is_instance_valid(_canvas):
		_summon_hint = Label.new()
		_summon_hint.name = "RtVRadarSummonHint"
		_summon_hint.mouse_filter = Control.MOUSE_FILTER_IGNORE
		_summon_hint.focus_mode = Control.FOCUS_NONE
		_summon_hint.anchor_left = 1.0
		_summon_hint.anchor_right = 1.0
		_summon_hint.offset_left = -310.0
		_summon_hint.offset_right = -18.0
		_summon_hint.offset_top = 306.0
		_summon_hint.offset_bottom = 334.0
		_summon_hint.horizontal_alignment = HORIZONTAL_ALIGNMENT_RIGHT
		_summon_hint.add_theme_font_size_override("font_size", 12)
		_summon_hint.add_theme_color_override("font_color", Color(0.69, 0.86, 0.89))
		_canvas.add_child(_summon_hint)


func _process(delta: float) -> void:
	super._process(delta)
	var now := Time.get_ticks_msec()
	_poll_summon_keys(Input.is_key_pressed(KEY_F10), Input.is_key_pressed(KEY_F11), now)
	_command_elapsed += delta
	if _command_elapsed >= 0.25:
		_command_elapsed = 0.0
		_poll_control_request()


func _poll_summon_keys(f10: bool, f11: bool, now: int) -> void:
	var scene := get_tree().current_scene
	var player := get_tree().get_first_node_in_group("Player")
	var playable := _controls_enabled and _has_player and is_instance_valid(_radar) and _radar.visible and scene != null and player != null and scene.is_ancestor_of(player)
	if not playable or (_summon_menu >= 0 and now > _summon_menu_until_ms):
		_summon_menu = -1
	if playable and f10 and not _f10_down:
		_summon_menu = (_summon_menu + 1) % OPTIONS.size()
		_summon_menu_until_ms = now + MENU_TIMEOUT_MS
	if playable and f11 and not _f11_down and _summon_menu >= 0:
		var action: String = OPTIONS[_summon_menu]
		_summon_menu = -1
		var result := _try_summon(action, scene, now)
		_show_mode_hint(result)
	_f10_down = f10
	_f11_down = f11
	if is_instance_valid(_summon_hint):
		_summon_hint.visible = playable
		if playable:
			_summon_hint.text = "F10 %s   F11 CONFIRM" % _summon_label(OPTIONS[_summon_menu]) if _summon_menu >= 0 else "F10 SUMMON MENU"


func _summon_label(action: String) -> String:
	match action:
		"spawn_airdrop": return "CASA AIRDROP"
		"spawn_punisher": return "PUNISHER"
		"spawn_bogeyman": return "BOGEYMAN"
	return "UNKNOWN"


func _try_summon(action: String, scene: Node, now: int) -> String:
	if now < _summon_cooldown_until_ms:
		return "SUMMON COOLDOWN"
	if action == "spawn_airdrop":
		var event_system := _find_native(scene, EVENT_SCRIPT)
		if not _has_native_method(event_system, "Airdrop", 0):
			return "CASA EVENT UNAVAILABLE"
		var event_map: Variant = event_system.get("map")
		if not event_map is Node or not is_instance_valid(event_map) or event_map.is_queued_for_deletion() or (event_map != scene and not scene.is_ancestor_of(event_map)):
			return "CASA EVENT NOT READY"
		event_system.call_deferred("Airdrop")
		_summon_cooldown_until_ms = now + SUMMON_COOLDOWN_MS
		return "CASA AIRDROP QUEUED"
	if action not in ["spawn_punisher", "spawn_bogeyman"]:
		return "UNKNOWN SUMMON"
	var spawner := _find_native(scene, SPAWNER_SCRIPT)
	if not _has_native_method(spawner, "SpawnBoss", 4) or not bool(spawner.get("active")):
		return "BOSS SPAWNER UNAVAILABLE"
	var name := "Punisher" if action == "spawn_punisher" else "Bogeyman"
	var pool := spawner.get_node_or_null("B_Pool")
	if pool == null or pool.get_node_or_null("AI_" + name) == null:
		return "%s UNAVAILABLE" % name.to_upper()
	var player_position: Variant = _game_data.get("playerPosition") if is_instance_valid(_game_data) else null
	if not player_position is Vector3:
		return "PLAYER POSITION UNAVAILABLE"
	var configured_distance: Variant = spawner.get("spawnDistance")
	if not configured_distance is float and not configured_distance is int:
		return "BOSS SPAWN DISTANCE UNAVAILABLE"
	var minimum := maxf(float(configured_distance), 100.0)
	var positions: Array[Node3D] = []
	var waypoints: Array[Node3D] = []
	if action == "spawn_punisher":
		positions = _valid_points(scene, "AI_SP", player_position, minimum)
		waypoints = _valid_points(scene, "AI_WP", player_position, 0.0)
	else:
		positions = _valid_points(scene, "AI_LP", player_position, minimum)
		waypoints = positions
	if positions.is_empty() or waypoints.is_empty():
		return "NO SAFE %s WAYPOINT" % name.to_upper()
	var point: Node3D = positions.pick_random()
	var waypoint: Node3D = waypoints.pick_random() if action == "spawn_punisher" else point
	spawner.call_deferred("SpawnBoss", name, "Attack" if action == "spawn_punisher" else "Lurk", point.global_position, waypoint)
	_summon_cooldown_until_ms = now + SUMMON_COOLDOWN_MS
	return "%s QUEUED" % name.to_upper()


func _has_native_method(node: Node, method_name: String, arguments: int) -> bool:
	if node == null or not node.has_method(method_name):
		return false
	for method in node.get_method_list():
		if str(method.get("name", "")) == method_name:
			var parameters: Variant = method.get("args", [])
			return parameters is Array and parameters.size() == arguments
	return false


func _valid_points(scene: Node, group_name: String, player_position: Vector3, minimum: float) -> Array[Node3D]:
	var valid: Array[Node3D] = []
	for node in get_tree().get_nodes_in_group(group_name):
		if node is Node3D and scene.is_ancestor_of(node) and not node.is_queued_for_deletion() and node.global_position.distance_to(player_position) > minimum:
			valid.append(node)
	return valid


func _find_native(scene: Node, script_path: String) -> Node:
	var pending: Array[Node] = [scene]
	while not pending.is_empty():
		var node: Node = pending.pop_back()
		if node.is_queued_for_deletion():
			continue
		var script: Variant = node.get_script()
		if script is Script and (script as Script).resource_path == script_path:
			return node
		for child in node.get_children():
			pending.append(child)
	return null


func _poll_control_request() -> void:
	if not FileAccess.file_exists(COMMAND_PATH):
		return
	var config := ConfigFile.new()
	if config.load(COMMAND_PATH) != OK:
		_reject_command("", "malformed command")
		return
	var command_id := str(config.get_value("command", "id", ""))
	var action := str(config.get_value("command", "action", ""))
	var parts := command_id.split("-")
	if parts.size() != 2 or not parts[0].is_valid_int() or not parts[1].is_valid_int():
		_reject_command(command_id, "invalid command id")
		return
	var age := Time.get_unix_time_from_system() - float(parts[1]) / 1000000000.0
	if age < -5.0 or age > COMMAND_MAX_AGE_SECONDS:
		_reject_command(command_id, "command expired")
		return
	if command_id == _last_command_id:
		_reject_command(command_id, "duplicate command")
		return
	if action == "spawn_boss":
		action = "spawn_punisher" # Prior Toolkit CLI compatibility.
	if action not in OPTIONS:
		_reject_command(command_id, "unsupported action")
		return
	var scene := get_tree().current_scene
	var player := get_tree().get_first_node_in_group("Player")
	if not _has_player or scene == null or player == null or not scene.is_ancestor_of(player):
		_reject_command(command_id, "not in a playable map")
		return
	# Consume first: a crash or relaunch cannot repeat the same queued action.
	if not _remove_command():
		return
	_last_command_id = command_id
	var message := _try_summon(action, scene, Time.get_ticks_msec())
	var accepted := message.ends_with(" QUEUED")
	_write_result(command_id, "accepted" if accepted else "rejected", message)
	_show_mode_hint(message)


func _reject_command(command_id: String, reason: String) -> void:
	if _remove_command():
		_write_result(command_id, "rejected", reason)


func _remove_command() -> bool:
	return DirAccess.remove_absolute(ProjectSettings.globalize_path(COMMAND_PATH)) == OK


func _write_result(command_id: String, status: String, message: String) -> void:
	var config := ConfigFile.new()
	config.set_value("result", "id", command_id)
	config.set_value("result", "status", status)
	config.set_value("result", "message", message)
	if config.save(RESULT_PATH) != OK:
		push_warning("[RTV_RADAR_SUMMONS] Could not save command result")
