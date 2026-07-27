//! Enemy and auto-battle decision making.
//!
//! The AI is intentionally shallow: three readable profiles, no search, no
//! scoring heuristics. Two reasons. First, in a party-scale turn-based game the
//! *legibility* of enemy behavior matters more than its strength; a player must
//! be able to predict and counter it. Second, the AI draws from the battle RNG,
//! so it stays deterministic and replayable, which is worth more than cleverness.
//!
//! Because it is deterministic, thousands of AI-vs-AI battles can be run as a
//! balance harness (roadmap M2).

use crate::data::{AiProfile, Database, Effect, Id, SkillDef};
use crate::state::BattleState;
use crate::Command;

/// Chance that an aggressive actor reaches for its strongest skill instead of a
/// basic attack. Not 100%, so fights do not degenerate into the same opening
/// every time, and not low enough to feel passive.
const AGGRESSIVE_SKILL_CHANCE: i32 = 65;

/// Below this fraction of max HP, a support actor triages instead of attacking.
const SUPPORT_HEAL_THRESHOLD_PERCENT: i32 = 55;

/// Picks a command for `actor`. Takes `&mut BattleState` because the RNG lives
/// inside it; the state is otherwise not modified.
pub fn choose(db: &Database, st: &mut BattleState, actor: usize) -> Command {
    // Copy what is needed before touching the RNG, to keep borrows short.
    let team = st.combatants[actor].team;
    let profile = st.combatants[actor].ai;
    let sp = st.combatants[actor].sp;
    let skills: Vec<Id> = st.combatants[actor].skills.clone();

    let enemies = st.alive_on(team.opposite());
    let allies = st.alive_on(team);
    if enemies.is_empty() {
        // Nothing hostile left; the scheduler is about to end the battle.
        return Command::Wait;
    }

    // Focus fire. Ties break on the lower index so the choice is deterministic
    // rather than dependent on iteration details.
    let weakest_enemy = enemies
        .iter()
        .copied()
        .min_by_key(|&index| (st.combatants[index].hp, index))
        .unwrap_or(enemies[0]);

    match profile {
        AiProfile::Support => {
            let wounded = allies
                .iter()
                .copied()
                .filter(|&index| {
                    st.combatants[index].hp * 100
                        < st.combatants[index].max_hp * SUPPORT_HEAL_THRESHOLD_PERCENT
                })
                .min_by_key(|&index| {
                    (
                        st.combatants[index].hp * 100 / st.combatants[index].max_hp.max(1),
                        index,
                    )
                });
            if let Some(target) = wounded {
                if let Some(skill) = affordable(db, &skills, sp, is_healing) {
                    return Command::Skill { skill, target };
                }
            }
            if let Some(skill) = best_offensive(db, &skills, sp) {
                return Command::Skill {
                    skill,
                    target: weakest_enemy,
                };
            }
            Command::Attack {
                target: weakest_enemy,
            }
        }
        AiProfile::Aggressive => {
            if st.rng.chance(AGGRESSIVE_SKILL_CHANCE) {
                if let Some(skill) = best_offensive(db, &skills, sp) {
                    return Command::Skill {
                        skill,
                        target: weakest_enemy,
                    };
                }
            }
            Command::Attack {
                target: weakest_enemy,
            }
        }
        AiProfile::Trickster => {
            // Random on purpose: this profile is for low-tier enemies that must
            // not feel optimal.
            let target = st
                .rng
                .pick(&enemies)
                .unwrap_or(weakest_enemy);
            let usable: Vec<Id> = skills
                .iter()
                .filter_map(|id| db.skill(id))
                .filter(|def| def.sp_cost <= sp && def.target.is_hostile())
                .map(|def| def.id.clone())
                .collect();
            if usable.is_empty() {
                return Command::Attack { target };
            }
            let index = st.rng.below(usable.len() as u32) as usize;
            Command::Skill {
                skill: usable[index].clone(),
                target,
            }
        }
    }
}

fn is_healing(def: &SkillDef) -> bool {
    def.effects
        .iter()
        .any(|effect| matches!(effect, Effect::Heal { .. }))
}

/// Total nominal offensive power of a skill. A crude proxy, but it reads the
/// same data a designer edits, so tuning JSON also tunes the AI.
fn total_power(def: &SkillDef) -> i32 {
    def.effects
        .iter()
        .map(|effect| match effect {
            Effect::Damage { power, .. } | Effect::Drain { power, .. } => *power,
            _ => 0,
        })
        .sum()
}

fn affordable(
    db: &Database,
    skills: &[Id],
    sp: i32,
    predicate: fn(&SkillDef) -> bool,
) -> Option<Id> {
    skills
        .iter()
        .filter_map(|id| db.skill(id))
        .find(|def| def.sp_cost <= sp && predicate(def))
        .map(|def| def.id.clone())
}

fn best_offensive(db: &Database, skills: &[Id], sp: i32) -> Option<Id> {
    skills
        .iter()
        .filter_map(|id| db.skill(id))
        .filter(|def| def.sp_cost <= sp && def.target.is_hostile() && total_power(def) > 0)
        .max_by_key(|def| total_power(def))
        .map(|def| def.id.clone())
}
