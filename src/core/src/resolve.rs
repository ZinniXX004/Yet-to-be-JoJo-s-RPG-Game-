//! Turns a [`Command`] into [`Event`]s.
//!
//! Contract, in order:
//!
//! 1. Validate everything (actor alive, skill known, SP available, target legal)
//!    **before** mutating anything. A rejected command must leave the state
//!    untouched and the turn unspent, so the UI can explain and re-prompt.
//! 2. Mutate, emitting one event per observable fact.
//! 3. Return the tempo cost actually consumed.
//!
//! There is no `match` on a specific skill id anywhere below. Basic attack and
//! guard are ordinary data entries; the only thing the code knows is their id.

use crate::command::Command;
use crate::data::{
    Database, Effect, Element, SkillDef, StatusKind, TargetKind, MAX_RESISTANCE, MIN_RESISTANCE,
};
use crate::event::Event;
use crate::state::{BattleState, TEMPO_THRESHOLD};

/// Content ids the engine looks up by name. These two are the only ids the code
/// knows about, and both have code fallbacks so a missing entry degrades into a
/// sane default instead of a panic.
pub const BASIC_ATTACK_ID: &str = "skill.strike";
pub const GUARD_ID: &str = "skill.guard_stance";

/// Used only if `data/skills.json` is missing the basic attack. The validator
/// should catch that first; this exists so that a content mistake cannot make
/// the game unplayable.
fn fallback_attack() -> SkillDef {
    SkillDef {
        id: BASIC_ATTACK_ID.to_string(),
        name: "Strike".to_string(),
        description: "Fallback basic attack.".to_string(),
        sp_cost: 0,
        target: TargetKind::OneEnemy,
        accuracy: 95,
        tempo_cost: TEMPO_THRESHOLD,
        effects: vec![Effect::Damage {
            power: 100,
            element: Element::Physical,
            variance: 10,
        }],
        provenance: None,
    }
}

fn fallback_guard() -> SkillDef {
    SkillDef {
        id: GUARD_ID.to_string(),
        name: "Guard".to_string(),
        description: "Fallback defensive stance.".to_string(),
        sp_cost: 0,
        target: TargetKind::SelfOnly,
        accuracy: 100,
        tempo_cost: TEMPO_THRESHOLD * 3 / 5,
        effects: vec![Effect::Status {
            status: StatusKind::DefUp,
            potency: 50,
            duration: 2,
            chance: 100,
        }],
        provenance: None,
    }
}

/// Resolves one command and returns the tempo cost to charge the actor.
pub fn resolve_command(
    db: &Database,
    st: &mut BattleState,
    actor: usize,
    command: &Command,
    log: &mut Vec<Event>,
) -> Result<u32, String> {
    match st.get(actor) {
        Some(combatant) if combatant.alive() => {}
        Some(combatant) => return Err(format!("{} is down and cannot act", combatant.name)),
        None => return Err(format!("no combatant at index {actor}")),
    }

    // Waiting is deliberately free of validation: it is the escape hatch that
    // guarantees an actor always has a legal move, so the scheduler cannot
    // deadlock.
    let (skill, requested): (SkillDef, Option<usize>) = match command {
        Command::Wait => {
            log.push(Event::Waited { actor });
            return Ok(TEMPO_THRESHOLD / 2);
        }
        Command::Attack { target } => (
            db.skill(BASIC_ATTACK_ID)
                .cloned()
                .unwrap_or_else(fallback_attack),
            Some(*target),
        ),
        Command::Guard => (
            db.skill(GUARD_ID).cloned().unwrap_or_else(fallback_guard),
            None,
        ),
        Command::Skill { skill, target } => {
            let def = db
                .skill(skill)
                .ok_or_else(|| format!("unknown skill '{skill}'"))?;
            if !st.combatants[actor].skills.iter().any(|id| id == skill) {
                return Err(format!(
                    "{} does not know '{skill}'",
                    st.combatants[actor].name
                ));
            }
            (def.clone(), Some(*target))
        }
    };

    if st.combatants[actor].sp < skill.sp_cost {
        return Err(format!(
            "not enough SP for '{}': {} required, {} available",
            skill.name, skill.sp_cost, st.combatants[actor].sp
        ));
    }

    // Target resolution is the last thing that can fail. After this point the
    // action is committed.
    let targets = resolve_targets(st, actor, skill.target, requested)?;

    log.push(Event::ActionUsed {
        actor,
        skill: skill.id.clone(),
        name: skill.name.clone(),
    });

    if skill.sp_cost > 0 {
        st.combatants[actor].sp -= skill.sp_cost;
        log.push(Event::SpConsumed {
            actor,
            amount: skill.sp_cost,
        });
    }

    for target in targets {
        // Accuracy is rolled per target, so a multi-target skill can partially
        // miss. Friendly effects never roll; see TargetKind::is_hostile.
        if skill.target.is_hostile() && !st.rng.chance(skill.accuracy) {
            log.push(Event::Missed { actor, target });
            continue;
        }
        for effect in &skill.effects {
            apply_effect(st, actor, target, effect, log);
        }
    }

    Ok(skill.tempo_cost.max(1))
}

