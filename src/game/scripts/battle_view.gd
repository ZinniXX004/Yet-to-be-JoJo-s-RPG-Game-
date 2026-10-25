extends Control
## Presentation-only battle driver.
##
## Hard rules:
##  1. Never compute an outcome; only read events and animate them.
##  2. Never duplicate a core formula. Numbers must come from an event or
##     the state snapshot (e.g. the tempo gauge sorts by the already-computed
##     `tempo` field the same way Battle::ready_actor does; it does not
##     recompute effective speed, which lives behind Combatant::spd).
##  3. Never assume an action succeeded; check `ok` and surface `error`.
##
## The UI below is built procedurally (see _build_ui/_build_status_rows)
## instead of being authored in battle_view.tscn, which only owns the root
## Control and this script.
##
## Data note: content lives in the repo-level /data directory and is copied
## into res://data by tools/sync_data.*; do not fork the JSON by hand.
##
## Number note: GDScript's JSON has no integer type, so parsed numbers arrive
## as floats; the Rust bridge normalizes them back to integers at the
## boundary. Do not re-round them here.
##
## Event note: every case below reads fields by *name*, never by position, so
## a new field on an existing event cannot break this script. A new event
## *kind* can: unrecognised kinds fall to the catch-all, dump raw JSON into
## the log and push a warning. When rpg-core gains a variant, it needs a case
## here; the headless smoke test fails on that warning until it has one.
##
## Command line, everything after `--` on the Godot command line:
##   --encounter=<id>  an encounter from data/matchups.json (default
##                     matchup.dio_boss), so the screen plays the same fights
##                     the balance harness measures instead of a roster
##                     hard-coded here, which had drifted from every one of them
##   --seed=<n>        replay one battle exactly; default is the clock
##   --autoplay        the AI plays the party too
##   --quit-on-end     exit when the battle ends: 0 for any outcome, 1 if the
##                     battle could not be set up or advanced
## tools/godot_smoke.py combines all four to play every encounter in CI.
##
## What the screen shows, and where each number comes from (#18):
##   - Floating numbers over a combatant's card: one per damaged, healed,
##     status_damaged, status_healed, missed and status_resisted event, with
##     the event's own amount. Nothing is summed, rounded or predicted here, so
##     every number on screen is a number in the log.
##   - Status badges on each card: the state snapshot's `statuses`, with
##     `remaining` (turns of that combatant's own left), plus a LOCK badge
##     while `tempo_lock` is above zero.
##   - The tempo panel: each combatant's `tempo` against the scheduler's
##     threshold, read from the bridge rather than copied into this file, in the
##     order Battle::ready_actor breaks ties (higher tempo first, then the lower
##     index). Downed combatants stay in the panel, marked down, so it no longer
##     collapses to one name late in a fight -- correct, but it read as a bug.

const DATA_DIR := "res://data"
const DEFAULT_ENCOUNTER := "matchup.dio_boss"
# Seeds reach Rust through GDScript's JSON, whose numbers are doubles; above
# 2^53 a seed would change on the way and the battle would not replay.
const MAX_SEED := 9007199254740992

# Command::Attack and Command::Guard always resolve to these ids in
# rpg-core regardless of a combatant's skill list (see resolve.rs). Every
# combatant's skills also include both; this only decides which button
# they appear under.
const BASIC_ATTACK_ID := "skill.strike"
const GUARD_ID := "skill.guard_stance"

# Statuses whose potency is meaningless (stun is on/off; data/skills.json
# always sets its potency to 0).
const BINARY_STATUSES := ["stun"]

const EVENT_DELAY_SECONDS := 0.45
const FLOAT_SECONDS := 1.1
const FLOAT_RISE_PIXELS := 42.0

