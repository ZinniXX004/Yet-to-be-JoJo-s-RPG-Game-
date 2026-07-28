extends Control
## Presentation-only battle driver.
##
## Hard rules for this layer:
##  1. Never compute an outcome. Read events, animate them, and nothing else.
##  2. Never duplicate a formula from the Rust core. If a number is needed on
##     screen, it must arrive in an event or in the state snapshot.
##  3. Never assume an action succeeded. Check `ok` and surface `error`.
##
## M1 note: every widget below is built procedurally in `_build_ui`/
## `_build_status_rows` instead of being laid out in `battle_view.tscn`. The
## scene only owns the root `Control` and the script attachment; this keeps
## the UI in one reviewable place and avoids hand-authoring a `.tscn` resource
## tree, which is normally an editor job, not a text-diff job.
##
## The tempo gauge is current standing, not a forecast: it sorts by each
## combatant's live `tempo` value using the same comparison
## `Battle::ready_actor` uses (higher tempo first, ties to the lower index).
## That is display of an already-computed number, not a re-derivation of who
## is fastest -- rule 2 above would forbid recomputing effective speed here,
## since status modifiers are applied inside `Combatant::spd`, which this file
## has no access to and must not reimplement.
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

# Mirrors the two content ids that have dedicated Command variants in
# rpg-core (see command.rs / resolve.rs: Command::Attack and Command::Guard
# always resolve to these ids regardless of what is in a combatant's skill
# list). Every combatant's skills also happen to include both, so this is
# only about which button a player sees them under, not a rule.
const BASIC_ATTACK_ID := "skill.strike"
const GUARD_ID := "skill.guard_stance"

# Statuses whose potency is not meaningful to a player. Stun is on or off; a
# "40% stun" would read as a made-up number, because the effect has none
# (data/skills.json sets potency 0 on every stun entry).
const BINARY_STATUSES := ["stun"]

const EVENT_DELAY_SECONDS := 0.45

var _session: RefCounted = null
var _awaiting_actor: int = -1
var _finished: bool = false
var _skills_by_id: Dictionary = {}
var _event_queue: Array = []
var _animating: bool = false

# UI, built in _build_ui() / _build_status_rows().
var _log: RichTextLabel
var _tempo_label: Label
var _status_row_container: HBoxContainer
var _status_rows: Dictionary = {} # actor index -> {name_label, hp_bar, hp_label, sp_bar, sp_label}
var _command_box: HBoxContainer
var _target_box: HBoxContainer
var _result_label: Label


func _ready() -> void:
	_build_ui()

	if not ClassDB.class_exists("BattleSession"):
		push_error("GDExtension not loaded. Build rpg-bridge and copy the library into res://bin/.")
		return

	_session = ClassDB.instantiate("BattleSession")

	var skills: Array = _load_json("%s/skills.json" % DATA_DIR)
	for skill in skills:
		if skill is Dictionary and skill.has("id"):
			_skills_by_id[skill["id"]] = skill

	var config := {
		# Seeded from the clock only at battle creation. The seed is then stored
		# inside the simulation, so the fight stays fully reproducible from it.
		"seed": int(Time.get_unix_time_from_system()),
		"stands": _load_json("%s/stands.json" % DATA_DIR),
		"skills": skills,
		"combatants": _load_json("%s/combatants.json" % DATA_DIR),
		"party": PARTY,
		"foes": FOES,
	}

	var created: Dictionary = _call_bridge("create", [JSON.stringify(config)])
	if not created.get("ok", false):
		push_error("battle creation failed: %s" % created.get("error", "unknown"))
		return

	_log_line("Seed %d. %s vs %s." % [config["seed"], _pretty_names(PARTY), _pretty_names(FOES)])
	_build_status_rows()
	await _advance()


## Pumps the scheduler until a party member must decide, or the battle ends.
## Enemy turns are resolved by the core's AI, not here.
func _advance() -> void:
	while not _finished:
		var result: Dictionary = _call_bridge("advance", [])
		if not result.get("ok", false):
			push_error("advance failed: %s" % result.get("error", "unknown"))
			return

		await _animate(result.get("events", []))
		_refresh_status_panel()

		var phase: Dictionary = result.get("phase", {})
		if phase.get("phase", "") == "finished":
			_finished = true
			_show_result(String(phase.get("outcome", "unknown")))
			return

		var actor: int = int(phase.get("actor", -1))
		if _is_party_member(actor):
			_awaiting_actor = actor
			_show_command_menu(actor)
			return

		# Foe turn: hand it to the core AI so the decision stays inside the
		# deterministic simulation and remains replayable.
		var ai_result: Dictionary = _call_bridge("step_with_ai", [])
		await _animate(ai_result.get("events", []))
		_refresh_status_panel()