/// Expands a target kind into concrete indices.
///
/// A `None` request on a single-target skill falls back to the first legal
/// target instead of erroring: AI and scripted actions should not be able to
/// waste a turn on an omitted field.
fn resolve_targets(
    st: &BattleState,
    actor: usize,
    kind: TargetKind,
    requested: Option<usize>,
) -> Result<Vec<usize>, String> {
    let team = st.combatants[actor].team;
    let single_team = match kind {
        TargetKind::SelfOnly => return Ok(vec![actor]),
        TargetKind::AllEnemies => return Ok(st.alive_on(team.opposite())),
        TargetKind::AllAllies => return Ok(st.alive_on(team)),
        TargetKind::OneEnemy => team.opposite(),
        TargetKind::OneAlly => team,
    };

    let candidates = st.alive_on(single_team);
    if candidates.is_empty() {
        return Err("no legal target remains".to_string());
    }
    match requested {
        Some(index) if candidates.contains(&index) => Ok(vec![index]),
        Some(index) => Err(format!(
            "index {index} is not a legal target for this action"
        )),
        None => Ok(vec![candidates[0]]),
    }
}

fn apply_effect(
    st: &mut BattleState,
    actor: usize,
    target: usize,
    effect: &Effect,
    log: &mut Vec<Event>,
) {
    match *effect {
        Effect::Damage {
            power,
            element,
            variance,
        } => {
            let (amount, crit) = compute_damage(st, actor, target, power, element, variance);
            deal_damage(st, actor, target, amount, crit, element, log);
        }
        Effect::Drain { power, element } => {
            let (amount, crit) = compute_damage(st, actor, target, power, element, 0);
            let dealt = deal_damage(st, actor, target, amount, crit, element, log);
            // Recovery is based on damage *actually dealt*, not on the rolled
            // amount, so overkill does not become a healing exploit.
            let recovered = heal(st, actor, dealt / 2);
            if recovered > 0 {
                // A self-heal: the drainer is both the healer and the healed.
                // Both fields name the same index deliberately -- it is one
                // heal performed by one combatant, and a report must credit it
                // once, not once per field.
                log.push(Event::Healed {
                    actor,
                    target: actor,
                    amount: recovered,
                });
            }
        }
        Effect::Heal { power } => {
            // Healing scales on the healer's WILL so that support characters
            // have a stat to invest in.
            let will = st.combatants[actor].will();
            let amount = (power * (100 + will) / 100).max(1);
            let healed = heal(st, target, amount);
            if healed > 0 {
                log.push(Event::Healed {
                    actor,
                    target,
                    amount: healed,
                });
            }
        }
        Effect::Status {
            status,
            potency,
            duration,
            chance,
        } => {
            if !st.combatants[target].alive() {
                return;
            }
            // Hostile statuses are resisted by WILL; buffs on your own side are
            // not, because a buff that randomly fails is only frustrating.
            let hostile = st.combatants[target].team != st.combatants[actor].team;
            let effective = if hostile {
                (chance - st.combatants[target].will() / 2).clamp(5, 100)
            } else {
                chance
            };
            if st.rng.chance(effective) {
                st.combatants[target].apply_status(status, potency, duration);
                log.push(Event::StatusApplied {
                    target,
                    status,
                    potency,
                    duration,
                });
            } else {
                log.push(Event::StatusResisted { target, status });
            }
        }
        Effect::TempoLock { ticks } => {
            if !st.combatants[target].alive() {
                return;
            }
            let combatant = &mut st.combatants[target];
            // Take the longer lock rather than summing, so chaining the ability
            // cannot produce an unbounded freeze.
            combatant.tempo_lock = combatant.tempo_lock.max(ticks);
            log.push(Event::TempoLocked { target, ticks });
        }
    }
}

