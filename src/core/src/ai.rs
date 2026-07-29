//! Enemy and auto-battle decision making.
//!
//! The AI is intentionally shallow: three readable profiles, no search, and one
//! explicit score per candidate action rather than a decision tree. Two
//! reasons. First, in a party-scale turn-based game the *legibility* of enemy
//! behavior matters more than its strength; a player must be able to predict
//! and counter it. Second, the AI draws from the battle RNG, so it stays
//! deterministic and replayable, which is worth more than cleverness.
//!
//! Because it is deterministic, thousands of AI-vs-AI battles can be run as a
//! balance harness (roadmap M2).
//!
//! Scoring lives in one place on purpose. Every profile below decides *when* to
//! consider a class of action; only [`score_offensive`] decides *which* action
//! wins. Adding a consideration means changing a score, never adding a branch
//! per skill id.

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

/// How good an attack this skill is, or `None` if it is not an attack the actor
/// could make at all.
///
/// Splitting "is it a candidate" from "how good is it" is the point of this
/// function: an unaffordable skill and a worthless one are different facts, and
/// only the first is a hard exclusion. A zero-power skill is excluded here
/// because there is no offensive number to compare; a defensive skill is not an
/// attack that scored badly.
fn score_offensive(def: &SkillDef, sp: i32) -> Option<i32> {
    if def.sp_cost > sp || !def.target.is_hostile() {
        return None;
    }
    let power = total_power(def);
    if power <= 0 {
        return None;
    }
    Some(power)
}

/// Highest scoring candidate in the actor's skill list, or `None` when nothing
/// scores.
///
/// Ties resolve to the *last* candidate in declaration order. That is not a
/// preference, it is preservation: `Iterator::max_by_key` returns the last
/// maximum, and this reduction replaced one. Changing it would move win rates
/// for a reason unrelated to any design decision.
fn best_scored(db: &Database, skills: &[Id], score: impl Fn(&SkillDef) -> Option<i32>) -> Option<Id> {
    let mut best: Option<(i32, Id)> = None;
    for def in skills.iter().filter_map(|id| db.skill(id)) {
        let Some(points) = score(def) else {
            continue;
        };
        let wins = match &best {
            Some((leader, _)) => points >= *leader,
            None => true,
        };
        if wins {
            best = Some((points, def.id.clone()));
        }
    }
    best.map(|(_, id)| id)
}

fn best_offensive(db: &Database, skills: &[Id], sp: i32) -> Option<Id> {
    best_scored(db, skills, |def| score_offensive(def, sp))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::{Element, StatusKind, TargetKind};
    use crate::state::TEMPO_THRESHOLD;

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
        }
    }

    fn damage(power: i32) -> Effect {
        Effect::Damage {
            power,
            element: Element::Physical,
            variance: 0,
        }
    }

    /// A database of skills alone: no combatant references them, so nothing has
    /// to be invented to keep validation happy.
    fn database(skills: Vec<SkillDef>) -> Database {
        Database::new(Vec::new(), skills, Vec::new()).expect("test content should validate")
    }

    fn ids(skills: &[SkillDef]) -> Vec<Id> {
        skills.iter().map(|def| def.id.clone()).collect()
    }

    #[test]
    fn a_skill_the_actor_cannot_pay_for_is_not_a_candidate() {
        let expensive = skill("expensive", TargetKind::OneEnemy, 30, vec![damage(200)]);
        assert_eq!(score_offensive(&expensive, 29), None);
        assert_eq!(score_offensive(&expensive, 30), Some(200));
    }

    #[test]
    fn a_skill_that_deals_no_damage_is_not_an_offensive_candidate() {
        let guard = skill(
            "guard",
            TargetKind::SelfOnly,
            0,
            vec![Effect::Status {
                status: StatusKind::DefUp,
                potency: 60,
                duration: 2,
                chance: 100,
            }],
        );
        assert_eq!(score_offensive(&guard, 100), None);
    }

    #[test]
    fn a_skill_aimed_at_an_ally_is_never_scored_as_an_attack() {
        let heal = skill(
            "heal",
            TargetKind::OneAlly,
            10,
            vec![Effect::Heal { power: 140 }],
        );
        assert_eq!(score_offensive(&heal, 100), None);
    }

    #[test]
    fn an_area_attack_is_a_candidate_like_any_other() {
        // Recorded because the opposite was believed and published: the filter
        // has never looked at how many targets a skill hits.
        let volley = skill("volley", TargetKind::AllEnemies, 24, vec![damage(110)]);
        assert_eq!(score_offensive(&volley, 24), Some(110));
    }

    #[test]
    fn the_strongest_affordable_attack_wins() {
        let skills = vec![
            skill("weak", TargetKind::OneEnemy, 0, vec![damage(100)]),
            skill("strong", TargetKind::OneEnemy, 18, vec![damage(195)]),
        ];
        let list = ids(&skills);
        let db = database(skills);
        assert_eq!(best_offensive(&db, &list, 18).as_deref(), Some("strong"));
        assert_eq!(best_offensive(&db, &list, 17).as_deref(), Some("weak"));
    }

    #[test]
    fn a_tie_resolves_to_the_last_declared_skill() {
        let skills = vec![
            skill("first", TargetKind::OneEnemy, 0, vec![damage(100)]),
            skill("second", TargetKind::OneEnemy, 0, vec![damage(100)]),
        ];
        let list = ids(&skills);
        let db = database(skills);
        assert_eq!(best_offensive(&db, &list, 0).as_deref(), Some("second"));
    }

    #[test]
    fn an_actor_with_nothing_affordable_scores_nothing() {
        let skills = vec![skill("costly", TargetKind::OneEnemy, 45, vec![damage(60)])];
        let list = ids(&skills);
        let db = database(skills);
        assert_eq!(best_offensive(&db, &list, 10), None);
    }

    #[test]
    fn drain_counts_towards_offensive_score() {
        let drain = skill(
            "drain",
            TargetKind::OneEnemy,
            12,
            vec![Effect::Drain {
                power: 120,
                element: Element::Physical,
            }],
        );
        assert_eq!(score_offensive(&drain, 12), Some(120));
    }
}