func submit_attack(target: int) -> void:
	_submit({"kind": "attack", "target": target})


func submit_skill(skill_id: String, target: int) -> void:
	_submit({"kind": "skill", "skill": skill_id, "target": target})


func submit_guard() -> void:
	_submit({"kind": "guard"})


func _submit(command: Dictionary) -> void:
	if _awaiting_actor < 0:
		return
	var actor := _awaiting_actor
	_clear_container(_command_box)
	_clear_container(_target_box)

	var result: Dictionary = _call_bridge("submit", [JSON.stringify(command)])
	if not result.get("ok", false):
		# The core kept the turn, so re-prompt instead of skipping it.
		_log_line("[color=orange]Rejected: %s[/color]" % result.get("error", "unknown"))
		_show_command_menu(actor)
		return

	await _animate(result.get("events", []))
	_refresh_status_panel()
	_awaiting_actor = -1
	await _advance()


## ---------------------------------------------------------------------------
## Command menu / target picker
## ---------------------------------------------------------------------------

func _show_command_menu(actor: int) -> void:
	_clear_container(_target_box)
	_clear_container(_command_box)

	var combatants: Array = _state().get("combatants", [])
	if actor < 0 or actor >= combatants.size():
		return
	var combatant: Dictionary = combatants[actor]
	var sp: int = int(combatant.get("sp", 0))
	var known_skills: Array = combatant.get("skills", [])

	_add_command_button("Attack", func():
		_begin_target_selection(actor, "one_enemy", func(target: int): submit_attack(target))
	)

	for skill_id in known_skills:
		if skill_id == BASIC_ATTACK_ID or skill_id == GUARD_ID:
			continue
		var def: Dictionary = _skills_by_id.get(skill_id, {})
		if def.is_empty():
			continue
		var cost: int = int(def.get("sp_cost", 0))
		var label: String = "%s (%d SP)" % [def.get("name", skill_id), cost] if cost > 0 else String(def.get("name", skill_id))
		var button := _add_command_button(label, func(): _use_skill(actor, skill_id))
		button.disabled = sp < cost
		if button.disabled:
			button.tooltip_text = "Needs %d SP, %s has %d" % [cost, combatant.get("name", "?"), sp]

	_add_command_button("Guard", func(): submit_guard())
	_add_command_button("Wait", func(): _submit({"kind": "wait"}))


func _use_skill(actor: int, skill_id: String) -> void:
	var def: Dictionary = _skills_by_id.get(skill_id, {})
	var target_kind: String = String(def.get("target", "one_enemy"))
	match target_kind:
		"self_only", "all_enemies", "all_allies":
			# rpg_core::resolve::resolve_targets ignores the requested target for
			# these kinds and computes the real target set itself; the actor's
			# own index is only a placeholder to satisfy the JSON schema.
			submit_skill(skill_id, actor)
		"one_ally":
			_begin_target_selection(actor, "one_ally", func(target: int): submit_skill(skill_id, target))
		_:
			_begin_target_selection(actor, "one_enemy", func(target: int): submit_skill(skill_id, target))


func _begin_target_selection(actor: int, kind: String, on_pick: Callable) -> void:
	_clear_container(_command_box)
	_clear_container(_target_box)

	var combatants: Array = _state().get("combatants", [])
	if actor < 0 or actor >= combatants.size():
		return
	var actor_team: String = String(combatants[actor].get("team", ""))
	var wanted_team: String = actor_team if kind == "one_ally" else ("foe" if actor_team == "party" else "party")

	for index in combatants.size():
		var combatant: Dictionary = combatants[index]
		if String(combatant.get("team", "")) == wanted_team and int(combatant.get("hp", 0)) > 0:
			var button := Button.new()
			button.text = String(combatant.get("name", "?"))
			button.pressed.connect(func(): on_pick.call(index))
			_target_box.add_child(button)

	var cancel := Button.new()
	cancel.text = "Cancel"
	cancel.pressed.connect(func(): _show_command_menu(actor))
	_target_box.add_child(cancel)


