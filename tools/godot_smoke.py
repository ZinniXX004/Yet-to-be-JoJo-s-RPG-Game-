#!/usr/bin/env python3
"""Play every encounter in data/matchups.json to the end in headless Godot.

This is the only automated check on the Godot layer. Before it existed, the
project's committed src/game/data/ sat at 0.1.0 content through two milestones
and battle_view.gd fought a roster that matched no declared encounter, and
nothing failed, because no job ran Godot at all.

What it proves, per encounter: the GDExtension loads, the content parses on
the GDScript side, the battle runs from the first turn to an outcome through
the real bridge, and the console stays clean -- no SCRIPT ERROR, no ERROR, no
WARNING. battle_view.gd pushes a warning for an event kind it has no case for,
so a new rpg-core Event variant fails here until the presentation handles it.

What it does not prove: anything about how the screen looks or responds to a
mouse. Those still close on a recorded playthrough (docs/ROADMAP.md).

Usage, from the repository root:

    python tools/godot_smoke.py --godot "C:\\Tools\\Godot\\Godot_v4.7.1-stable_win64_console.exe"
    GODOT=/path/to/godot python3 tools/godot_smoke.py
    python tools/godot_smoke.py --encounter matchup.diavolo_boss --seed 7

The Godot binary comes from --godot, else $GODOT, else `godot` on PATH. On
Windows the *_console.exe build is preferred when it sits next to the one
given, because the windowed build may not write to a pipe.

Steps: build rpg-bridge (debug), copy it into src/game/bin, sync data/ into
src/game/data, import the project, then one autoplayed battle per encounter.
The copy overwrites the library the editor loads, which is the point: a smoke
test of a stale library is the defect this file exists to catch. Close the
Godot editor first on Windows, where a loaded DLL cannot be replaced.

Standard library only, like src/data-pipeline/validate_data.py.
"""

from __future__ import annotations

import argparse
import json
import os
import platform
import re
import shutil
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
GAME = ROOT / "src" / "game"
CI_WORKFLOW = ROOT / ".github" / "workflows" / "ci.yml"

PROBLEM = re.compile(r"^\s*(?:SCRIPT ERROR|USER ERROR|USER WARNING|ERROR|WARNING):")
FINISHED = re.compile(r"^\[battle\] finished encounter=(\S+) seed=(\d+) outcome=(\S+)\s*$")
ANSI = re.compile(r"\x1b\[[0-9;]*m")

# A battle is synchronous when headless (no event delay), so it ends inside
# the first frame. The frame cap only bounds a battle that waits for input it
# will never get: measured at about 145 headless frames per second on Linux,
# 600 frames gives up after roughly four seconds instead of the two minutes
# that 20000 took. The timeout is a second, independent bound.
QUIT_AFTER_FRAMES = "600"
IMPORT_TIMEOUT_S = 300
BATTLE_TIMEOUT_S = 120


def fail(message: str) -> "NoReturn":  # type: ignore[name-defined]
    print(f"error: {message}", file=sys.stderr)
    sys.exit(1)


def pinned_version() -> str | None:
    """The Godot version CI pins, e.g. '4.7.1', read from ci.yml."""
    try:
        text = CI_WORKFLOW.read_text(encoding="utf-8")
    except OSError:
        return None
    match = re.search(r'GODOT_VERSION:\s*"?(\d+\.\d+(?:\.\d+)?)-stable', text)
    return match.group(1) if match else None


def find_godot(given: str | None) -> Path:
    candidate = given or os.environ.get("GODOT") or shutil.which("godot") or shutil.which("godot4")
    if not candidate:
        fail("Godot not found. Pass --godot <path>, set GODOT, or put godot on PATH.")
    path = Path(candidate)
    if not path.exists():
        resolved = shutil.which(candidate)
        if not resolved:
            fail(f"Godot binary does not exist: {candidate}")
        path = Path(resolved)
    if path.suffix.lower() == ".exe" and not path.stem.endswith("_console"):
        console = path.with_name(path.stem + "_console.exe")
        if console.exists():
            path = console
    return path


def library_name() -> str:
    system = platform.system()
    if system == "Windows":
        return "rpg_bridge.dll"
    if system == "Darwin":
        return "librpg_bridge.dylib"
    return "librpg_bridge.so"


def run(command: list[str], timeout: int, cwd: Path | None = None) -> tuple[int, str]:
    try:
        completed = subprocess.run(
            command,
            cwd=cwd,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            timeout=timeout,
            check=False,
        )
    except subprocess.TimeoutExpired as expired:
        output = (expired.stdout or b"").decode("utf-8", errors="replace")
        return 124, ANSI.sub("", output) + f"\n(timed out after {timeout} s)"
    return completed.returncode, ANSI.sub("", completed.stdout.decode("utf-8", errors="replace"))


def problems(output: str) -> list[str]:
    return [line.rstrip() for line in output.splitlines() if PROBLEM.match(line)]


