#!/usr/bin/env python3
"""Report dashes used as punctuation in the repository's Markdown prose.

The project's prose rule: no em dash (U+2014), no en dash (U+2013) and no
double hyphen used as punctuation. A comma, a colon, a semicolon, parentheses
or a full stop says the same thing. This script keeps the rule true: it runs
in the fast gate (tools/check.sh, tools/check.ps1) and in the spelling job of
.github/workflows/ci.yml.

What it reads: every tracked Markdown file (`git ls-files "*.md"`), or the
paths given on the command line.

What it ignores, because there a hyphen is syntax rather than punctuation:

- fenced code blocks (``` or ~~~) and inline code spans, including spans
  that wrap onto the next line of a paragraph;
- HTML comments, including ones that span lines;
- link destinations, reference definitions and bare URLs (an anchor such as
  #section--name is generated from a heading);
- table separator rows (| --- | :---: |) and thematic breaks (---);
- command-line flags written in prose without backticks: a double hyphen
  that starts a word and is followed by a letter or a digit, as in --fast.

Everything else is reported as path:line:column with the kind of dash. The
exit code is 1 when anything is found, 0 when the prose is clean and 2 when
the files cannot be listed or read.

    python3 tools/check_prose.py              check every tracked .md file
    python3 tools/check_prose.py FILE...      check only these files
    python3 tools/check_prose.py --summary    also print a count per file

Standard library only, so CI and the gate need nothing installed.
"""

from __future__ import annotations

import argparse
import re
import subprocess
import sys
from dataclasses import dataclass
from pathlib import Path

EM_DASH = "—"
EN_DASH = "–"

# An opening or closing code fence. CommonMark allows three spaces of
# indentation, but a fence inside a list item is indented further, so any
# indentation is accepted. Group 1 is the fence, group 2 the info string.
FENCE = re.compile(r"^\s*(`{3,}|~{3,})(.*)$")

# | --- | :---: | ---: |, with or without the outer pipes.
TABLE_SEPARATOR = re.compile(r"^\s*\|?\s*:?-+:?\s*(\|\s*:?-+:?\s*)*\|?\s*$")

# A thematic break or a setext heading underline: hyphens only, maybe spaced.
THEMATIC_BREAK = re.compile(r"^\s*(-\s*){3,}$")

HTML_COMMENT = re.compile(r"<!--.*?-->", re.DOTALL)
BACKTICK_RUN = re.compile(r"`+")

# [text](destination "title") and ![alt](destination).
LINK_DESTINATION = re.compile(r"\]\([^)\s]*(\s+\"[^\"]*\")?\)")
# [label]: destination
REFERENCE_DEFINITION = re.compile(r"^\s*\[[^\]]+\]:\s*\S+")
AUTOLINK = re.compile(r"<[a-zA-Z][a-zA-Z0-9+.-]*:[^>\s]*>")
BARE_URL = re.compile(r"https?://[^\s)>\]]+")

HYPHEN_RUN = re.compile(r"-{2,}")


@dataclass(frozen=True)
class Finding:
    path: str
    line: int
    column: int
    kind: str

    def __str__(self) -> str:
        return f"{self.path}:{self.line}:{self.column}: {self.kind} used as punctuation"


def blank(text: str) -> str:
    """Spaces for every character except line breaks, so positions stay put."""
    return re.sub(r"[^\n]", " ", text)


def blank_match(match: re.Match[str]) -> str:
    return blank(match.group(0))


def blank_fences(lines: list[str]) -> list[str]:
    """Return the lines with every fenced code block, fences included, empty."""
    result: list[str] = []
    fence: str | None = None
    for line in lines:
        match = FENCE.match(line)
        if fence is None:
            if match is not None:
                fence = match.group(1)
                result.append("")
            else:
                result.append(line)
            continue
        if (
            match is not None
            and match.group(1)[0] == fence[0]
            and len(match.group(1)) >= len(fence)
            and match.group(2).strip() == ""
        ):
            fence = None
        result.append("")
    return result


def blank_code_spans(text: str) -> str:
    """Blank out inline code spans in one block of text.

    CommonMark: a run of N backticks opens a span that the next run of exactly
    N backticks closes, possibly on a later line of the same paragraph; a run
    with no matching closer is literal text.
    """
    result = text
    position = 0
    while True:
        opener = BACKTICK_RUN.search(result, position)
        if opener is None:
            return result
        size = len(opener.group(0))
        closer = re.compile(rf"(?<!`)`{{{size}}}(?!`)").search(result, opener.end())
        if closer is None:
            position = opener.end()
            continue
        start, end = opener.start(), closer.end()
        result = result[:start] + blank(result[start:end]) + result[end:]
        position = end


