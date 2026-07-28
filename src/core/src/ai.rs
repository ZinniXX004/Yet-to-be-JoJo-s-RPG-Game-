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

/// Chance that an actor commits to finishing off the weakest enemy instead of
/// spreading its damage across the enemy line.
///
/// Deliberately not 100%. Unconditional lowest-HP focus fire *compounds* any
/// damage advantage: every kill permanently removes output from the losing
/// side while the winning side keeps all of its own, so a small edge in damage
/// per turn becomes a near-certain victory. The M2 balance harness measured
/// this directly. `matchup.assassin_ambush` held a 100% win rate on a
/// damage-per-turn edge of only about 2.3 to 1, and Kakyoin was downed in 12
/// of 12 boss battles because every hostile actor converged on him the instant
/// he became the softest target.
///
/// Mixing in random targets keeps finishing blows the common case — a
/// wounded enemy still dies soon — without letting either side's advantage
/// snowball, and it stops the squishiest party member from being a guaranteed
/// casualty rather than a likely one.
const FOCUS_FIRE_CHANCE: i32 = 55;

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

    // Ties break on the lower index so the fallback is deterministic rather
    // than dependent on iteration details.
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
            let target = pick_hostile_target(st, &enemies, weakest_enemy);
            if let Some(skill) = best_offensive(db, &skills, sp) {
                return Command::Skill { skill, target };
            }
            Command::Attack { target }
        }
        AiProfile::Aggressive => {
            let target = pick_hostile_target(st, &enemies, weakest_enemy);
            if st.rng.chance(AGGRESSIVE_SKILL_CHANCE) {
                if let Some(skill) = best_offensive(db, &skills, sp) {
                    return Command::Skill { skill, target };
                }
            }
            Command::Attack { target }
        }
        AiProfile::Trickster => {
            // Fully random on purpose: this profile is for low-tier enemies
            // that must not feel optimal.
            let target = st.rng.pick(&enemies).unwrap_or(weakest_enemy);
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

/// Chooses which enemy to strike: usually the weakest, sometimes any of them.
///
/// `weakest` is passed in rather than recomputed so the caller's tie-breaking
/// is the single source of truth, and so this function costs exactly one or two
/// RNG draws regardless of party size.
fn pick_hostile_target(st: &mut BattleState, enemies: &[usize], weakest: usize) -> usize {
    if st.rng.chance(FOCUS_FIRE_CHANCE) {
        weakest
    } else {
        st.rng.pick(enemies).unwrap_or(weakest)
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
