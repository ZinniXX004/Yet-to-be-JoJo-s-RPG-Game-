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
//! consider a class of action; only [`score_action`] decides *which* action
//! wins. Adding a consideration means changing a score, never adding a branch
//! per skill id.
//!
//! # The unit
//!
//! Scores are in **nominal power points**: the same unit a designer writes in
//! the `power` field of `data/skills.json`. Everything else is converted into
//! it, because a choice between an attack and a buff is meaningless until both
//! are quoted in one currency.
//!
//! The conversion that is easy to get wrong, and was wrong once: a `power`
//! value is *not* damage. [`crate::resolve`] turns it into damage by
//! multiplying by `atk / 100` and subtracting `def / 2`. So a quantity that is
//! already in HP -- damage prevented by a defence buff, HP lost to a bleed --
//! must be divided by the actor's attack before it can be compared to a power
//! value. Skipping that step silently overvalues every defensive and
//! damage-over-time effect by the actor's attack multiplier, and the balance
//! harness measured the result: a party member who braced instead of attacking
//! and lost the encounter doing it.
//!
//! Effects whose value is a number of *actions* rather than an amount of HP --
//! a tempo lock, a stun, an attack or speed buff -- are priced against
//! [`Situation::baseline`], which is already a power value, so they need no
//! conversion.
//!
//! # The price of SP
//!
//! Points on their own are only half of a decision. SP does not regenerate, so
//! a battle hands each combatant one fixed budget and every skill spends from
//! it; a skill worth 220 points at 45 SP and one worth 195 at 18 are not
//! comparable until the SP is priced too.
//!
//! Treating "affordable" as "free" is not a rounding error. The harness
//! measured it: Jotaro spent 45 of his 80 SP on a single tempo lock, which is
//! 71% of his budget for 13% of his turns, and then could no longer pay the 18
//! SP for the barrage that is his actual output, so three quarters of his
//! remaining turns were basic attacks. Dio committed 90% of his SP the same
//! way. Both were following the score exactly.
//!
//! So each candidate is charged for the SP it spends, at the surplus the **best
//! other** affordable paid skill would return per SP. Excluding the candidate
//! itself matters: a skill is not its own alternative, and pricing it against
//! its own rate would leave every paid skill worth roughly the same as a free
//! attack. Surplus rather than raw points, because the free attack is available
//! whether the SP is spent or not, so only the amount *above* it was bought.
//!
//! Two consequences worth stating, since both are design decisions and not
//! oversights. Free skills are charged nothing, so the basic attack and
//! `guard_stance` are unaffected. And the rate is derived entirely from the
//! skills the actor is holding, so there is no new constant to tune --
//! `data/skills.json` keeps deciding behaviour.

use crate::data::{AiProfile, Database, Effect, Id, SkillDef, StatusKind, TargetKind};
use crate::resolve::BASIC_ATTACK_ID;
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

/// Scheduler ticks a typical combatant needs to earn one action.
///
/// [`crate::state::TEMPO_THRESHOLD`] divided by the roughly 85 SPD the shipped
/// roster averages. Used to convert a tempo lock into actions denied. Crude on
/// purpose: the alternative is reading the actual SPD of every enemy, which
/// makes the score depend on the battle state rather than on content, and makes
/// a scoring bug impossible to reproduce from a skill definition alone.
const TICKS_PER_ACTION: i32 = 12;

/// A score at or below this is not worth an action.
const WORTHLESS: i32 = 0;

/// Fallback if `data/skills.json` somehow has no basic attack. The validator
/// catches that first; this only stops a content mistake from making every
/// score zero.
const FALLBACK_BASELINE: i32 = 100;

/// What the scorer is allowed to know about the moment.
///
/// Everything here is a plain number copied out of the state before scoring, so
/// scoring cannot mutate the battle and cannot draw from the RNG.
struct Situation {
    /// SP the actor can spend right now.
    sp: i32,
    /// Actor's current effective attack. Only used as the exchange rate between
    /// HP and power points; see the module docs.
    atk: i32,
    /// Living hostiles, which is both the target count of an area attack and
    /// the number of incoming hits defence is worth something against.
    enemies: i32,
    /// Living allies, target count of an all-ally effect.
    allies: i32,
    /// Nominal power of the best single hit this actor can currently pay for.
    /// Used as the exchange rate for effects whose value is measured in enemy
    /// or own *actions* rather than in HP.
    baseline: i32,
}

