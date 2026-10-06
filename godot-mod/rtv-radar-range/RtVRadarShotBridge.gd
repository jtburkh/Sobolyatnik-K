extends "res://RtVRadarLite.gd"

# Opt-in Build 2 experiment. Observe already-created 3D sound instances rather
# than rewriting AI.gd or polling ammo; never infer a shot from audio volume or
# location alone (explosions can use identical 50/400 attenuation settings).
const AUDIO_SCRIPT := "res://Scripts/AudioInstance3D.gd"
const FIRE_EVENTS := ["fireSemi", "fireAuto", "fireSuppressed"]
const MUZZLE_TOLERANCE_METRES := 0.75
const MODE_HINT_LIFETIME_MS := 1800

var _mode_hint: Label
var _range_hint: Label
var _mode_hint_until_ms := 0
var _f9_down := false


func _ready() -> void:
	super._ready()
	if is_instance_valid(_radar) and is_instance_valid(_canvas):
		_range_hint = Label.new()
		_range_hint.name = "RtVRadarRangeControlHint"
		_range_hint.mouse_filter = Control.MOUSE_FILTER_IGNORE
		_range_hint.focus_mode = Control.FOCUS_NONE
		_range_hint.anchor_left = 1.0
		_range_hint.anchor_right = 1.0
		_range_hint.offset_left = -248.0
		_range_hint.offset_right = -18.0
		_range_hint.offset_top = 252.0
		_range_hint.offset_bottom = 278.0
		_range_hint.horizontal_alignment = HORIZONTAL_ALIGNMENT_RIGHT
		_range_hint.add_theme_font_size_override("font_size", 13)
		_range_hint.add_theme_color_override("font_color", Color(0.69, 0.86, 0.89))
		_range_hint.visible = false
		_canvas.add_child(_range_hint)
		_mode_hint = Label.new()
		_mode_hint.name = "RtVRadarModeHint"
		_mode_hint.mouse_filter = Control.MOUSE_FILTER_IGNORE
		_mode_hint.focus_mode = Control.FOCUS_NONE
		_mode_hint.anchor_left = 1.0
		_mode_hint.anchor_right = 1.0
		_mode_hint.offset_left = -248.0
		_mode_hint.offset_right = -18.0
		_mode_hint.offset_top = 280.0
		_mode_hint.offset_bottom = 306.0
		_mode_hint.horizontal_alignment = HORIZONTAL_ALIGNMENT_RIGHT
		_mode_hint.add_theme_font_size_override("font_size", 12)
		_mode_hint.add_theme_color_override("font_color", Color(0.69, 0.86, 0.89))
		_mode_hint.visible = false
		_canvas.add_child(_mode_hint)
	get_tree().node_added.connect(_on_node_added)


func _process(delta: float) -> void:
	super._process(delta)
	_poll_range_key(Input.is_key_pressed(KEY_F9))
	_update_range_hint()
	_update_mode_hint(Time.get_ticks_msec())


func _poll_radar_keys(f7: bool, f8: bool) -> void:
	var previous_mode := int(_radar.get("_layer_mode")) if is_instance_valid(_radar) else -1
	super._poll_radar_keys(f7, f8)
	if is_instance_valid(_radar) and not _radar.visible and is_instance_valid(_mode_hint):
		_mode_hint.visible = false
	_update_range_hint()
	if previous_mode >= 0 and is_instance_valid(_radar) and int(_radar.get("_layer_mode")) != previous_mode and _radar.visible:
		_show_mode_hint("RADAR: %s" % str(_radar.call("get_mode_name")))


func _poll_range_key(pressed: bool) -> void:
	# F6 is used by the game's RagdollDebug.gd. F9 is unused in reviewed Build 2 scripts.
	# Poll without consuming input; F7/F8 semantics and hidden state remain untouched.
	if _has_player and _controls_enabled and is_instance_valid(_radar) and pressed and not _f9_down:
		_radar.call("cycle_range")
		if _radar.visible:
			_show_mode_hint("RADAR RANGE: %dm" % int(_radar.call("get_range_metres")))
	_f9_down = pressed
	_update_range_hint()


