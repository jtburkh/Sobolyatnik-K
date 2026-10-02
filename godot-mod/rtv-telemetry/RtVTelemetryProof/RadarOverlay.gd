extends Control

signal summon_requested(action: String)

const BACKGROUND := Color(0.018, 0.035, 0.055, 0.88)
const MODAL_BACKGROUND := Color(0.012, 0.025, 0.040, 0.97)
const BORDER := Color(0.19, 0.48, 0.58, 0.92)
const GRID := Color(0.16, 0.32, 0.38, 0.72)
const TEXT := Color(0.69, 0.86, 0.89, 1.0)
const MUTED := Color(0.38, 0.58, 0.62, 1.0)
const PLAYER := Color(0.20, 0.92, 1.0, 1.0)
const CONTACT := Color(1.0, 0.64, 0.18, 1.0)
const BOSS := Color(1.0, 0.20, 0.72, 1.0)
const NOMAD := Color(0.48, 0.81, 0.96, 1.0)
const SHOT := Color(1.0, 0.28, 0.18, 1.0)
const LOOT := Color(0.28, 0.92, 0.53, 1.0)
const LOCKED_LOOT := Color(1.0, 0.67, 0.20, 1.0)
const CORPSE_LOOT := Color(0.52, 0.72, 0.76, 1.0)
const SELECTED := Color(0.08, 0.30, 0.38, 0.96)
const SHOT_LIFETIME_MS := 5000
const TRAIL_LIFETIME_MS := 15000
const TRAIL_STALE_MS := 2000
const TRAIL_SAMPLE_INTERVAL_MS := 400
const TRAIL_MINIMUM_MOVEMENT := 0.35
const TRAIL_TELEPORT_DISTANCE := 30.0
const LAYER_KEY := KEY_F7
const SUMMON_KEY := KEY_F9

const LAYER_OPTIONS := [
	{"key": "ai", "label": "AI CONTACTS"},
	{"key": "bosses", "label": "BOSS CONTACTS"},
	{"key": "trails", "label": "MOVEMENT TRAILS"},
	{"key": "loot", "label": "LOOTABLE ITEMS"},
	{"key": "gunshots", "label": "GUNSHOTS"},
]
const SUMMON_OPTIONS := [
	{"action": "spawn_airdrop", "label": "CASA AIRDROP"},
	{"action": "spawn_boss", "label": "PUNISHER BOSS"},
]

enum PopupMode { NONE, LAYERS, SUMMON, CONFIRM }

var radar_range := 200.0
var _panel_size := 230.0
var _snapshot: Dictionary = {}
var _shots: Array[Dictionary] = []
var _tracks: Dictionary = {}
var _enabled := true
var _toggle_label := "F8"
var _show_ai := true
var _show_bosses := true
var _show_trails := true
var _show_loot := true
var _show_gunshots := true
var _popup_mode := PopupMode.NONE
var _popup_selected := 0
var _pending_summon := ""
var _pending_label := ""
var _status_message := ""
var _status_success := true
var _status_expires_ms := 0
var _access_unlocked := false
var _diagnostic_mode := false
var _artifact_position: Array = []


func _ready() -> void:
	mouse_filter = Control.MOUSE_FILTER_IGNORE
	process_mode = Node.PROCESS_MODE_ALWAYS
	clip_contents = true
	_apply_layout()
	visible = false


func configure(
	configured_range: float,
	configured_size: float,
	toggle_label: String = "F8",
	layers: Dictionary = {}
) -> void:
	radar_range = clampf(configured_range, 50.0, 400.0)
	_panel_size = clampf(configured_size, 180.0, 320.0)
	_toggle_label = toggle_label
	_show_ai = bool(layers.get("ai", true))
	_show_bosses = bool(layers.get("bosses", true))
	_show_trails = bool(layers.get("trails", true))
	_show_loot = bool(layers.get("loot", true))
	_show_gunshots = bool(layers.get("gunshots", true))
	_apply_layout()
	queue_redraw()


