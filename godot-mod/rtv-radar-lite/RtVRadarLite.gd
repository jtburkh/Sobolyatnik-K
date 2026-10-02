extends Node

# Minimal external UDP radar bridge for Build 2 isolation. No input handlers,
# UI, game-state writes, summon commands, AI hooks, or sensor/loot reads.
const PREFIX := "[RTV_RADAR_LITE]"
const CONFIG_PATH := "user://rtv-telemetry.cfg"
const GAME_DATA_PATH := "res://Resources/GameData.tres"
const AI_ROOT := "/root/Map/AI"
const LOOT_CONTAINER_SCRIPT := "res://Scripts/LootContainer.gd"
const LOOT_SCAN_INTERVAL := 1.0
const LOOT_MAX_DISTANCE := 425.0
const LOOT_MAX_MARKERS := 128
const FACTIONS := ["Nomad", "Bandit", "Guard", "Military", "Boss"]
const SAMPLE_INTERVAL := 0.2
const CONTACTS_DEFAULT := false # Packager sets true only in separate opt-in contacts VMZ.
const OVERLAY_DEFAULT := false # Only the separately packaged in-game VMZ enables this.
const CONTROLS_DEFAULT := false # Only the separately packaged controls VMZ enables this.
const LOOT_DEFAULT := false # Only the separately packaged loot VMZ enables this.
const CONTACT_TRACE_LIMIT := 1024 * 1024

var _game_data: Resource
var _udp := PacketPeerUDP.new()
var _elapsed := 0.0
var _include_contacts := false
var _overlay_enabled := false
var _controls_enabled := false
var _include_loot := false
var _loot_scan_elapsed := 0.0
var _loot_scan_initialized := false
var _loot_cache: Array[Dictionary] = []
var _hud_visible := true
var _has_player := false
var _f7_down := false
var _f8_down := false
var _canvas: CanvasLayer
var _radar: Control
var _host := "127.0.0.1"
var _port := 47777
var _trace_file: FileAccess
var _trace_path := ""
var _trace_bytes := 0
var _trace_sequence := 0


func _ready() -> void:
	var config := ConfigFile.new()
	if FileAccess.file_exists(CONFIG_PATH) and config.load(CONFIG_PATH) != OK:
		push_warning("%s Cannot read config; using safe defaults" % PREFIX)
	_host = str(config.get_value("network", "host", _host))
	_port = clampi(int(config.get_value("network", "port", _port)), 1, 65535)
	_include_contacts = bool(config.get_value("radar_lite", "contacts", CONTACTS_DEFAULT))
	_overlay_enabled = bool(config.get_value("radar_lite", "overlay", OVERLAY_DEFAULT))
	_controls_enabled = bool(config.get_value("radar_lite", "controls", CONTROLS_DEFAULT)) and _overlay_enabled
	_include_loot = bool(config.get_value("radar_lite", "loot", LOOT_DEFAULT))
	if _include_contacts:
		var utc := Time.get_datetime_string_from_system(true, true).replace(":", "-").replace("T", "-")
		_trace_path = "user://rtv-radar-lite-trace-%s-%d.log" % [utc, Time.get_ticks_usec()]
		_trace_file = FileAccess.open(_trace_path, FileAccess.WRITE)
		if _trace_file == null:
			push_warning("%s Contact trace unavailable: %s" % [PREFIX, error_string(FileAccess.get_open_error())])
		else:
			print("%s contact trace: %s (limit %d bytes)" % [PREFIX, _trace_path, CONTACT_TRACE_LIMIT])
			_trace("ready.contacts")
	_game_data = load(GAME_DATA_PATH) as Resource
	if _game_data == null:
		push_error("%s GameData unavailable; not sending" % PREFIX)
		set_process(false)
		return
	_udp.set_dest_address(_host, _port)
	if _overlay_enabled:
		_mount_overlay()
	print("%s loaded; UDP -> %s:%d; AI contacts=%s; in-game overlay=%s; F7/F8 controls=%s; loot=%s" % [PREFIX, _host, _port, _include_contacts, _radar != null, _controls_enabled and _radar != null, _include_loot])


