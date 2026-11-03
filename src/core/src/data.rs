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

/// Every content struct below refuses a key it does not declare.
///
/// serde's default is to drop one, and that is how `npc.diavolo` fought from
/// #17 until 0.4.1 with no resistance table: its data said `"resistances"`, the
/// struct reads `resist`, and nothing complained. `validate_data.py` rejects an
/// unknown key too, so the two checks agree, and a key only one of them knows
/// fails the other one in CI instead of drifting apart silently (#65).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
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

/// `Hash` is derived because an element is a key in a resistance table; see
/// [`Resistances`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Element {
    Physical,
    Fire,
    Ice,
    Electric,
    Psychic,
    Temporal,
}

/// Per-element damage modifiers, in percent of the incoming hit.
///
/// Positive removes damage, negative adds it, and an absent key means zero.
/// Absence rather than a required full table is deliberate: most combatants
/// care about one or two elements, and a table that has to list all six is a
/// table people copy-paste wrongly.
pub type Resistances = HashMap<Element, i32>;

/// Bounds enforced on every entry of a [`Resistances`] table.
///
/// The upper bound is total immunity to the element's *modifier*, not to the
/// hit: the damage floor in [`crate::resolve`] still applies, so 100 cannot
/// stall a fight the way a true immunity would. The lower bound caps a
/// vulnerability at double damage, which is already the largest multiplier any
/// other system in the game can produce.
pub const MAX_RESISTANCE: i32 = 100;
pub const MIN_RESISTANCE: i32 = -100;

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

/// `deny_unknown_fields` applies per variant here: a `damage` effect may carry
/// `kind`, `power`, `element` and `variance`, and nothing else.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
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

/// Where an entry's numbers came from; see `docs/DATA-SOURCES.md` section 2.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProvenanceOrigin {
    /// Written for this project. An entry with no `provenance` block is this
    /// by definition, so the block is only worth writing for the other two.
    HandAuthored,
    /// Computed from an external source by a stated transform.
    Derived,
    /// Copied from an external source as it stands.
    Imported,
}

/// The optional `provenance` block on a skill, stand or combatant.
///
/// Inert: no rule reads it. It is declared, rather than tolerated as an unknown
/// key, so that the loader can refuse every other unknown key and still load an
/// entry that documents its source. `origin` is required because a block that
/// does not say what kind of source it describes says nothing; the other
/// fields are free text, and which of them a `derived` or `imported` entry
/// must fill in is a validator rule that waits for the first such entry.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Provenance {
    pub origin: ProvenanceOrigin,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_url: Option<String>,
    /// The date the source was read, as written in the file. Kept a string:
    /// the core has no clock and no use for a parsed date.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retrieved: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub licence: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transform: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provenance: Option<Provenance>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StandDef {
    pub id: Id,
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub bonus: Stats,
    #[serde(default)]
    pub skills: Vec<Id>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provenance: Option<Provenance>,
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
#[serde(deny_unknown_fields)]
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
    /// Resistance lives on the combatant, not on the stand, because it is a
    /// property of the body being hit. A stand grants skills and stat bonuses;
    /// it does not make its user fireproof.
    #[serde(default)]
    pub resist: Resistances,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provenance: Option<Provenance>,
}

impl CombatantDef {
    /// Resistance to `element` in percent, zero when the table says nothing.
    ///
    /// Reading through an accessor rather than the map keeps "absent means
    /// neutral" in one place; the resolver never sees an `Option`.
    pub fn resistance(&self, element: Element) -> i32 {
        self.resist.get(&element).copied().unwrap_or(0)
    }
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
        // Not `let mut push`: the closure mutates through its `&mut Vec<Id>`
        // parameter rather than capturing anything, so the binding is immutable.
        // Passing the vector explicitly is what keeps the borrow checker happy
        // while `out` is also read inside the loop.
        let push = |id: &Id, out: &mut Vec<Id>| {
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
            // Sorted so that a combatant with two bad entries reports them in a
            // stable order; `issues` as a whole is sorted later, but these two
            // lines would otherwise differ per run through the HashMap.
            let mut out_of_range: Vec<String> = def
                .resist
                .iter()
                .filter(|(_, percent)| !(MIN_RESISTANCE..=MAX_RESISTANCE).contains(percent))
                .map(|(element, percent)| format!("{element:?} at {percent}"))
                .collect();
            out_of_range.sort();
            for entry in out_of_range {
                issues.push(format!(
                    "combatant '{}' has a resistance outside {MIN_RESISTANCE}..={MAX_RESISTANCE}: {entry}",
                    def.id
                ));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const STRIKE: &str = r#"{
        "id": "skill.strike",
        "name": "Strike",
        "target": "one_enemy",
        "effects": [{ "kind": "damage", "power": 100, "element": "physical" }]
    }"#;

    const HERO: &str = r#"{
        "id": "pc.hero",
        "name": "Hero",
        "team": "party",
        "stats": { "hp": 100, "atk": 10, "def": 10, "spd": 100, "will": 10 },
        "skills": ["skill.strike"]
    }"#;

