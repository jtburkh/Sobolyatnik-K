extends Control

# Draw-only radar: no input callbacks, buttons, actions, menus, or game reads.
# Input passes through this panel to the game's inventory and controls.
const RANGE_METRES := 100.0
const RADIUS_PIXELS := 82.0
const PANEL_SIZE := Vector2(230, 230)
const PANEL_CENTER := Vector2(115, 131)
const BACKGROUND := Color(0.018, 0.035, 0.055, 0.88)
const BORDER := Color(0.19, 0.48, 0.58, 0.92)
const GRID := Color(0.16, 0.32, 0.38, 0.72)
const TEXT := Color(0.69, 0.86, 0.89, 1.0)
const MUTED := Color(0.38, 0.58, 0.62, 1.0)
const PLAYER := Color(0.20, 0.92, 1.0, 1.0)
const NOMAD := Color(0.48, 0.81, 0.96, 1.0)
const LOOT := Color(0.28, 0.92, 0.53, 1.0)
const LOCKED_LOOT := Color(1.0, 0.67, 0.20, 1.0)
const CORPSE_LOOT := Color(0.52, 0.72, 0.76, 1.0)
const TRAIL_LIFETIME_MS := 15000
const TRAIL_SAMPLE_MS := 400
const TRAIL_TELEPORT_METRES := 30.0

var _player: Dictionary = {}
var _contacts: Array = []
var _loot: Array = []
var _controls_enabled := false
var _loot_enabled := false
var _layer_mode := 0 # F7: all, AI only, trails only, optionally loot only.
var _map_id := ""
var _map_name := "UNKNOWN"
var _tracks: Dictionary = {}


func _ready() -> void:
	mouse_filter = Control.MOUSE_FILTER_IGNORE
	focus_mode = Control.FOCUS_NONE
	anchor_left = 1.0
	anchor_right = 1.0
	anchor_top = 0.0
	anchor_bottom = 0.0
	offset_left = -248.0
	offset_right = -18.0
	offset_top = 18.0
	offset_bottom = 248.0


func set_controls_enabled(enabled: bool) -> void:
	_controls_enabled = enabled


func set_loot_enabled(enabled: bool) -> void:
	_loot_enabled = enabled


func cycle_layers() -> void:
	if not _controls_enabled:
		return
	_layer_mode = (_layer_mode + 1) % (4 if _loot_enabled else 3)
	queue_redraw()


func update_snapshot(snapshot: Dictionary) -> void:
	var map: Dictionary = snapshot.get("map", {})
	var map_id := str(map.get("id", ""))
	_map_name = str(map.get("name", "UNKNOWN")).to_upper()
	if map_id != _map_id:
		_tracks.clear()
		_map_id = map_id
	_player = snapshot.get("player", {})
	_contacts = snapshot.get("ai", [])
	_loot = snapshot.get("loot", []) if _loot_enabled else []
	if _controls_enabled:
		_record_tracks()
	queue_redraw()


func _draw() -> void:
	draw_rect(Rect2(Vector2.ZERO, PANEL_SIZE), BACKGROUND, true)
	draw_rect(Rect2(Vector2(0.5, 0.5), PANEL_SIZE - Vector2.ONE), BORDER, false, 1.0)
	for fraction in [0.333, 0.666, 1.0]:
		draw_arc(PANEL_CENTER, RADIUS_PIXELS * fraction, 0.0, TAU, 64, GRID, 1.0, true)
	draw_line(PANEL_CENTER + Vector2(-RADIUS_PIXELS, 0), PANEL_CENTER + Vector2(RADIUS_PIXELS, 0), GRID, 1.0)
	draw_line(PANEL_CENTER + Vector2(0, -RADIUS_PIXELS), PANEL_CENTER + Vector2(0, RADIUS_PIXELS), GRID, 1.0)
	var player_position: Array = _player.get("position", [])
	if player_position.size() != 3:
		return
	var heading := deg_to_rad(float(_player.get("heading", 0.0)))
	if _controls_enabled and _layer_mode in [0, 2]:
		_draw_tracks(player_position, heading)
	if _loot_enabled and _layer_mode in [0, 3]:
		_draw_loot(player_position, heading)
	if not _controls_enabled or _layer_mode in [0, 1]:
		for contact in _contacts:
			if not contact is Dictionary or not bool(contact.get("alive", false)):
				continue
			var position: Array = contact.get("position", [])
			if position.size() != 3:
				continue
			var point: Variant = _project(Vector2(float(position[0]), float(position[2])), player_position, heading)
			if not point is Vector2:
				continue
			var color := _contact_color(contact)
			var tip := Vector2(0.0, -5.0)
			var side := Vector2(4.0, 0.0)
			draw_colored_polygon(PackedVector2Array([point + tip, point + side, point - tip, point - side]), color)
	draw_colored_polygon(PackedVector2Array([
		PANEL_CENTER + Vector2(0.0, -8.0),
		PANEL_CENTER + Vector2(6.0, 7.0),
		PANEL_CENTER,
		PANEL_CENTER + Vector2(-6.0, 7.0),
	]), PLAYER)
	var font := ThemeDB.fallback_font
	if font != null:
		draw_string(font, Vector2(10, 18), "SOBOLYATNIK-K", HORIZONTAL_ALIGNMENT_LEFT, -1, 13, TEXT)
		draw_string(font, Vector2(PANEL_SIZE.x - 67, 18), "1L108K", HORIZONTAL_ALIGNMENT_LEFT, -1, 10, MUTED)
		draw_string(font, Vector2(10, 35), "MAP  %s" % _map_name, HORIZONTAL_ALIGNMENT_LEFT, -1, 12, PLAYER)
		draw_string(font, Vector2(PANEL_SIZE.x - 45, 35), "%dm" % int(RANGE_METRES), HORIZONTAL_ALIGNMENT_LEFT, -1, 12, MUTED)
		if _controls_enabled:
			var modes := ["AI+TRAILS+LOOT", "AI ONLY", "TRAILS ONLY", "LOOT ONLY"] if _loot_enabled else ["AI + TRAILS", "AI ONLY", "TRAILS ONLY"]
			var mode: String = modes[_layer_mode]
			draw_string(font, Vector2(8, PANEL_SIZE.y - 7), "F7 %s  F8 HIDE" % mode, HORIZONTAL_ALIGNMENT_LEFT, -1, 9, MUTED)



