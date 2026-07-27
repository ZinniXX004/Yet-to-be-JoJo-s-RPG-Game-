//! Content definitions loaded from `data/*.json`.
//!
//! Nothing here contains game *behavior*; these are inert descriptions that
//! [`crate::resolve`] interprets. Adding a new skill must never require a code
//! change, only a new JSON entry. If you find yourself adding a `match` on a
//! specific skill id, the design has regressed.

use std::collections::HashMap;
use std::fmt;

use serde::{Deserialize, Serialize};

pub type Id = String;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Team {
    Party,
    Foe,
}

impl Team {
    pub fn opposite(self) -> Team {
        match self {
            Team::Party => Team::Foe,
            Team::Foe => Team::Party,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Stats {
    #[serde(default)]
    pub hp: i32,
    #[serde(default)]
    pub sp: i32,
    #[serde(default)]
    pub atk: i32,
    #[serde(default)]
    pub def: i32,
    #[serde(default)]
    pub spd: i32,
    #[serde(default)]
    pub will: i32,
}

impl Stats {
    pub fn plus(self, other: Stats) -> Stats {
        Stats {
            hp: self.hp + other.hp,
            sp: self.sp + other.sp,
            atk: self.atk + other.atk,
            def: self.def + other.def,
            spd: self.spd + other.spd,
            will: self.will + other.will,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Element {
    Physical,
    Fire,
    Ice,
    Electric,
    Psychic,
    Temporal,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StatusKind {
    AtkUp,
    AtkDown,
    DefUp,
    DefDown,
    SpdUp,
    SpdDown,
    Bleed,
    Regen,
    Stun,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TargetKind {
    SelfOnly,
    OneEnemy,
    AllEnemies,
    OneAlly,
    AllAllies,
}

impl TargetKind {
    /// Whether an accuracy roll applies. Buffing your own party never misses;
    /// that is a design decision, encoded once here instead of per skill.
    pub fn is_hostile(self) -> bool {
        matches!(self, TargetKind::OneEnemy | TargetKind::AllEnemies)
    }
}

fn default_percent() -> i32 {
    100
}

fn default_tempo_cost() -> u32 {
    crate::state::TEMPO_THRESHOLD
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Effect {
    Damage {
        power: i32,
        element: Element,
        #[serde(default)]
        variance: i32,
    },
    Heal {
        power: i32,
    },
    /// Damage that returns half of the dealt amount to the actor.
    Drain {
        power: i32,
        element: Element,
    },
    Status {
        status: StatusKind,
        potency: i32,
        duration: u8,
        #[serde(default = "default_percent")]
        chance: i32,
    },
    /// Suspends the target's tempo accumulation for `ticks`. This is the
    /// generic primitive behind time-stop abilities; the scheduler has no
    /// special case for them.
    TempoLock {
        ticks: u32,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillDef {
    pub id: Id,
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub sp_cost: i32,
    pub target: TargetKind,
    #[serde(default = "default_percent")]
    pub accuracy: i32,
    #[serde(default = "default_tempo_cost")]
    pub tempo_cost: u32,
    pub effects: Vec<Effect>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StandDef {
    pub id: Id,
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub bonus: Stats,
    #[serde(default)]
    pub skills: Vec<Id>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AiProfile {
    #[default]
    Aggressive,
    Support,
    Trickster,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CombatantDef {
    pub id: Id,
    pub name: String,
    pub team: Team,
    pub stats: Stats,
    #[serde(default)]
    pub stand: Option<Id>,
    #[serde(default)]
    pub skills: Vec<Id>,
    #[serde(default)]
    pub ai: AiProfile,
}

#[derive(Debug)]
pub enum DataError {
    Parse(String),
    Invalid(Vec<String>),
}

impl fmt::Display for DataError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DataError::Parse(message) => write!(f, "content parse error: {message}"),
            DataError::Invalid(issues) => {
                writeln!(f, "content validation failed ({} issue(s)):", issues.len())?;
                for issue in issues {
                    writeln!(f, "  - {issue}")?;
                }
                Ok(())
            }
        }
    }
}

impl std::error::Error for DataError {}

/// Indexed, validated content. Construction is fallible on purpose: a database
/// that exists is a database whose references all resolve, so lookup code never
/// has to defend against dangling ids.
#[derive(Clone, Debug, Default)]
pub struct Database {
    stands: HashMap<Id, StandDef>,
    skills: HashMap<Id, SkillDef>,
    combatants: HashMap<Id, CombatantDef>,
}

impl Database {
    pub fn new(
        stands: Vec<StandDef>,
        skills: Vec<SkillDef>,
        combatants: Vec<CombatantDef>,
    ) -> Result<Self, DataError> {
        let mut db = Database::default();
        let mut issues: Vec<String> = Vec::new();

        for def in skills {
            if db.skills.insert(def.id.clone(), def.clone()).is_some() {
                issues.push(format!("duplicate skill id '{}'", def.id));
            }
        }
        for def in stands {
            if db.stands.insert(def.id.clone(), def.clone()).is_some() {
                issues.push(format!("duplicate stand id '{}'", def.id));
            }
        }
        for def in combatants {
            if db.combatants.insert(def.id.clone(), def.clone()).is_some() {
                issues.push(format!("duplicate combatant id '{}'", def.id));
            }
        }

        db.collect_reference_issues(&mut issues);
        if issues.is_empty() {
            Ok(db)
        } else {
            issues.sort();
            Err(DataError::Invalid(issues))
        }
    }

    pub fn from_json(
        stands_json: &str,
        skills_json: &str,
        combatants_json: &str,
    ) -> Result<Self, DataError> {
        let stands: Vec<StandDef> = serde_json::from_str(stands_json)
            .map_err(|e| DataError::Parse(format!("stands: {e}")))?;
        let skills: Vec<SkillDef> = serde_json::from_str(skills_json)
            .map_err(|e| DataError::Parse(format!("skills: {e}")))?;
        let combatants: Vec<CombatantDef> = serde_json::from_str(combatants_json)
            .map_err(|e| DataError::Parse(format!("combatants: {e}")))?;
        Database::new(stands, skills, combatants)
    }

    pub fn skill(&self, id: &str) -> Option<&SkillDef> {
        self.skills.get(id)
    }

    pub fn stand(&self, id: &str) -> Option<&StandDef> {
        self.stands.get(id)
    }

    pub fn combatant(&self, id: &str) -> Option<&CombatantDef> {
        self.combatants.get(id)
    }

    pub fn counts(&self) -> (usize, usize, usize) {
        (self.stands.len(), self.skills.len(), self.combatants.len())
    }

    /// Resolves a combatant's full skill list: its own skills followed by the
    /// skills granted by its stand, order-stable and de-duplicated. Order
    /// matters because it is surfaced directly in the command menu.
    pub fn skills_for(&self, def: &CombatantDef) -> Vec<Id> {
        let mut out: Vec<Id> = Vec::new();
        let mut push = |id: &Id, out: &mut Vec<Id>| {
            if !out.iter().any(|existing| existing == id) {
                out.push(id.clone());
            }
        };
        for id in &def.skills {
            push(id, &mut out);
        }
        if let Some(stand_id) = &def.stand {
            if let Some(stand) = self.stands.get(stand_id) {
                for id in &stand.skills {
                    push(id, &mut out);
                }
            }
        }
        out
    }

    fn collect_reference_issues(&self, issues: &mut Vec<String>) {
        for stand in self.stands.values() {
            for skill in &stand.skills {
                if !self.skills.contains_key(skill) {
                    issues.push(format!(
                        "stand '{}' references unknown skill '{skill}'",
                        stand.id
                    ));
                }
            }
        }
        for def in self.combatants.values() {
            if let Some(stand) = &def.stand {
                if !self.stands.contains_key(stand) {
                    issues.push(format!(
                        "combatant '{}' references unknown stand '{stand}'",
                        def.id
                    ));
                }
            }
            for skill in &def.skills {
                if !self.skills.contains_key(skill) {
                    issues.push(format!(
                        "combatant '{}' references unknown skill '{skill}'",
                        def.id
                    ));
                }
            }
            if def.stats.hp <= 0 {
                issues.push(format!("combatant '{}' has non-positive hp", def.id));
            }
            if def.stats.spd <= 0 {
                issues.push(format!(
                    "combatant '{}' has non-positive spd and could never act",
                    def.id
                ));
            }
            if self.skills_for(def).is_empty() {
                issues.push(format!("combatant '{}' has no usable skills", def.id));
            }
        }
    }
}
