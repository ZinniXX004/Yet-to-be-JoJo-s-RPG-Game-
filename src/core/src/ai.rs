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
//! consider a class of action; only `score_action` decides *which* action
//! wins. Adding a consideration means changing a score, never adding a branch
//! per skill id.
//!
//! # The unit
//!
//! Scores are in **HP**: the amount the effect is expected to take from the
//! enemy side, or keep on ours, after the defence toll and the resistance
//! table that [`crate::resolve`] applies to the real hit. A choice between an
//! attack and a buff is meaningless until both are quoted in one currency,
//! and HP is the one the battle itself is decided in.
//!
//! The mistake this replaced, twice (#9, then #63): quoting some effects in
//! *power points* -- the `power` field of `data/skills.json` -- and others in
//! HP. A power value is not damage; `resolve` multiplies it by `atk / 100` and
//! subtracts `def / 2`. Within one decision the actor's attack is fixed, so a
//! scorer quoted entirely in power points would rank candidates exactly as
//! this one does; what breaks a decision is the *mix*, and above all a mix
//! that charges the defence toll on one side of the comparison and not the
//! other.
//!
//! Each effect therefore lands in HP by the route its mechanic takes:
//!
//! - damage and drain: the hit itself, against the chosen target;
//! - bleed and a guard: already HP, so no conversion at all;
//! - an attack buff or debuff: a percentage, of the base attack, of the HP a
//!   hit carries before the toll, since the toll is flat and does not scale;
//! - anything measured in *actions* -- a tempo lock, a stun, a speed buff or
//!   debuff -- at `Situation::baseline_hp`, what one of the actor's actions
//!   is worth against this target right now.
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

