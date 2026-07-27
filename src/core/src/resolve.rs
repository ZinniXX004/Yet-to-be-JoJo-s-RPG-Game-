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
use crate::data::{Database, Effect, Element, SkillDef, TargetKind};
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
            status: crate::data::StatusKind::DefUp,
            potency: 50,
            duration: 2,
            chance: 100,
        }],
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
            let (amount, crit) = compute_damage(st, actor, target, power, variance);
            deal_damage(st, actor, target, amount, crit, element, log);
        }
        Effect::Drain { power, element } => {
            let (amount, crit) = compute_damage(st, actor, target, power, 0);
            let dealt = deal_damage(st, actor, target, amount, crit, element, log);
            // Recovery is based on damage *actually dealt*, not on the rolled
            // amount, so overkill does not become a healing exploit.
            let recovered = heal(st, actor, dealt / 2);
            if recovered > 0 {
                log.push(Event::Healed {
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
/// Stats are copied into locals before touching the RNG: the RNG lives inside
/// `BattleState`, so holding a borrow on a combatant across the roll would not
/// borrow-check.
fn compute_damage(
    st: &mut BattleState,
    actor: usize,
    target: usize,
    power: i32,
    variance: i32,
) -> (i32, bool) {
    let atk = st.combatants[actor].atk();
    let will_actor = st.combatants[actor].will();
    let def = st.combatants[target].def();
    let will_target = st.combatants[target].will();

    let base = power * atk / 100;
    // Floor at 1: no amount of defence makes a target immune, which keeps a
    // fight from stalling into an unwinnable state.
    let mut damage = (base - def / 2).max(1);

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

/// Applies damage and returns the amount actually dealt (never more than the
/// target's remaining HP).
pub(crate) fn deal_damage(
    st: &mut BattleState,
    actor: usize,
    target: usize,
    amount: i32,
    crit: bool,
    element: Element,
    log: &mut Vec<Event>,
) -> i32 {
    if !st.combatants[target].alive() {
        return 0;
    }
    let before = st.combatants[target].hp;
    let dealt = amount.clamp(0, before);
    st.combatants[target].hp = before - dealt;

    log.push(Event::Damaged {
        actor,
        target,
        amount: dealt,
        crit,
        element,
    });
    if st.combatants[target].hp == 0 {
        log.push(Event::Downed { target });
    }
    dealt
}

/// Restores HP and returns the amount actually restored. Healing never revives:
/// resurrection must be an explicit effect, not an accident of ordering.
pub(crate) fn heal(st: &mut BattleState, target: usize, amount: i32) -> i32 {
    let combatant = &mut st.combatants[target];
    if !combatant.alive() || amount <= 0 {
        return 0;
    }
    let before = combatant.hp;
    combatant.hp = (before + amount).min(combatant.max_hp);
    combatant.hp - before
}
