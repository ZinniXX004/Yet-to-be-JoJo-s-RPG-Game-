#!/usr/bin/env python3
"""Validate the content data that drives the battle core.

This exists because the Rust core deserializes `data/*.json` at load time, and a
broken reference there would only surface as a runtime failure inside the game.
Catching it in CI is cheaper.

Checks performed:
  1. Every file parses as JSON and is a top-level list of objects.
  2. Required keys are present and correctly typed.
  3. Enum-valued fields use values the Rust core actually accepts.
  4. Cross-file references resolve (stand -> skills, combatant -> stand/skills).
  5. IDs are unique and follow the `namespace.name` convention.
  6. Soft balance warnings (unreachable skills, zero-cost nukes).

Usage:
    python3 src/data-pipeline/validate_data.py [data_dir]

Exit codes:
    0 = valid (warnings may still be printed)
    1 = validation errors found
    2 = usage or I/O error
"""

from __future__ import annotations

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


def validate_skills(rows: list[dict[str, Any]], report: Report) -> set[str]:
    ids = check_ids(rows, "skills.json", report)
    for index, row in enumerate(rows):
        where = f"skills.json[{index}] ({row.get('id', '?')})"
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
    return ids


def validate_stands(rows: list[dict[str, Any]], skill_ids: set[str],
                    report: Report) -> tuple[set[str], set[str]]:
    ids = check_ids(rows, "stands.json", report)
    referenced: set[str] = set()
    for index, row in enumerate(rows):
        where = f"stands.json[{index}] ({row.get('id', '?')})"
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


def validate_combatants(rows: list[dict[str, Any]], stand_ids: set[str],
                        skill_ids: set[str], report: Report) -> set[str]:
    check_ids(rows, "combatants.json", report)
    referenced: set[str] = set()
    teams: dict[str, int] = {"party": 0, "foe": 0}
    for index, row in enumerate(rows):
        where = f"combatants.json[{index}] ({row.get('id', '?')})"
        if not isinstance(row.get("name"), str):
            report.error(where, "missing or non-string 'name'")
        check_enum(row, "team", TEAMS, where, report)
        check_enum(row, "ai", AI_PROFILES, where, report, required=False)
        if row.get("team") in teams:
            teams[row["team"]] += 1
        check_stats(row.get("stats"), where, report)

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
            report.warn("combatants.json", f"no combatants on team '{team}'")
    return referenced


def main(argv: list[str]) -> int:
    data_dir = Path(argv[1]) if len(argv) > 1 else Path("data")
    if not data_dir.is_dir():
        print(f"error: '{data_dir}' is not a directory", file=sys.stderr)
        return 2

    report = Report()
    skills = load_list(data_dir / "skills.json", report)
    stands = load_list(data_dir / "stands.json", report)
    combatants = load_list(data_dir / "combatants.json", report)

    skill_ids = validate_skills(skills, report)
    stand_ids, from_stands = validate_stands(stands, skill_ids, report)
    from_combatants = validate_combatants(combatants, stand_ids, skill_ids, report)

    orphans = sorted(skill_ids - from_stands - from_combatants)
    for orphan in orphans:
        report.warn("skills.json", f"'{orphan}' is unreachable: no stand or"
                                   " combatant grants it")

    for warning in report.warnings:
        print(f"WARN  {warning}")
    for error in report.errors:
        print(f"ERROR {error}", file=sys.stderr)

    print(
        f"\nchecked {len(skills)} skills, {len(stands)} stands, "
        f"{len(combatants)} combatants: "
        f"{len(report.errors)} error(s), {len(report.warnings)} warning(s)"
    )
    return 1 if report.errors else 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
