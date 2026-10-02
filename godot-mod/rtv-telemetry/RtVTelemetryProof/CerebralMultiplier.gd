extends RigidBody3D

const GAME_DATA_PATH := "res://Resources/GameData.tres"

var _game_data: Resource
var _pulse := 0.0


func _ready() -> void:
	_game_data = load(GAME_DATA_PATH)
	freeze = true
	sleeping = true
	can_sleep = true
	continuous_cd = false


func _process(delta: float) -> void:
	_pulse = fmod(_pulse + delta, TAU)
	var lamp := get_node_or_null("StatusLamp") as MeshInstance3D
	if lamp != null:
		lamp.scale = Vector3.ONE * (0.82 + sin(_pulse * 2.2) * 0.12)
	var glow := get_node_or_null("SignalGlow") as OmniLight3D
	if glow != null:
		glow.light_energy = 0.22 + sin(_pulse * 2.2) * 0.08


func UpdateTooltip() -> void:
	if _game_data != null:
		_game_data.set("tooltip", "CM-7 Cerebral Multiplier [Recover Interface Artifact]")


func Interact() -> void:
	var bridge := get_node_or_null("/root/RtVTelemetryProof")
	if bridge == null or not bridge.has_method("unlock_cerebral_multiplier"):
		if _game_data != null:
			_game_data.set("tooltip", "CM-7 Cerebral Multiplier [Interface Offline]")
		return
	if bool(bridge.call("unlock_cerebral_multiplier")):
		queue_free()
