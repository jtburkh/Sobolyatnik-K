extends Node3D

var calls := 0
var map: Node

func _ready() -> void:
	map = get_tree().current_scene

func Airdrop() -> void:
	calls += 1
