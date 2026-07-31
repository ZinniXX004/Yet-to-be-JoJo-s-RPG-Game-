#!/usr/bin/env python3
"""Validate the content data that drives the battle core.

This exists because the Rust core deserializes `data/*.json` at load time, and a
broken reference there would only surface as a runtime failure inside the game.
Catching it in CI is cheaper.

Checks performed:
  1. Every file parses as JSON and is a top-level list of objects.
  2. Required keys are present and correctly typed.
  3. Enum-valued fields use values the Rust core actually accepts.
  4. Cross-file references resolve (stand -> skills, combatant -> stand/skills,
     matchup -> combatants).
  5. IDs are unique and follow the `namespace.name` convention.
  6. Soft balance warnings (unreachable skills, resistances to elements nothing
     deals, zero-cost nukes, encounters whose declaration cannot produce a
     meaningful measurement).
  7. With `--mirror`, every combatant that also exists in a reference directory
     is identical to the entry there, field for field.

`tools/probe/` holds files of the same shape that are deliberately absurd and are
never shipped; see `tools/probe/README.md`. Checking that directory needs the
`--combatants` and `--matchups` overrides, because its files are named
`*.probe.json` and it has no `skills.json` or `stands.json` of its own -- it
borrows the shipped ones. An earlier version of this docstring claimed a bare
directory argument was "how tools/probe/ is checked by hand"; that never worked,
and the run died on four missing files before checking anything.

Checks 1 to 6 look at one set of files in isolation. That is not enough for a
hand-maintained copy: an entry can satisfy every one of them and still have
stopped describing the game, which is exactly what happened in issue #14. Check
7 is the only one that can notice, because the information it needs is in
another directory.

Usage:
    python3 src/data-pipeline/validate_data.py [data_dir]

    python3 src/data-pipeline/validate_data.py \\
        --combatants tools/probe/combatants.probe.json \\
        --matchups   tools/probe/matchups.probe.json \\
        --mirror     data

With no argument the content directory is located relative to this file, so the
command works from any working directory. Individual files may be overridden;
anything not overridden is read from the content directory.

Exit codes:
    0 = valid (warnings may still be printed)
    1 = validation errors found
    2 = usage or I/O error
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path
from typing import Any

ELEMENTS = {"physical", "fire", "ice", "electric", "psychic", "temporal"}
TARGETS = {"self_only", "one_enemy", "all_enemies", "one_ally", "all_allies"}
TEAMS = {"party", "foe"}
AI_PROFILES = {"aggressive", "support", "trickster"}
STATUSES = {
    "atk_up",
    "atk_down",
    "def_up",
    "def_down",
    "spd_up",
    "spd_down",
    "bleed",
    "regen",
    "stun",
}
EFFECT_KINDS = {"damage", "heal", "drain", "status", "tempo_lock"}
STAT_KEYS = {"hp", "sp", "atk", "def", "spd", "will"}

# The four content files, in the order they must be validated: later ones
# reference ids defined by earlier ones.
CONTENT_FILES = ("skills", "stands", "combatants", "matchups")

# Mirrors MIN_RESISTANCE and MAX_RESISTANCE in src/core/src/data.rs. The core
# clamps to the same interval, so a value outside it is not dangerous -- it is
# simply a number that does not mean what its author thinks it means.
MIN_RESISTANCE = -100
MAX_RESISTANCE = 100

# Twelve seeds give a resolution of 8.3 percentage points per battle. Eight is
# the point below which a single seed flipping moves the reported win rate by
# more than 12 points, which is wider than most declared bands are forgiving.
MIN_USEFUL_SEEDS = 8

# Fields that must agree when a combatant is mirrored from another directory.
# `id` is excluded because it is the key the two entries are matched on.
#
# `resist` is in this list for the reason the whole comparison exists: it is
# `#[serde(default)]` in the core, so a file that omits it loads clean, and an
# omitted table is indistinguishable from a table of zeroes to every other check
# in this script.
MIRROR_FIELDS = ("name", "team", "stats", "stand", "skills", "ai", "resist")

# Absent and empty mean the same thing to serde for these, so they must mean the
# same thing here. Without this, a combatant with no resistances would be
# reported as differing from an identical one that spells the emptiness out.
# `stand` is deliberately absent from this map: null is a real value there --
# npc.thug has no Stand -- and must compare equal only to another null.
MIRROR_DEFAULTS: dict[str, Any] = {"resist": {}, "skills": []}

# This file lives at <repo>/src/data-pipeline/, so the content directory is two
# levels up. Resolving it from __file__ rather than from the process working
# directory is the difference between a validator that runs from anywhere and
# one that only runs from the repository root.
REPO_ROOT = Path(__file__).resolve().parents[2]


def default_data_dir() -> Path:
    """The content directory to validate when no path is given.

    Prefers the copy that belongs to this checkout. The cwd-relative fallback
    exists so that a script copied out of the tree still behaves the way it
    always did instead of failing on a path the caller never typed.
    """
    candidate = REPO_ROOT / "data"
    if candidate.is_dir():
        return candidate
    return Path("data")


class Report:
    def __init__(self) -> None:
        self.errors: list[str] = []
        self.warnings: list[str] = []

    def error(self, where: str, message: str) -> None:
        self.errors.append(f"{where}: {message}")

    def warn(self, where: str, message: str) -> None:
        self.warnings.append(f"{where}: {message}")


def load_list(path: Path, report: Report) -> list[dict[str, Any]]:
    try:
        payload = json.loads(path.read_text(encoding="utf-8"))
    except FileNotFoundError:
        report.error(path.name, "file not found")
        return []
    except json.JSONDecodeError as exc:
        report.error(path.name, f"invalid JSON at line {exc.lineno}: {exc.msg}")
        return []

    if not isinstance(payload, list):
        report.error(path.name, "top level must be a JSON array")
        return []

    rows: list[dict[str, Any]] = []
    for index, row in enumerate(payload):
        if not isinstance(row, dict):
            report.error(f"{path.name}[{index}]", "entry must be an object")
            continue
        rows.append(row)
    return rows


def check_ids(rows: list[dict[str, Any]], filename: str, report: Report) -> set[str]:
    seen: set[str] = set()
    for index, row in enumerate(rows):
        where = f"{filename}[{index}]"
        ident = row.get("id")
        if not isinstance(ident, str) or not ident:
            report.error(where, "missing or non-string 'id'")
            continue
        if ident in seen:
            report.error(where, f"duplicate id '{ident}'")
        if "." not in ident:
            report.warn(where, f"id '{ident}' should follow 'namespace.name'")
        seen.add(ident)
    return seen


def check_int(row: dict[str, Any], key: str, where: str, report: Report,
              required: bool = True, low: int | None = None,
              high: int | None = None) -> None:
    if key not in row:
        if required:
            report.error(where, f"missing required integer '{key}'")
        return
    value = row[key]
    if isinstance(value, bool) or not isinstance(value, int):
        report.error(where, f"'{key}' must be an integer, got {type(value).__name__}")
        return
    if low is not None and value < low:
        report.error(where, f"'{key}' = {value} is below minimum {low}")
    if high is not None and value > high:
        report.error(where, f"'{key}' = {value} is above maximum {high}")


def check_enum(row: dict[str, Any], key: str, allowed: set[str], where: str,
               report: Report, required: bool = True) -> None:
    if key not in row:
        if required:
            report.error(where, f"missing required '{key}'")
        return
    value = row[key]
    if value not in allowed:
        report.error(
            where,
            f"'{key}' = {value!r} is not one of {sorted(allowed)}",
        )


def check_stats(block: Any, where: str, report: Report) -> None:
    if not isinstance(block, dict):
        report.error(where, "'stats' must be an object")
        return
    unknown = set(block) - STAT_KEYS
    if unknown:
        report.error(where, f"unknown stat keys: {sorted(unknown)}")
    for key in ("hp", "atk", "def", "spd", "will"):
        check_int(block, key, f"{where}.stats", report, required=True, low=1)
    check_int(block, "sp", f"{where}.stats", report, required=False, low=0)


def check_resistances(row: dict[str, Any], where: str, report: Report,
                      elements_dealt: set[str]) -> None:
    """Validates one combatant's optional `resist` table.

    An unknown key is an error rather than a warning on purpose: serde ignores
    it, so a misspelled element produces a combatant that silently has no
    resistance at all, which is indistinguishable in the harness from a
    resistance that is too weak to matter.

    Note what this cannot do: an entirely absent table is legal and returns
    immediately, because most combatants genuinely have none. That is why the
    probe roster went two milestones with every table missing and no check in
    this file complained. See `check_mirror`.
    """
    if "resist" not in row:
        return
    block = row["resist"]
    if not isinstance(block, dict):
        report.error(where, "'resist' must be an object")
        return

    unknown = sorted(set(block) - ELEMENTS)
    if unknown:
        report.error(where, f"unknown resist elements: {unknown}")

    for element in sorted(set(block) & ELEMENTS):
        check_int(block, element, f"{where}.resist", report, required=True,
                  low=MIN_RESISTANCE, high=MAX_RESISTANCE)
        if element not in elements_dealt:
            report.warn(
                where,
                f"resistance to '{element}' cannot be measured: no skill"
                " deals that element",
            )


def validate_skills(rows: list[dict[str, Any]], filename: str,
                    report: Report) -> tuple[set[str], set[str]]:
    """Returns the skill ids and the set of elements the skills actually deal."""
    ids = check_ids(rows, filename, report)
    elements_dealt: set[str] = set()
    for index, row in enumerate(rows):
        where = f"{filename}[{index}] ({row.get('id', '?')})"
        if not isinstance(row.get("name"), str):
            report.error(where, "missing or non-string 'name'")
        check_int(row, "sp_cost", where, report, required=False, low=0)
        check_int(row, "accuracy", where, report, required=False, low=1, high=100)
        check_int(row, "tempo_cost", where, report, required=False, low=1)
        check_enum(row, "target", TARGETS, where, report)

        effects = row.get("effects")
        if not isinstance(effects, list) or not effects:
            report.error(where, "'effects' must be a non-empty array")
            continue

        total_power = 0
        for e_index, effect in enumerate(effects):
            e_where = f"{where}.effects[{e_index}]"
            if not isinstance(effect, dict):
                report.error(e_where, "effect must be an object")
                continue
            kind = effect.get("kind")
            if kind not in EFFECT_KINDS:
                report.error(e_where, f"unknown effect kind {kind!r}")
                continue
            if kind in ("damage", "drain"):
                check_int(effect, "power", e_where, report, required=True, low=0)
                check_enum(effect, "element", ELEMENTS, e_where, report)
                check_int(effect, "variance", e_where, report, required=False,
                          low=0, high=99)
                if effect.get("element") in ELEMENTS:
                    elements_dealt.add(effect["element"])
                total_power += int(effect.get("power") or 0)
            elif kind == "heal":
                check_int(effect, "power", e_where, report, required=True, low=1)
            elif kind == "status":
                check_enum(effect, "status", STATUSES, e_where, report)
                check_int(effect, "potency", e_where, report, required=True, low=0)
                check_int(effect, "duration", e_where, report, required=True,
                          low=1, high=99)
                check_int(effect, "chance", e_where, report, required=False,
                          low=1, high=100)
            elif kind == "tempo_lock":
                check_int(effect, "ticks", e_where, report, required=True,
                          low=1, high=60)

        if int(row.get("sp_cost") or 0) == 0 and total_power > 130:
            report.warn(where, f"free skill with total power {total_power}"
                               " dominates the basic attack")
    return ids, elements_dealt


def validate_stands(rows: list[dict[str, Any]], filename: str,
                    skill_ids: set[str],
                    report: Report) -> tuple[set[str], set[str]]:
    ids = check_ids(rows, filename, report)
    referenced: set[str] = set()
    for index, row in enumerate(rows):
        where = f"{filename}[{index}] ({row.get('id', '?')})"
        if not isinstance(row.get("name"), str):
            report.error(where, "missing or non-string 'name'")
        bonus = row.get("bonus", {})
        if not isinstance(bonus, dict):
            report.error(where, "'bonus' must be an object")
        else:
            unknown = set(bonus) - STAT_KEYS
            if unknown:
                report.error(where, f"unknown bonus keys: {sorted(unknown)}")
        skills = row.get("skills", [])
        if not isinstance(skills, list):
            report.error(where, "'skills' must be an array")
            continue
        for skill in skills:
            if skill not in skill_ids:
                report.error(where, f"references unknown skill '{skill}'")
            else:
                referenced.add(skill)
    return ids, referenced


def validate_combatants(rows: list[dict[str, Any]], filename: str,
                        stand_ids: set[str], skill_ids: set[str],
                        elements_dealt: set[str],
                        report: Report) -> tuple[dict[str, str], set[str]]:
    """Returns the combatant id -> team map and the set of skills granted."""
    check_ids(rows, filename, report)
    referenced: set[str] = set()
    team_of: dict[str, str] = {}
    teams: dict[str, int] = {"party": 0, "foe": 0}
    for index, row in enumerate(rows):
        where = f"{filename}[{index}] ({row.get('id', '?')})"
        if not isinstance(row.get("name"), str):
            report.error(where, "missing or non-string 'name'")
        check_enum(row, "team", TEAMS, where, report)
        check_enum(row, "ai", AI_PROFILES, where, report, required=False)
        if row.get("team") in teams:
            teams[row["team"]] += 1
        if isinstance(row.get("id"), str) and row.get("team") in TEAMS:
            team_of[row["id"]] = row["team"]
        check_stats(row.get("stats"), where, report)
        check_resistances(row, where, report, elements_dealt)

        stand = row.get("stand")
        if stand is not None and stand not in stand_ids:
            report.error(where, f"references unknown stand '{stand}'")

        skills = row.get("skills", [])
        if not isinstance(skills, list):
            report.error(where, "'skills' must be an array")
            continue
        if not skills and stand is None:
            report.error(where, "has no stand and no skills, so it can never act")
        for skill in skills:
            if skill not in skill_ids:
                report.error(where, f"references unknown skill '{skill}'")
            else:
                referenced.add(skill)

    for team, count in teams.items():
        if count == 0:
            report.warn(filename, f"no combatants on team '{team}'")
    return team_of, referenced


def check_side(row: dict[str, Any], key: str, expected_team: str,
               team_of: dict[str, str], where: str,
               report: Report) -> list[str]:
    """Validates one side of an encounter and returns the ids it holds."""
    side = row.get(key)
    if not isinstance(side, list) or not side:
        report.error(where, f"'{key}' must be a non-empty array")
        return []

    members: list[str] = []
    for member in side:
        if not isinstance(member, str):
            report.error(where, f"'{key}' contains a non-string entry {member!r}")
            continue
        if member not in team_of:
            report.error(where, f"'{key}' references unknown combatant '{member}'")
            continue
        # Legal -- the harness places a combatant on whichever side names it --
        # but a foe listed under 'party' is nearly always a copy-paste error,
        # and it silently changes which AI profile fights for whom.
        if team_of[member] != expected_team:
            report.warn(
                where,
                f"'{member}' is declared team '{team_of[member]}' in"
                f" the combatant file but fights under '{key}' here",
            )
        if member in members:
            report.error(where, f"'{member}' is listed twice under '{key}'")
        members.append(member)
    return members


def validate_matchups(rows: list[dict[str, Any]], filename: str,
                      team_of: dict[str, str], report: Report) -> set[str]:
    """Returns the set of combatants that at least one encounter exercises.

    The Rust harness (`tests/balance_bounds.rs`) asserts most of this too, but
    only after compiling the crate and simulating three hundred battles per
    encounter. A malformed declaration should be reported in the second it takes
    to read the file, and a duplicated seed in particular is otherwise counted
    as two independent samples of the same battle.
    """
    check_ids(rows, filename, report)
    exercised: set[str] = set()

    for index, row in enumerate(rows):
        where = f"{filename}[{index}] ({row.get('id', '?')})"
        if not isinstance(row.get("name"), str):
            report.error(where, "missing or non-string 'name'")
        if not isinstance(row.get("description", ""), str):
            report.error(where, "'description' must be a string when present")

        party = check_side(row, "party", "party", team_of, where, report)
        foes = check_side(row, "foes", "foe", team_of, where, report)
        both = set(party) & set(foes)
        if both:
            report.error(where, f"combatant(s) on both sides: {sorted(both)}")
        exercised.update(party)
        exercised.update(foes)

        seeds = row.get("seeds")
        if not isinstance(seeds, list) or not seeds:
            report.error(where, "'seeds' must be a non-empty array")
        else:
            seen_seeds: set[int] = set()
            for seed in seeds:
                if isinstance(seed, bool) or not isinstance(seed, int):
                    report.error(where, f"seed {seed!r} is not an integer")
                    continue
                if seed in seen_seeds:
                    # Two identical seeds replay one battle and report it as
                    # two, which quietly biases the win rate.
                    report.error(where, f"seed {seed} appears more than once")
                seen_seeds.add(seed)
            if len(seeds) < MIN_USEFUL_SEEDS:
                report.warn(
                    where,
                    f"{len(seeds)} seeds give a resolution of"
                    f" {round(100 / len(seeds))} percentage points per battle",
                )

        band = row.get("band")
        if not isinstance(band, dict):
            report.error(where, "'band' must be an object")
        else:
            unknown = set(band) - {"min_percent", "max_percent"}
            if unknown:
                report.error(where, f"unknown band keys: {sorted(unknown)}")
            b_where = f"{where}.band"
            check_int(band, "min_percent", b_where, report, low=0, high=100)
            check_int(band, "max_percent", b_where, report, low=0, high=100)
            low = band.get("min_percent")
            high = band.get("max_percent")
            if isinstance(low, int) and isinstance(high, int):
                if low > high:
                    report.error(b_where, f"min_percent {low} exceeds"
                                          f" max_percent {high}")
                elif low == 0 and high == 100:
                    report.warn(b_where, "a 0..100 band declares no intent and"
                                         " can never fail")

        check_int(row, "max_turns", where, report, required=False, low=1)

    for combatant in sorted(set(team_of) - exercised):
        report.warn(filename, f"'{combatant}' appears in no encounter,"
                              " so nothing measures it")
    return exercised


def normalise_for_mirror(row: dict[str, Any]) -> dict[str, Any]:
    """Reduces a combatant to the fields a mirrored copy must reproduce."""
    normalised: dict[str, Any] = {}
    for key in MIRROR_FIELDS:
        value = row.get(key)
        if value is None:
            value = MIRROR_DEFAULTS.get(key)
        normalised[key] = value
    return normalised


def check_mirror(rows: list[dict[str, Any]], filename: str,
                 reference_dir: Path, report: Report) -> int:
    """Compares mirrored combatants against the directory they were copied from.

    Returns the number of ids compared.

    Only ids present in both files are compared. An id that exists only in the
    copy is a deliberate invention -- the six `npc.probe_a*` clones have no
    shipped counterpart, and that is the entire point of them -- so there is
    nothing to compare it to.

    The reverse case needs no machinery here. Deleting a mirrored id from the
    copy would leave nothing to compare, but a matchup that references an
    unknown combatant is already an error, so an entry that some encounter
    actually uses cannot vanish unnoticed. An entry nothing references can, and
    its absence is harmless by definition.
    """
    reference_path = reference_dir / "combatants.json"

    # Read into a throwaway report: if the reference itself is malformed that is
    # a fault in the reference, and the run that validates it directly is the
    # one that should say so. Here it only means the comparison is impossible.
    reference_report = Report()
    reference_rows = load_list(reference_path, reference_report)
    if reference_report.errors:
        for message in reference_report.errors:
            report.error("--mirror", f"cannot read reference: {message}")
        return 0

    reference_by_id = {
        row["id"]: row for row in reference_rows if isinstance(row.get("id"), str)
    }
    target_by_id = {
        row["id"]: row for row in rows if isinstance(row.get("id"), str)
    }

    shared = sorted(set(target_by_id) & set(reference_by_id))
    if not shared:
        report.warn(
            "--mirror",
            f"no combatant id appears in both {filename} and {reference_path},"
            " so nothing was compared",
        )
        return 0

    for ident in shared:
        here = normalise_for_mirror(target_by_id[ident])
        there = normalise_for_mirror(reference_by_id[ident])
        for key in MIRROR_FIELDS:
            if here[key] == there[key]:
                continue
            report.error(
                f"{filename} ({ident})",
                f"has drifted from {reference_path}: '{key}' is"
                f" {json.dumps(here[key], sort_keys=True)} here but"
                f" {json.dumps(there[key], sort_keys=True)} there",
            )
    return len(shared)


def parse_args(argv: list[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        prog="validate_data.py",
        description="Validate the content data that drives the battle core.",
    )
    parser.add_argument(
        "data_dir",
        nargs="?",
        default=None,
        help="content directory to validate; defaults to <repo>/data",
    )
    for name in CONTENT_FILES:
        parser.add_argument(
            f"--{name}",
            default=None,
            metavar="PATH",
            help=f"read {name} from PATH instead of <data_dir>/{name}.json",
        )
    parser.add_argument(
        "--mirror",
        default=None,
        metavar="DIR",
        help=(
            "require every combatant that also exists in DIR/combatants.json"
            " to be identical to it, field for field"
        ),
    )
    return parser.parse_args(argv[1:])


def main(argv: list[str]) -> int:
    args = parse_args(argv)
    data_dir = Path(args.data_dir) if args.data_dir else default_data_dir()

    overrides = {name: getattr(args, name) for name in CONTENT_FILES}
    paths = {
        name: Path(override) if override else data_dir / f"{name}.json"
        for name, override in overrides.items()
    }

    # The content directory only has to exist if something is still being read
    # from it. A run that overrides all four files never touches it.
    if not all(overrides.values()) and not data_dir.is_dir():
        print(f"error: '{data_dir}' is not a directory", file=sys.stderr)
        return 2

    reference_dir = Path(args.mirror) if args.mirror else None
    if reference_dir is not None and not reference_dir.is_dir():
        print(f"error: --mirror '{reference_dir}' is not a directory",
              file=sys.stderr)
        return 2

    report = Report()
    skills = load_list(paths["skills"], report)
    stands = load_list(paths["stands"], report)
    combatants = load_list(paths["combatants"], report)
    matchups = load_list(paths["matchups"], report)

    skill_ids, elements_dealt = validate_skills(
        skills, paths["skills"].name, report
    )
    stand_ids, from_stands = validate_stands(
        stands, paths["stands"].name, skill_ids, report
    )
    team_of, from_combatants = validate_combatants(
        combatants, paths["combatants"].name, stand_ids, skill_ids,
        elements_dealt, report
    )
    validate_matchups(matchups, paths["matchups"].name, team_of, report)

    mirrored = 0
    if reference_dir is not None:
        mirrored = check_mirror(
            combatants, paths["combatants"].name, reference_dir, report
        )

    orphans = sorted(skill_ids - from_stands - from_combatants)
    for orphan in orphans:
        report.warn(paths["skills"].name,
                    f"'{orphan}' is unreachable: no stand or combatant grants it")

    for warning in report.warnings:
        print(f"WARN  {warning}")
    for error in report.errors:
        print(f"ERROR {error}", file=sys.stderr)

    mirror_note = f", {mirrored} mirrored" if reference_dir is not None else ""
    print(
        f"\nchecked {len(skills)} skills, {len(stands)} stands, "
        f"{len(combatants)} combatants, {len(matchups)} matchups"
        f"{mirror_note}: "
        f"{len(report.errors)} error(s), {len(report.warnings)} warning(s)"
    )
    if any(overrides.values()):
        print("note: paths were overridden, so this run does not describe the"
              " shipped content")
    return 1 if report.errors else 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
