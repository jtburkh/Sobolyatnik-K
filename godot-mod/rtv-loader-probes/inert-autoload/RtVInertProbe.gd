extends Node

# Startup marker only: no input, process, hooks, game reads or save writes.
func _ready() -> void:
	print("[RTV_INERT_PROBE] loaded; no gameplay callbacks")
