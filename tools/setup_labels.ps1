<#
.SYNOPSIS
    Creates or repairs the issue label taxonomy documented in docs/TRIAGE.md.

.DESCRIPTION
    The issue forms in .github/ISSUE_TEMPLATE/ apply labels by name. A label
    that does not exist is not an error anywhere: the issue opens without it,
    silently, and the triage queries in TRIAGE.md then miss it. This script
    makes the label set match the document.

    It is idempotent. Running it twice is harmless, and running it after a
    manual edit resets that label's colour and description to the documented
    values.

.PARAMETER Repo
    Target repository as owner/name. Defaults to this project.

.PARAMETER WhatIf
    Print what would be created without touching the repository.

.EXAMPLE
    .\tools\setup_labels.ps1

.EXAMPLE
    .\tools\setup_labels.ps1 -WhatIf

.NOTES
    Requires the GitHub CLI (https://cli.github.com) and an authenticated
    session with write access to the repository's issues.
    Windows PowerShell 5.1 compatible; PowerShell 7 is not required.
#>
[CmdletBinding(SupportsShouldProcess = $true)]
param(
    [string]$Repo = 'ZinniXX004/Yet-to-be-JoJo-s-RPG-Game-'
)

$ErrorActionPreference = 'Stop'

if (-not (Get-Command gh -ErrorAction SilentlyContinue)) {
    Write-Error @'
The GitHub CLI (gh) is not on PATH.

Install it with:  winget install --id GitHub.cli
Then restart the shell and run:  gh auth login

Alternatively, create the labels by hand in Issues -> Labels; the full table
is in docs/TRIAGE.md.
'@
    exit 1
}

gh auth status 2>&1 | Out-Null
if ($LASTEXITCODE -ne 0) {
    Write-Error 'The GitHub CLI is installed but not authenticated. Run: gh auth login'
    exit 1
}

# name, colour, description. Kept in the same order as the table in
# docs/TRIAGE.md so the two can be diffed by eye.
$labels = @(
    @{ Name = 'type: bug';      Color = 'd73a4a'; Description = 'Behaviour contradicts documented intent' },
    @{ Name = 'type: balance';  Color = '0e8a16'; Description = 'Measured win rate, or an encounter that is not the fight it claims to be' },
    @{ Name = 'type: task';     Color = '1d76db'; Description = 'Planned milestone work' },
    @{ Name = 'type: docs';     Color = '0075ca'; Description = 'Documentation only' },

    @{ Name = 'area: core';     Color = '5319e7'; Description = 'rpg-core: rules, scheduler, resolution, AI, RNG' },
    @{ Name = 'area: bridge';   Color = '8250df'; Description = 'rpg-bridge: the FFI boundary' },
    @{ Name = 'area: godot';    Color = '478cbf'; Description = 'src/game: scenes, GDScript, presentation' },
    @{ Name = 'area: content';  Color = '006b75'; Description = 'data/*.json' },
    @{ Name = 'area: ci';       Color = '444444'; Description = 'Workflows, gates, tooling' },

    @{ Name = 'sev: blocker';   Color = 'b60205'; Description = 'main is broken, or a release cannot be cut' },
    @{ Name = 'sev: major';     Color = 'd93f0b'; Description = 'A feature is unusable; no acceptable workaround' },
    @{ Name = 'sev: minor';     Color = 'fbca04'; Description = 'Wrong but survivable' },

    @{ Name = 'needs: repro';   Color = 'e4e669'; Description = 'Cannot be acted on until a seed is supplied' },
    @{ Name = 'wontfix';        Color = 'ffffff'; Description = 'Closed deliberately, with the reason written in the issue' }
)

Write-Host "Applying $($labels.Count) labels to $Repo" -ForegroundColor Cyan

$failed = 0
foreach ($label in $labels) {
    if (-not $PSCmdlet.ShouldProcess($label.Name, 'create or update label')) {
        continue
    }

    $output = gh label create $label.Name `
        --repo $Repo `
        --color $label.Color `
        --description $label.Description `
        --force 2>&1

    if ($LASTEXITCODE -eq 0) {
        Write-Host "  ok    $($label.Name)" -ForegroundColor Green
    }
    else {
        Write-Host "  FAIL  $($label.Name): $output" -ForegroundColor Red
        $failed++
    }
}

if ($failed -gt 0) {
    Write-Error "$failed label(s) could not be applied. The issue forms will open issues without them."
    exit 1
}

Write-Host ''
Write-Host 'Done. Verify with: gh label list --repo ' -NoNewline
Write-Host $Repo