# Badge text and colour per status kind. Text only until the project has art;
# the remaining duration is appended at render time.
const STATUS_BADGES := {
	"atk_up": ["ATK+", Color(0.45, 0.85, 0.45)],
	"def_up": ["DEF+", Color(0.45, 0.85, 0.45)],
	"spd_up": ["SPD+", Color(0.45, 0.85, 0.45)],
	"atk_down": ["ATK-", Color(0.95, 0.45, 0.40)],
	"def_down": ["DEF-", Color(0.95, 0.45, 0.40)],
	"spd_down": ["SPD-", Color(0.95, 0.45, 0.40)],
	"bleed": ["BLEED", Color(0.85, 0.25, 0.30)],
	"regen": ["REGEN", Color(0.35, 0.85, 0.80)],
	"stun": ["STUN", Color(0.95, 0.85, 0.30)],
}
const LOCK_COLOR := Color(0.70, 0.55, 0.95)
const DAMAGE_COLOR := Color(1.0, 1.0, 1.0)
const CRIT_COLOR := Color(1.0, 0.82, 0.25)
const STATUS_DAMAGE_COLOR := Color(0.85, 0.35, 0.45)
const HEAL_COLOR := Color(0.45, 0.95, 0.50)
const MISS_COLOR := Color(0.70, 0.70, 0.70)

var _session: RefCounted = null
var _awaiting_actor: int = -1
var _finished: bool = false
var _skills_by_id: Dictionary = {}
var _event_queue: Array = []
var _animating: bool = false

var _encounter_id: String = DEFAULT_ENCOUNTER
var _seed: int = -1
var _autoplay: bool = false
var _quit_on_end: bool = false
var _event_delay: float = EVENT_DELAY_SECONDS
var _party: Array = []
var _foes: Array = []

var _tempo_threshold: int = 0
var _current_actor: int = -1
var _float_stack: Dictionary = {}

var _log: RichTextLabel
var _tempo_title: Label
var _tempo_row: HBoxContainer
var _tempo_entries: Dictionary = {}
var _float_layer: Control
var _status_row_container: HBoxContainer
var _status_rows: Dictionary = {}
var _command_box: HBoxContainer
var _target_box: HBoxContainer
var _result_label: Label


func _ready() -> void:
	_build_ui()
	if not _parse_args():
		return

	if not ClassDB.class_exists("BattleSession"):
		_fail("GDExtension not loaded. Build rpg-bridge and copy the library into res://bin/ (docs/DEVELOPMENT.md, section 3).")
		return

	_session = ClassDB.instantiate("BattleSession")
	if not _session.has_method("constants"):
		_fail("the GDExtension in res://bin/ predates BattleSession.constants(); rebuild rpg-bridge and copy it again (docs/DEVELOPMENT.md, section 3).")
		return
	var constants: Dictionary = _call_bridge("constants", [])
	_tempo_threshold = int(constants.get("tempo_threshold", 0))
	if not constants.get("ok", false) or _tempo_threshold <= 0:
		_fail("bridge constants unusable: %s" % JSON.stringify(constants))
		return

	var stands: Variant = _load_content("stands.json")
	var skills: Variant = _load_content("skills.json")
	var combatants: Variant = _load_content("combatants.json")
	var matchups: Variant = _load_content("matchups.json")
	if stands == null or skills == null or combatants == null or matchups == null:
		return

	for skill in skills:
		if skill is Dictionary and skill.has("id"):
			_skills_by_id[skill["id"]] = skill

	var encounter: Dictionary = _find_encounter(matchups, _encounter_id)
	if encounter.is_empty():
		return
	_party = encounter.get("party", [])
	_foes = encounter.get("foes", [])

	if _seed < 0:
		_seed = int(Time.get_unix_time_from_system())

	var config := {
		"seed": _seed,
		"stands": stands,
		"skills": skills,
		"combatants": combatants,
		"party": _party,
		"foes": _foes,
	}

	var created: Dictionary = _call_bridge("create", [JSON.stringify(config)])
	if not created.get("ok", false):
		_fail("battle creation failed: %s" % created.get("error", "unknown"))
		return

	_log_line("%s [%s], seed %d. %s vs %s." % [
		String(encounter.get("name", _encounter_id)), _encounter_id, _seed,
		_pretty_names(_party), _pretty_names(_foes),
	])
	_build_status_rows()
	await _advance()


func _parse_args() -> bool:
	var seed_text := ""
	for arg in OS.get_cmdline_user_args():
		if arg.begins_with("--encounter="):
			_encounter_id = arg.trim_prefix("--encounter=")
		elif arg.begins_with("--seed="):
			seed_text = arg.trim_prefix("--seed=")
		elif arg == "--autoplay":
			_autoplay = true
		elif arg == "--quit-on-end":
			_quit_on_end = true
		else:
			push_warning("ignoring unknown argument '%s'" % arg)

	# Validated after the loop, so --quit-on-end is known however the
	# arguments were ordered and a bad seed exits instead of hanging.
	if not seed_text.is_empty():
		if not seed_text.is_valid_int() or seed_text.to_int() < 0 or seed_text.to_int() >= MAX_SEED:
			_fail("--seed must be a whole number from 0 to 2^53 - 1, got '%s'" % seed_text)
			return false
		_seed = seed_text.to_int()

	# Nobody watches a headless run, and a pause per event would turn a
	# hundred-turn boss fight into minutes of sleeping in CI.
	if DisplayServer.get_name() == "headless":
		_event_delay = 0.0
	return true