func _update_range_hint() -> void:
	if not is_instance_valid(_range_hint):
		return
	_range_hint.visible = _has_player and _controls_enabled and is_instance_valid(_radar) and _radar.visible
	if _range_hint.visible:
		_range_hint.text = "F9 TOGGLE DISTANCE  %dm" % int(_radar.call("get_range_metres"))


func _show_mode_hint(text: String) -> void:
	if is_instance_valid(_mode_hint):
		_mode_hint.text = text
		_mode_hint.modulate.a = 1.0
		_mode_hint.visible = true
		_mode_hint_until_ms = Time.get_ticks_msec() + MODE_HINT_LIFETIME_MS


func _update_mode_hint(now_ms: int) -> void:
	if not is_instance_valid(_mode_hint) or not _mode_hint.visible:
		return
	_mode_hint.modulate.a = clampf(float(_mode_hint_until_ms - now_ms) / float(MODE_HINT_LIFETIME_MS), 0.0, 1.0)
	if _mode_hint.modulate.a <= 0.0:
		_mode_hint.visible = false


func _exit_tree() -> void:
	if get_tree().node_added.is_connected(_on_node_added):
		get_tree().node_added.disconnect(_on_node_added)
	super._exit_tree()


func _on_node_added(node: Node) -> void:
	if not node is AudioStreamPlayer3D or node.get_parent() != get_tree().root:
		return
	var script: Variant = node.get_script()
	if not script is Script or (script as Script).resource_path != AUDIO_SCRIPT:
		return
	# AI.PlayFire sets the stream/position only *after* adding the audio node.
	# Deferred inspection sees the completed, original game-owned instance.
	call_deferred("_observe_fire_audio", node)


func _observe_fire_audio(node: Node) -> void:
	if not is_instance_valid(node) or node.is_queued_for_deletion() or not node.is_inside_tree():
		return
	var sound := node as AudioStreamPlayer3D
	if sound.get_parent() != get_tree().root or sound.stream == null:
		return
	if not is_equal_approx(sound.unit_size, 50.0) or not is_equal_approx(sound.max_distance, 400.0):
		return
	var scene := get_tree().current_scene
	var ai_root := get_node_or_null(AI_ROOT)
	if scene == null or not is_instance_valid(scene) or ai_root == null or not scene.is_ancestor_of(ai_root):
		return
	for group_name in ["Enemies", "Nomads"]:
		var group := ai_root.get_node_or_null(NodePath(group_name))
		if group == null:
			continue
		for agent in group.get_children():
			if not agent is Node3D or not is_instance_valid(agent) or agent.is_queued_for_deletion():
				continue
			if not bool(agent.get("active")) or bool(agent.get("dead")):
				continue
			var muzzle: Variant = agent.get("muzzle")
			var weapon_data: Variant = agent.get("weaponData")
			if not muzzle is Node3D or not is_instance_valid(muzzle) or not weapon_data is Resource:
				continue
			if not scene.is_ancestor_of(muzzle) or sound.global_position.distance_to(muzzle.global_position) > MUZZLE_TOLERANCE_METRES:
				continue
			if _matches_fire_stream(weapon_data, sound.stream):
				_send_ai_shot(agent.get_instance_id(), sound.global_position)
				return


func _matches_fire_stream(weapon_data: Resource, stream: AudioStream) -> bool:
	for event_name in FIRE_EVENTS:
		var event: Variant = weapon_data.get(event_name)
		if event is Resource and is_instance_valid(event):
			var clips: Variant = event.get("audioClips")
			if clips is Array and clips.has(stream):
				return true
	return false


func _send_ai_shot(shooter_id: int, position: Vector3) -> void:
	var scene := get_tree().current_scene
	if scene == null:
		return
	var coordinates := [position.x, position.y, position.z]
	var packet := {
		"version": 1,
		"type": "gunshot",
		"timestamp_ms": Time.get_ticks_msec(),
		"map_id": scene.scene_file_path if not scene.scene_file_path.is_empty() else scene.name,
		"shooter_id": shooter_id,
		"position": coordinates,
	}
	if is_instance_valid(_radar):
		_radar.call("add_gunshot", coordinates)
	_udp.put_packet(JSON.stringify(packet).to_utf8_buffer())