func _add_command_button(label: String, on_pressed: Callable) -> Button:
	var button := Button.new()
	button.text = label
	button.pressed.connect(on_pressed)
	_command_box.add_child(button)
	return button


func _clear_container(container: Container) -> void:
	for child in container.get_children():
		child.queue_free()


## ---------------------------------------------------------------------------
## Event animation
## ---------------------------------------------------------------------------

## Queues events and plays them one at a time. Safe to call while a previous
## batch is still draining: the new events are appended, not raced, because
## the loop below only starts once and keeps consuming until the queue empties.
func _animate(events: Array) -> void:
	for event in events:
		_event_queue.append(event)
	if _animating:
		return
	_animating = true
	while not _event_queue.is_empty():
		var event: Dictionary = _event_queue.pop_front()
		_render_event(event)
		await get_tree().create_timer(EVENT_DELAY_SECONDS).timeout
	_animating = false


func _render_event(event: Dictionary) -> void:
	var kind: String = String(event.get("kind", ""))
	match kind:
		"battle_started":
			pass # already announced from _ready with the party/foe lineup
		"turn_started":
			_log_line("[b]%s's turn.[/b]" % _name(event.get("actor")))
		"turn_skipped":
			_log_line("%s's turn is skipped (%s)." % [_name(event.get("actor")), event.get("reason", "")])
		"action_used":
			_log_line("%s uses %s!" % [_name(event.get("actor")), event.get("name", "?")])
		"waited":
			_log_line("%s waits." % _name(event.get("actor")))
		"sp_consumed":
			_log_line("  (%d SP spent)" % int(event.get("amount", 0)))
		"missed":
			_log_line("%s's attack misses %s." % [_name(event.get("actor")), _name(event.get("target"))])
		"damaged":
			var element: String = String(event.get("element", "physical"))
			var suffix: String = " (%s)" % element if element != "physical" else ""
			var crit: String = " Critical hit!" if event.get("crit", false) else ""
			_log_line("%s hits %s for %d damage%s.%s" % [
				_name(event.get("actor")), _name(event.get("target")),
				int(event.get("amount", 0)), suffix, crit,
			])
		"status_damaged":
			# Deliberately distinct from "damaged": rpg_core::event::Event::StatusDamaged
			# has no attacker, because the cause is the status, not a blow anyone struck.
			_log_line("%s takes %d damage from %s." % [
				_name(event.get("target")), int(event.get("amount", 0)),
				_status_name(String(event.get("status", ""))),
			])
		"healed":
			_log_line("%s recovers %d HP." % [_name(event.get("target")), int(event.get("amount", 0))])
		"status_applied":
			var status: String = String(event.get("status", ""))
			if BINARY_STATUSES.has(status):
				_log_line("%s is %s!" % [_name(event.get("target")), _status_name(status)])
			else:
				_log_line("%s gains %s (%d) for %d turns." % [
					_name(event.get("target")), _status_name(status),
					int(event.get("potency", 0)), int(event.get("duration", 0)),
				])
		"status_resisted":
			_log_line("%s resists %s." % [_name(event.get("target")), _status_name(String(event.get("status", "")))])
		"status_expired":
			_log_line("%s's %s fades." % [_name(event.get("target")), _status_name(String(event.get("status", "")))])
		"tempo_locked":
			_log_line("%s's tempo is frozen for %d ticks." % [_name(event.get("target")), int(event.get("ticks", 0))])
		"downed":
			_log_line("[color=red]%s is downed![/color]" % _name(event.get("target")))
		"battle_ended":
			pass # the outcome is surfaced from the phase in _advance, not the log
		_:
			_log_line(JSON.stringify(event))


## ---------------------------------------------------------------------------
## Status panel / tempo gauge
## ---------------------------------------------------------------------------

