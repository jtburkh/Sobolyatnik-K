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
	var agent: Node3D = load("res://Scripts/MockAI.gd").new()
	agent.name = "Enemy"
	agent.variant = load("res://Scripts/MockVariant.gd").new()
	agent.position = Vector3(20, 0, 10)
	enemies.add_child(agent)
	var crate: Node3D = load("res://Scripts/LootContainer.gd").new()
	crate.position = Vector3(6, 0, 0)
	map.add_child(crate)
	var area := Area3D.new()
	crate.add_child(area)
	var proxy := CollisionShape3D.new()
	proxy.shape = SphereShape3D.new()
	proxy.add_to_group("Interactable")
	area.add_child(proxy)

	var receiver := PacketPeerUDP.new()
	if receiver.bind(0, "127.0.0.1") != OK:
		_fail("UDP bind failed")
		return
	var bridge: Node = load("res://RtVRadarLite.gd").new()
	root.add_child(bridge)
	var started_with_contacts: bool = bridge.get("_include_contacts")
	var started_with_loot: bool = bridge.get("_include_loot")
	bridge.set("_include_contacts", false)
	bridge.get("_udp").set_dest_address("127.0.0.1", receiver.get_local_port())
	bridge.process_mode = Node.PROCESS_MODE_DISABLED
	bridge.call("_process", 0.21)
	var no_ai := _receive(receiver)
	if no_ai.is_empty() or no_ai.get("type") != "snapshot" or no_ai.get("ai", []).size() != 0 or no_ai.get("loot", []).size() != (1 if started_with_loot else 0):
		_fail("Player-only mode leaked contacts or produced invalid packet: %s" % no_ai)
		return
	print("SMOKE OK: lite player only; no contacts")

	bridge.set("_include_contacts", true)
	bridge.call("_process", 0.21)
	var with_ai := _receive(receiver)
	var contacts: Array = with_ai.get("ai", [])
	if contacts.size() != 1 or contacts[0].get("faction") != "Bandit" or contacts[0].get("position") != [20.0, 0.0, 10.0]:
		_fail("Lite AI positions/faction incorrect: %s" % with_ai)
		return
	print("SMOKE OK: lite one Bandit; no sensor reads")
	if started_with_loot:
		var loot_items: Array = with_ai.get("loot", [])
		if loot_items.size() != 1 or loot_items[0].get("name") != "Mock Crate" or loot_items[0].get("position") != [6.0, 0.0, 0.0]:
			_fail("Enabled crate not transmitted: %s" % with_ai)
			return
		proxy.disabled = true
		bridge.call("_process", 0.21)
		var disabled := _receive(receiver)
		if disabled.get("loot", []).size() != 0:
			_fail("Disabled interactable proxy still appeared as loot")
			return
		proxy.disabled = false
		crate.loot = []
		bridge.call("_process", 0.21)
		var empty := _receive(receiver)
		if empty.get("loot", []).size() != 0:
			_fail("Empty container still appeared as loot")
			return
		crate.loot = ["test-item"]
		bridge.call("_process", 0.21)
		var restored := _receive(receiver)
		if restored.get("loot", []).size() != 1:
			_fail("Loot marker did not return after restoring contents")
			return
		crate.storaged = true
		crate.loot = []
		crate.locked = true
		crate.corpse = true
		bridge.call("_process", 0.21)
		var stored := _receive(receiver)
		var stored_items: Array = stored.get("loot", [])
		if stored_items.size() != 1 or not stored_items[0].get("locked") or not stored_items[0].get("corpse"):
			_fail("Stored loot/corpse/lock metadata not reflected: %s" % stored)
			return
		print("SMOKE OK: loot available/empty and interaction proxy gating")
	if bool(bridge.get("_overlay_enabled")):
		var overlay: Control = bridge.get("_radar")
		if overlay == null or overlay.mouse_filter != Control.MOUSE_FILTER_IGNORE or overlay.focus_mode != Control.FOCUS_NONE or not overlay.visible or overlay.get("_contacts").size() != 1:
			_fail("Draw-only in-game overlay did not receive contacts or blocked input")
			return
		await process_frame # Let the Control's queued _draw run in the mock viewport.
		await process_frame
		if started_with_loot and (not bool(overlay.get("_loot_enabled")) or overlay.get("_loot").size() != 1):
			_fail("Overlay did not receive one available loot marker")
			return
		print("SMOKE OK: in-game overlay receives AI and ignores input")
		if bool(bridge.get("_controls_enabled")):
			OS.delay_msec(450)
			agent.position = Vector3(22, 0, 10)
			bridge.call("_process", 0.21)
			var samples: Array = overlay.get("_tracks").get(str(agent.get_instance_id()), [])
			if samples.size() < 2:
				_fail("Movement trails did not record a changed AI position")
				return
			bridge.call("_poll_radar_keys", true, false)
			if int(overlay.get("_layer_mode")) != 1:
				_fail("F7 failed to cycle layers")
				return
			bridge.call("_poll_radar_keys", false, false)
			bridge.call("_poll_radar_keys", false, true)
			if overlay.visible:
				_fail("F8 failed to hide overlay")
				return
			bridge.call("_poll_radar_keys", false, false)
			bridge.call("_poll_radar_keys", false, true)
			if not overlay.visible:
				_fail("F8 failed to restore overlay")
				return
			bridge.call("_poll_radar_keys", false, false)
			if started_with_loot:
				for expected_mode in [2, 3, 0]:
					bridge.call("_poll_radar_keys", true, false)
					if int(overlay.get("_layer_mode")) != expected_mode:
						_fail("F7 did not cycle through loot-only mode")
						return
					bridge.call("_poll_radar_keys", false, false)
			print("SMOKE OK: F7 layer/trail and F8 visibility state transitions")
	if started_with_contacts:
		var trace_path: String = bridge.get("_trace_path")
		var trace := FileAccess.get_file_as_string(trace_path)
		if trace_path.is_empty() or not trace.contains("contacts.enter") or not trace.contains("contacts.done count=1") or not trace.contains("udp.done"):
			_fail("Contact trace missing flushed phases: %s" % trace)
			return
		print("SMOKE OK: contact trace flushed")
	if started_with_loot:
		# Discard any packets left by the movement-trail portion of this test.
		while receiver.get_available_packet_count() > 0:
			receiver.get_packet()
		# Keep the old map (and its Interactable group member) alive while the
		# new scene starts. Only loot belonging to current_scene may be read.
		player.remove_from_group("Player")
		bridge.call("_process", 0.21)
		if not bridge.get("_loot_cache").is_empty():
			_fail("Leaving a scene did not clear cached loot references")
			return
		var new_map := Node3D.new()
		new_map.name = "MapNew"
		root.add_child(new_map)
		current_scene = new_map
		player.add_to_group("Player") # An old Player can linger after current_scene changes.
		bridge.call("_process", 0.21)
		if receiver.get_available_packet_count() != 0 or not bridge.get("_loot_cache").is_empty():
			_fail("Old scene player generated telemetry after current_scene changed")
			return
		player.remove_from_group("Player")
		var new_player := Node3D.new()
		new_player.add_to_group("Player")
		new_map.add_child(new_player)
		bridge.call("_process", 0.21)
		var changed := _receive(receiver)
		if not changed.get("loot", []).is_empty() or not bridge.get("_loot_cache").is_empty():
			_fail("Old map loot leaked into a new scene: %s" % changed)
			return
		var new_crate: Node3D = load("res://Scripts/LootContainer.gd").new()
		new_crate.position = Vector3(9, 0, 0)
		new_map.add_child(new_crate)
		var new_proxy := CollisionShape3D.new()
		new_proxy.shape = SphereShape3D.new()
		new_proxy.add_to_group("Interactable")
		new_crate.add_child(new_proxy)
		bridge.call("_process", 1.01) # Discover the new map's loot on the 1 Hz scan.
		var fresh := _receive(receiver)
		if fresh.get("loot", []).size() != 1 or fresh["loot"][0].get("position") != [9.0, 0.0, 0.0]:
			_fail("New map loot did not replace stale scene cache: %s" % fresh)
			return
		print("SMOKE OK: stale scene loot cleared; new map loot discovered")
	quit()


func _receive(receiver: PacketPeerUDP) -> Dictionary:
	for attempt in 50:
		if receiver.get_available_packet_count() > 0:
			var value: Variant = JSON.parse_string(receiver.get_packet().get_string_from_utf8())
			return value if value is Dictionary else {}
		OS.delay_msec(5)
	return {}


func _fail(message: String) -> void:
	push_error(message)
	quit(1)