func _contact_color(contact: Dictionary) -> Color:
	var faction := str(contact.get("faction", "Unknown"))
	if faction == "Nomad":
		return NOMAD
	if faction == "Boss":
		return Color(1.0, 0.20, 0.72)
	return Color(0.95, 0.3, 0.24)


func _project(world_xz: Vector2, player_position: Array, heading: float) -> Variant:
	var east := world_xz.x - float(player_position[0])
	var south := world_xz.y - float(player_position[2])
	if Vector2(east, south).length() > RANGE_METRES:
		return null
	var screen_right := east * cos(heading) + south * sin(heading)
	var screen_forward := east * sin(heading) - south * cos(heading)
	return PANEL_CENTER + Vector2(screen_right, -screen_forward) * (RADIUS_PIXELS / RANGE_METRES)


func _draw_loot(player_position: Array, heading: float) -> void:
	for container in _loot:
		if not container is Dictionary:
			continue
		var position: Array = container.get("position", [])
		if position.size() != 3:
			continue
		var point: Variant = _project(Vector2(float(position[0]), float(position[2])), player_position, heading)
		if not point is Vector2:
			continue
		var color := LOOT
		if bool(container.get("corpse", false)):
			color = CORPSE_LOOT
		elif bool(container.get("locked", false)):
			color = LOCKED_LOOT
		draw_rect(Rect2(point - Vector2(2.5, 2.5), Vector2(5, 5)), color, false, 1.2)


func _record_tracks() -> void:
	var now := Time.get_ticks_msec()
	for contact in _contacts:
		if not contact is Dictionary or not bool(contact.get("alive", false)):
			continue
		var position: Array = contact.get("position", [])
		if position.size() != 3:
			continue
		var id := str(contact.get("id", ""))
		if id.is_empty():
			continue
		var world_xz := Vector2(float(position[0]), float(position[2]))
		var samples: Array = _tracks.get(id, [])
		if not samples.is_empty():
			var last: Dictionary = samples.back()
			var moved := world_xz.distance_to(last["position"])
			if moved > TRAIL_TELEPORT_METRES:
				samples.clear()
			elif now - int(last["time_ms"]) < TRAIL_SAMPLE_MS or moved < 0.35:
				continue
		samples.append({"position": world_xz, "time_ms": now, "color": _contact_color(contact)})
		while not samples.is_empty() and now - int(samples.front()["time_ms"]) > TRAIL_LIFETIME_MS:
			samples.pop_front()
		_tracks[id] = samples
	for id in _tracks.keys():
		var samples: Array = _tracks[id]
		if samples.is_empty() or now - int(samples.back()["time_ms"]) > TRAIL_LIFETIME_MS:
			_tracks.erase(id)


func _draw_tracks(player_position: Array, heading: float) -> void:
	var now := Time.get_ticks_msec()
	for samples in _tracks.values():
		for sample in samples:
			var age: int = now - int(sample["time_ms"])
			if age > TRAIL_LIFETIME_MS:
				continue
			var point: Variant = _project(sample["position"], player_position, heading)
			if point is Vector2:
				var color: Color = sample["color"]
				color.a = 0.6 * (1.0 - float(age) / float(TRAIL_LIFETIME_MS))
				draw_circle(point, 2.0, color)