func set_diagnostic_mode(enabled: bool) -> void:
	_diagnostic_mode = enabled
	if enabled:
		_popup_mode = PopupMode.NONE


func set_access_unlocked(unlocked: bool) -> void:
	_access_unlocked = unlocked
	if unlocked:
		_artifact_position = []
	queue_redraw()


func set_artifact_position(position: Array) -> void:
	if position.size() == 3:
		_artifact_position = position.duplicate()
		queue_redraw()


func set_overlay_enabled(enabled: bool) -> void:
	_enabled = enabled
	visible = _enabled and not _snapshot.is_empty()
	if not _enabled:
		_popup_mode = PopupMode.NONE


func update_snapshot(snapshot: Dictionary) -> void:
	_snapshot = snapshot
	_update_tracks(snapshot)
	visible = _enabled
	queue_redraw()


func clear_snapshot() -> void:
	_snapshot = {}
	_tracks.clear()
	_popup_mode = PopupMode.NONE
	visible = false


func add_gunshot(position: Array) -> void:
	if position.size() != 3:
		return
	_shots.append({
		"position": position.duplicate(),
		"expires_ms": Time.get_ticks_msec() + SHOT_LIFETIME_MS,
	})
	queue_redraw()


func show_summon_result(message: String, success: bool) -> void:
	_status_message = message
	_status_success = success
	_status_expires_ms = Time.get_ticks_msec() + 5000
	queue_redraw()


func _input(event: InputEvent) -> void:
	if _diagnostic_mode or not _enabled or _snapshot.is_empty() or not event is InputEventKey:
		return
	if not event.pressed or event.echo:
		return
	if not _access_unlocked:
		if event.keycode in [LAYER_KEY, SUMMON_KEY]:
			_status_message = "RECOVER CM-7 TO UNLOCK"
			_status_success = false
			_status_expires_ms = Time.get_ticks_msec() + 3000
			get_viewport().set_input_as_handled()
			queue_redraw()
		return
	var handled := false
	if event.keycode == LAYER_KEY:
		_popup_mode = PopupMode.NONE if _popup_mode == PopupMode.LAYERS else PopupMode.LAYERS
		_popup_selected = 0
		handled = true
	elif event.keycode == SUMMON_KEY:
		_popup_mode = PopupMode.NONE if _popup_mode == PopupMode.SUMMON else PopupMode.SUMMON
		_popup_selected = 0
		handled = true
	elif _popup_mode != PopupMode.NONE:
		handled = _handle_popup_key(event.keycode)
	if handled:
		get_viewport().set_input_as_handled()
		queue_redraw()


func _handle_popup_key(keycode: Key) -> bool:
	if keycode == KEY_ESCAPE:
		_popup_mode = PopupMode.NONE
		return true
	if _popup_mode == PopupMode.LAYERS:
		if keycode == KEY_UP:
			_popup_selected = wrapi(_popup_selected - 1, 0, LAYER_OPTIONS.size())
			return true
		if keycode == KEY_DOWN:
			_popup_selected = wrapi(_popup_selected + 1, 0, LAYER_OPTIONS.size())
			return true
		if keycode in [KEY_ENTER, KEY_SPACE]:
			_toggle_layer(str(LAYER_OPTIONS[_popup_selected]["key"]))
			return true
	elif _popup_mode == PopupMode.SUMMON:
		if keycode == KEY_UP:
			_popup_selected = wrapi(_popup_selected - 1, 0, SUMMON_OPTIONS.size())
			return true
		if keycode == KEY_DOWN:
			_popup_selected = wrapi(_popup_selected + 1, 0, SUMMON_OPTIONS.size())
			return true
		if keycode == KEY_ENTER:
			_pending_summon = str(SUMMON_OPTIONS[_popup_selected]["action"])
			_pending_label = str(SUMMON_OPTIONS[_popup_selected]["label"])
			_popup_mode = PopupMode.CONFIRM
			return true
	elif _popup_mode == PopupMode.CONFIRM:
		if keycode in [KEY_ENTER, KEY_Y]:
			var action := _pending_summon
			_popup_mode = PopupMode.NONE
			_status_message = "REQUESTING %s..." % _pending_label
			_status_success = true
			_status_expires_ms = Time.get_ticks_msec() + 5000
			summon_requested.emit(action)
			return true
		if keycode in [KEY_N, KEY_BACKSPACE]:
			_popup_mode = PopupMode.SUMMON
			return true
	return false