func _process(delta: float) -> void:
	if _controls_enabled and is_instance_valid(_radar):
		_poll_radar_keys(Input.is_key_pressed(KEY_F7), Input.is_key_pressed(KEY_F8))
	_elapsed += delta
	if _elapsed < SAMPLE_INTERVAL:
		return
	var sample_delta := _elapsed
	_elapsed = 0.0
	if not is_instance_valid(_game_data):
		return
	var player := get_tree().get_first_node_in_group("Player")
	if player == null:
		_has_player = false
		if is_instance_valid(_radar):
			_radar.visible = false
		return
	var position: Variant = _game_data.get("playerPosition")
	var forward: Variant = _game_data.get("playerVector")
	if not position is Vector3 or not forward is Vector3:
		_has_player = false
		if is_instance_valid(_radar):
			_radar.visible = false
		return
	_has_player = true
	var scene := get_tree().current_scene
	var map_identity := {}
	if scene != null:
		var scene_path := scene.scene_file_path
		map_identity = {
			"id": scene_path if not scene_path.is_empty() else str(scene.name),
			"name": scene_path.get_file().get_basename().capitalize() if not scene_path.is_empty() else str(scene.name),
		}
	var contacts: Array[Dictionary] = []
	if _include_contacts:
		_trace("contacts.enter")
		contacts = _collect_contacts()
		_trace("contacts.done count=%d" % contacts.size())
	var loot: Array[Dictionary] = []
	if _include_loot:
		_loot_scan_elapsed += sample_delta
		if not _loot_scan_initialized or _loot_scan_elapsed >= LOOT_SCAN_INTERVAL:
			_loot_scan_elapsed = 0.0
			_loot_scan_initialized = true
			_trace("loot.scan.enter")
			_loot_cache = _scan_loot_nodes()
			_trace("loot.scan.done count=%d" % _loot_cache.size())
		_trace("loot.read.enter")
		loot = _collect_loot(position)
		_trace("loot.read.done count=%d" % loot.size())
	var packet := {
		"version": 1,
		"type": "snapshot",
		"timestamp_ms": Time.get_ticks_msec(),
		"map": map_identity,
		"player": {
			"id": player.get_instance_id(),
			"position": [position.x, position.y, position.z],
			"heading": fposmod(rad_to_deg(atan2(forward.x, -forward.z)), 360.0),
		},
		"ai": contacts,
		"loot": loot,
	}
	if is_instance_valid(_radar):
		_radar.visible = _hud_visible
		_radar.call("update_snapshot", packet)
	if _include_contacts:
		_trace("udp.enter")
	_udp.put_packet(JSON.stringify(packet).to_utf8_buffer())
	if _include_contacts:
		_trace("udp.done")


func _poll_radar_keys(f7: bool, f8: bool) -> void:
	# Poll only reserved F keys; never consume input or install a _input callback.
	if _has_player and f7 and not _f7_down:
		_radar.call("cycle_layers")
	if _has_player and f8 and not _f8_down:
		_hud_visible = not _hud_visible
		_radar.visible = _hud_visible
	_f7_down = f7
	_f8_down = f8


func _mount_overlay() -> void:
	var script: Variant = load("res://RtVRadarLiteOverlay.gd")
	if not script is Script:
		push_warning("%s Radar overlay unavailable; continuing UDP-only" % PREFIX)
		return
	_canvas = CanvasLayer.new()
	_canvas.name = "RtVRadarLiteCanvas"
	_canvas.layer = 20
	add_child(_canvas)
	_radar = (script as Script).new() as Control
	if _radar == null:
		push_warning("%s Radar overlay must extend Control; continuing UDP-only" % PREFIX)
		_canvas.queue_free()
		_canvas = null
		return
	_radar.name = "RtVRadarLiteOverlay"
	if _controls_enabled:
		_radar.call("set_controls_enabled", true)
	if _include_loot:
		_radar.call("set_loot_enabled", true)
	_radar.visible = false
	_canvas.add_child(_radar)


