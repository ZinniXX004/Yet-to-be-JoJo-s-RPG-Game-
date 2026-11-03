#!/usr/bin/env python3
"""Compare two balance reports and print the win-rate diff as Markdown.

The balance harness writes its report with

    cd src
    cargo run -q -p rpg-core --bin balance -- --json --no-fail > report.json

and the schema is BatchReport in src/core/src/report.rs. This script reads
two such reports, a base and a head, and prints one table row per encounter:

    encounter | base % | head % | change (pp) | head 95% interval | band | in band

Win percentages here carry one decimal and are wins over battles, so they
are finer than the harness's whole-percent win_rate_percent. The band verdict
is the harness's own: win_rate_percent against the declared band, inclusive
at both ends, exactly as WinRateBand::contains judges it. The interval is the
Wilson score interval at 95 percent (z = 1.96), the one docs/BALANCE-LOG.md
quotes. A change is printed in bold when it is larger than the half-width of
the base interval, that is, when it is larger than the noise of one 300-seed
run alone. An encounter present on one side only is listed as added or
removed.

Floating point is fine here: this is a reporting tool outside rpg-core, and
nothing it prints feeds back into a rule.

A report redirected with `>` in Windows PowerShell 5.1 is UTF-16 with a byte
order mark, so every input is decoded by its BOM (UTF-8, UTF-16 or UTF-32)
and read as UTF-8 when it has none.

CI's `balance diff` job (.github/workflows/ci.yml) runs this on every pull
request and posts the result as one pull request comment. To run it locally:

    python3 tools/balance_diff.py BASE.json HEAD.json
    python3 tools/balance_diff.py --base-ref f575394 --head-ref HEAD BASE.json HEAD.json
    python3 tools/balance_diff.py --self-test

Exit codes: 0 when the table was printed (whatever it shows), 1 when the
self-test fails, 2 when an input cannot be read or is not a balance report.

Standard library only, so CI and the gate need nothing installed.
"""

from __future__ import annotations

import argparse
import codecs
import json
import math
import sys
from dataclasses import dataclass
from pathlib import Path

# Two-sided 95 percent normal quantile, as used for every interval in
# docs/BALANCE-LOG.md.
Z_95 = 1.96

# Longest BOM first: the UTF-32 LE mark begins with the UTF-16 LE one.
BOMS = (
    (codecs.BOM_UTF32_LE, "utf-32-le"),
    (codecs.BOM_UTF32_BE, "utf-32-be"),
    (codecs.BOM_UTF8, "utf-8"),
    (codecs.BOM_UTF16_LE, "utf-16-le"),
    (codecs.BOM_UTF16_BE, "utf-16-be"),
)


class ReportError(Exception):
    """An input that is unreadable or is not a balance report."""


@dataclass(frozen=True)
class Encounter:
    id: str
    battles: int
    wins: int
    stalemates: int
    win_rate_percent: int
    band_min: int
    band_max: int

    @property
    def win_percent(self) -> float:
        if self.battles == 0:
            return 0.0
        return 100.0 * self.wins / self.battles

    @property
    def in_band(self) -> bool:
        # The harness's verdict, on the harness's whole-percent figure.
        return self.band_min <= self.win_rate_percent <= self.band_max

    def wilson(self) -> tuple[float, float]:
        return wilson_interval(self.wins, self.battles)


def wilson_interval(wins: int, battles: int, z: float = Z_95) -> tuple[float, float]:
    """Wilson score interval for wins out of battles, in percent."""
    if battles <= 0:
        return (0.0, 100.0)
    p = wins / battles
    z2 = z * z
    denominator = 1.0 + z2 / battles
    centre = p + z2 / (2.0 * battles)
    spread = z * math.sqrt(p * (1.0 - p) / battles + z2 / (4.0 * battles * battles))
    lower = (centre - spread) / denominator
    upper = (centre + spread) / denominator
    return (max(0.0, 100.0 * lower), min(100.0, 100.0 * upper))


def decode(raw: bytes, source: str) -> str:
    """Decode by byte order mark; no BOM means UTF-8."""
    for bom, encoding in BOMS:
        if raw.startswith(bom):
            try:
                return raw[len(bom) :].decode(encoding)
            except UnicodeDecodeError as error:
                raise ReportError(f"{source}: not valid {encoding}: {error}") from error
    # UTF-16 or UTF-32 text without a BOM is still valid UTF-8 byte for byte,
    # with a NUL after every ASCII character. A JSON report never holds a NUL,
    # so refuse it here with the likely cause rather than as broken JSON.
    if b"\x00" in raw:
        raise ReportError(
            f"{source}: NUL bytes and no byte order mark; "
            "UTF-16 or UTF-32 without a BOM is not supported"
        )
    try:
        return raw.decode("utf-8")
    except UnicodeDecodeError as error:
        raise ReportError(
            f"{source}: no byte order mark and not valid UTF-8: {error}"
        ) from error