func _toggle_layer(key: String) -> void:
	match key:
		"ai": _show_ai = not _show_ai
		"bosses": _show_bosses = not _show_bosses
		"trails": _show_trails = not _show_trails
		"loot": _show_loot = not _show_loot
		"gunshots": _show_gunshots = not _show_gunshots


func _layer_enabled(key: String) -> bool:
	match key:
		"ai": return _show_ai
		"bosses": return _show_bosses
		"trails": return _show_trails
		"loot": return _show_loot
		"gunshots": return _show_gunshots
	return false


func _process(_delta: float) -> void:
	var now := Time.get_ticks_msec()
	var previous_count := _shots.size()
	_shots = _shots.filter(func(shot: Dictionary) -> bool: return int(shot["expires_ms"]) > now)
	if _status_expires_ms != 0 and now >= _status_expires_ms:
		_status_expires_ms = 0
		_status_message = ""
		queue_redraw()
	if _shots.size() != previous_count:
		queue_redraw()


func _update_tracks(snapshot: Dictionary) -> void:
	var now := Time.get_ticks_msec()
	for ai_value in snapshot.get("ai", []):
		if not ai_value is Dictionary:
			continue
		var ai: Dictionary = ai_value
		var position: Variant = ai.get("position", [])
		if not position is Array or position.size() != 3:
			continue
		var id := int(ai.get("id", 0))
		var track: Dictionary = _tracks.get(id, {"points": [], "last_seen_ms": now})
		var points: Array = track["points"]
		if points.is_empty():
			points.append({"position": position.duplicate(), "time_ms": now})
		else:
			var previous: Array = points[-1]["position"]
			var movement := _horizontal_distance(previous, position)
			if movement > TRAIL_TELEPORT_DISTANCE:
				points.clear()
				points.append({"position": position.duplicate(), "time_ms": now})
			elif now - int(points[-1]["time_ms"]) >= TRAIL_SAMPLE_INTERVAL_MS and movement >= TRAIL_MINIMUM_MOVEMENT:
				points.append({"position": position.duplicate(), "time_ms": now})
		points = points.filter(func(point: Dictionary) -> bool: return now - int(point["time_ms"]) <= TRAIL_LIFETIME_MS)
		track["points"] = points
		track["last_seen_ms"] = now
		_tracks[id] = track
	for id in _tracks.keys():
		if now - int(_tracks[id]["last_seen_ms"]) > TRAIL_STALE_MS:
			_tracks.erase(id)


func _horizontal_distance(left: Array, right: Array) -> float:
	return Vector2(float(right[0]) - float(left[0]), float(right[2]) - float(left[2])).length()


func _apply_layout() -> void:
	anchor_left = 1.0
	anchor_right = 1.0
	anchor_top = 0.0
	anchor_bottom = 0.0
	offset_left = -_panel_size - 18.0
	offset_right = -18.0
	offset_top = 18.0
	offset_bottom = 18.0 + _panel_size