func _trace(phase: String) -> void:
	if _trace_file == null or _trace_bytes >= CONTACT_TRACE_LIMIT:
		return
	_trace_sequence += 1
	var line := "%d\t%d\t%s\n" % [Time.get_ticks_usec(), _trace_sequence, phase]
	if _trace_bytes + line.to_utf8_buffer().size() > CONTACT_TRACE_LIMIT:
		_trace_file.store_line("trace.limit_reached")
		_trace_file.flush()
		_trace_bytes = CONTACT_TRACE_LIMIT
		return
	_trace_file.store_string(line)
	_trace_file.flush() # Keep the last phase even after an abrupt native exit.
	_trace_bytes += line.to_utf8_buffer().size()


func _exit_tree() -> void:
	if _trace_file != null:
		_trace_file.close()
		_trace_file = null


func _scan_loot_nodes() -> Array[Dictionary]:
	var found: Dictionary = {}
	for proxy in get_tree().get_nodes_in_group("Interactable"):
		if not is_instance_valid(proxy) or proxy.is_queued_for_deletion():
			continue
		var current: Node = proxy
		for depth in 8:
			if current == null:
				break
			var script: Variant = current.get_script()
			if current is Node3D and script is Script and (script as Script).resource_path == LOOT_CONTAINER_SCRIPT:
				var id := current.get_instance_id()
				if not found.has(id):
					found[id] = {"node": current, "proxies": []}
				var shapes: Array = [proxy] if proxy is CollisionShape3D else proxy.find_children("*", "CollisionShape3D", true, false)
				found[id]["proxies"].append({"proxy": proxy, "shapes": shapes})
				break
			current = current.get_parent()
	var nodes: Array[Dictionary] = []
	for entry in found.values():
		nodes.append(entry)
	return nodes


func _loot_proxy_enabled(entry: Dictionary) -> bool:
	for proxy_entry in entry["proxies"]:
		var proxy: Node = proxy_entry["proxy"]
		if not is_instance_valid(proxy) or not proxy.is_inside_tree():
			continue
		var shapes: Array = proxy_entry["shapes"]
		if shapes.is_empty():
			return true
		for shape in shapes:
			if is_instance_valid(shape) and shape is CollisionShape3D and not (shape as CollisionShape3D).disabled:
				return true
	return false


func _collect_loot(player_position: Vector3) -> Array[Dictionary]:
	var result: Array[Dictionary] = []
	for entry in _loot_cache:
		var node: Variant = entry.get("node")
		if not node is Node3D or not is_instance_valid(node) or node.is_queued_for_deletion():
			continue
		var container := node as Node3D
		if not container.is_visible_in_tree() or not _loot_proxy_enabled(entry):
			continue
		var position := container.global_position
		if position.distance_to(player_position) > LOOT_MAX_DISTANCE:
			continue
		var contents: Variant = container.get("storage") if bool(container.get("storaged")) else container.get("loot")
		if not contents is Array or contents.is_empty():
			continue
		result.append({
			"id": container.get_instance_id(),
			"position": [position.x, position.y, position.z],
			"name": str(container.get("containerName")),
			"locked": bool(container.get("locked")),
			"corpse": bool(container.get("corpse")),
		})
		if result.size() >= LOOT_MAX_MARKERS:
			break
	return result


func _collect_contacts() -> Array[Dictionary]:
	var contacts: Array[Dictionary] = []
	var root := get_node_or_null(AI_ROOT)
	if root == null:
		return contacts
	for group_name in ["Enemies", "Nomads"]:
		var group := root.get_node_or_null(NodePath(group_name))
		if group == null:
			continue
		for agent in group.get_children():
			if not (agent is Node3D) or not is_instance_valid(agent) or agent.is_queued_for_deletion():
				continue
			if not bool(agent.get("active")):
				continue
			var variant: Variant = agent.get("variant")
			var faction_index := -1
			if variant is Object and is_instance_valid(variant):
				faction_index = int(variant.get("faction"))
			var faction: String = FACTIONS[faction_index] if faction_index >= 0 and faction_index < FACTIONS.size() else "Unknown"
			var p := (agent as Node3D).global_position
			contacts.append({
				"id": agent.get_instance_id(),
				"position": [p.x, p.y, p.z],
				"alive": not bool(agent.get("dead")),
				"faction": faction,
				"boss": faction == "Boss",
				"friendly": faction == "Nomad" and float(_game_data.get("reputation")) >= 50.0,
			})
	return contacts
