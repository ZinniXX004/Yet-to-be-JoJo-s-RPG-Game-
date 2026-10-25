#!/usr/bin/env sh
# The local gate: the same checks .github/workflows/ci.yml runs, in one command.
#
#   sh tools/check.sh          full gate, run before opening a pull request
#   sh tools/check.sh --fast   formatting, content and spelling only (seconds);
#                              this is what the pre-push hook runs
#
# Why this file exists: all five CI failures between late July and September
# 2026 were checks that could have run locally -- the probe mirror twice (#29,
# #33), rustfmt twice (#22, feat/ai-scored-choice), typos once
# (feat/more-enemies). The list
# in docs/DEVELOPMENT.md did not include --mirror and called typos optional, so
# "run the local checks" and "run what CI runs" had quietly become different
# things. A list a person copies by hand drifts; a script CI and humans share
# cannot. When a job is added to ci.yml, add it here in the same commit.
#
# Not reproduced: the lychee link check (network-bound; see DEVELOPMENT.md).
# cargo-deny runs only if installed, because it also needs the network. The
# Godot smoke test runs only if a Godot binary is found ($GODOT, or godot on
# PATH), because Godot is not installed into the repository.
#
# Windows: run from Git Bash, or use tools/check.ps1 from PowerShell.

set -u

root=$(cd "$(dirname "$0")/.." && pwd)
cd "$root" || exit 2

fast=0
case "${1:-}" in
  --fast) fast=1 ;;
  "") ;;
  -h|--help) sed -n '2,8p' "$0"; exit 0 ;;
  *) echo "unknown argument: $1 (expected --fast or nothing)" >&2; exit 2 ;;
esac

failed=""
passed=0

step() {
  # step <label> <command...>
  label=$1
  shift
  printf '\n==> %s\n' "$label"
  if "$@"; then
    passed=$((passed + 1))
  else
    failed="$failed\n  - $label"
  fi
}

# A Python that actually runs. On Windows, `python3` can be the Microsoft Store
# stub, which exists on PATH and prints an advert instead of running anything.
py=""
for candidate in python3 python; do
  if command -v "$candidate" > /dev/null 2>&1 \
      && "$candidate" -c 'import sys; sys.exit(sys.version_info < (3, 11))' > /dev/null 2>&1; then
    py=$candidate
    break
  fi
done
if [ -z "$py" ] && command -v py > /dev/null 2>&1; then
  py="py -3"
fi

run_py() {
  if [ -z "$py" ]; then
    echo "error: Python 3.11+ not found (tried python3, python, py -3)" >&2
    return 1
  fi
  # $py is deliberately unquoted: it may be the two words "py -3".
  # shellcheck disable=SC2086
  $py "$@"
}

# The typos version CI uses is read from ci.yml, so this script never needs
# editing when Dependabot bumps the pin.
typos_check() {
  if ! command -v typos > /dev/null 2>&1; then
    echo "error: typos not installed. CI pins crate-ci/typos@v$typos_pin:" >&2
    echo "  cargo install typos-cli --version $typos_pin --locked" >&2
    return 1
  fi
  have=$(typos --version | awk '{print $2}')
  if [ -n "$typos_pin" ] && [ "$have" != "$typos_pin" ]; then
    echo "warning: local typos $have, CI pins $typos_pin. Dictionaries differ between"
    echo "         releases, so a pass here may still fail in CI (PR #36). To match:"
    echo "         cargo install typos-cli --version $typos_pin --locked"
  fi
  typos
}
typos_pin=$(sed -n 's/.*crate-ci\/typos@v\([0-9][0-9.]*\).*/\1/p' .github/workflows/ci.yml | head -n 1)

# --- job: rustfmt -----------------------------------------------------------
step "rustfmt" sh -c 'cd src && cargo fmt --all --check'

# --- job: content validation ------------------------------------------------
step "content: data/" run_py src/data-pipeline/validate_data.py
step "content: tools/probe mirrors data/" run_py src/data-pipeline/validate_data.py \
  --combatants tools/probe/combatants.probe.json \
  --matchups tools/probe/matchups.probe.json \
  --mirror data

# --- job: spelling ----------------------------------------------------------
step "spelling (typos)" typos_check

if [ "$fast" -eq 0 ]; then
  # --- job: core ------------------------------------------------------------
  step "core: clippy" sh -c 'cd src && cargo clippy -p rpg-core --all-targets --all-features -- -D warnings'
  step "core: tests" sh -c 'cd src && cargo test -p rpg-core --all-targets --all-features'
  step "core: docs" sh -c 'cd src && cargo doc -p rpg-core --no-deps'

  # --- job: balance harness -------------------------------------------------
  step "balance: every encounter inside its band" sh -c 'cd src && cargo run -q -p rpg-core --bin balance'

  # --- job: bridge ----------------------------------------------------------
  step "bridge: clippy" sh -c 'cd src && cargo clippy -p rpg-bridge --all-targets -- -D warnings'
  step "bridge: build" sh -c 'cd src && cargo build -p rpg-bridge'

  # --- job: godot smoke (optional locally) ----------------------------------
  if [ -n "${GODOT:-}" ] || command -v godot > /dev/null 2>&1; then
    step "godot: every encounter plays to an outcome" run_py tools/godot_smoke.py
  else
    printf '\n==> godot smoke: skipped (set GODOT to the Godot binary; CI still runs it)\n'
  fi

  # --- job: supply chain (optional locally) ---------------------------------
  if command -v cargo-deny > /dev/null 2>&1; then
    step "supply chain (cargo-deny)" sh -c 'cd src && cargo deny check advisories bans licenses sources'
  else
    printf '\n==> supply chain: skipped (cargo-deny not installed; CI still runs it)\n'
  fi
fi

printf '\n'
if [ -n "$failed" ]; then
  printf 'FAILED:%b\n' "$failed"
  printf '%s step(s) passed. Fix the failures above before pushing.\n' "$passed"
  exit 1
fi
if [ "$fast" -eq 1 ]; then
  printf 'All %s fast checks passed. Run without --fast before opening a PR.\n' "$passed"
else
  printf 'All %s checks passed. Not run locally: markdown links (CI only).\n' "$passed"
fi