func _draw() -> void:
	if _snapshot.is_empty():
		return
	draw_rect(Rect2(Vector2.ZERO, size), BACKGROUND, true)
	draw_rect(Rect2(Vector2(0.5, 0.5), size - Vector2.ONE), BORDER, false, 1.0)

	var map: Dictionary = _snapshot.get("map", {})
	var map_name := str(map.get("name", "Unknown"))
	var font := ThemeDB.fallback_font
	draw_string(font, Vector2(10.0, 18.0), "TACTICAL RADAR", HORIZONTAL_ALIGNMENT_LEFT, -1.0, 13, TEXT)
	draw_string(font, Vector2(10.0, 35.0), "MAP  %s" % map_name.to_upper(), HORIZONTAL_ALIGNMENT_LEFT, -1.0, 12, PLAYER)
	draw_string(font, Vector2(size.x - 72.0, 18.0), "%dm" % int(radar_range), HORIZONTAL_ALIGNMENT_LEFT, -1.0, 12, MUTED)

	var center := Vector2(size.x * 0.5, 45.0 + (size.y - 58.0) * 0.5)
	var radius := minf(size.x * 0.5 - 12.0, (size.y - 58.0) * 0.5 - 4.0)
	if not _access_unlocked and not _diagnostic_mode:
		_draw_locked_interface(font, center, radius)
		return
	for fraction in [0.333, 0.666, 1.0]:
		draw_arc(center, radius * fraction, 0.0, TAU, 64, GRID, 1.0, true)
	draw_line(center - Vector2(radius, 0.0), center + Vector2(radius, 0.0), GRID, 1.0)
	draw_line(center - Vector2(0.0, radius), center + Vector2(0.0, radius), GRID, 1.0)

	var player: Dictionary = _snapshot.get("player", {})
	var player_position: Array = player.get("position", [])
	if player_position.size() != 3:
		return
	var heading := float(player.get("heading", 0.0))
	if _show_trails:
		_draw_trails(player_position, heading, center, radius)
	if _show_loot:
		_draw_loot(player_position, heading, center, radius)
	_draw_ai(player_position, heading, center, radius, font)
	if _show_gunshots:
		_draw_shots(player_position, heading, center, radius)

	draw_colored_polygon(PackedVector2Array([
		center + Vector2(0.0, -8.0),
		center + Vector2(6.0, 7.0),
		center,
		center + Vector2(-6.0, 7.0),
	]), PLAYER)
	_draw_footer(font)
	if _popup_mode != PopupMode.NONE:
		_draw_popup(font)
	elif not _status_message.is_empty():
		_draw_status(font)


func _draw_locked_interface(font: Font, center: Vector2, radius: float) -> void:
	for fraction in [0.333, 0.666, 1.0]:
		draw_arc(center, radius * fraction, 0.0, TAU, 64, Color(GRID, 0.36), 1.0, true)
	var player: Dictionary = _snapshot.get("player", {})
	var player_position: Array = player.get("position", [])
	var heading := float(player.get("heading", 0.0))
	if player_position.size() == 3 and _artifact_position.size() == 3:
		var point := _project_contact(player_position, heading, _artifact_position, center, radius)
		if point != Vector2.INF:
			var pulse := 5.0 + sin(Time.get_ticks_msec() / 180.0) * 1.5
			draw_circle(point, pulse, BOSS, false, 2.0)
			draw_string(font, point + Vector2(-3.5, 4.0), "?", HORIZONTAL_ALIGNMENT_LEFT, -1.0, 11, BOSS)
		var distance := _horizontal_distance(player_position, _artifact_position)
		draw_string(font, Vector2(13.0, 61.0), "UNIDENTIFIED SIGNAL  %.0fm" % distance, HORIZONTAL_ALIGNMENT_LEFT, -1.0, 11, BOSS)
	else:
		draw_string(font, Vector2(13.0, 61.0), "SEARCHING FOR CM-7 SIGNAL", HORIZONTAL_ALIGNMENT_LEFT, -1.0, 11, MUTED)
	draw_string(font, Vector2(13.0, 80.0), "RECOVER DEVICE TO UNLOCK", HORIZONTAL_ALIGNMENT_LEFT, -1.0, 10, TEXT)
	draw_colored_polygon(PackedVector2Array([
		center + Vector2(0.0, -8.0),
		center + Vector2(6.0, 7.0),
		center,
		center + Vector2(-6.0, 7.0),
	]), PLAYER)
	draw_string(font, Vector2(8.0, size.y - 7.0), "%s HIDE" % _toggle_label, HORIZONTAL_ALIGNMENT_LEFT, -1.0, 9, MUTED)
	if not _status_message.is_empty():
		_draw_status(font)


