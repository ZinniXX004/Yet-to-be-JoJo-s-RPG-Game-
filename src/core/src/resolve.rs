//! Turns a [`Command`] (intent) into [`Event`]s (facts).
//!
//! This is the only module allowed to mutate combatant vitals. Keeping mutation
//! in one place is what makes the event log trustworthy: if something changed
//! and no event was emitted, the bug is here and nowhere else.

use crate::command::Command;
use crate::data::{Database, Effect, Element, SkillDef, StatusKind, TargetKind};
use crate::event::Event;
use crate::state::{BattleState, TEMPO_THRESHOLD};

/// Ids the engine looks up for built-in commands. They live in `data/` so their
/// numbers stay tunable without a recompile; the fallbacks below only exist so
/// that a stripped-down data set can still run.
pub const BASIC_ATTACK_ID: &str = "skill.strike";
pub const GUARD_ID: &str = "skill.guard_stance";

fn fallback_attack() -> SkillDef {
    SkillDef {
        id: BASIC_ATTACK_ID.to_string(),
        name: "Strike".to_string(),
        description: "Built-in fallback basic attack.".to_string(),
        sp_cost: 0,
        target: TargetKind::OneEnemy,
        accuracy: 95,
        tempo_cost: TEMPO_THRESHOLD,
        effects: vec![Effect::Damage {
            power: 100,
            element: Element::Physical,
            variance: 8,
        }],
    }
}

fn fallback_guard() -> SkillDef {
    SkillDef {
        id: GUARD_ID.to_string(),
        name: "Guard".to_string(),
        description: "Built-in fallback guard.".to_string(),
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
    }
}

/// Validates and applies a command. Returns the tempo cost to charge the actor.
///
/// On `Err` nothing has been mutated in a way that ends the turn, so the caller
/// may surface the message and ask for another command. This is why validation
/// (targets, SP) happens strictly before any mutation.
pub fn resolve_command(
    db: &Database,
    st: &mut BattleState,
    actor: usize,
    cmd: &Command,
    log: &mut Vec<Event>,
) -> Result<u32, String> {
    if st.get(actor).is_none() {
        return Err(format!("actor index {actor} is out of range"));
    }
    if !st.combatants[actor].alive() {
        return Err(format!("{} is down and cannot act", st.combatants[actor].name));
    }

    let skill: SkillDef = match cmd {
        Command::Wait => {
            log.push(Event::Waited { actor });
            return Ok(TEMPO_THRESHOLD / 2);
        }
        Command::Attack { .. } => db
            .skill(BASIC_ATTACK_ID)
            .cloned()
            .unwrap_or_else(fallback_attack),
        Command::Guard => db.skill(GUARD_ID).cloned().unwrap_or_else(fallback_guard),
        Command::Skill { skill, .. } => {
            if !st.combatants[actor].skills.iter().any(|known| known == skill) {
                return Err(format!(
                    "{} does not know skill '{skill}'",
                    st.combatants[actor].name
                ));
            }
            db.skill(skill)
                .cloned()
                .ok_or_else(|| format!("unknown skill '{skill}'"))?
        }
    };

    let requested = match cmd {
        Command::Attack { target } | Command::Skill { target, .. } => *target,
        _ => actor,
    };

    let targets = resolve_targets(st, actor, skill.target, requested)?;
    if targets.is_empty() {
        return Err(format!("'{}' has no legal target", skill.name));
    }
    if st.combatants[actor].sp < skill.sp_cost {
        return Err(format!(
            "{} needs {} SP for '{}' but has {}",
            st.combatants[actor].name, skill.sp_cost, skill.name, st.combatants[actor].sp
        ));
    }

    // --- past this point, mutation is committed ---
    if skill.sp_cost > 0 {
        st.combatants[actor].sp -= skill.sp_cost;
        log.push(Event::SpConsumed {
            actor,
            amount: skill.sp_cost,
        });
    }
    log.push(Event::ActionUsed {
        actor,
        skill: skill.id.clone(),
        name: skill.name.clone(),
    });

    for target in targets {
        if skill.target.is_hostile() && !st.rng.chance(skill.accuracy) {
            log.push(Event::Missed { actor, target });
            continue;
        }
        for effect in &skill.effects {
            apply_effect(st, actor, target, *effect, log);
        }
    }

    Ok(skill.tempo_cost.max(1))
}