func _load_content(file_name: String) -> Variant:
	var path := "%s/%s" % [DATA_DIR, file_name]
	if not FileAccess.file_exists(path):
		_fail("%s is missing. src/game/data/ is not in git; run tools/sync_data.ps1 or tools/sync_data.sh from the repository root." % path)
		return null
	var parsed: Variant = _load_json(path)
	if not parsed is Array:
		_fail("%s did not parse as a JSON array." % path)
		return null
	return parsed


func _find_encounter(matchups: Array, id: String) -> Dictionary:
	var known: Array = []
	for matchup in matchups:
		if not matchup is Dictionary:
			continue
		var matchup_id := String(matchup.get("id", ""))
		if matchup_id == id:
			return matchup
		known.append(matchup_id)
	_fail("unknown encounter '%s'. Known: %s" % [id, ", ".join(known)])
	return {}


func _fail(message: String) -> void:
	push_error(message)
	if _log != null:
		_log_line("[color=red]%s[/color]" % message)
	if _quit_on_end:
		get_tree().quit(1)


func _finish(outcome: String) -> void:
	# One machine-readable line per battle. tools/godot_smoke.py requires it,
	# so a battle that silently never ended cannot pass for one that did.
	print("[battle] finished encounter=%s seed=%d outcome=%s" % [_encounter_id, _seed, outcome])
	if _quit_on_end:
		get_tree().quit(0)


func _advance() -> void:
	while not _finished:
		var result: Dictionary = _call_bridge("advance", [])
		if not result.get("ok", false):
			_fail("advance failed: %s" % result.get("error", "unknown"))
			return

		await _animate(result.get("events", []))
		_refresh_status_panel()

		var phase: Dictionary = result.get("phase", {})
		if phase.get("phase", "") == "finished":
			_finished = true
			_show_result(String(phase.get("outcome", "unknown")))
			_finish(String(phase.get("outcome", "unknown")))
			return

		var actor: int = int(phase.get("actor", -1))
		if _is_party_member(actor) and not _autoplay:
			_awaiting_actor = actor
			_show_command_menu(actor)
			return

		var ai_result: Dictionary = _call_bridge("step_with_ai", [])
		if not ai_result.get("ok", false):
			_fail("step_with_ai failed: %s" % ai_result.get("error", "unknown"))
			return
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
		_log_line("[color=orange]Rejected: %s[/color]" % result.get("error", "unknown"))
		_show_command_menu(actor)
		return

	await _animate(result.get("events", []))
	_refresh_status_panel()
	_awaiting_actor = -1
	await _advance()


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
			button.tooltip_text = "Needs %d SP, has %d" % [cost, sp]

	_add_command_button("Guard", func(): submit_guard())
	_add_command_button("Wait", func(): _submit({"kind": "wait"}))


func _use_skill(actor: int, skill_id: String) -> void:
	var def: Dictionary = _skills_by_id.get(skill_id, {})
	var target_kind: String = String(def.get("target", "one_enemy"))
	match target_kind:
		"self_only", "all_enemies", "all_allies":
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


func _animate(events: Array) -> void:
	for event in events:
		_event_queue.append(event)
	if _animating:
		return
	_animating = true
	while not _event_queue.is_empty():
		var event: Dictionary = _event_queue.pop_front()
		_render_event(event)
		if _event_delay > 0.0:
			await get_tree().create_timer(_event_delay).timeout
	_animating = false


