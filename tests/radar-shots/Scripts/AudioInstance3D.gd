extends AudioStreamPlayer3D

func PlayInstance(event: Resource, unit_size_value: float, max_distance_value: float) -> void:
	stream = event.audioClips[0]
	unit_size = unit_size_value
	max_distance = max_distance_value
