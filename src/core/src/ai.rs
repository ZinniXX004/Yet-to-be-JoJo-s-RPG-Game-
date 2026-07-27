//! Enemy and auto-battle decision making.
//!
//! Kept intentionally simple and *deterministic*: it draws from the battle's own
//! RNG, so an AI-driven battle is as reproducible as a player-driven one. That
//! is what makes the balance harness in the roadmap possible.
//!
//! Tie-breaking always resolves to the lowest index (`min_by_key` returns the
//! first minimum). Never introduce hash-map iteration here.

use crate::command::Command;
use crate::data::{AiProfile, Database, Effect, Id, SkillDef, TargetKind};
use crate::state::BattleState;

pub fn choose(db: &Database, st: &mut BattleState, actor: usize) -> Command {
    let team = st.combatants[actor].team;
    let ai = st.combatants[actor].ai;
    let sp = st.combatants[actor].sp;
    let skills: Vec<Id> = st.combatants[actor].skills.clone();

    let enemies = st.alive_on(team.opposite());
    let allies = st.alive_on(team);
    if enemies.is_empty() {
        return Command::Wait;
    }

    // Support triage runs before offense: a dead healer target is worth more
    // than any damage this turn.
    if ai == AiProfile::Support {
        if let Some(&wounded) = allies
            .iter()
            .min_by_key(|&&index| health_percent(st, index))
        {
            if health_percent(st, wounded) < 55 {
                if let Some(skill) = find_skill(db, &skills, sp, |def| {
                    matches!(def.target, TargetKind::OneAlly | TargetKind::AllAllies)
                        && def
                            .effects
                            .iter()
                            .any(|effect| matches!(effect, Effect::Heal { .. }))
                }) {
                    return Command::Skill {
                        skill,
                        target: wounded,
                    };
                }
            }
        }
    }

    let offensive: Vec<Id> = skills
        .iter()
        .filter(|id| {
            db.skill(id)
                .map(|def| def.target.is_hostile() && def.sp_cost <= sp)
                .unwrap_or(false)
        })
        .cloned()
        .collect();

    // Focus fire the weakest target: removing a combatant removes its entire
    // future action economy, which beats spreading damage evenly.
    let focus = enemies
        .iter()
        .copied()
        .min_by_key(|&index| st.combatants[index].hp)
        .unwrap_or(enemies[0]);

    match ai {
        AiProfile::Trickster => {
            let target = st.rng.pick(&enemies).unwrap_or(focus);
            if !offensive.is_empty() && st.rng.chance(50) {
                let index = st.rng.below(offensive.len() as u32) as usize;
                return Command::Skill {
                    skill: offensive[index].clone(),
                    target,
                };
            }
            Command::Attack { target }
        }
        AiProfile::Aggressive | AiProfile::Support => {
            // Not always the biggest hit: holding SP back some of the time keeps
            // fights from being decided entirely in the first two turns.
            if !offensive.is_empty() && st.rng.chance(65) {
                let best = offensive
                    .iter()
                    .max_by_key(|id| db.skill(id).map(declared_power).unwrap_or(0))
                    .cloned();
                if let Some(skill) = best {
                    return Command::Skill {
                        skill,
                        target: focus,
                    };
                }
            }
            Command::Attack { target: focus }
        }
    }
}

fn health_percent(st: &BattleState, index: usize) -> i32 {
    let combatant = &st.combatants[index];
    combatant.hp * 100 / combatant.max_hp.max(1)
}

fn find_skill<F>(db: &Database, skills: &[Id], sp: i32, predicate: F) -> Option<Id>
where
    F: Fn(&SkillDef) -> bool,
{
    skills
        .iter()
        .find(|id| {
            db.skill(id)
                .map(|def| def.sp_cost <= sp && predicate(def))
                .unwrap_or(false)
        })
        .cloned()
}

/// Crude heuristic weight used only for AI ordering. Not a balance metric.
fn declared_power(def: &SkillDef) -> i32 {
    def.effects
        .iter()
        .map(|effect| match effect {
            Effect::Damage { power, .. } | Effect::Drain { power, .. } => *power,
            Effect::TempoLock { ticks } => *ticks as i32 * 30,
            Effect::Status { potency, .. } => *potency,
            Effect::Heal { .. } => 0,
        })
        .sum()
}
