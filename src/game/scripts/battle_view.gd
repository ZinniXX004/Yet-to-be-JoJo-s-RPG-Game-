extends Control
## Presentation-only battle driver.
##
## Hard rules for this layer:
##  1. Never compute an outcome. Read events, animate them, and nothing else.
##  2. Never duplicate a formula from the Rust core. If a number is needed on
##     screen, it must arrive in an event or in the state snapshot.
##  3. Never assume an action succeeded. Check `ok` and surface `error`.
##
## Data note: Godot cannot read outside res://, while the canonical content lives
## in the repository-level /data directory. `tools/sync_data.ps1` on Windows and
## `tools/sync_data.sh` elsewhere copy it into res://data as a build step. Do not
## fork the JSON by hand; two divergent copies of content is a bug factory.
##
## Number note: GDScript's JSON has no integer type, so `JSON.parse_string`
## turns every number into a float and `JSON.stringify` writes it back as `95.0`.
## The simulation's schema is integer-only on purpose (floats are not
## bit-reproducible, and determinism is the point of this architecture), so the
## Rust bridge normalizes integral floats before deserializing. Do not "fix"
## this by rounding numbers here: the boundary is the correct place for it, and
## duplicating the repair in two languages guarantees they drift apart.

const DATA_DIR := "res://data"
const PARTY := ["pc.jotaro", "pc.josuke", "pc.kakyoin"]
const FOES := ["npc.dio", "npc.flame_assassin"]

var _session: RefCounted = null
var _awaiting_actor: int = -1
var _finished: bool = false


func _ready() -> void:
	if not ClassDB.class_exists("BattleSession"):
		push_error("GDExtension not loaded. Build rpg-bridge and copy the library into res://bin/.")
		return

	_session = ClassDB.instantiate("BattleSession")

	var config := {
		# Seeded from the clock only at battle creation. The seed is then stored
		# inside the simulation, so the fight stays fully reproducible from it.
		"seed": int(Time.get_unix_time_from_system()),
		"stands": _load_json("%s/stands.json" % DATA_DIR),
		"skills": _load_json("%s/skills.json" % DATA_DIR),
		"combatants": _load_json("%s/combatants.json" % DATA_DIR),
		"party": PARTY,
		"foes": FOES,
	}

	var created: Dictionary = _call_bridge("create", [JSON.stringify(config)])
	if not created.get("ok", false):
		push_error("battle creation failed: %s" % created.get("error", "unknown"))
		return

	print("seed %d, party %s vs %s" % [config["seed"], PARTY, FOES])
	_advance()


## Pumps the scheduler until a party member must decide, or the battle ends.
## Enemy turns are resolved by the core's AI, not here.
func _advance() -> void:
	while not _finished:
		var result: Dictionary = _call_bridge("advance", [])
		if not result.get("ok", false):
			push_error("advance failed: %s" % result.get("error", "unknown"))
			return

		_animate(result.get("events", []))

		var phase: Dictionary = result.get("phase", {})
		if phase.get("phase", "") == "finished":
			_finished = true
			print("battle ended: %s" % phase.get("outcome", "unknown"))
			return

		var actor: int = int(phase.get("actor", -1))
		if _is_party_member(actor):
			_awaiting_actor = actor
			_prompt_for_command(actor)
			return

		# Foe turn: hand it to the core AI so the decision stays inside the
		# deterministic simulation and remains replayable.
		var ai_result: Dictionary = _call_bridge("step_with_ai", [])
		_animate(ai_result.get("events", []))


func submit_attack(target: int) -> void:
	_submit({"kind": "attack", "target": target})


func submit_skill(skill_id: String, target: int) -> void:
	_submit({"kind": "skill", "skill": skill_id, "target": target})


func submit_guard() -> void:
	_submit({"kind": "guard"})


func _submit(command: Dictionary) -> void:
	if _awaiting_actor < 0:
		return
	var result: Dictionary = _call_bridge("submit", [JSON.stringify(command)])
	if not result.get("ok", false):
		# The core kept the turn, so re-prompt instead of skipping it.
		print("rejected: %s" % result.get("error", "unknown"))
		_prompt_for_command(_awaiting_actor)
		return
	_animate(result.get("events", []))
	_awaiting_actor = -1
	_advance()


## M1: replace printing with a queued animation player and real UI widgets.
## The event list is intentionally the single source of truth for what is shown.
func _animate(events: Array) -> void:
	for event in events:
		print("  %s" % JSON.stringify(event))


## M1: replace with the command menu and target picker. Until the UI exists,
## defaulting to a basic attack keeps the loop runnable end to end.
func _prompt_for_command(actor: int) -> void:
	var state: Dictionary = _state()
	var combatants: Array = state.get("combatants", [])
	for index in combatants.size():
		var combatant: Dictionary = combatants[index]
		if combatant.get("team", "") == "foe" and int(combatant.get("hp", 0)) > 0:
			submit_attack(index)
			return
	submit_guard()


func _is_party_member(actor: int) -> bool:
	var combatants: Array = _state().get("combatants", [])
	if actor < 0 or actor >= combatants.size():
		return false
	return combatants[actor].get("team", "") == "party"


func _state() -> Dictionary:
	var parsed: Variant = JSON.parse_string(_session.state_json())
	return parsed if parsed is Dictionary else {}


func _call_bridge(method: String, args: Array) -> Dictionary:
	var raw: String = _session.callv(method, args)
	var parsed: Variant = JSON.parse_string(raw)
	if parsed is Dictionary:
		return parsed
	return {"ok": false, "error": "bridge returned unparseable payload: %s" % raw}


func _load_json(path: String) -> Variant:
	var file := FileAccess.open(path, FileAccess.READ)
	if file == null:
		push_error("missing content file: %s (run tools/sync_data.ps1 or tools/sync_data.sh)" % path)
		return []
	var parsed: Variant = JSON.parse_string(file.get_as_text())
	return parsed if parsed != null else []
