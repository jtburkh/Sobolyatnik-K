extends Node3D

var active := true
var dead := false
var variant: Resource
var weaponData: Resource
var muzzle: Node3D

func _ready() -> void:
	muzzle = Node3D.new()
	muzzle.name = "Muzzle"
	add_child(muzzle)