def parse_report(text: str, source: str) -> dict[str, Encounter]:
    """Encounters by id, in report order."""
    try:
        document = json.loads(text)
    except json.JSONDecodeError as error:
        raise ReportError(f"{source}: not JSON: {error}") from error
    if not isinstance(document, dict) or not isinstance(document.get("matchups"), list):
        raise ReportError(f"{source}: no 'matchups' list; is this a balance report?")

    encounters: dict[str, Encounter] = {}
    for index, matchup in enumerate(document["matchups"]):
        where = f"{source}: matchups[{index}]"
        if not isinstance(matchup, dict):
            raise ReportError(f"{where}: not an object")
        try:
            band = matchup["band"]
            if not isinstance(band, dict):
                raise ReportError(f"{where}: 'band' is not an object")
            encounter = Encounter(
                id=require_str(matchup, "id", where),
                battles=require_int(matchup, "battles", where),
                wins=require_int(matchup, "wins", where),
                stalemates=require_int(matchup, "stalemates", where),
                win_rate_percent=require_int(matchup, "win_rate_percent", where),
                band_min=require_int(band, "min_percent", f"{where}.band"),
                band_max=require_int(band, "max_percent", f"{where}.band"),
            )
        except KeyError as error:
            raise ReportError(f"{where}: missing key {error}") from error
        if encounter.id in encounters:
            raise ReportError(f"{where}: duplicate encounter id '{encounter.id}'")
        if not 0 <= encounter.wins <= encounter.battles:
            raise ReportError(
                f"{where}: wins {encounter.wins} outside 0..{encounter.battles}"
            )
        encounters[encounter.id] = encounter
    return encounters


def require_int(mapping: dict, key: str, where: str) -> int:
    value = mapping[key]
    # bool is an int in Python; a report never holds one here.
    if isinstance(value, bool) or not isinstance(value, int):
        raise ReportError(f"{where}: '{key}' is not an integer")
    return value


def require_str(mapping: dict, key: str, where: str) -> str:
    value = mapping[key]
    if not isinstance(value, str):
        raise ReportError(f"{where}: '{key}' is not a string")
    return value


def load_report(path: Path) -> dict[str, Encounter]:
    try:
        raw = path.read_bytes()
    except OSError as error:
        raise ReportError(f"{path}: cannot read: {error}") from error
    return parse_report(decode(raw, str(path)), str(path))


def cell(text: str) -> str:
    """Content from a report, made safe for one Markdown table cell."""
    return " ".join(text.replace("|", "/").replace("`", "'").split())


def percent(value: float) -> str:
    return f"{value:.1f}"


def signed(value: float) -> str:
    rounded = round(value, 1)
    if rounded == 0:
        return "0.0"
    return f"{rounded:+.1f}"


def render(
    base: dict[str, Encounter],
    head: dict[str, Encounter],
    base_ref: str = "base",
    head_ref: str = "head",
) -> str:
    """The Markdown block CI posts: a heading line, the table, the notes."""
    order = list(head) + [
        encounter_id for encounter_id in base if encounter_id not in head
    ]

    rows: list[str] = []
    moved: list[str] = []
    out_of_band: list[str] = []
    stalled: list[str] = []
    added: list[str] = []
    removed: list[str] = []

    for encounter_id in order:
        name = f"`{cell(encounter_id)}`"
        old = base.get(encounter_id)
        new = head.get(encounter_id)
        if new is None:
            assert old is not None
            removed.append(name)
            rows.append(
                f"| {name} | {percent(old.win_percent)} | removed | n/a | n/a "
                f"| {old.band_min}..{old.band_max} | n/a |"
            )
            continue

        low, high = new.wilson()
        interval = f"[{percent(low)}, {percent(high)}]"
        band = f"{new.band_min}..{new.band_max}"
        verdict = "yes" if new.in_band else "**no**"
        if not new.in_band:
            out_of_band.append(name)
        if new.stalemates > 0:
            stalled.append(f"{name} ({new.stalemates} of {new.battles} battles)")

        if old is None:
            added.append(name)
            base_cell = "new"
            change = "n/a"
        else:
            base_cell = percent(old.win_percent)
            delta = new.win_percent - old.win_percent
            base_low, base_high = old.wilson()
            half_width = (base_high - base_low) / 2.0
            change = signed(delta)
            if abs(delta) > half_width:
                change = f"**{change}**"
                moved.append(name)
        rows.append(
            f"| {name} | {base_cell} | {percent(new.win_percent)} | {change} "
            f"| {interval} | {band} | {verdict} |"
        )

    lines = [
        f"Base `{cell(base_ref)}`, head `{cell(head_ref)}`.",
        "",
        "| Encounter | Base % | Head % | Change (pp) | Head 95% interval | Band | In band |",
        "| --- | ---: | ---: | ---: | --- | --- | --- |",
        *rows,
        "",
        "Win percent is wins over battles. The interval is Wilson at 95%. In band is "
        "the harness verdict on its whole-percent win rate. A change in bold is larger "
        "than the half-width of the base interval.",
        "",
    ]
    if moved:
        lines.append(f"Moved beyond the base interval: {', '.join(moved)}.")
    else:
        lines.append("No encounter moved beyond the half-width of its base interval.")
    if out_of_band:
        lines.append(f"Outside the declared band: {', '.join(out_of_band)}.")
    if stalled:
        lines.append(f"Battles that hit the turn limit: {', '.join(stalled)}.")
    if added:
        lines.append(f"Added on head: {', '.join(added)}.")
    if removed:
        lines.append(f"Removed on head: {', '.join(removed)}.")
    return "\n".join(lines) + "\n"


