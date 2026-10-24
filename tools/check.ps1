<#
.SYNOPSIS
    The local gate: the same checks .github/workflows/ci.yml runs, in one
    command (Windows equivalent of tools/check.sh).

.DESCRIPTION
    All five CI failures between late July and September 2026 were checks
    that could have run locally: the probe mirror twice (#29, #33), rustfmt
    twice (#22, feat/ai-scored-choice), typos once (feat/more-enemies). The hand-copied
    list in docs/DEVELOPMENT.md omitted --mirror and called typos optional, so
    "the local checks" and "what CI runs" had become different things. This
    script and tools/check.sh are the list now; when a job is added to ci.yml,
    add it to both in the same commit.

    Not reproduced: the lychee link check (network-bound). cargo-deny runs only
    if installed.

    Windows PowerShell 5.1 compatible; pwsh is not required.

.PARAMETER Fast
    Formatting, content and spelling only. Takes seconds, and covers every
    check that has actually failed in CI so far. This is what the pre-push hook
    runs. Run the full gate before opening a pull request.

.EXAMPLE
    .\tools\check.ps1

.EXAMPLE
    .\tools\check.ps1 -Fast

.EXAMPLE
    powershell -ExecutionPolicy Bypass -File tools\check.ps1
#>

[CmdletBinding()]
param(
    [switch]$Fast
)

$ErrorActionPreference = 'Continue'

$root = Split-Path -Parent $PSScriptRoot
Set-Location $root

$script:failed = @()
$script:passed = 0

function Invoke-Step {
    param(
        [string]$Label,
        [scriptblock]$Command
    )
    Write-Host ''
    Write-Host "==> $Label" -ForegroundColor Cyan
    $global:LASTEXITCODE = 0
    $ok = $true
    try {
        & $Command
        if ($LASTEXITCODE -ne 0) { $ok = $false }
    }
    catch {
        Write-Host $_ -ForegroundColor Red
        $ok = $false
    }
    if ($ok) { $script:passed++ } else { $script:failed += $Label }
}

# A Python that actually runs. 'python3' / 'python' can be the Microsoft Store
# stub, which is on PATH and opens the Store instead of running anything.
$script:py = $null
foreach ($candidate in @(@('py', '-3'), @('python'), @('python3'))) {
    if (Get-Command $candidate[0] -ErrorAction SilentlyContinue) {
        $probeArgs = @()
        if ($candidate.Count -gt 1) { $probeArgs += $candidate[1..($candidate.Count - 1)] }
        $probeArgs += @('-c', 'import sys; sys.exit(sys.version_info < (3, 11))')
        & $candidate[0] @probeArgs 2>$null | Out-Null
        if ($LASTEXITCODE -eq 0) { $script:py = $candidate; break }
    }
}

function Invoke-Python {
    if (-not $script:py) { throw 'Python 3.11+ not found (tried py -3, python, python3)' }
    $pyArgs = @()
    if ($script:py.Count -gt 1) { $pyArgs += $script:py[1..($script:py.Count - 1)] }
    $pyArgs += $args
    & $script:py[0] @pyArgs
}

# The typos version CI uses, read from ci.yml so a Dependabot bump needs no edit here.
$typosPin = $null
$pinLine = Select-String -Path '.github/workflows/ci.yml' -Pattern 'crate-ci/typos@v([0-9][0-9.]*)' | Select-Object -First 1
if ($pinLine) { $typosPin = $pinLine.Matches[0].Groups[1].Value }

function Invoke-Typos {
    if (-not (Get-Command typos -ErrorAction SilentlyContinue)) {
        throw "typos not installed. CI pins crate-ci/typos@v${typosPin}: cargo install typos-cli --version $typosPin --locked"
    }
    $have = ((& typos --version) -split ' ')[1]
    if ($typosPin -and $have -ne $typosPin) {
        Write-Host "warning: local typos $have, CI pins $typosPin. Dictionaries differ between releases," -ForegroundColor Yellow
        Write-Host "         so a pass here may still fail in CI (PR #36). To match:" -ForegroundColor Yellow
        Write-Host "         cargo install typos-cli --version $typosPin --locked" -ForegroundColor Yellow
    }
    & typos
}

# Every cargo step runs inside src\ and returns to the root, even on failure.
#
# Callers must write clippy's separator as '--', quoted. PowerShell treats a
# bare -- as its own end-of-parameters marker when calling a function and
# removes it before the function sees $args, so cargo then receives
# `clippy ... -D warnings` and rejects -D as an unknown cargo argument. That
# exact failure is how this script's first version was caught.
function Invoke-Cargo {
    Push-Location (Join-Path $root 'src')
    try { & cargo @args } finally { Pop-Location }
}

# --- job: rustfmt ---
Invoke-Step 'rustfmt' { Invoke-Cargo fmt --all --check }

# --- job: content validation ---
Invoke-Step 'content: data/' { Invoke-Python src/data-pipeline/validate_data.py }
Invoke-Step 'content: tools/probe mirrors data/' {
    Invoke-Python src/data-pipeline/validate_data.py `
        --combatants tools/probe/combatants.probe.json `
        --matchups tools/probe/matchups.probe.json `
        --mirror data
}

# --- job: spelling ---
Invoke-Step 'spelling (typos)' { Invoke-Typos }

if (-not $Fast) {
    # --- job: core ---
    Invoke-Step 'core: clippy' { Invoke-Cargo clippy -p rpg-core --all-targets --all-features '--' -D warnings }
    Invoke-Step 'core: tests' { Invoke-Cargo test -p rpg-core --all-targets --all-features }
    Invoke-Step 'core: docs' { Invoke-Cargo doc -p rpg-core --no-deps }

    # --- job: balance harness ---
    Invoke-Step 'balance: every encounter inside its band' { Invoke-Cargo run -q -p rpg-core --bin balance }

    # --- job: bridge ---
    Invoke-Step 'bridge: clippy' { Invoke-Cargo clippy -p rpg-bridge --all-targets '--' -D warnings }
    Invoke-Step 'bridge: build' { Invoke-Cargo build -p rpg-bridge }

    # --- job: supply chain (optional locally) ---
    if (Get-Command cargo-deny -ErrorAction SilentlyContinue) {
        Invoke-Step 'supply chain (cargo-deny)' { Invoke-Cargo deny check advisories bans licenses sources }
    }
    else {
        Write-Host ''
        Write-Host '==> supply chain: skipped (cargo-deny not installed; CI still runs it)'
    }
}

Write-Host ''
if ($script:failed.Count -gt 0) {
    Write-Host 'FAILED:' -ForegroundColor Red
    foreach ($label in $script:failed) { Write-Host "  - $label" -ForegroundColor Red }
    Write-Host "$($script:passed) step(s) passed. Fix the failures above before pushing."
    exit 1
}
if ($Fast) {
    Write-Host "All $($script:passed) fast checks passed. Run without -Fast before opening a PR." -ForegroundColor Green
}
else {
    Write-Host "All $($script:passed) checks passed. Not run locally: markdown links (CI only)." -ForegroundColor Green
}
exit 0