def blocks(lines: list[str]) -> list[tuple[int, int]]:
    """Split lines into the ranges a code span cannot cross.

    A code span stays inside one paragraph, and inside one row of a table, so
    a blank line ends a block and every table row is a block of its own.
    """
    ranges: list[tuple[int, int]] = []
    start: int | None = None
    for index, line in enumerate(lines):
        if not line.strip():
            if start is not None:
                ranges.append((start, index))
                start = None
        elif line.lstrip().startswith("|"):
            if start is not None:
                ranges.append((start, index))
                start = None
            ranges.append((index, index + 1))
        elif start is None:
            start = index
    if start is not None:
        ranges.append((start, len(lines)))
    return ranges


def blank_links(line: str) -> str:
    """Blank out link destinations, reference definitions and URLs."""
    for pattern in (LINK_DESTINATION, AUTOLINK, BARE_URL):
        line = pattern.sub(blank_match, line)
    definition = REFERENCE_DEFINITION.match(line)
    if definition is not None:
        line = blank(line[: definition.end()]) + line[definition.end() :]
    return line


def is_flag(text: str, start: int, end: int) -> bool:
    """True for a command-line flag such as --fast or --no-fail.

    The run must be exactly two hyphens, start a word (line start, whitespace,
    an opening bracket, a quote, a slash or an equals sign before it) and be
    followed by a letter or a digit.
    """
    if end - start != 2:
        return False
    before = text[start - 1] if start > 0 else " "
    after = text[end] if end < len(text) else " "
    return (before.isspace() or before in "([{\"'/=") and after.isalnum()


def check_text(path: str, text: str) -> list[Finding]:
    lines = blank_fences(text.splitlines())
    lines = HTML_COMMENT.sub(blank_match, "\n".join(lines)).split("\n")
    for start, end in blocks(lines):
        lines[start:end] = blank_code_spans("\n".join(lines[start:end])).split("\n")

    findings: list[Finding] = []
    for number, line in enumerate(lines, start=1):
        if TABLE_SEPARATOR.match(line) or THEMATIC_BREAK.match(line):
            continue
        line = blank_links(line)
        for index, character in enumerate(line):
            if character == EM_DASH:
                findings.append(Finding(path, number, index + 1, "em dash"))
            elif character == EN_DASH:
                findings.append(Finding(path, number, index + 1, "en dash"))
        for run in HYPHEN_RUN.finditer(line):
            if not is_flag(line, run.start(), run.end()):
                findings.append(Finding(path, number, run.start() + 1, "double hyphen"))
    return sorted(findings, key=lambda finding: (finding.line, finding.column))


def tracked_markdown() -> list[str]:
    """Every tracked Markdown file, or an error message as SystemExit."""
    try:
        listing = subprocess.run(
            ["git", "ls-files", "-z", "*.md"],
            check=True,
            capture_output=True,
            text=True,
        )
    except FileNotFoundError as error:
        raise SystemExit("error: git not found; pass the files to check") from error
    except subprocess.CalledProcessError as error:
        message = f"error: git ls-files failed: {error.stderr.strip()}"
        raise SystemExit(message) from error
    return sorted(name for name in listing.stdout.split("\0") if name)


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description="Report em dashes, en dashes and double hyphens used as "
        "punctuation in Markdown prose."
    )
    parser.add_argument(
        "files", nargs="*", help="files to check (default: every tracked .md file)"
    )
    parser.add_argument(
        "--summary", action="store_true", help="also print a count per file"
    )
    arguments = parser.parse_args(argv)

    try:
        paths = arguments.files or tracked_markdown()
    except SystemExit as error:
        print(error, file=sys.stderr)
        return 2

    findings: list[Finding] = []
    for path in paths:
        try:
            text = Path(path).read_text(encoding="utf-8")
        except (OSError, UnicodeDecodeError) as error:
            print(f"error: cannot read {path}: {error}", file=sys.stderr)
            return 2
        findings.extend(check_text(path, text))

    for finding in findings:
        print(finding)

    if arguments.summary:
        counts: dict[str, dict[str, int]] = {}
        for finding in findings:
            per_file = counts.setdefault(finding.path, {})
            per_file[finding.kind] = per_file.get(finding.kind, 0) + 1
        for path, per_file in counts.items():
            parts = ", ".join(
                f"{kind} {count}" for kind, count in sorted(per_file.items())
            )
            print(f"{path}: {parts}")

    if findings:
        files = len({finding.path for finding in findings})
        print(
            f"{len(findings)} dash(es) used as punctuation in {files} file(s). "
            "Use a comma, a colon, a semicolon, parentheses or a full stop.",
            file=sys.stderr,
        )
        return 1
    print(f"prose punctuation: {len(paths)} Markdown file(s) clean")
    return 0


if __name__ == "__main__":
    sys.exit(main())