# --- self-test ---------------------------------------------------------------


def fixture(*matchups: tuple[str, int, int, int, int, int]) -> str:
    """A minimal report: (id, battles, wins, stalemates, band_min, band_max)."""
    entries = []
    for encounter_id, battles, wins, stalemates, band_min, band_max in matchups:
        rate = (wins * 100 + battles // 2) // battles if battles else 0
        entries.append(
            {
                "id": encounter_id,
                "name": encounter_id,
                "battles": battles,
                "wins": wins,
                "losses": battles - wins - stalemates,
                "stalemates": stalemates,
                "win_rate_percent": rate,
                "median_turns": 10,
                "shortest_turns": 5,
                "longest_turns": 20,
                "band": {"min_percent": band_min, "max_percent": band_max},
                "combatants": [],
            }
        )
    return json.dumps({"matchups": entries}, indent=2)


# Win counts of the f575394 report, 300 seeds each, with the intervals
# docs/BALANCE-LOG.md quotes for them.
F575394 = (
    ("matchup.thug_solo", 300, 300, 0, 85, 100),
    ("matchup.assassin_ambush", 300, 170, 0, 45, 90),
    ("matchup.dio_boss", 300, 167, 0, 35, 75),
    ("matchup.dancer_rush", 300, 240, 0, 60, 90),
    ("matchup.weaver_gambit", 300, 247, 0, 58, 86),
    ("matchup.bell_race", 300, 211, 0, 48, 80),
    ("matchup.diavolo_boss", 300, 114, 0, 25, 55),
)
F575394_INTERVALS = {
    "matchup.thug_solo": ("100.0", "98.7", "100.0"),
    "matchup.assassin_ambush": ("56.7", "51.0", "62.2"),
    "matchup.dio_boss": ("55.7", "50.0", "61.2"),
    "matchup.dancer_rush": ("80.0", "75.1", "84.1"),
    "matchup.weaver_gambit": ("82.3", "77.6", "86.2"),
    "matchup.bell_race": ("70.3", "64.9", "75.2"),
    "matchup.diavolo_boss": ("38.0", "32.7", "43.6"),
}


def self_test() -> list[str]:
    """Every failed expectation, as a message; empty when all pass."""
    failures: list[str] = []

    def expect(condition: bool, message: str) -> None:
        if not condition:
            failures.append(message)

    base_text = fixture(*F575394)
    base = parse_report(base_text, "fixture")

    # The intervals the balance log quotes.
    for encounter_id, (point, low, high) in F575394_INTERVALS.items():
        encounter = base[encounter_id]
        got_low, got_high = encounter.wilson()
        got = (percent(encounter.win_percent), percent(got_low), percent(got_high))
        expect(
            got == (point, low, high),
            f"{encounter_id}: got {got}, want {(point, low, high)}",
        )
    expect(wilson_interval(0, 0) == (0.0, 100.0), "an empty run must span 0..100")
    low, high = wilson_interval(0, 300)
    expect(
        percent(low) == "0.0" and percent(high) == "1.3", f"0 of 300: got {low}, {high}"
    )

    # Every encoding a report can arrive in decodes to the same encounters.
    for bom, encoding in BOMS:
        encoded = bom + base_text.encode(encoding)
        expect(
            parse_report(decode(encoded, encoding), encoding) == base,
            f"a {encoding} report with a BOM must decode",
        )
    expect(
        parse_report(decode(base_text.encode("utf-8"), "plain"), "plain") == base,
        "a UTF-8 report without a BOM must decode",
    )
    try:
        decode(base_text.encode("utf-16-le"), "no-bom")
        failures.append("UTF-16 without a BOM must be refused, not misread")
    except ReportError:
        pass

    # The same report on both sides: nothing moved, every row in band.
    same = render(base, base, "f575394", "f575394")
    expect(
        same.count("| 0.0 |") == len(F575394), "identical reports: every change is 0.0"
    )
    expect("**" not in same, "identical reports: nothing is bold")
    expect("No encounter moved" in same, "identical reports: the no-move line")
    expect(
        "| `matchup.assassin_ambush` | 56.7 | 56.7 | 0.0 | [51.0, 62.2] | 45..90 | yes |"
        in same,
        "identical reports: the assassin_ambush row",
    )

    # Inside the noise, beyond the noise, out of band, stalled, added, removed.
    head = parse_report(
        fixture(
            ("matchup.thug_solo", 300, 300, 0, 85, 100),
            ("matchup.assassin_ambush", 300, 180, 0, 45, 90),  # +3.3, half-width 5.6
            ("matchup.dio_boss", 300, 210, 0, 35, 75),  # +14.3, 70% in band
            ("matchup.dancer_rush", 300, 285, 0, 60, 90),  # 95%, out of band
            ("matchup.weaver_gambit", 300, 247, 4, 58, 86),
            ("matchup.bell_race", 300, 211, 0, 48, 80),
            ("matchup.newcomer", 300, 150, 0, 30, 70),
        ),
        "head",
    )
    diff = render(base, head)
    expect("| +3.3 |" in diff, "a move inside the base interval is not bold")
    expect("| **+14.3** |" in diff, "a move beyond the base interval is bold")
    expect(
        "Moved beyond the base interval: `matchup.dio_boss`, `matchup.dancer_rush`."
        in diff,
        f"the moved list:\n{diff}",
    )
    expect("| 60..90 | **no** |" in diff, "an out-of-band row says no")
    expect(
        "Outside the declared band: `matchup.dancer_rush`." in diff,
        "the out-of-band list",
    )
    expect("`matchup.weaver_gambit` (4 of 300 battles)" in diff, "the stalled list")
    expect("| `matchup.newcomer` | new | 50.0 | n/a |" in diff, "an added encounter")
    expect("| `matchup.diavolo_boss` | 38.0 | removed |" in diff, "a removed encounter")
    expect(
        diff.index("matchup.newcomer") < diff.index("| `matchup.diavolo_boss`"),
        "head order first, removed encounters last",
    )

    # A negative move, and a cell that cannot break the table.
    lower = parse_report(fixture(("a|b`c", 300, 150, 0, 0, 100)), "lower")
    higher = parse_report(fixture(("a|b`c", 300, 200, 0, 0, 100)), "higher")
    expect("| **-16.7** |" in render(higher, lower), "a fall is signed with a minus")
    expect(
        "`a/b'c`" in render(lower, lower), "a pipe or backtick in an id is neutralised"
    )

    # Inputs that are not a balance report fail with a message, not a traceback.
    for bad, why in (
        ("[]", "a list"),
        ('{"matchups": [{"id": "x"}]}', "a missing key"),
        (
            '{"matchups": [{"id": "x", "battles": 1, "wins": 2, "stalemates": 0,'
            ' "win_rate_percent": 1, "band": {"min_percent": 0, "max_percent": 1}}]}',
            "more wins than battles",
        ),
        (
            '{"matchups": [{"id": "x", "battles": "1", "wins": 0, "stalemates": 0,'
            ' "win_rate_percent": 0, "band": {"min_percent": 0, "max_percent": 1}}]}',
            "a string count",
        ),
        ("{", "broken JSON"),
    ):
        try:
            parse_report(bad, "bad")
            failures.append(f"{why} must be refused")
        except ReportError:
            pass

    return failures


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description="Print the win-rate diff of two balance reports as Markdown.",
    )
    parser.add_argument(
        "base", nargs="?", type=Path, help="base report (balance --json)"
    )
    parser.add_argument(
        "head", nargs="?", type=Path, help="head report (balance --json)"
    )
    parser.add_argument(
        "--base-ref", default="base", help="label for the base, e.g. a SHA"
    )
    parser.add_argument(
        "--head-ref", default="head", help="label for the head, e.g. a SHA"
    )
    parser.add_argument(
        "--self-test", action="store_true", help="run the built-in fixtures and exit"
    )
    args = parser.parse_args(argv)

    if args.self_test:
        failures = self_test()
        for failure in failures:
            print(f"balance_diff self-test: FAIL: {failure}", file=sys.stderr)
        if failures:
            return 1
        print("balance_diff self-test: all checks passed")
        return 0

    if args.base is None or args.head is None:
        parser.error("BASE and HEAD are required unless --self-test is given")

    try:
        base = load_report(args.base)
        head = load_report(args.head)
    except ReportError as error:
        print(f"balance_diff: {error}", file=sys.stderr)
        return 2
    sys.stdout.write(render(base, head, args.base_ref, args.head_ref))
    return 0


if __name__ == "__main__":
    sys.exit(main())