func _draw_trails(player_position: Array, heading: float, center: Vector2, radius: float) -> void:
	for track in _tracks.values():
		for trail_point in track["points"]:
			var point := _project_contact(player_position, heading, trail_point["position"], center, radius)
			if point != Vector2.INF:
				draw_circle(point, 1.5, Color(CONTACT, 0.48))


func _draw_loot(player_position: Array, heading: float, center: Vector2, radius: float) -> void:
	for loot_value in _snapshot.get("loot", []):
		if not loot_value is Dictionary:
			continue
		var loot: Dictionary = loot_value
		var point := _project_contact(player_position, heading, loot.get("position", []), center, radius)
		if point == Vector2.INF:
			continue
		if bool(loot.get("corpse", false)):
			draw_circle(point, 3.0, CORPSE_LOOT, false, 1.2)
		elif bool(loot.get("locked", false)):
			draw_rect(Rect2(point - Vector2(2.5, 2.5), Vector2(5.0, 5.0)), LOCKED_LOOT, true)
		else:
			draw_rect(Rect2(point - Vector2(2.5, 2.5), Vector2(5.0, 5.0)), LOOT, false, 1.2)


func _draw_ai(
	player_position: Array,
	heading: float,
	center: Vector2,
	radius: float,
	font: Font
) -> void:
	for ai_value in _snapshot.get("ai", []):
		if not ai_value is Dictionary:
			continue
		var ai: Dictionary = ai_value
		var is_boss := bool(ai.get("boss", false))
		var is_nomad := str(ai.get("faction", "")) == "Nomad"
		if (is_boss and not _show_bosses) or (not is_boss and not _show_ai):
			continue
		var point := _project_contact(player_position, heading, ai.get("position", []), center, radius)
		if point == Vector2.INF:
			continue
		var is_alive := bool(ai.get("alive", true))
		if is_boss:
			draw_circle(point, 7.0, BOSS if is_alive else MUTED)
			draw_string(font, point + Vector2(-4.0, 4.0), "B", HORIZONTAL_ALIGNMENT_LEFT, -1.0, 11, Color.BLACK)
		else:
			var color := (NOMAD if is_nomad else CONTACT) if is_alive else MUTED
			draw_colored_polygon(PackedVector2Array([
				point + Vector2(0.0, -5.0),
				point + Vector2(5.0, 0.0),
				point + Vector2(0.0, 5.0),
				point + Vector2(-5.0, 0.0),
			]), color)


func _draw_shots(player_position: Array, heading: float, center: Vector2, radius: float) -> void:
	for shot in _shots:
		var point := _project_contact(player_position, heading, shot.get("position", []), center, radius)
		if point != Vector2.INF:
			draw_circle(point, 4.0, SHOT, false, 2.0)


func _draw_footer(font: Font) -> void:
	var footer := "DIAGNOSTIC  %s HIDE" % _toggle_label if _diagnostic_mode else "F7 LAYERS  %s HIDE  F9 SUMMON" % _toggle_label
	draw_string(font, Vector2(8.0, size.y - 7.0), footer, HORIZONTAL_ALIGNMENT_LEFT, -1.0, 9, MUTED)