    /// A block with every field `docs/DATA-SOURCES.md` section 2 names.
    const FULL_PROVENANCE: &str = r#"{
        "origin": "derived",
        "source": "Example wiki",
        "source_url": "https://example.org/stand",
        "retrieved": "2026-07-27",
        "licence": "CC BY-SA 4.0",
        "transform": "speed rank A mapped to spd 120"
    }"#;

    fn load(stands: &str, skills: &str, combatants: &str) -> Result<Database, DataError> {
        Database::from_json(
            &format!("[{stands}]"),
            &format!("[{skills}]"),
            &format!("[{combatants}]"),
        )
    }

    /// The parse message for `json`, which must fail to load.
    fn parse_error(stands: &str, skills: &str, combatants: &str) -> String {
        match load(stands, skills, combatants) {
            Err(DataError::Parse(message)) => message,
            Err(other) => panic!("expected a parse error, got: {other}"),
            Ok(_) => panic!("content with an unknown key must not load"),
        }
    }

    /// Inserts `extra` as the first key of the JSON object `object`.
    fn with_key(object: &str, extra: &str) -> String {
        object.replacen('{', &format!("{{ {extra},"), 1)
    }

    #[test]
    fn the_fixture_loads_without_provenance() {
        let db = load("", STRIKE, HERO).expect("fixture content must load");
        assert_eq!(db.counts(), (0, 1, 1));
    }

    /// `accuracy`, `tempo_cost` and a status's `chance` may be left out of
    /// the content (the validator allows it, and `STRIKE` above does), so
    /// their defaults are part of the schema: a sure hit, one full turn, a
    /// sure application.
    #[test]
    fn omitted_optional_numbers_take_their_defaults() {
        let hex = r#"{
            "id": "skill.hex",
            "name": "Hex",
            "target": "one_enemy",
            "effects": [{ "kind": "status", "status": "atk_down", "potency": 10, "duration": 2 }]
        }"#;
        let db = load("", &format!("{STRIKE}, {hex}"), HERO).expect("the content loads");
        let strike = db.skill("skill.strike").expect("the strike loaded");
        assert_eq!(strike.accuracy, 100);
        assert_eq!(strike.tempo_cost, 1000);
        match db
            .skill("skill.hex")
            .expect("the hex loaded")
            .effects
            .as_slice()
        {
            [Effect::Status { chance, .. }] => assert_eq!(*chance, 100),
            other => panic!("expected one status effect, got {other:?}"),
        }
    }

    /// The loader's own reference check, which is the only one on the path
    /// from Godot: the bridge loads content without the Python validator.
    #[test]
    fn a_reference_to_missing_content_fails_the_load_and_names_it() {
        let stand = r#"{ "id": "stand.ghost", "name": "Ghost", "skills": ["skill.nothing"] }"#;
        let hero = HERO.replace(
            r#""skills": ["skill.strike"]"#,
            r#""stand": "stand.absent", "skills": ["skill.strike", "skill.missing"]"#,
        );
        match load(stand, STRIKE, &hero) {
            Err(DataError::Invalid(issues)) => {
                for expected in [
                    "stand 'stand.ghost' references unknown skill 'skill.nothing'",
                    "combatant 'pc.hero' references unknown stand 'stand.absent'",
                    "combatant 'pc.hero' references unknown skill 'skill.missing'",
                ] {
                    assert!(issues.iter().any(|issue| issue == expected), "{issues:?}");
                }
            }
            Err(other) => panic!("expected a validation error, got: {other}"),
            Ok(_) => panic!("content with dangling references must not load"),
        }
    }

    #[test]
    fn a_load_error_prints_every_issue_on_its_own_line() {
        let invalid = DataError::Invalid(vec!["first".to_string(), "second".to_string()]);
        assert_eq!(
            invalid.to_string(),
            "content validation failed (2 issue(s)):\n  - first\n  - second\n"
        );
        let parse = DataError::Parse("skills: oops".to_string());
        assert_eq!(parse.to_string(), "content parse error: skills: oops");
    }

    /// The real defect: `npc.diavolo` wrote `resistances` from #17 to 0.4.1,
    /// serde dropped it, and the boss fought with no table.
    #[test]
    fn a_misspelled_combatant_key_fails_the_load_and_names_the_key() {
        let hero = with_key(HERO, r#""resistances": { "physical": 20 }"#);
        let message = parse_error("", STRIKE, &hero);
        assert!(message.starts_with("combatants:"), "{message}");
        assert!(message.contains("unknown field `resistances`"), "{message}");
    }

    /// The attribute is per variant on the internally tagged enum: `potency`
    /// is a key of `status`, not of `damage`.
    #[test]
    fn a_key_of_another_effect_kind_fails_the_load_and_names_the_key() {
        let skill = STRIKE.replace(r#""power": 100"#, r#""power": 100, "potency": 30"#);
        let message = parse_error("", &skill, HERO);
        assert!(message.starts_with("skills:"), "{message}");
        assert!(message.contains("unknown field `potency`"), "{message}");
    }

    #[test]
    fn a_misspelled_stat_fails_the_load_and_names_the_key() {
        let hero = HERO.replace(r#""spd": 100"#, r#""spd": 100, "speed": 120"#);
        let message = parse_error("", STRIKE, &hero);
        assert!(message.contains("unknown field `speed`"), "{message}");
    }

    #[test]
    fn a_misspelled_stand_key_fails_the_load_and_names_the_key() {
        let stand = r#"{ "id": "stand.example", "name": "Example", "skill": ["skill.strike"] }"#;
        let message = parse_error(stand, STRIKE, HERO);
        assert!(message.starts_with("stands:"), "{message}");
        assert!(message.contains("unknown field `skill`"), "{message}");
    }

    #[test]
    fn a_full_provenance_block_loads_on_all_three_kinds() {
        let block = format!(r#""provenance": {FULL_PROVENANCE}"#);
        let stand = with_key(
            r#"{ "id": "stand.example", "name": "Example", "skills": ["skill.strike"] }"#,
            &block,
        );
        let skill = with_key(STRIKE, &block);
        let hero = with_key(HERO, &block);
        let db = load(&stand, &skill, &hero).expect("a declared provenance block must load");

        let expected = Provenance {
            origin: ProvenanceOrigin::Derived,
            source: Some("Example wiki".to_string()),
            source_url: Some("https://example.org/stand".to_string()),
            retrieved: Some("2026-07-27".to_string()),
            licence: Some("CC BY-SA 4.0".to_string()),
            transform: Some("speed rank A mapped to spd 120".to_string()),
        };
        let stand = db.stand("stand.example").expect("stand");
        let skill = db.skill("skill.strike").expect("skill");
        let hero = db.combatant("pc.hero").expect("combatant");
        assert_eq!(stand.provenance.as_ref(), Some(&expected));
        assert_eq!(skill.provenance.as_ref(), Some(&expected));
        assert_eq!(hero.provenance.as_ref(), Some(&expected));
    }

    #[test]
    fn every_origin_value_loads_under_its_documented_spelling() {
        for (text, origin) in [
            ("hand-authored", ProvenanceOrigin::HandAuthored),
            ("derived", ProvenanceOrigin::Derived),
            ("imported", ProvenanceOrigin::Imported),
        ] {
            let block = format!(r#""provenance": {{ "origin": "{text}" }}"#);
            let db = load("", STRIKE, &with_key(HERO, &block)).expect(text);
            let provenance = db.combatant("pc.hero").and_then(|c| c.provenance.clone());
            assert_eq!(provenance.map(|p| p.origin), Some(origin), "{text}");
        }
    }

    #[test]
    fn an_unknown_key_inside_provenance_fails_the_load_and_names_the_key() {
        let block = r#""provenance": { "origin": "derived", "url": "https://example.org" }"#;
        let message = parse_error("", &with_key(STRIKE, block), HERO);
        assert!(message.contains("unknown field `url`"), "{message}");
    }

    #[test]
    fn an_unknown_origin_fails_the_load() {
        let block = r#""provenance": { "origin": "hand_authored" }"#;
        let message = parse_error("", STRIKE, &with_key(HERO, block));
        assert!(
            message.contains("unknown variant `hand_authored`"),
            "{message}"
        );
    }

    #[test]
    fn a_provenance_block_without_origin_fails_the_load() {
        let block = r#""provenance": { "source": "Example wiki" }"#;
        let message = parse_error("", STRIKE, &with_key(HERO, block));
        assert!(message.contains("missing field `origin`"), "{message}");
    }

    /// An entry without the block serialises exactly as it did before the
    /// field existed, so nothing that writes these structs back out changes.
    #[test]
    fn nothing_is_serialised_when_provenance_is_absent() {
        let db = load("", STRIKE, HERO).expect("fixture content must load");
        let skill = serde_json::to_value(db.skill("skill.strike")).expect("skill");
        let hero = serde_json::to_value(db.combatant("pc.hero")).expect("combatant");
        assert!(skill.get("provenance").is_none(), "{skill}");
        assert!(hero.get("provenance").is_none(), "{hero}");
    }

    #[test]
    fn only_the_fields_present_are_serialised_inside_provenance() {
        let block = r#""provenance": { "origin": "imported", "licence": "CC0" }"#;
        let db = load("", STRIKE, &with_key(HERO, block)).expect("must load");
        let hero = serde_json::to_value(db.combatant("pc.hero")).expect("combatant");
        assert_eq!(
            hero["provenance"],
            serde_json::json!({ "origin": "imported", "licence": "CC0" })
        );
    }
}