func _render_event(event: Dictionary) -> void:
	var kind: String = String(event.get("kind", ""))
	match kind:
		"battle_started":
			pass
		"turn_started":
			_current_actor = int(event.get("actor", -1))
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
			_spawn_float(int(event.get("target", -1)), "miss", MISS_COLOR, 16)
		"damaged":
			var element: String = String(event.get("element", "physical"))
			var suffix: String = " (%s)" % element if element != "physical" else ""
			var crit: String = " Critical hit!" if event.get("crit", false) else ""
			_log_line("%s hits %s for %d damage%s.%s" % [
				_name(event.get("actor")), _name(event.get("target")),
				int(event.get("amount", 0)), suffix, crit,
			])
			if event.get("crit", false):
				_spawn_float(int(event.get("target", -1)), "-%d!" % int(event.get("amount", 0)), CRIT_COLOR, 26)
			else:
				_spawn_float(int(event.get("target", -1)), "-%d" % int(event.get("amount", 0)), DAMAGE_COLOR, 20)
		"status_damaged":
			_log_line("%s takes %d damage from %s." % [
				_name(event.get("target")), int(event.get("amount", 0)),
				_status_name(String(event.get("status", ""))),
			])
			_spawn_float(int(event.get("target", -1)), "-%d %s" % [
				int(event.get("amount", 0)), String(event.get("status", "")),
			], STATUS_DAMAGE_COLOR, 16)
		"healed":
			# The event now names a healer, so a drain's self-heal and a heal
			# cast on someone else can finally read as the different actions
			# they are. Before this they were the same sentence.
			var healer: int = int(event.get("actor", -1))
			var healed: int = int(event.get("target", -1))
			if healer == healed:
				_log_line("%s recovers %d HP." % [_name(healed), int(event.get("amount", 0))])
			else:
				_log_line("%s restores %d HP to %s." % [
					_name(healer), int(event.get("amount", 0)), _name(healed),
				])
			_spawn_float(healed, "+%d" % int(event.get("amount", 0)), HEAL_COLOR, 20)
		"status_healed":
			# The mirror of status_damaged: recovery with no actor, so the
			# condition is named instead of anyone acting.
			_log_line("%s recovers %d HP from %s." % [
				_name(event.get("target")), int(event.get("amount", 0)),
				_status_name(String(event.get("status", ""))),
			])
			_spawn_float(int(event.get("target", -1)), "+%d %s" % [
				int(event.get("amount", 0)), String(event.get("status", "")),
			], HEAL_COLOR, 16)
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
			_spawn_float(int(event.get("target", -1)), "resist", MISS_COLOR, 16)
		"status_expired":
			_log_line("%s's %s fades." % [_name(event.get("target")), _status_name(String(event.get("status", "")))])
		"tempo_locked":
			_log_line("%s's tempo is frozen for %d ticks." % [_name(event.get("target")), int(event.get("ticks", 0))])
		"downed":
			_log_line("[color=red]%s is downed![/color]" % _name(event.get("target")))
		"battle_ended":
			pass
		_:
			push_warning("unhandled event kind '%s': battle_view.gd needs a case for it" % kind)
			_log_line(JSON.stringify(event))


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

		# Wraps inside the card's width, so five badges do not widen the row.
		var badges := HFlowContainer.new()
		badges.add_theme_constant_override("h_separation", 4)
		badges.add_theme_constant_override("v_separation", 2)
		badges.custom_minimum_size = Vector2(0, 18)
		card.add_child(badges)

		_status_row_container.add_child(card)
		_status_rows[index] = {
			"card": card,
			"name_label": name_label,
			"hp_bar": hp_bar,
			"hp_label": hp_label,
			"sp_bar": sp_bar,
			"sp_label": sp_label,
			"badges": badges,
		}
		_add_tempo_entry(index, String(combatant.get("name", "?")))

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
		_refresh_badges(row["badges"], combatant)

	_refresh_tempo_panel(combatants)