use crate::data::{
    AiProfile, Database, Effect, Element, Id, Resistances, SkillDef, StatusKind, TargetKind,
    MAX_RESISTANCE, MIN_RESISTANCE,
};
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
    /// Actor's current effective attack: what turns a `power` value into the
    /// HP a hit carries before the defence toll.
    atk: i32,
    /// Actor's attack before any status: the stat a percentage modifier is a
    /// percentage *of*. `state.rs` sums every modifier and applies the total
    /// to this, so one more +35% always adds `base_atk * 35 / 100`, however
    /// many buffs are already active.
    base_atk: i32,
    /// Living hostiles, which is both the target count of an area attack and
    /// the number of incoming hits per round.
    enemies: i32,
    /// Living allies, the actor included: the target count of an all-ally
    /// effect, and the number of bodies those incoming hits are spread over.
    allies: i32,
    /// Nominal power of the best single hit this actor can currently pay for.
    /// The share of it that a percentage of attack moves is what an attack
    /// buff or debuff is worth (see `attack_share_hp`).
    baseline: i32,
    /// HP per target of the best hit this actor can currently pay for,
    /// against `target_def` and `target_resist`, through the same toll,
    /// resistance and floor as a Damage effect. The price of one action, for
    /// effects measured in actions (see the module docs).
    baseline_hp: i32,
    /// The resistance table of whichever hostile target has already been
    /// chosen for this turn (see `choose`) -- empty, which reads as neutral
    /// for every element, when no hostile target applies (a self-only or
    /// ally-only decision). Populated before scoring, not after, so
    /// `effect_points` can weight `Effect::Damage`/`Effect::Drain` by it
    /// (issue #22) without the scorer ever picking a target itself.
    target_resist: Resistances,
    /// The `def` stat of that same hostile target, zero when none applies.
    /// `effect_points` subtracts this the same way `compute_damage` does
    /// (issue #33): the flat toll after atk scaling, not a fraction of
    /// power. Zero is a safe default for a self/ally-only decision, since
    /// no shipped skill fires a Damage/Drain effect at a non-hostile target.
    target_def: i32,
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
    let base_atk = st.combatants[actor].base.atk;
    let skills: Vec<Id> = st.combatants[actor].skills.clone();

    let enemies = st.alive_on(team.opposite());
    let allies = st.alive_on(team);
    if enemies.is_empty() {
        // Nothing hostile left; the scheduler is about to end the battle.
        return Command::Wait;
    }

    // Numeric context that does not depend on which specific hostile gets
    // picked, so it is safe to compute before the RNG draw that picks one.
    let enemy_count = enemies.len() as i32;
    let ally_count = allies.len() as i32;
    let baseline = baseline_power(db, &skills, sp);

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
            // Unchanged position relative to the RNG draws above: the hostile
            // target is picked here exactly as before. Only what happens
            // *after* that pick (building `situation` with its resistance
            // table now known) is new.
            let hostile = pick_hostile_target(st, &enemies, weakest_enemy);
            let situation = Situation {
                sp,
                atk,
                base_atk,
                enemies: enemy_count,
                allies: ally_count,
                baseline,
                baseline_hp: 0,
                target_resist: st.combatants[hostile].resist.clone(),
                target_def: st.combatants[hostile].def(),
            }
            .priced(db, &skills);
            if let Some(skill) = best_action(db, &skills, &situation) {
                let target = resolved_target(db, &skill, actor, hostile);
                return Command::Skill { skill, target };
            }
            Command::Attack { target: hostile }
        }
        AiProfile::Aggressive => {
            // Same note as the Support branch above: this RNG draw is in the
            // same place it always was.
            let hostile = pick_hostile_target(st, &enemies, weakest_enemy);
            if st.rng.chance(AGGRESSIVE_SKILL_CHANCE) {
                let situation = Situation {
                    sp,
                    atk,
                    base_atk,
                    enemies: enemy_count,
                    allies: ally_count,
                    baseline,
                    baseline_hp: 0,
                    target_resist: st.combatants[hostile].resist.clone(),
                    target_def: st.combatants[hostile].def(),
                }
                .priced(db, &skills);
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

/// Nominal power of the best hit the actor can currently pay for.
///
/// Effects like a tempo lock are worth some number of actions; to price them we
/// need to know what an action is worth. The best hit the actor can currently
/// pay for is the honest answer: a boss with a 140-power slam values a denied
/// enemy turn more than a thug with a 100-power punch does. This is that hit's
/// `power`; [`baseline_hp`] quotes an action in the scorer's unit, and this
/// value only feeds the attack-share price of an attack buff or debuff.
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

impl Situation {
    /// Fills in [`Situation::baseline_hp`], which needs every other field.
    ///
    /// Pure: it reads the database and numbers already copied out of the
    /// state, and draws nothing from the RNG, so calling it after the target
    /// draw in `choose` leaves every recorded battle's draws where they were.
    fn priced(mut self, db: &Database, skills: &[Id]) -> Self {
        self.baseline_hp = baseline_hp(db, skills, &self);
        self
    }
}

/// HP one Damage or Drain effect of `power` deals to the chosen target.
///
/// Mirrors `compute_damage` in resolve.rs (issue #33), not just a resistance
/// weighting on raw power (issue #22's fix, extended here rather than
/// replaced): the flat defence toll is subtracted after atk scaling, so a
/// low-power skill can collapse toward the floor against a high-defence target
/// in a way power-only pricing never saw. Same floor-at-1 in both places
/// compute_damage floors, same order of operations. Variance and crit are left
/// out -- this prices the expected floor of a hit, not a specific roll.
fn hit_hp(power: i32, element: Element, at: &Situation) -> i32 {
    let base = power * at.atk / 100;
    let after_def = (base - at.target_def / 2).max(1);
    let resist = at
        .target_resist
        .get(&element)
        .copied()
        .unwrap_or(0)
        .clamp(MIN_RESISTANCE, MAX_RESISTANCE);
    if resist != 0 {
        (after_def * (100 - resist) / 100).max(1)
    } else {
        after_def
    }
}

/// HP one application of a skill's damage deals to the chosen target, or zero
/// for a skill that deals none.
fn skill_hit_hp(def: &SkillDef, at: &Situation) -> i32 {
    def.effects
        .iter()
        .map(|effect| match *effect {
            Effect::Damage { power, element, .. } | Effect::Drain { power, element, .. } => {
                hit_hp(power, element, at)
            }
            _ => 0,
        })
        .sum()
}

/// The price of one action, in HP, for this actor against this target, now.
///
/// The same reasoning as [`baseline_power`] -- the best hit the actor can pay
/// for is what an action is worth to it -- quoted in the scorer's unit. Chosen
/// by HP rather than by nominal power, so a hit the target resists does not
/// set the price of an action it would never take.
///
/// Still a proxy where an effect denies the *enemy* an action: that action is
/// valued at the actor's own hit, because the scorer does not read what the
/// target would have done with it.
fn baseline_hp(db: &Database, skills: &[Id], at: &Situation) -> i32 {
    let best = skills
        .iter()
        .filter_map(|id| db.skill(id))
        .filter(|def| def.sp_cost <= at.sp && def.target.is_hostile())
        .map(|def| skill_hit_hp(def, at))
        .max()
        .unwrap_or(0);
    if best > WORTHLESS {
        return best;
    }
    db.skill(BASIC_ATTACK_ID)
        .map(|def| skill_hit_hp(def, at))
        .filter(|&hp| hp > WORTHLESS)
        .unwrap_or_else(|| hit_hp(FALLBACK_BASELINE, Element::Physical, at))
}

/// HP a percentage of attack adds to (or, as a debuff, takes from) one hit.
///
/// The defence toll is flat, so it does not scale with attack and drops out of
/// the difference; what scales is the hit before the toll. And a percentage
/// modifier is a percentage of the *base* stat (`state.rs` sums modifiers
/// before applying them), so the delta is `baseline * base_atk / 100` times
/// the potency, not the current attack: reading the current attack counts an
/// active buff twice and prices each refresh above the buff it refreshes.
/// The harness measured that once, in the #63 entry of docs/BALANCE-LOG.md:
/// the Flame Assassin stopped using `sun_flare` and spent 65% of its turns on
/// `rage_focus`.
///
/// Resistance is not applied, which overstates the delta against a target
/// that resists the actor's best hit.
fn attack_share_hp(potency: i32, at: &Situation) -> i32 {
    at.baseline * at.base_atk / 100 * potency / 100
}

/// Whether this skill is a candidate for the action slot at all.
///
/// Enemy-facing skills and self-buffs are. Ally-facing skills are not: healing
/// is decided by the support triage branch above, which runs first and knows
/// *who* is hurt. `OneAlly` and `AllAllies` are admitted -- `resolved_target`
/// already sends any non-hostile skill back to the actor itself, so no new
/// ally-selection rule is needed to reach this point. What is still missing is
/// the ability to aim a one-ally skill at anyone *other* than the caster: the
/// scorer has no per-ally HP, so every one-ally skill an actor picks through
/// this generic path is a self-cast. `Effect::Heal` is priced at `WORTHLESS`
/// regardless (see `effect_points`), so `skill.restore` reaching this function
/// changes nothing -- healing is still decided entirely by the support triage
/// in `choose`.
fn is_candidate(def: &SkillDef) -> bool {
    matches!(
        def.target,
        TargetKind::OneEnemy
            | TargetKind::AllEnemies
            | TargetKind::SelfOnly
            | TargetKind::OneAlly
            | TargetKind::AllAllies
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
        // More attack is a larger hit on each own turn under the buff.
        StatusKind::AtkUp => attack_share_hp(potency, at) * own_turns,
        // More speed is more actions, each one worth a whole hit.
        StatusKind::SpdUp => at.baseline_hp * potency / 100 * own_turns,
        // Damage prevented, in HP: the model subtracts def/2 from every incoming
        // hit, and every living enemy is one hit per round -- but a guard only
        // stops the hits aimed at its holder, and the enemy's attention is
        // spread over every living member of the holder's side. Pricing it as
        // though all of them landed on the holder had Josuke bracing on 47% of
        // his turns against Dio and 59% against Diavolo at 0.4.0, and losing
        // those fights by it (issue #60). Uniform over `allies` is crude
        // on purpose: focus fire on the weakest is the enemy's rule, and a
        // price read from content alone cannot know who that is.
        //
        // Already HP, so no conversion (#63). `potency / 2` reads the potency
        // as flat defence points, while `state.rs` applies it as a percentage
        // of the holder's defence; that is a separate defect, left for its own
        // measurement.
        StatusKind::DefUp => potency / 2 * at.enemies.max(1) * own_turns / at.allies.max(1),
        // Output taken away from the target, valued at our own exchange rate:
        // the mirror of AtkUp, on the target's turns.
        StatusKind::AtkDown => attack_share_hp(potency, at) * duration,
        // Actions taken away from the target, each valued at one of ours.
        StatusKind::SpdDown => at.baseline_hp * potency / 100 * duration,
        // Each of our hits on a softer target clears `def * potency / 100`
        // less toll, and the toll is half of defence.
        StatusKind::DefDown => at.target_def * potency / 200 * duration,
        // Bleed is HP, dealt over turns rather than at once.
        StatusKind::Bleed => potency * duration,
        // Healing is the triage branch's decision, not the scorer's.
        StatusKind::Regen => WORTHLESS,
        // A stunned combatant loses its action outright.
        StatusKind::Stun => at.baseline_hp * duration,
    }
}

/// Price of one effect on one full application of the skill.
fn effect_points(effect: &Effect, at: &Situation, targets: i32) -> i32 {
    match *effect {
        Effect::Damage { power, element, .. } | Effect::Drain { power, element, .. } => {
            hit_hp(power, element, at) * targets
        }
        // See status_points: the support branch owns healing.
        Effect::Heal { .. } => WORTHLESS,
        Effect::Status {
            status,
            potency,
            duration,
            chance,
        } => status_points(status, potency, duration, at) * targets * chance / 100,
        Effect::TempoLock { ticks } => at.baseline_hp * ticks as i32 * targets / TICKS_PER_ACTION,
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
            provenance: None,
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
    /// baseline against a target with no defence: one action is a 120 HP hit.
    fn healthy(sp: i32) -> Situation {
        Situation {
            sp,
            atk: 120,
            base_atk: 120,
            enemies: 2,
            allies: 1,
            baseline: 100,
            baseline_hp: 120,
            target_resist: Resistances::new(),
            target_def: 0,
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
        assert_eq!(score_action(&expensive, &healthy(30)), Some(240));
    }

    #[test]
    fn a_healing_skill_aimed_at_an_ally_still_scores_worthless() {
        // OneAlly is a candidate now (see is_candidate), but Effect::Heal is
        // priced at WORTHLESS regardless of candidacy -- the support triage in
        // `choose`, not this generic scorer, is what picks a heal. This test
        // used to be named around candidacy; it is candidacy that changed, not
        // this assertion.
        let heal = skill(
            "heal",
            TargetKind::OneAlly,
            10,
            vec![Effect::Heal { power: 140 }],
        );
        assert_eq!(score_action(&heal, &healthy(100)), None);
    }

    #[test]
    fn an_ally_targeted_buff_is_now_a_reachable_candidate() {
        let encourage = skill(
            "encourage",
            TargetKind::OneAlly,
            0,
            vec![status(StatusKind::AtkUp, 40, 3, 100)],
        );
        assert!(score_action(&encourage, &healthy(100)).is_some());
    }

    #[test]
    fn an_all_allies_buff_is_now_a_reachable_candidate_and_scales_with_ally_count() {
        let rally = skill(
            "rally",
            TargetKind::AllAllies,
            0,
            vec![status(StatusKind::AtkUp, 40, 3, 100)],
        );
        let two_allies = Situation {
            allies: 2,
            ..healthy(100)
        };
        let one_ally = Situation {
            allies: 1,
            ..healthy(100)
        };
        let with_two = score_action(&rally, &two_allies).expect("should score");
        let with_one = score_action(&rally, &one_ally).expect("should score");
        assert_eq!(with_two, with_one * 2);
    }

    #[test]
    fn a_non_hostile_skill_resolves_its_target_to_the_caster() {
        let encourage = skill(
            "encourage",
            TargetKind::OneAlly,
            0,
            vec![status(StatusKind::AtkUp, 40, 3, 100)],
        );
        let db = database(vec![encourage.clone()]);
        let actor = 0;
        let some_hostile = 7;
        assert_eq!(
            resolved_target(&db, &encourage.id, actor, some_hostile),
            actor
        );
    }

    #[test]
    fn an_area_attack_is_worth_its_power_once_per_target() {
        let volley = skill("volley", TargetKind::AllEnemies, 24, vec![damage(110)]);
        assert_eq!(score_action(&volley, &healthy(24)), Some(264));
        let alone = Situation {
            enemies: 1,
            ..healthy(24)
        };
        assert_eq!(score_action(&volley, &alone), Some(132));
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
        assert_eq!(score_action(&drain, &healthy(12)), Some(144));
    }

    /// Issue #63. HP prevented is HP whoever prevents it: the guard's price
    /// does not depend on the holder's attack, as it did while it was
    /// converted into power units and compared against damage in HP.
    #[test]
    fn hp_denominated_effects_are_not_converted() {
        let braced = guard(60);
        assert_eq!(score_action(&braced, &healthy(0)), Some(60));
        let weak_attacker = Situation {
            atk: 60,
            ..healthy(0)
        };
        assert_eq!(score_action(&braced, &weak_attacker), Some(60));
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
        assert_eq!(
            best_action(&db, &list, &healthy(0)).as_deref(),
            Some("punch")
        );
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
        assert_eq!(
            best_action(&db, &list, &healthy(0)).as_deref(),
            Some("guard")
        );
    }

    /// Issue #60. A guard protects one body, and the enemy's hits are
    /// spread over every living ally. Fighting alone it is priced as before;
    /// in a party of four it prevents a quarter as much.
    #[test]
    fn a_guard_is_priced_over_the_hits_aimed_at_its_holder() {
        let braced = guard(60);
        let party_of_four = Situation {
            allies: 4,
            ..healthy(0)
        };
        // 30 per hit, two enemies, one turn under cover: 60 HP. Divided over
        // four allies: 15 HP.
        assert_eq!(score_action(&braced, &healthy(0)), Some(60));
        assert_eq!(score_action(&braced, &party_of_four), Some(15));
    }

    /// And the decision follows the price: the guard that out-earns a punch
    /// for a combatant on its own is declined by one of four.
    #[test]
    fn a_guard_worth_taking_alone_is_declined_in_a_party_of_four() {
        let skills = vec![
            skill("punch", TargetKind::OneEnemy, 0, vec![damage(100)]),
            guard(240),
        ];
        let list = ids(&skills);
        let db = database(skills);
        let party_of_four = Situation {
            allies: 4,
            ..healthy(0)
        };
        assert_eq!(
            best_action(&db, &list, &healthy(0)).as_deref(),
            Some("guard")
        );
        assert_eq!(
            best_action(&db, &list, &party_of_four).as_deref(),
            Some("punch")
        );
    }

    /// Issue #22: a target's resistance now discounts (or amplifies) how much
    /// a Damage effect is worth, the same way resolve.rs's compute_damage
    /// already treats the real hit.
    #[test]
    fn a_resisted_element_is_priced_lower_than_an_unresisted_one() {
        let physical = skill("jab", TargetKind::OneEnemy, 0, vec![damage(100)]);
        let mut resisted = healthy(0);
        resisted.target_resist.insert(Element::Physical, 25);
        let mut neutral = healthy(0);
        neutral.target_resist.insert(Element::Physical, 0);

        let resisted_score = score_action(&physical, &resisted).unwrap();
        let neutral_score = score_action(&physical, &neutral).unwrap();
        assert!(resisted_score < neutral_score);
        assert_eq!(resisted_score, neutral_score * 75 / 100);
    }

    /// Issue #27: emerald_snare's share of Kakyoin's turns falls as the
    /// enemy's atk rises, and nothing in the probe explained why. Traced it
    /// with temporary instrumentation on best_action rather than guessing:
    /// the score for both skills is exactly constant across every sp level
    /// and every enemy atk value tested, so the scorer's own preference
    /// never moves. What moves is which enemy-count state Kakyoin is alive
    /// to see. An AllEnemies skill's value is multiplied by the live enemy
    /// count (affected_count, already covered by
    /// an_area_attack_is_worth_its_power_once_per_target above), so it wins
    /// outright with two enemies up and loses that edge the moment only one
    /// remains -- at which point a smaller one_enemy skill that also carries
    /// a status effect wins instead. As enemy atk rises, Kakyoin's survival
    /// collapses long before either enemy dies, so a shrinking share of his
    /// turns ever happen in the one-enemy state. The drift is a composition
    /// effect, not a defect: pinned here so the arithmetic cannot silently
    /// change out from under that conclusion.
    ///
    /// Since #63 the slow is priced at one 120 HP action rather than at 100
    /// nominal power, and without a free attack in the list the snare now
    /// edges the splash even with two enemies up (11 against 10 net). The
    /// strike below is Kakyoin's own, and it is what the decision is really
    /// measured against: SP buys only the surplus over a free hit.
    #[test]
    fn an_all_enemies_skill_loses_its_edge_once_only_one_enemy_remains() {
        let skills = vec![
            skill(BASIC_ATTACK_ID, TargetKind::OneEnemy, 0, vec![damage(100)]),
            skill(
                "snare",
                TargetKind::OneEnemy,
                15,
                vec![damage(75), status(StatusKind::SpdDown, 30, 3, 80)],
            ),
            skill("splash", TargetKind::AllEnemies, 22, vec![damage(105)]),
        ];
        let list = ids(&skills);
        let db = database(skills);

        let two_enemies = Situation {
            enemies: 2,
            ..healthy(145)
        };
        let one_enemy = Situation {
            enemies: 1,
            ..healthy(145)
        };

        assert_eq!(
            best_action(&db, &list, &two_enemies).as_deref(),
            Some("splash"),
            "with two enemies up, the AllEnemies multiplier should win"
        );
        assert_eq!(
            best_action(&db, &list, &one_enemy).as_deref(),
            Some("snare"),
            "drop to one enemy and the OneEnemy skill should win instead"
        );
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
        // 40% of a 120 HP hit is 48, on each of two own turns under the buff.
        assert_eq!(score_action(&rage, &healthy(10)), Some(96));
        let skills = vec![rage, punch];
        let list = ids(&skills);
        let db = database(skills);
        assert_eq!(
            best_action(&db, &list, &healthy(10)).as_deref(),
            Some("punch")
        );
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
            baseline_hp: 168,
            ..healthy(45)
        };
        // 168 * 6 * 3 / 12 = 252 HP of denied actions (a 140-power hit at atk
        // 120 is 168 HP), plus 72 damage (60 power at atk 120) on each of
        // three.
        assert_eq!(score_action(&halt, &against_three), Some(468));
    }

    #[test]
    fn a_status_that_may_not_land_is_priced_at_its_chance() {
        let slam = skill(
            "slam",
            TargetKind::OneEnemy,
            22,
            vec![damage(140), status(StatusKind::Stun, 0, 1, 40)],
        );
        // 168 damage (140 power at atk 120) plus 40% of one denied 120 HP
        // action.
        assert_eq!(score_action(&slam, &healthy(22)), Some(216));
    }

    #[test]
    fn bleed_is_priced_as_the_damage_it_will_deal() {
        let flare = skill(
            "flare",
            TargetKind::OneEnemy,
            16,
            vec![damage(150), status(StatusKind::Bleed, 25, 3, 60)],
        );
        // 180 damage (150 power at atk 120), plus 75 HP over three turns, of
        // which 60% is expected to land: 45. Both terms are HP (#63); until
        // then the 75 was converted to 62 power points and added to the 180.
        assert_eq!(score_action(&flare, &healthy(16)), Some(225));
    }

    /// The exchange rate between SP and points, spelled out on the repertoire
    /// the harness measured. The barrage buys 114 HP above a free strike for
    /// 18 SP; the tempo lock buys 144 for 45.
    #[test]
    fn sp_is_priced_at_the_best_other_use_of_it() {
        let skills = jotaro_skills();
        let candidates: Vec<&SkillDef> = skills.iter().collect();
        let at = healthy(80);

        assert_eq!(free_points(&candidates, &at), 120);
        // Pricing the lock, the alternative is the barrage: (234 - 120) / 18 = 6.
        assert_eq!(best_other_rate(&candidates, &at, "halt", 120), 6);
        // Pricing the barrage, the alternative is the lock: (264 - 120) / 45 = 3.
        assert_eq!(best_other_rate(&candidates, &at, "rush", 120), 3);
        // A skill with no paid alternative is charged nothing -- 264 is the
        // lock's own score, so nothing left can outbid it.
        assert_eq!(best_other_rate(&candidates, &at, "rush", 264), 0);
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
        assert_eq!(
            best_action(&db, &list, &healthy(80)).as_deref(),
            Some("rush")
        );

        let without_barrage: Vec<SkillDef> = jotaro_skills()
            .into_iter()
            .filter(|def| def.id != "rush")
            .collect();
        let list = ids(&without_barrage);
        let db = database(without_barrage);
        assert_eq!(
            best_action(&db, &list, &healthy(80)).as_deref(),
            Some("halt")
        );
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

    /// A situation against a real target: defence 40 (a 20 HP toll), no
    /// resistances, Jotaro's attack. One action is a 100-power hit: 120 - 20.
    fn armoured(sp: i32) -> Situation {
        Situation {
            target_def: 40,
            baseline_hp: 100,
            ..healthy(sp)
        }
    }

    /// Issue #63, the defect in one table: every effect kind against the same
    /// situation, each priced in HP. A one-turn stun, a one-tick-per-action
    /// lock, a 100% slow for one turn and a hit all cost the target exactly
    /// one action's worth; the bleed and the guard are the HP they move.
    #[test]
    fn every_effect_kind_is_priced_in_hp() {
        let at = Situation {
            enemies: 1,
            ..armoured(0)
        };
        let one = |effect: Effect| effect_points(&effect, &at, 1);

        // The hit itself: 120 before the toll, 100 after it.
        assert_eq!(one(damage(100)), 100);
        assert_eq!(
            one(Effect::Drain {
                power: 100,
                element: Element::Physical,
            }),
            100
        );
        // One action denied, three ways, each worth that same hit.
        assert_eq!(one(status(StatusKind::Stun, 0, 1, 100)), 100);
        assert_eq!(
            one(Effect::TempoLock {
                ticks: TICKS_PER_ACTION as u32
            }),
            100
        );
        assert_eq!(one(status(StatusKind::SpdDown, 100, 1, 100)), 100);
        // HP as HP: no conversion by the actor's attack.
        assert_eq!(one(status(StatusKind::Bleed, 25, 3, 100)), 75);
        assert_eq!(one(status(StatusKind::DefUp, 60, 2, 100)), 30);
        // A share of the hit before the toll, which a percentage of attack
        // scales: 25% of 120, not of 100.
        assert_eq!(one(status(StatusKind::AtkDown, 25, 1, 100)), 30);
        assert_eq!(one(status(StatusKind::AtkUp, 25, 2, 100)), 30);
        // Half of the defence the target loses comes off each hit's toll.
        assert_eq!(one(status(StatusKind::DefDown, 50, 1, 100)), 10);
    }

    /// The attack multiplier is not in the price of anything that is already
    /// HP. Halve the actor's attack and the bleed and guard keep their price,
    /// while the hit -- and so every action-valued effect, through
    /// `baseline_hp` -- falls.
    #[test]
    fn only_hit_valued_effects_follow_the_actors_attack() {
        let strong = armoured(0);
        let weak = Situation {
            atk: 60,
            baseline_hp: 40,
            ..armoured(0)
        };
        let bleed = status(StatusKind::Bleed, 25, 3, 100);
        let guard = status(StatusKind::DefUp, 60, 3, 100);
        let stun = status(StatusKind::Stun, 0, 1, 100);

        assert_eq!(effect_points(&bleed, &strong, 1), 75);
        assert_eq!(effect_points(&bleed, &weak, 1), 75);
        assert_eq!(effect_points(&guard, &strong, 1), 120);
        assert_eq!(effect_points(&guard, &weak, 1), 120);
        assert_eq!(effect_points(&damage(100), &weak, 1), 40);
        assert_eq!(effect_points(&stun, &weak, 1), 40);
    }

    /// The price of an action is the best affordable hit against *this*
    /// target, after its defence and its resistance -- so a resisted element
    /// can lose the baseline to a weaker one the target does not resist.
    #[test]
    fn the_hp_baseline_is_the_best_affordable_hit_against_the_target() {
        let skills = vec![
            skill(BASIC_ATTACK_ID, TargetKind::OneEnemy, 0, vec![damage(100)]),
            skill(
                "flare",
                TargetKind::OneEnemy,
                16,
                vec![Effect::Damage {
                    power: 150,
                    element: Element::Fire,
                    variance: 0,
                }],
            ),
        ];
        let list = ids(&skills);
        let db = database(skills);

        // 150 power at atk 120 is 180, minus the 20 toll.
        assert_eq!(baseline_hp(&db, &list, &armoured(16)), 160);
        // Unaffordable: the strike, 120 - 20.
        assert_eq!(baseline_hp(&db, &list, &armoured(15)), 100);
        // Fire resisted 75%: 160 becomes 40, and the strike's 100 wins.
        let mut fireproof = armoured(16);
        fireproof.target_resist.insert(Element::Fire, 75);
        assert_eq!(baseline_hp(&db, &list, &fireproof), 100);
    }

    #[test]
    fn the_hp_baseline_falls_back_to_a_nominal_hit_without_a_basic_attack() {
        let skills = vec![skill("costly", TargetKind::OneEnemy, 45, vec![damage(195)])];
        let list = ids(&skills);
        let db = database(skills);
        // FALLBACK_BASELINE is 100 power: 120 at atk 120, minus the 20 toll.
        assert_eq!(baseline_hp(&db, &list, &armoured(0)), 100);
    }

    /// `priced` fills in the baseline and nothing else.
    #[test]
    fn pricing_a_situation_sets_only_its_baseline() {
        let skills = vec![skill(
            BASIC_ATTACK_ID,
            TargetKind::OneEnemy,
            0,
            vec![damage(100)],
        )];
        let list = ids(&skills);
        let db = database(skills);
        let priced = Situation {
            baseline_hp: 0,
            ..armoured(7)
        }
        .priced(&db, &list);
        assert_eq!(priced.baseline_hp, 100);
        assert_eq!(priced.sp, 7);
        assert_eq!(priced.baseline, 100);
        assert_eq!(priced.target_def, 40);
    }

    /// The #63 measurement, pinned: an actor already under a +35% buff has a
    /// current attack of 162 on a base of 120, and the next +35% must still
    /// be priced at 35% of the base. Priced off the current attack it came to
    /// 56 per turn instead of 42, so every refresh outbid the buff it
    /// refreshed.
    #[test]
    fn an_attack_buff_is_priced_on_the_base_attack_not_the_buffed_one() {
        let rage = status(StatusKind::AtkUp, 35, 2, 100);
        let fresh = healthy(0);
        let buffed = Situation {
            atk: 162,
            ..healthy(0)
        };
        // 100 power at base 120 is 120 HP; 35% of it is 42, for one own turn.
        assert_eq!(effect_points(&rage, &fresh, 1), 42);
        assert_eq!(effect_points(&rage, &buffed, 1), 42);
        // The debuff mirrors it, on the actor's own exchange rate.
        let wither = status(StatusKind::AtkDown, 35, 1, 100);
        assert_eq!(effect_points(&wither, &buffed, 1), 42);
        // The hit itself does use the buffed attack: 162 against no defence.
        assert_eq!(effect_points(&damage(100), &buffed, 1), 162);
    }
}