def check_version(godot: Path) -> None:
    code, output = run([str(godot), "--headless", "--version"], timeout=60)
    reported = output.strip().splitlines()[-1] if output.strip() else ""
    if code != 0 or not reported:
        fail(f"`{godot} --version` failed:\n{output}")
    pin = pinned_version()
    print(f"godot: {reported} ({godot})")
    if pin and not reported.startswith(pin + "."):
        message = f"Godot {reported} does not match the {pin} that CI pins in ci.yml."
        if os.environ.get("CI"):
            fail(message)
        print(f"warning: {message} A pass here may not hold in CI.")


def build_and_install_bridge() -> None:
    print("building rpg-bridge (debug)...", flush=True)
    code = subprocess.run(["cargo", "build", "-p", "rpg-bridge"], cwd=ROOT / "src", check=False).returncode
    if code != 0:
        fail("cargo build -p rpg-bridge failed")
    built = ROOT / "src" / "target" / "debug" / library_name()
    if not built.exists():
        fail(f"build succeeded but {built} does not exist")
    destination = GAME / "bin"
    destination.mkdir(parents=True, exist_ok=True)
    try:
        shutil.copy2(built, destination / built.name)
    except PermissionError:
        fail(f"cannot replace {destination / built.name}: it is in use. Close the Godot editor and retry.")
    print(f"installed {built.name} into src/game/bin")


def sync_data() -> None:
    # The same copy tools/sync_data.* perform; repeated here so this script
    # needs neither a shell nor PowerShell to run.
    destination = GAME / "data"
    destination.mkdir(parents=True, exist_ok=True)
    for stale in destination.glob("*.json"):
        stale.unlink()
    files = sorted((ROOT / "data").glob("*.json"))
    for source in files:
        shutil.copy2(source, destination / source.name)
    print(f"synced {len(files)} content file(s) into src/game/data")


def import_project(godot: Path) -> None:
    command = [str(godot), "--headless", "--path", str(GAME), "--import"]
    code, output = run(command, timeout=IMPORT_TIMEOUT_S)
    if code != 0 and (GAME / ".godot" / "extension_list.cfg").exists():
        # Observed with Godot 4.7.1 + godot-rust 0.5.4 on Linux: the first
        # import of a project with no .godot/ aborts (SIGABRT, no message) while
        # shutting down, after the import itself has completed and registered
        # the extension. A second import exits 0. A project without the
        # extension imports cleanly, so the abort is tied to the extension's
        # teardown, not to the content. Retrying once is a workaround with a
        # stated condition, not a silenced failure: the second run must be clean.
        print(f"note: first import exited {code}; retrying once (see the comment in this file)")
        code, output = run(command, timeout=IMPORT_TIMEOUT_S)
    found = problems(output)
    if code != 0 or found:
        fail(f"project import failed (exit {code}):\n" + "\n".join(found or output.splitlines()[-20:]))
    print("project imported")


def play(godot: Path, encounter: str, seed: int) -> tuple[bool, str]:
    command = [
        str(godot), "--headless", "--path", str(GAME), "--quit-after", QUIT_AFTER_FRAMES,
        "--", f"--encounter={encounter}", f"--seed={seed}", "--autoplay", "--quit-on-end",
    ]
    code, output = run(command, timeout=BATTLE_TIMEOUT_S)
    found = problems(output)
    finished = [m for m in (FINISHED.match(line) for line in output.splitlines()) if m]
    reasons = []
    if code != 0:
        reasons.append(f"exit code {code}")
    if found:
        reasons.extend(found)
    if not finished:
        reasons.append("no '[battle] finished' line: the battle never reached an outcome")
    elif finished[-1].group(1) != encounter or int(finished[-1].group(2)) != seed:
        reasons.append(f"finished line names {finished[-1].group(1)} seed {finished[-1].group(2)}")
    if reasons:
        return False, "\n      ".join(reasons)
    return True, finished[-1].group(3)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("--godot", help="path to the Godot binary (else $GODOT, else godot on PATH)")
    parser.add_argument("--encounter", action="append", help="encounter id; repeatable; default: all")
    parser.add_argument("--seed", type=int, help="seed for every battle; default: each encounter's first seed")
    parser.add_argument("--skip-build", action="store_true", help="use the library already in src/game/bin")
    args = parser.parse_args()

    matchups = json.loads((ROOT / "data" / "matchups.json").read_text(encoding="utf-8"))
    declared = {m["id"]: m for m in matchups}
    wanted = args.encounter or list(declared)
    unknown = [e for e in wanted if e not in declared]
    if unknown:
        fail(f"unknown encounter(s): {', '.join(unknown)}")
    if args.seed is not None and not 0 <= args.seed < 2**53:
        fail("--seed must be from 0 to 2^53 - 1")

    godot = find_godot(args.godot)
    check_version(godot)
    if not args.skip_build:
        build_and_install_bridge()
    sync_data()
    import_project(godot)

    failures = 0
    for encounter in wanted:
        seed = args.seed if args.seed is not None else int(declared[encounter]["seeds"][0])
        ok, detail = play(godot, encounter, seed)
        status = "ok  " if ok else "FAIL"
        print(f"  {status} {encounter:<26} seed {seed:<6} {detail}", flush=True)
        failures += 0 if ok else 1

    if failures:
        print(f"\n{failures} of {len(wanted)} encounter(s) failed in Godot.")
        return 1
    print(f"\nAll {len(wanted)} encounter(s) played to an outcome with a clean console.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