func _refresh_tempo_panel(combatants: Array) -> void:
	# Living combatants in the scheduler's own tie-break order, then the
	# downed ones, which stay visible so the panel never shrinks to one name.
	var living: Array = []
	var downed: Array = []
	for index in combatants.size():
		if not _tempo_entries.has(index):
			continue
		var combatant: Dictionary = combatants[index]
		var item := {
			"index": index,
			"tempo": int(combatant.get("tempo", 0)),
			"lock": int(combatant.get("tempo_lock", 0)),
		}
		if int(combatant.get("hp", 0)) > 0:
			living.append(item)
		else:
			downed.append(item)

	living.sort_custom(func(a, b):
		if a["tempo"] == b["tempo"]:
			return a["index"] < b["index"]
		return a["tempo"] > b["tempo"]
	)

	var slot := 0
	for entry in living + downed:
		var widgets: Dictionary = _tempo_entries[entry["index"]]
		_tempo_row.move_child(widgets["box"], slot)
		slot += 1
		var is_down: bool = downed.has(entry)
		var bar: ProgressBar = widgets["bar"]
		bar.max_value = _tempo_threshold
		bar.value = 0 if is_down else min(entry["tempo"], _tempo_threshold)
		var value_label: Label = widgets["value"]
		if is_down:
			value_label.text = "down"
		elif entry["lock"] > 0:
			value_label.text = "%d / %d  locked %d" % [entry["tempo"], _tempo_threshold, entry["lock"]]
		else:
			value_label.text = "%d / %d" % [entry["tempo"], _tempo_threshold]
		var name_label: Label = widgets["name"]
		var marker := "> " if entry["index"] == _current_actor and not is_down else ""
		name_label.text = marker + String(widgets["base_name"])
		widgets["box"].modulate = Color(1, 1, 1, 0.4) if is_down else Color(1, 1, 1, 1)

	_tempo_title.text = "Tempo -- a combatant acts on reaching %d" % _tempo_threshold


func _add_tempo_entry(index: int, display_name: String) -> void:
	var box := VBoxContainer.new()
	box.custom_minimum_size = Vector2(110, 0)

	var name_label := Label.new()
	name_label.text = display_name
	name_label.add_theme_font_size_override("font_size", 12)
	box.add_child(name_label)

	var bar := ProgressBar.new()
	bar.show_percentage = false
	bar.custom_minimum_size = Vector2(0, 8)
	box.add_child(bar)

	var value_label := Label.new()
	value_label.add_theme_font_size_override("font_size", 11)
	box.add_child(value_label)

	_tempo_row.add_child(box)
	_tempo_entries[index] = {
		"box": box,
		"name": name_label,
		"base_name": display_name,
		"bar": bar,
		"value": value_label,
	}


func _refresh_badges(container: HFlowContainer, combatant: Dictionary) -> void:
	for child in container.get_children():
		child.queue_free()
	if int(combatant.get("hp", 0)) <= 0:
		return
	for status in combatant.get("statuses", []):
		if not status is Dictionary:
			continue
		var kind := String(status.get("kind", ""))
		var spec: Array = STATUS_BADGES.get(kind, [kind.to_upper(), Color(0.8, 0.8, 0.8)])
		var remaining := int(status.get("remaining", 0))
		var badge := _make_badge("%s %d" % [spec[0], remaining], spec[1])
		badge.tooltip_text = "%s, potency %d, %d of its own turns left" % [
			_status_name(kind), int(status.get("potency", 0)), remaining,
		]
		container.add_child(badge)
	var lock := int(combatant.get("tempo_lock", 0))
	if lock > 0:
		var lock_badge := _make_badge("LOCK %d" % lock, LOCK_COLOR)
		lock_badge.tooltip_text = "tempo frozen for %d more ticks" % lock
		container.add_child(lock_badge)


func _make_badge(text: String, color: Color) -> Label:
	var badge := Label.new()
	badge.text = text
	badge.add_theme_font_size_override("font_size", 11)
	badge.add_theme_color_override("font_color", color)
	badge.mouse_filter = Control.MOUSE_FILTER_PASS
	return badge


func _spawn_float(target: int, text: String, color: Color, font_size: int) -> void:
	# The text is built by the caller from one event's own fields. This
	# function only decides where it appears and how it fades.
	if not _status_rows.has(target):
		return
	var card: Control = _status_rows[target]["card"]
	var label := Label.new()
	label.text = text
	label.mouse_filter = Control.MOUSE_FILTER_IGNORE
	label.add_theme_font_size_override("font_size", font_size)
	label.add_theme_color_override("font_color", color)
	label.add_theme_color_override("font_outline_color", Color(0, 0, 0))
	label.add_theme_constant_override("outline_size", 5)
	_float_layer.add_child(label)

	# Numbers that land on the same card in quick succession stack upward
	# instead of overprinting each other.
	var stack: int = _float_stack.get(target, 0)
	_float_stack[target] = stack + 1
	var origin: Vector2 = card.global_position - _float_layer.global_position
	# Starts over the HP bar, not the name, so the card stays readable.
	label.position = origin + Vector2(card.size.x * 0.55, card.size.y * 0.4 - 20.0 * stack)

	var tween := label.create_tween()
	tween.set_parallel(true)
	tween.tween_property(label, "position:y", label.position.y - FLOAT_RISE_PIXELS, FLOAT_SECONDS)
	tween.tween_property(label, "modulate:a", 0.0, FLOAT_SECONDS * 0.5).set_delay(FLOAT_SECONDS * 0.5)
	tween.chain().tween_callback(_on_float_done.bind(target, label))