/// Expands a target kind into concrete indices, rejecting illegal single
/// targets rather than silently redirecting them. Silent redirection hides UI
/// bugs; an error surfaces them.
pub fn resolve_targets(
    st: &BattleState,
    actor: usize,
    kind: TargetKind,
    requested: usize,
) -> Result<Vec<usize>, String> {
    let team = st.combatants[actor].team;
    match kind {
        TargetKind::SelfOnly => Ok(vec![actor]),
        TargetKind::AllEnemies => Ok(st.alive_on(team.opposite())),
        TargetKind::AllAllies => Ok(st.alive_on(team)),
        TargetKind::OneEnemy => single(st, requested, team.opposite()),
        TargetKind::OneAlly => single(st, requested, team),
    }
}

fn single(st: &BattleState, requested: usize, expected_team: crate::data::Team) -> Result<Vec<usize>, String> {
    match st.get(requested) {
        None => Err(format!("target index {requested} is out of range")),
        Some(target) if !target.alive() => Err(format!("{} is already down", target.name)),
        Some(target) if target.team != expected_team => {
            Err(format!("{} is not a legal target for this skill", target.name))
        }
        Some(_) => Ok(vec![requested]),
    }
}

fn apply_effect(
    st: &mut BattleState,
    actor: usize,
    target: usize,
    effect: Effect,
    log: &mut Vec<Event>,
) {
    match effect {
        Effect::Damage {
            power,
            element,
            variance,
        } => {
            let (amount, crit) = compute_damage(st, actor, target, power, variance);
            deal_flat_damage(st, actor, target, amount, element, crit, log);
        }
        Effect::Drain { power, element } => {
            let (amount, crit) = compute_damage(st, actor, target, power, 6);
            deal_flat_damage(st, actor, target, amount, element, crit, log);
            heal(st, actor, amount / 2, log);
        }
        Effect::Heal { power } => heal(st, target, power, log),
        Effect::Status {
            status,
            potency,
            duration,
            chance,
        } => {
            if !st.combatants[target].alive() {
                return;
            }
            // Will reduces incoming status chance. Capped so a high-will boss is
            // resistant, never immune.
            let resist = (st.combatants[target].will() / 8).clamp(0, 30);
            let effective = (chance - resist).max(5);
            if st.rng.chance(effective) {
                st.combatants[target].apply_status(status, potency, duration.max(1));
                log.push(Event::StatusApplied {
                    target,
                    status,
                    potency,
                    duration: duration.max(1),
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
            combatant.tempo_lock = combatant.tempo_lock.max(ticks);
            log.push(Event::TempoLocked { target, ticks });
        }
    }
}

/// See docs/ARCHITECTURE.md for the formula and why the 1-damage floor exists.
fn compute_damage(
    st: &mut BattleState,
    actor: usize,
    target: usize,
    power: i32,
    variance: i32,
) -> (i32, bool) {
    let atk = st.combatants[actor].atk();
    let def = st.combatants[target].def();
    let actor_will = st.combatants[actor].will();
    let target_will = st.combatants[target].will();

    let base = power * atk / 100;
    let mut amount = (base - def / 2).max(1);

    if variance > 0 {
        let jitter = st.rng.range_i32(100 - variance, 100 + variance);
        amount = (amount * jitter / 100).max(1);
    }

    let crit_chance = (5 + (actor_will - target_will) / 2).clamp(1, 50);
    let crit = st.rng.chance(crit_chance);
    if crit {
        amount = (amount * 3 / 2).max(1);
    }

    (amount, crit)
}

/// Applies already-computed damage. Also used for damage-over-time, which must
/// not re-roll crits or variance.
pub fn deal_flat_damage(
    st: &mut BattleState,
    actor: usize,
    target: usize,
    amount: i32,
    element: Element,
    crit: bool,
    log: &mut Vec<Event>,
) {
    if amount <= 0 {
        return;
    }
    let combatant = &mut st.combatants[target];
    if !combatant.alive() {
        return;
    }
    combatant.hp = (combatant.hp - amount).max(0);
    let downed = combatant.hp == 0;
    log.push(Event::Damaged {
        actor,
        target,
        amount,
        crit,
        element,
    });
    if downed {
        log.push(Event::Downed { target });
    }
}

pub fn heal(st: &mut BattleState, target: usize, amount: i32, log: &mut Vec<Event>) {
    if amount <= 0 {
        return;
    }
    let combatant = &mut st.combatants[target];
    // Deliberate design rule: healing never revives. Revival needs its own
    // effect so that AI and UI can reason about it explicitly.
    if !combatant.alive() {
        return;
    }
    let before = combatant.hp;
    combatant.hp = (combatant.hp + amount).min(combatant.max_hp);
    let restored = combatant.hp - before;
    if restored > 0 {
        log.push(Event::Healed {
            target,
            amount: restored,
        });
    }
}