/// Integer damage model. See docs/ARCHITECTURE.md for the rationale.
///
/// Order of operations, and it is load-bearing: power against attack, minus half
/// of defence, then the target's resistance to the element, then variance, then
/// crit. Resistance sits after defence so that it scales the damage that got
/// through armour rather than the number the skill declared, and before variance
/// so that the jitter is jitter on what the target really takes.
///
/// Stats are copied into locals before touching the RNG: the RNG lives inside
/// `BattleState`, so holding a borrow on a combatant across the roll would not
/// borrow-check.
fn compute_damage(
    st: &mut BattleState,
    actor: usize,
    target: usize,
    power: i32,
    element: Element,
    variance: i32,
) -> (i32, bool) {
    let atk = st.combatants[actor].atk();
    let will_actor = st.combatants[actor].will();
    let def = st.combatants[target].def();
    let will_target = st.combatants[target].will();
    // Clamped here as well as in the validator: content can reach this function
    // through the bridge without passing the Python validator at all.
    let resist = st.combatants[target]
        .resistance(element)
        .clamp(MIN_RESISTANCE, MAX_RESISTANCE);

    let base = power * atk / 100;
    // Floor at 1: no amount of defence makes a target immune, which keeps a
    // fight from stalling into an unwinnable state.
    let mut damage = (base - def / 2).max(1);

    if resist != 0 {
        // The floor is reapplied rather than skipped: at 100 the modifier is
        // gone, the hit is not.
        damage = (damage * (100 - resist) / 100).max(1);
    }

    if variance > 0 {
        let variance = variance.min(99);
        let jitter = st.rng.range_i32(100 - variance, 100 + variance);
        damage = (damage * jitter / 100).max(1);
    }

    let crit_chance = (5 + (will_actor - will_target) / 2).clamp(1, 50);
    let crit = st.rng.chance(crit_chance);
    if crit {
        damage = (damage * 3 / 2).max(1);
    }

    (damage, crit)
}

/// Subtracts HP without emitting anything, returning the amount actually lost.
///
/// `None` means the target was already down. Both damage paths share this so
/// that HP can never go negative in one of them and not the other.
fn subtract_hp(st: &mut BattleState, target: usize, amount: i32) -> Option<i32> {
    if !st.combatants[target].alive() {
        return None;
    }
    let before = st.combatants[target].hp;
    let lost = amount.clamp(0, before);
    st.combatants[target].hp = before - lost;
    Some(lost)
}

/// Emits [`Event::Downed`] if the target just reached zero HP.
///
/// Shared by every damage path on purpose: a lethal bleed must end the battle
/// exactly like a lethal sword blow, and duplicating this check is how one of
/// them eventually stops doing it.
fn note_if_downed(st: &BattleState, target: usize, log: &mut Vec<Event>) {
    if st.combatants[target].hp == 0 {
        log.push(Event::Downed { target });
    }
}

/// Applies damage from an actor's action and returns the amount actually dealt
/// (never more than the target's remaining HP).
pub(crate) fn deal_damage(
    st: &mut BattleState,
    actor: usize,
    target: usize,
    amount: i32,
    crit: bool,
    element: Element,
    log: &mut Vec<Event>,
) -> i32 {
    let Some(dealt) = subtract_hp(st, target, amount) else {
        return 0;
    };

    log.push(Event::Damaged {
        actor,
        target,
        amount: dealt,
        crit,
        element,
    });
    note_if_downed(st, target, log);
    dealt
}

/// Applies damage caused by a status the target is carrying, such as bleed.
///
/// Deliberately separate from [`deal_damage`]: there is no attacker and no
/// element, so passing the victim's own index as the actor would emit a log
/// entry claiming the character attacked itself. See [`Event::StatusDamaged`].
pub(crate) fn status_damage(
    st: &mut BattleState,
    target: usize,
    status: StatusKind,
    amount: i32,
    log: &mut Vec<Event>,
) -> i32 {
    let Some(dealt) = subtract_hp(st, target, amount) else {
        return 0;
    };

    log.push(Event::StatusDamaged {
        target,
        status,
        amount: dealt,
    });
    note_if_downed(st, target, log);
    dealt
}