func _draw_popup(font: Font) -> void:
	var area := Rect2(Vector2(8.0, 43.0), Vector2(size.x - 16.0, size.y - 54.0))
	draw_rect(area, MODAL_BACKGROUND, true)
	draw_rect(area, BORDER, false, 1.0)
	if _popup_mode == PopupMode.LAYERS:
		_draw_menu_title(font, "RADAR LAYERS")
		for index in LAYER_OPTIONS.size():
			var option: Dictionary = LAYER_OPTIONS[index]
			var enabled := _layer_enabled(str(option["key"]))
			_draw_menu_row(font, index, "%s  %s" % ["[X]" if enabled else "[ ]", option["label"]])
		draw_string(font, Vector2(16.0, size.y - 22.0), "ENTER/SPACE TOGGLE  ESC CLOSE", HORIZONTAL_ALIGNMENT_LEFT, -1.0, 9, MUTED)
	elif _popup_mode == PopupMode.SUMMON:
		_draw_menu_title(font, "SUMMON")
		for index in SUMMON_OPTIONS.size():
			_draw_menu_row(font, index, str(SUMMON_OPTIONS[index]["label"]))
		draw_string(font, Vector2(16.0, size.y - 22.0), "ENTER SELECT  ESC CLOSE", HORIZONTAL_ALIGNMENT_LEFT, -1.0, 9, MUTED)
	else:
		_draw_menu_title(font, "CONFIRM SUMMON")
		draw_string(font, Vector2(16.0, 91.0), _pending_label, HORIZONTAL_ALIGNMENT_LEFT, -1.0, 13, BOSS)
		draw_string(font, Vector2(16.0, 116.0), "USE NATIVE GAME EVENT?", HORIZONTAL_ALIGNMENT_LEFT, -1.0, 10, TEXT)
		draw_string(font, Vector2(16.0, 139.0), "ENTER/Y CONFIRM", HORIZONTAL_ALIGNMENT_LEFT, -1.0, 10, PLAYER)
		draw_string(font, Vector2(16.0, 157.0), "N/ESC CANCEL", HORIZONTAL_ALIGNMENT_LEFT, -1.0, 10, MUTED)


func _draw_menu_title(font: Font, title: String) -> void:
	draw_string(font, Vector2(16.0, 62.0), title, HORIZONTAL_ALIGNMENT_LEFT, -1.0, 13, PLAYER)


func _draw_menu_row(font: Font, index: int, label: String) -> void:
	var y := 82.0 + index * 21.0
	if index == _popup_selected:
		draw_rect(Rect2(Vector2(13.0, y - 14.0), Vector2(size.x - 26.0, 19.0)), SELECTED, true)
	draw_string(font, Vector2(18.0, y), "%s %s" % [">" if index == _popup_selected else " ", label], HORIZONTAL_ALIGNMENT_LEFT, -1.0, 10, TEXT)


func _draw_status(font: Font) -> void:
	var area := Rect2(Vector2(8.0, size.y - 44.0), Vector2(size.x - 16.0, 30.0))
	draw_rect(area, MODAL_BACKGROUND, true)
	draw_rect(area, PLAYER if _status_success else SHOT, false, 1.0)
	draw_string(font, area.position + Vector2(7.0, 19.0), _status_message, HORIZONTAL_ALIGNMENT_LEFT, area.size.x - 14.0, 9, TEXT)


func _project_contact(
	player_position: Array,
	heading_degrees: float,
	target_position: Variant,
	center: Vector2,
	radius: float
) -> Vector2:
	if not target_position is Array or target_position.size() != 3:
		return Vector2.INF
	var east := float(target_position[0]) - float(player_position[0])
	var north := -(float(target_position[2]) - float(player_position[2]))
	var distance := Vector2(east, north).length()
	if distance > radar_range:
		return Vector2.INF
	var heading := deg_to_rad(heading_degrees)
	var right := east * cos(heading) - north * sin(heading)
	var forward := east * sin(heading) + north * cos(heading)
	return center + Vector2(right, -forward) / radar_range * radius