func _build_status_rows() -> void:
	for child in _status_row_container.get_children():
		child.queue_free()
	_status_rows.clear()

	var combatants: Array = _state().get("combatants", [])
	for index in combatants.size():
		var combatant: Dictionary = combatants[index]

		var card := VBoxContainer.new()
		card.custom_minimum_size = Vector2(150, 0)

		var name_label := Label.new()
		name_label.text = String(combatant.get("name", "?"))
		card.add_child(name_label)

		var hp_bar := ProgressBar.new()
		hp_bar.show_percentage = false
		hp_bar.max_value = max(int(combatant.get("max_hp", 1)), 1)
		hp_bar.value = int(combatant.get("hp", 0))
		card.add_child(hp_bar)

		var hp_label := Label.new()
		hp_label.add_theme_font_size_override("font_size", 12)
		card.add_child(hp_label)

		var sp_bar := ProgressBar.new()
		sp_bar.show_percentage = false
		sp_bar.max_value = max(int(combatant.get("max_sp", 1)), 1)
		sp_bar.value = int(combatant.get("sp", 0))
		card.add_child(sp_bar)

		var sp_label := Label.new()
		sp_label.add_theme_font_size_override("font_size", 12)
		card.add_child(sp_label)

		_status_row_container.add_child(card)
		_status_rows[index] = {
			"name_label": name_label,
			"hp_bar": hp_bar,
			"hp_label": hp_label,
			"sp_bar": sp_bar,
			"sp_label": sp_label,
		}

	_refresh_status_panel()


func _refresh_status_panel() -> void:
	var combatants: Array = _state().get("combatants", [])
	for index in _status_rows.keys():
		if index >= combatants.size():
			continue
		var combatant: Dictionary = combatants[index]
		var row: Dictionary = _status_rows[index]
		var hp: int = int(combatant.get("hp", 0))
		var max_hp: int = max(int(combatant.get("max_hp", 1)), 1)
		var sp: int = int(combatant.get("sp", 0))
		var max_sp: int = max(int(combatant.get("max_sp", 1)), 1)

		row["hp_bar"].value = hp
		row["hp_label"].text = "HP %d / %d" % [hp, max_hp]
		row["sp_bar"].value = sp
		row["sp_label"].text = "SP %d / %d" % [sp, max_sp]
		row["name_label"].modulate = Color(1, 1, 1, 0.4) if hp <= 0 else Color(1, 1, 1, 1)

	_refresh_tempo_label(combatants)


## Sorts living combatants by their current tempo value, highest first, tied
## on the lower index -- the exact comparison `Battle::ready_actor` makes.
## This is a display of state that already exists; it is not a prediction.
func _refresh_tempo_label(combatants: Array) -> void:
	var living: Array = []
	for index in combatants.size():
		var combatant: Dictionary = combatants[index]
		if int(combatant.get("hp", 0)) > 0:
			living.append({
				"index": index,
				"name": String(combatant.get("name", "?")),
				"tempo": int(combatant.get("tempo", 0)),
			})

	living.sort_custom(func(a, b):
		if a["tempo"] == b["tempo"]:
			return a["index"] < b["index"]
		return a["tempo"] > b["tempo"]
	)

	var parts: Array = []
	for entry in living:
		parts.append("%s (%d)" % [entry["name"], entry["tempo"]])

	_tempo_label.text = "Tempo order: %s" % ", ".join(parts) if not parts.is_empty() else "Tempo order: --"


## ---------------------------------------------------------------------------
## Victory / defeat
## ---------------------------------------------------------------------------

func _show_result(outcome: String) -> void:
	_clear_container(_command_box)
	_clear_container(_target_box)

	var text: String
	match outcome:
		"party_wins":
			text = "Victory!"
		"party_wipes":
			text = "Defeat."
		"stalemate":
			text = "Stalemate."
		_:
			text = "Battle ended (%s)." % outcome

	_result_label.text = text
	_result_label.visible = true
	_log_line("[b]%s[/b]" % text)


## ---------------------------------------------------------------------------
## UI construction
## ---------------------------------------------------------------------------

func _build_ui() -> void:
	var margin := MarginContainer.new()
	margin.name = "Margin"
	margin.anchor_right = 1.0
	margin.anchor_bottom = 1.0
	margin.add_theme_constant_override("margin_left", 16)
	margin.add_theme_constant_override("margin_right", 16)
	margin.add_theme_constant_override("margin_top", 16)
	margin.add_theme_constant_override("margin_bottom", 16)
	add_child(margin)

	var root_box := VBoxCont