func _on_float_done(target: int, label: Label) -> void:
	label.queue_free()
	_float_stack[target] = max(int(_float_stack.get(target, 1)) - 1, 0)


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


func _build_ui() -> void:
	var margin := MarginContainer.new()
	margin.anchor_right = 1.0
	margin.anchor_bottom = 1.0
	margin.add_theme_constant_override("margin_left", 16)
	margin.add_theme_constant_override("margin_right", 16)
	margin.add_theme_constant_override("margin_top", 16)
	margin.add_theme_constant_override("margin_bottom", 16)
	add_child(margin)

	var root_box := VBoxContainer.new()
	root_box.add_theme_constant_override("separation", 10)
	margin.add_child(root_box)

	_tempo_title = Label.new()
	_tempo_title.text = "Tempo"
	_tempo_title.add_theme_font_size_override("font_size", 12)
	root_box.add_child(_tempo_title)

	_tempo_row = HBoxContainer.new()
	_tempo_row.add_theme_constant_override("separation", 10)
	root_box.add_child(_tempo_row)

	_status_row_container = HBoxContainer.new()
	_status_row_container.add_theme_constant_override("separation", 16)
	root_box.add_child(_status_row_container)

	_log = RichTextLabel.new()
	_log.bbcode_enabled = true
	_log.scroll_following = true
	_log.size_flags_vertical = Control.SIZE_EXPAND_FILL
	_log.custom_minimum_size = Vector2(0, 220)
	root_box.add_child(_log)

	_command_box = HBoxContainer.new()
	_command_box.add_theme_constant_override("separation", 8)
	root_box.add_child(_command_box)

	_target_box = HBoxContainer.new()
	_target_box.add_theme_constant_override("separation", 8)
	root_box.add_child(_target_box)

	_result_label = Label.new()
	_result_label.add_theme_font_size_override("font_size", 28)
	_result_label.visible = false
	root_box.add_child(_result_label)

	# Added last so floating numbers draw above every card; it ignores the
	# mouse so it never steals a click meant for a button underneath.
	_float_layer = Control.new()
	_float_layer.anchor_right = 1.0
	_float_layer.anchor_bottom = 1.0
	_float_layer.mouse_filter = Control.MOUSE_FILTER_IGNORE
	add_child(_float_layer)


func _log_line(text: String) -> void:
	_log.append_text(text + "\n")


func _status_name(kind: String) -> String:
	return kind.capitalize()


func _name(actor_value) -> String:
	var index: int = int(actor_value)
	var combatants: Array = _state().get("combatants", [])
	if index < 0 or index >= combatants.size():
		return "?"
	return String(combatants[index].get("name", "?"))


func _pretty_names(ids: Array) -> String:
	var names: Array = []
	for id in ids:
		var text: String = String(id)
		var dot: int = text.find(".")
		names.append(text.substr(dot + 1) if dot >= 0 else text)
	return ", ".join(names)


func _load_json(path: String) -> Variant:
	var file := FileAccess.open(path, FileAccess.READ)
	if file == null:
		push_error("failed to open %s" % path)
		return null
	return JSON.parse_string(file.get_as_text())


func _call_bridge(method: String, args: Array) -> Dictionary:
	var raw: String = _session.callv(method, args)
	var parsed: Variant = JSON.parse_string(raw)
	if parsed is Dictionary:
		return parsed
	return {"ok": false, "error": "bridge returned an unparsable payload: %s" % raw}


func _state() -> Dictionary:
	var parsed: Variant = JSON.parse_string(_session.state_json())
	return parsed if parsed is Dictionary else {}


func _is_party_member(actor: int) -> bool:
	var combatants: Array = _state().get("combatants", [])
	if actor < 0 or actor >= combatants.size():
		return false
	return String(combatants[actor].get("team", "")) == "party"
