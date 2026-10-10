extends Node3D

var active := true
var spawnDistance := 100.0
var calls: Array = []

func SpawnBoss(name: String, state: String, position: Vector3, waypoint: Node3D) -> void:
	calls.append([name, state, position, waypoint])
	var boss := get_node_or_null("B_Pool/AI_" + name)
	if boss != null:
		boss.queue_free()