/// Restores HP and returns the amount actually restored. Healing never revives:
/// resurrection must be an explicit effect, not an accident of ordering.
///
/// Emits nothing. The caller knows whether the recovery came from an action or
/// from a status, and those are two different events; deciding that here would
/// mean this function had to guess.
pub(crate) fn heal(st: &mut BattleState, target: usize, amount: i32) -> i32 {
    let combatant = &mut st.combatants[target];
    if !combatant.alive() || amount <= 0 {
        return 0;
    }
    let before = combatant.hp;
    combatant.hp = (before + amount).min(combatant.max_hp);
    combatant.hp - before
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::{AiProfile, CombatantDef, Resistances, Stats, Team};

    /// A zero-variance attack at perfect accuracy: the only roll left in the
    /// pipeline is the crit, and that one lands or not identically in every run
    /// below because the seed and the roll order are the same.
    fn attack(id: &str, element: Element) -> SkillDef {
        SkillDef {
            id: id.to_string(),
            name: id.to_string(),
            description: String::new(),
            sp_cost: 0,
            target: TargetKind::OneEnemy,
            accuracy: 100,
            tempo_cost: TEMPO_THRESHOLD,
            effects: vec![Effect::Damage {
                power: 100,
                element,
                variance: 0,
            }],
            provenance: None,
        }
    }

    fn fighter(id: &str, team: Team, resist: Resistances, skills: &[&str]) -> CombatantDef {
        CombatantDef {
            id: id.to_string(),
            name: id.to_string(),
            team,
            // atk 100 and def 0 make the pre-resistance damage exactly the
            // skill's power, so the assertions below are about one variable.
            stats: Stats {
                hp: 1000,
                sp: 0,
                atk: 100,
                def: 0,
                spd: 10,
                will: 0,
            },
            stand: None,
            skills: skills.iter().map(|id| id.to_string()).collect(),
            ai: AiProfile::Aggressive,
            resist,
            provenance: None,
        }
    }

    fn table(element: Element, percent: i32) -> Resistances {
        let mut resist = Resistances::new();
        resist.insert(element, percent);
        resist
    }

    /// Resolves one hit against a defender carrying `resist` and returns the
    /// damage the emitted event reports.
    fn hit(skill: &str, resist: Resistances) -> i32 {
        let db = Database::new(
            Vec::new(),
            vec![
                attack("skill.flame", Element::Fire),
                attack("skill.jab", Element::Physical),
            ],
            vec![
                fighter(
                    "pc.attacker",
                    Team::Party,
                    Resistances::new(),
                    &["skill.flame", "skill.jab"],
                ),
                fighter("npc.target", Team::Foe, resist, &["skill.jab"]),
            ],
        )
        .expect("the test content is valid");

        let party = vec!["pc.attacker".to_string()];
        let foes = vec!["npc.target".to_string()];
        let mut st =
            BattleState::build(&db, &party, &foes, 7).expect("the test battle instantiates");
        let mut log: Vec<Event> = Vec::new();
        let command = Command::Skill {
            skill: skill.to_string(),
            target: 1,
        };
        resolve_command(&db, &mut st, 0, &command, &mut log).expect("the command is legal");

        log.iter()
            .find_map(|event| match event {
                Event::Damaged { amount, .. } => Some(*amount),
                _ => None,
            })
            .expect("an attack at 100 accuracy always lands")
    }

    #[test]
    fn a_resisted_element_lands_for_less_and_a_vulnerable_one_for_more() {
        let neutral = hit("skill.flame", Resistances::new());
        // Ratios rather than literals: the crit roll is identical across the
        // three runs, so a literal would be asserting the roll as much as the
        // resistance, and would have to change if the seed ever did.
        assert_eq!(
            hit("skill.flame", table(Element::Fire, 50)) * 2,
            neutral,
            "50 should halve the hit"
        );
        assert_eq!(
            hit("skill.flame", table(Element::Fire, -50)) * 2,
            neutral * 3,
            "-50 should make the hit half again as strong"
        );
    }

    #[test]
    fn total_resistance_removes_the_modifier_not_the_hit() {
        assert_eq!(hit("skill.flame", table(Element::Fire, 100)), 1);
    }

    #[test]
    fn resistance_applies_only_to_the_element_that_was_used() {
        let neutral = hit("skill.jab", Resistances::new());
        assert_eq!(hit("skill.jab", table(Element::Fire, 75)), neutral);
    }

    /// A support skill and a drain both name a healer, and the drain names the
    /// same combatant twice on purpose. The report layer relies on that: it
    /// credits `actor`, so a self-heal must arrive as one event with one actor
    /// rather than as something a reader has to de-duplicate.
    #[test]
    fn a_heal_names_the_combatant_that_performed_it() {
        fn mender(id: &str, team: Team, skills: &[&str]) -> CombatantDef {
            let mut def = fighter(id, team, Resistances::new(), skills);
            def.stats.will = 0;
            def
        }

        let restore = SkillDef {
            id: "skill.mend".to_string(),
            name: "Mend".to_string(),
            description: String::new(),
            sp_cost: 0,
            target: TargetKind::OneAlly,
            accuracy: 100,
            tempo_cost: TEMPO_THRESHOLD,
            effects: vec![Effect::Heal { power: 50 }],
            provenance: None,
        };
        let siphon = SkillDef {
            id: "skill.siphon".to_string(),
            name: "Siphon".to_string(),
            description: String::new(),
            sp_cost: 0,
            target: TargetKind::OneEnemy,
            accuracy: 100,
            tempo_cost: TEMPO_THRESHOLD,
            effects: vec![Effect::Drain {
                power: 100,
                element: Element::Physical,
            }],
            provenance: None,
        };

        let db = Database::new(
            Vec::new(),
            vec![restore, siphon],
            vec![
                mender("pc.healer", Team::Party, &["skill.mend", "skill.siphon"]),
                mender("pc.ally", Team::Party, &["skill.mend"]),
                mender("npc.victim", Team::Foe, &["skill.mend"]),
            ],
        )
        .expect("the test content is valid");

        let party = vec!["pc.healer".to_string(), "pc.ally".to_string()];
        let foes = vec!["npc.victim".to_string()];
        let mut st =
            BattleState::build(&db, &party, &foes, 7).expect("the test battle instantiates");

        // Wound the ally and the healer, so both heals have room to land.
        st.combatants[0].hp = 500;
        st.combatants[1].hp = 500;

        let mut log: Vec<Event> = Vec::new();
        resolve_command(
            &db,
            &mut st,
            0,
            &Command::Skill {
                skill: "skill.mend".to_string(),
                target: 1,
            },
            &mut log,
        )
        .expect("mending a wounded ally is legal");

        let mended = log
            .iter()
            .find_map(|event| match event {
                Event::Healed {
                    actor,
                    target,
                    amount,
                } => Some((*actor, *target, *amount)),
                _ => None,
            })
            .expect("a heal on a wounded ally must land");
        assert_eq!(mended.0, 0, "the caster is the healer");
        assert_eq!(mended.1, 1, "the ally is the one healed");
        assert!(mended.2 > 0);

        let mut log: Vec<Event> = Vec::new();
        resolve_command(
            &db,
            &mut st,
            0,
            &Command::Skill {
                skill: "skill.siphon".to_string(),
                target: 2,
            },
            &mut log,
        )
        .expect("draining a living foe is legal");

        let drained = log
            .iter()
            .find_map(|event| match event {
                Event::Healed { actor, target, .. } => Some((*actor, *target)),
                _ => None,
            })
            .expect("a drain against a healthy foe recovers something");
        assert_eq!(
            drained,
            (0, 0),
            "a self-heal names the same combatant as healer and healed"
        );
    }

    fn skill(id: &str, target: TargetKind, sp_cost: i32, effects: Vec<Effect>) -> SkillDef {
        SkillDef {
            id: id.to_string(),
            name: id.to_string(),
            description: String::new(),
            sp_cost,
            target,
            accuracy: 100,
            tempo_cost: TEMPO_THRESHOLD,
            effects,
            provenance: None,
        }
    }

    /// One party fighter (index 0) against one foe (index 1), both knowing
    /// every skill in `skills`, with the stats of [`fighter`], on seed 7.
    fn duel(skills: Vec<SkillDef>) -> (Database, BattleState) {
        let ids: Vec<String> = skills.iter().map(|def| def.id.clone()).collect();
        let known: Vec<&str> = ids.iter().map(String::as_str).collect();
        let db = Database::new(
            Vec::new(),
            skills,
            vec![
                fighter("pc.a", Team::Party, Resistances::new(), &known),
                fighter("npc.b", Team::Foe, Resistances::new(), &known),
            ],
        )
        .expect("the test content is valid");
        let party = vec!["pc.a".to_string()];
        let foes = vec!["npc.b".to_string()];
        let st = BattleState::build(&db, &party, &foes, 7).expect("the test battle instantiates");
        (db, st)
    }

    fn act(db: &Database, st: &mut BattleState, command: &Command) -> (u32, Vec<Event>) {
        let mut log: Vec<Event> = Vec::new();
        let cost = resolve_command(db, st, 0, command, &mut log).expect("the command is legal");
        (cost, log)
    }

    fn cast(skill: &str, target: usize) -> Command {
        Command::Skill {
            skill: skill.to_string(),
            target,
        }
    }

    fn damage(power: i32, variance: i32) -> Effect {
        Effect::Damage {
            power,
            element: Element::Physical,
            variance,
        }
    }

    #[test]
    fn waiting_costs_half_a_turn_and_is_logged() {
        let (db, mut st) = duel(vec![attack("skill.jab", Element::Physical)]);
        let (cost, log) = act(&db, &mut st, &Command::Wait);
        assert_eq!(cost, TEMPO_THRESHOLD / 2);
        assert_eq!(cost, 500);
        assert!(matches!(log.as_slice(), [Event::Waited { actor: 0 }]));
    }

    #[test]
    fn a_downed_actor_cannot_act_even_to_wait() {
        let (db, mut st) = duel(vec![attack("skill.jab", Element::Physical)]);
        st.combatants[0].hp = 0;
        let mut log: Vec<Event> = Vec::new();
        let refused = resolve_command(&db, &mut st, 0, &Command::Wait, &mut log);
        assert!(refused
            .expect_err("a downed actor must be refused")
            .contains("is down"));
        assert!(log.is_empty());
    }

    /// Content without `skill.guard` still guards: the built-in stance costs
    /// three fifths of a turn and raises defence by half for two turns.
    #[test]
    fn guarding_without_a_guard_skill_uses_the_built_in_stance() {
        let (db, mut st) = duel(vec![attack("skill.jab", Element::Physical)]);
        assert!(
            db.skill(GUARD_ID).is_none(),
            "guard: the fallback must be in use"
        );
        let (cost, _) = act(&db, &mut st, &Command::Guard);
        assert_eq!(cost, 600);
        assert_eq!(
            st.combatants[0].statuses,
            vec![crate::state::Status {
                kind: StatusKind::DefUp,
                potency: 50,
                remaining: 2,
            }]
        );
    }

    #[test]
    fn a_free_skill_spends_no_sp_and_logs_no_spending() {
        let paid = skill("skill.paid", TargetKind::OneEnemy, 10, vec![damage(100, 0)]);
        let free = skill("skill.free", TargetKind::OneEnemy, 0, vec![damage(100, 0)]);
        let (db, mut st) = duel(vec![paid, free]);
        st.combatants[0].sp = 30;

        let (_, log) = act(&db, &mut st, &cast("skill.free", 1));
        assert!(!log.iter().any(|e| matches!(e, Event::SpConsumed { .. })));
        assert_eq!(st.combatants[0].sp, 30);

        let (_, log) = act(&db, &mut st, &cast("skill.paid", 1));
        assert!(log.iter().any(|e| matches!(
            e,
            Event::SpConsumed {
                actor: 0,
                amount: 10
            }
        )));
        assert_eq!(st.combatants[0].sp, 20);
    }

    #[test]
    fn a_tempo_lock_lands_on_a_living_target_and_keeps_the_longer_lock() {
        let lock = skill(
            "skill.lock",
            TargetKind::OneEnemy,
            0,
            vec![Effect::TempoLock { ticks: 6 }],
        );
        let (db, mut st) = duel(vec![lock]);

        let (_, log) = act(&db, &mut st, &cast("skill.lock", 1));
        assert_eq!(st.combatants[1].tempo_lock, 6);
        assert!(log.iter().any(|e| matches!(
            e,
            Event::TempoLocked {
                target: 1,
                ticks: 6
            }
        )));

        st.combatants[1].tempo_lock = 10;
        act(&db, &mut st, &cast("skill.lock", 1));
        assert_eq!(st.combatants[1].tempo_lock, 10, "locks do not sum");
    }

    #[test]
    fn a_drain_returns_half_the_damage_actually_dealt() {
        let siphon = skill(
            "skill.siphon",
            TargetKind::OneEnemy,
            0,
            vec![Effect::Drain {
                power: 100,
                element: Element::Physical,
            }],
        );
        let (db, mut st) = duel(vec![siphon]);
        st.combatants[0].hp = 500;

        let (_, log) = act(&db, &mut st, &cast("skill.siphon", 1));
        let dealt = log
            .iter()
            .find_map(|e| match e {
                Event::Damaged { amount, .. } => Some(*amount),
                _ => None,
            })
            .expect("a drain at 100 accuracy lands");
        assert!(dealt >= 2, "guard: half of the hit must be at least one");
        let healed: Vec<i32> = log
            .iter()
            .filter_map(|e| match e {
                Event::Healed {
                    actor: 0,
                    target: 0,
                    amount,
                } => Some(*amount),
                _ => None,
            })
            .collect();
        assert_eq!(healed, vec![dealt / 2]);
        assert_eq!(st.combatants[0].hp, 500 + dealt / 2);
    }

    #[test]
    fn a_heal_that_restores_nothing_is_not_logged() {
        let siphon = skill(
            "skill.siphon",
            TargetKind::OneEnemy,
            0,
            vec![Effect::Drain {
                power: 100,
                element: Element::Physical,
            }],
        );
        let mend = skill(
            "skill.mend",
            TargetKind::SelfOnly,
            0,
            vec![Effect::Heal { power: 50 }],
        );
        let (db, mut st) = duel(vec![siphon, mend]);

        let (_, log) = act(&db, &mut st, &cast("skill.siphon", 1));
        assert!(log.iter().any(|e| matches!(e, Event::Damaged { .. })));
        assert!(!log.iter().any(|e| matches!(e, Event::Healed { .. })));

        let (_, log) = act(&db, &mut st, &cast("skill.mend", 0));
        assert!(!log.iter().any(|e| matches!(e, Event::Healed { .. })));
    }

    #[test]
    fn healing_never_revives() {
        let (_, mut st) = duel(vec![attack("skill.jab", Element::Physical)]);
        st.combatants[1].hp = 0;
        assert_eq!(heal(&mut st, 1, 50), 0);
        assert_eq!(st.combatants[1].hp, 0);
        assert!(!st.combatants[1].alive());
    }

    #[test]
    fn only_a_lethal_hit_reports_the_target_downed() {
        let (db, mut st) = duel(vec![attack("skill.jab", Element::Physical)]);

        let (_, log) = act(&db, &mut st, &cast("skill.jab", 1));
        assert!(st.combatants[1].alive());
        assert!(!log.iter().any(|e| matches!(e, Event::Downed { .. })));

        st.combatants[1].hp = 5;
        let (_, log) = act(&db, &mut st, &cast("skill.jab", 1));
        let downed = log
            .iter()
            .filter(|e| matches!(e, Event::Downed { target: 1 }))
            .count();
        assert_eq!(downed, 1);
        assert!(!st.combatants[1].alive());
    }

    /// Variance 10 on a 100-damage hit draws every whole percent from 90 to
    /// 110 and nothing outside it, before any crit.
    #[test]
    fn variance_spreads_a_hit_over_its_whole_declared_range() {
        let (_, mut st) = duel(vec![attack("skill.jab", Element::Physical)]);
        let mut seen = std::collections::BTreeSet::new();
        for _ in 0..3_000 {
            let (amount, crit) = compute_damage(&mut st, 0, 1, 100, Element::Physical, 10);
            if crit {
                assert!((135..=165).contains(&amount), "crit {amount}");
            } else {
                assert!((90..=110).contains(&amount), "hit {amount}");
                seen.insert(amount);
            }
        }
        assert_eq!(seen.len(), 21, "every value from 90 to 110 occurs");
    }

    /// Crit chance is 5% plus half the actor's lead in will. Equal wills give
    /// 5% however high they are; a 60-point lead gives 35%.
    #[test]
    fn the_crit_chance_follows_the_difference_in_will() {
        let crits = |will_actor: i32, will_target: i32| {
            let (_, mut st) = duel(vec![attack("skill.jab", Element::Physical)]);
            st.combatants[0].base.will = will_actor;
            st.combatants[1].base.will = will_target;
            (0..4_000)
                .filter(|_| compute_damage(&mut st, 0, 1, 100, Element::Physical, 0).1)
                .count()
        };
        let even = crits(40, 40);
        assert!((120..=280).contains(&even), "5% of 4000, got {even}");
        let ahead = crits(60, 0);
        assert!((1_250..=1_550).contains(&ahead), "35% of 4000, got {ahead}");
    }
}