/// Picks a command for `actor`. Takes `&mut BattleState` because the RNG lives
/// inside it; the state is otherwise not modified.
///
/// The order of RNG draws is load-bearing and unchanged: the hostile target is
/// drawn before the skill is chosen, even when the chosen skill turns out to
/// target the actor itself and the drawn target goes unused. Reordering it to
/// look tidier would shift every recorded battle for no design reason.
pub fn choose(db: &Database, st: &mut BattleState, actor: usize) -> Command {
    // Copy what is needed before touching the RNG, to keep borrows short.
    let team = st.combatants[actor].team;
    let profile = st.combatants[actor].ai;
    let sp = st.combatants[actor].sp;
    let atk = st.combatants[actor].atk();
    let skills: Vec<Id> = st.combatants[actor].skills.clone();

    let enemies = st.alive_on(team.opposite());
    let allies = st.alive_on(team);
    if enemies.is_empty() {
        // Nothing hostile left; the scheduler is about to end the battle.
        return Command::Wait;
    }

    let situation = Situation {
        sp,
        atk,
        enemies: enemies.len() as i32,
        allies: allies.len() as i32,
        baseline: baseline_power(db, &skills, sp),
    };

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
            let hostile = pick_hostile_target(st, &enemies, weakest_enemy);
            if let Some(skill) = best_action(db, &skills, &situation) {
                let target = resolved_target(db, &skill, actor, hostile);
                return Command::Skill { skill, target };
            }
            Command::Attack { target: hostile }
        }
        AiProfile::Aggressive => {
            let hostile = pick_hostile_target(st, &enemies, weakest_enemy);
            if st.rng.chance(AGGRESSIVE_SKILL_CHANCE) {
                if let Some(skill) = best_action(db, &skills, &situation) {
                    let target = resolved_target(db, &skill, actor, hostile);
                    return Command::Skill { skill, target };
                }
            }
            Command::Attack { target: hostile }
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

/// Which index to name in the command. A self-targeted skill ignores the field
/// entirely in [`crate::resolve`], but naming the actor keeps the event log
/// honest for anyone reading it.
fn resolved_target(db: &Database, skill: &Id, actor: usize, hostile: usize) -> usize {
    match db.skill(skill) {
        Some(def) if !def.target.is_hostile() => actor,
        _ => hostile,
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

/// Total nominal offensive power of a skill, per target. A crude proxy, but it
/// reads the same data a designer edits, so tuning JSON also tunes the AI.
fn total_power(def: &SkillDef) -> i32 {
    def.effects
        .iter()
        .map(|effect| match effect {
            Effect::Damage { power, .. } | Effect::Drain { power, .. } => *power,
            _ => 0,
        })
        .sum()
}

/// The exchange rate between an *action* and *power points*, for this actor,
/// now.
///
/// Effects like a tempo lock are worth some number of actions; to price them we
/// need to know what an action is worth. The best hit the actor can currently
/// pay for is the honest answer: a boss with a 140-power slam values a denied
/// enemy turn more than a thug with a 100-power punch does.
fn baseline_power(db: &Database, skills: &[Id], sp: i32) -> i32 {
    let best = skills
        .iter()
        .filter_map(|id| db.skill(id))
        .filter(|def| def.sp_cost <= sp && def.target.is_hostile())
        .map(total_power)
        .max()
        .unwrap_or(0);
    if best > WORTHLESS {
        return best;
    }
    db.skill(BASIC_ATTACK_ID)
        .map(total_power)
        .unwrap_or(FALLBACK_BASELINE)
}

/// Converts an amount of HP into the power points that would have produced it.
///
/// The inverse of the `power * atk / 100` step in [`crate::resolve`], and the
/// only reason [`Situation::atk`] exists. The `def / 2` subtraction is
/// deliberately not inverted: it depends on the target, and a scorer that reads
/// the defence of a specific combatant stops being reproducible from content
/// alone. The omission biases attacks slightly upward against defensive
/// effects, which is the safer direction -- an AI that overrates attacking is
/// merely suboptimal, while one that overrates bracing stops trying to win.
fn as_power_units(hp: i32, at: &Situation) -> i32 {
    hp * 100 / at.atk.max(1)
}

/// Whether this skill is a candidate for the action slot at all.
///
/// Enemy-facing skills and self-buffs are. Ally-facing skills are not: healing
/// is decided by the support triage branch above, which runs first and knows
/// *who* is hurt, and no shipped skill buffs an ally. Admitting them here would
/// mean inventing an ally-selection rule in a scorer that cannot see HP.
fn is_candidate(def: &SkillDef) -> bool {
    matches!(
        def.target,
        TargetKind::OneEnemy | TargetKind::AllEnemies | TargetKind::SelfOnly
    )
}

/// How many combatants the skill lands on.
fn affected_count(def: &SkillDef, at: &Situation) -> i32 {
    match def.target {
        TargetKind::AllEnemies => at.enemies.max(1),
        TargetKind::AllAllies => at.allies.max(1),
        _ => 1,
    }
}

/// Price of one status application, before the chance to land it is applied.
///
/// Buffs on the actor are priced over `duration - 1`, not `duration`: the turn
/// spent applying them is not a turn spent benefiting from them. Debuffs on an
/// enemy get the full duration, because the actor's turn was spent attacking
/// that enemy anyway.
fn status_points(status: StatusKind, potency: i32, duration: u8, at: &Situation) -> i32 {
    let duration = duration as i32;
    let own_turns = (duration - 1).max(0);
    match status {
        // More attack, or more turns to attack in, both convert to hits, and a
        // hit is already a power value.
        StatusKind::AtkUp | StatusKind::SpdUp => at.baseline * potency / 100 * own_turns,
        // Damage prevented, in HP: the model subtracts def/2 from every incoming
        // hit, and every living enemy is one incoming hit per round. Converted,
        // because HP and power are not the same unit.
        StatusKind::DefUp => {
            let prevented = potency / 2 * at.enemies.max(1) * own_turns;
            as_power_units(prevented, at)
        }
        // Output taken away from the target, valued at our own exchange rate.
        StatusKind::AtkDown | StatusKind::SpdDown => at.baseline * potency / 100 * duration,
        // Halved: a softer target is worth less than a weaker attacker,
        // because def is halved again inside the damage model.
        StatusKind::DefDown => at.baseline * potency / 200 * duration,
        // Bleed is HP, dealt over turns rather than at once.
        StatusKind::Bleed => as_power_units(potency * duration, at),
        // Healing is the triage branch's decision, not the scorer's.
        StatusKind::Regen => WORTHLESS,
        // A stunned combatant loses its action outright.
        StatusKind::Stun => at.baseline * duration,
    }
}

/// Price of one effect on one full application of the skill.
fn effect_points(effect: &Effect, at: &Situation, targets: i32) -> i32 {
    match *effect {
        Effect::Damage { power, .. } | Effect::Drain { power, .. } => power * targets,
        // See status_points: the support branch owns healing.
        Effect::Heal { .. } => WORTHLESS,
        Effect::Status {
            status,
            potency,
            duration,
            chance,
        } => status_points(status, potency, duration, at) * targets * chance / 100,
        Effect::TempoLock { ticks } => at.baseline * ticks as i32 * targets / TICKS_PER_ACTION,
    }
}

/// What this skill is worth to the actor right now, **gross of its SP cost**, or
/// `None` if it is not a candidate at all.
///
/// Splitting "is it a candidate" from "how good is it" is the point of the
/// signature: an unaffordable skill and a worthless one are different facts,
/// and conflating them is what made three skills unreachable without anyone
/// deciding they should be.
///
/// Gross rather than net, because the price of SP depends on what *else* the
/// actor could buy with it, which a function looking at one skill cannot know.
/// [`best_action`] applies it.
fn score_action(def: &SkillDef, at: &Situation) -> Option<i32> {
    if def.sp_cost > at.sp || !is_candidate(def) {
        return None;
    }
    let targets = affected_count(def, at);
    let points: i32 = def
        .effects
        .iter()
        .map(|effect| effect_points(effect, at, targets))
        .sum();
    if points > WORTHLESS {
        Some(points)
    } else {
        None
    }
}

/// Points the actor gets for spending no SP at all.
///
/// The floor under every decision: a free skill is available whether the budget
/// is spent or not, so only the points *above* this were actually bought with
/// SP.
fn free_points(candidates: &[&SkillDef], at: &Situation) -> i32 {
    candidates
        .iter()
        .filter(|def| def.sp_cost == 0)
        .filter_map(|def| score_action(def, at))
        .max()
        .unwrap_or(WORTHLESS)
}

/// Points per SP the actor could get from the best paid skill that is *not*
/// `exclude`.
///
/// The opportunity cost of one SP. Excluding the candidate being priced is not
/// a detail: a skill is not its own alternative, and charging each skill its own
/// rate would value every paid skill at roughly a free attack, which is a
/// different -- and equally wrong -- model.
///
/// Floored at zero so that an actor whose only other paid options are bad does
/// not receive a discount for holding them.
fn best_other_rate(candidates: &[&SkillDef], at: &Situation, exclude: &str, free: i32) -> i32 {
    candidates
        .iter()
        .filter(|def| def.sp_cost > 0 && def.id != exclude)
        .filter_map(|def| score_action(def, at).map(|gross| (gross - free) / def.sp_cost))
        .max()
        .unwrap_or(WORTHLESS)
        .max(WORTHLESS)
}

/// Highest scoring candidate in the actor's skill list, net of what its SP would
/// have bought elsewhere, or `None` when nothing scores.
///
/// Ties resolve to the *last* candidate in declaration order. That is not a
/// preference, it is preservation: `Iterator::max_by_key` returns the last
/// maximum, and this reduction replaced one. Changing it would move win rates
/// for a reason unrelated to any design decision.
fn best_action(db: &Database, skills: &[Id], at: &Situation) -> Option<Id> {
    let candidates: Vec<&SkillDef> = skills.iter().filter_map(|id| db.skill(id)).collect();
    let free = free_points(&candidates, at);

    let mut best: Option<(i32, Id)> = None;
    for def in &candidates {
        let Some(gross) = score_action(def, at) else {
            continue;
        };
        let points = gross - def.sp_cost * best_other_rate(&candidates, at, &def.id, free);
        if points <= WORTHLESS {
            // Not "a bad skill": the same SP buys more elsewhere right now.
            continue;
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::Element;
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

    fn status(status: StatusKind, potency: i32, duration: u8, chance: i32) -> Effect {
        Effect::Status {
            status,
            potency,
            duration,
            chance,
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

    /// Two enemies, one ally, Jotaro's effective attack, an ordinary 100-power
    /// baseline.
    fn healthy(sp: i32) -> Situation {
        Situation {
            sp,
            atk: 120,
            enemies: 2,
            allies: 1,
            baseline: 100,
        }
    }

    fn guard(potency: i32) -> SkillDef {
        skill(
            "guard",
            TargetKind::SelfOnly,
            0,
            vec![status(StatusKind::DefUp, potency, 2, 100)],
        )
    }

    /// Jotaro's actual repertoire at full SP, which is the case the harness
    /// measured going wrong.
    fn jotaro_skills() -> Vec<SkillDef> {
        vec![
            skill(BASIC_ATTACK_ID, TargetKind::OneEnemy, 0, vec![damage(100)]),
            skill("rush", TargetKind::OneEnemy, 18, vec![damage(195)]),
            skill(
                "halt",
                TargetKind::AllEnemies,
                45,
                vec![Effect::TempoLock { ticks: 6 }, damage(60)],
            ),
        ]
    }

    #[test]
    fn a_skill_the_actor_cannot_pay_for_is_not_a_candidate() {
        let expensive = skill("expensive", TargetKind::OneEnemy, 30, vec![damage(200)]);
        assert_eq!(score_action(&expensive, &healthy(29)), None);
        assert_eq!(score_action(&expensive, &healthy(30)), Some(200));
    }

    #[test]
    fn a_skill_aimed_at_an_ally_is_not_a_candidate_for_the_action_slot() {
        let heal = skill(
            "heal",
            TargetKind::OneAlly,
            10,
            vec![Effect::Heal { power: 140 }],
        );
        assert_eq!(score_action(&heal, &healthy(100)), None);
    }

    #[test]
    fn an_area_attack_is_worth_its_power_once_per_target() {
        let volley = skill("volley", TargetKind::AllEnemies, 24, vec![damage(110)]);
        assert_eq!(score_action(&volley, &healthy(24)), Some(220));
        let alone = Situation {
            enemies: 1,
            ..healthy(24)
        };
        assert_eq!(score_action(&volley, &alone), Some(110));
    }

    #[test]
    fn drain_counts_as_damage() {
        let drain = skill(
            "drain",
            TargetKind::OneEnemy,
            12,
            vec![Effect::Drain {
                power: 120,
                element: Element::Psychic,
            }],
        );
        assert_eq!(score_action(&drain, &healthy(12)), Some(120));
    }

    /// The defect this commit exists for. 60 prevented HP is worth fewer power
    /// points to a strong attacker, because that attacker converts a power
    /// point into more than one point of damage.
    #[test]
    fn hp_denominated_effects_are_converted_into_power_units() {
        let braced = guard(60);
        assert_eq!(score_action(&braced, &healthy(0)), Some(50));
        let weak_attacker = Situation {
            atk: 60,
            ..healthy(0)
        };
        assert_eq!(score_action(&braced, &weak_attacker), Some(100));
    }

    /// Records a content finding rather than hiding it: at potency 60 over two
    /// turns, bracing prevents less than attacking deals, so declining it is
    /// correct. Changing that is a number in `data/skills.json`.
    #[test]
    fn bracing_is_declined_at_the_potency_the_game_ships() {
        let skills = vec![
            skill("punch", TargetKind::OneEnemy, 0, vec![damage(100)]),
            guard(60),
        ];
        let list = ids(&skills);
        let db = database(skills);
        assert_eq!(best_action(&db, &list, &healthy(0)).as_deref(), Some("punch"));
    }

    /// And the scorer is not biased against defence either: quadruple the
    /// potency and the same arithmetic chooses it.
    #[test]
    fn bracing_wins_once_it_prevents_more_than_an_attack_deals() {
        let skills = vec![
            skill("punch", TargetKind::OneEnemy, 0, vec![damage(100)]),
            guard(240),
        ];
        let list = ids(&skills);
        let db = database(skills);
        assert_eq!(best_action(&db, &list, &healthy(0)).as_deref(), Some("guard"));
    }

    /// The same finding for the attack buff: 40% over three turns returns 0.8
    /// of a hit for the price of one.
    #[test]
    fn an_attack_buff_worth_less_than_one_hit_is_declined() {
        let rage = skill(
            "rage",
            TargetKind::SelfOnly,
            10,
            vec![status(StatusKind::AtkUp, 40, 3, 100)],
        );
        let punch = skill("punch", TargetKind::OneEnemy, 0, vec![damage(100)]);
        assert_eq!(score_action(&rage, &healthy(10)), Some(80));
        let skills = vec![rage, punch];
        let list = ids(&skills);
        let db = database(skills);
        assert_eq!(best_action(&db, &list, &healthy(10)).as_deref(), Some("punch"));
    }

    #[test]
    fn a_tempo_lock_is_priced_in_the_enemy_actions_it_denies() {
        let halt = skill(
            "halt",
            TargetKind::AllEnemies,
            45,
            vec![Effect::TempoLock { ticks: 6 }, damage(60)],
        );
        let against_three = Situation {
            enemies: 3,
            baseline: 140,
            ..healthy(45)
        };
        // 140 * 6 * 3 / 12 = 210 denied, plus 60 damage on each of three.
        assert_eq!(score_action(&halt, &against_three), Some(390));
    }

    #[test]
    fn a_status_that_may_not_land_is_priced_at_its_chance() {
        let slam = skill(
            "slam",
            TargetKind::OneEnemy,
            22,
            vec![damage(140), status(StatusKind::Stun, 0, 1, 40)],
        );
        // 140 damage plus 40% of one denied 100-point action.
        assert_eq!(score_action(&slam, &healthy(22)), Some(180));
    }

    #[test]
    fn bleed_is_priced_as_the_damage_it_will_deal() {
        let flare = skill(
            "flare",
            TargetKind::OneEnemy,
            16,
            vec![damage(150), status(StatusKind::Bleed, 25, 3, 60)],
        );
        // 75 HP over three turns is 62 power points at atk 120, of which 60%
        // is expected to land.
        assert_eq!(score_action(&flare, &healthy(16)), Some(187));
    }

    /// The exchange rate between SP and points, spelled out on the repertoire
    /// the harness measured. The barrage buys 95 points above a free strike for
    /// 18 SP; the tempo lock buys 120 for 45.
    #[test]
    fn sp_is_priced_at_the_best_other_use_of_it() {
        let skills = jotaro_skills();
        let candidates: Vec<&SkillDef> = skills.iter().collect();
        let at = healthy(80);

        assert_eq!(free_points(&candidates, &at), 100);
        // Pricing the lock, the alternative is the barrage: 95 / 18 = 5.
        assert_eq!(best_other_rate(&candidates, &at, "halt", 100), 5);
        // Pricing the barrage, the alternative is the lock: 120 / 45 = 2.
        assert_eq!(best_other_rate(&candidates, &at, "rush", 100), 2);
        // A skill with no paid alternative is charged nothing.
        assert_eq!(best_other_rate(&candidates, &at, "rush", 195), 0);
    }

    /// The measured defect, as a unit test. 45 SP on a tempo lock is two and a
    /// half barrages never thrown, and the harness recorded exactly that: one
    /// lock per battle, then basic attacks for the rest of it.
    ///
    /// The second half of the test is the part that matters -- the lock is not
    /// disliked, it is *outbid*. Take the barrage away and the same arithmetic
    /// chooses it.
    #[test]
    fn an_expensive_denial_loses_to_the_barrage_it_would_forgo() {
        let skills = jotaro_skills();
        let list = ids(&skills);
        let db = database(skills);
        assert_eq!(best_action(&db, &list, &healthy(80)).as_deref(), Some("rush"));

        let without_barrage: Vec<SkillDef> = jotaro_skills()
            .into_iter()
            .filter(|def| def.id != "rush")
            .collect();
        let list = ids(&without_barrage);
        let db = database(without_barrage);
        assert_eq!(best_action(&db, &list, &healthy(80)).as_deref(), Some("halt"));
    }

    /// Efficiency, not size. A cheap converter outbids a bigger skill whose SP
    /// would buy more elsewhere, and the bigger skill comes back the moment the
    /// cheap one is gone.
    #[test]
    fn a_skill_is_declined_when_the_same_sp_buys_more_elsewhere() {
        let cheap = skill("cheap", TargetKind::OneEnemy, 5, vec![damage(150)]);
        let heavy = skill("heavy", TargetKind::OneEnemy, 40, vec![damage(120)]);
        let strike = skill(BASIC_ATTACK_ID, TargetKind::OneEnemy, 0, vec![damage(100)]);

        let skills = vec![strike.clone(), heavy.clone(), cheap];
        let list = ids(&skills);
        let db = database(skills);
        assert_eq!(
            best_action(&db, &list, &healthy(80)).as_deref(),
            Some("cheap")
        );

        let skills = vec![strike, heavy];
        let list = ids(&skills);
        let db = database(skills);
        assert_eq!(
            best_action(&db, &list, &healthy(80)).as_deref(),
            Some("heavy"),
            "with no cheaper converter to forgo, the heavy skill is worth its SP"
        );
    }

    #[test]
    fn a_tie_resolves_to_the_last_declared_skill() {
        let skills = vec![
            skill("first", TargetKind::OneEnemy, 0, vec![damage(100)]),
            skill("second", TargetKind::OneEnemy, 0, vec![damage(100)]),
        ];
        let list = ids(&skills);
        let db = database(skills);
        assert_eq!(
            best_action(&db, &list, &healthy(0)).as_deref(),
            Some("second")
        );
    }

    #[test]
    fn an_actor_with_nothing_affordable_scores_nothing() {
        let skills = vec![skill("costly", TargetKind::OneEnemy, 45, vec![damage(60)])];
        let list = ids(&skills);
        let db = database(skills);
        assert_eq!(best_action(&db, &list, &healthy(10)), None);
    }

    #[test]
    fn the_baseline_falls_back_to_the_basic_attack_when_nothing_is_affordable() {
        let skills = vec![
            skill(BASIC_ATTACK_ID, TargetKind::OneEnemy, 0, vec![damage(100)]),
            skill("costly", TargetKind::OneEnemy, 45, vec![damage(195)]),
        ];
        let list = ids(&skills);
        let db = database(skills);
        assert_eq!(baseline_power(&db, &list, 45), 195);
        assert_eq!(baseline_power(&db, &list, 0), 100);
    }
}
