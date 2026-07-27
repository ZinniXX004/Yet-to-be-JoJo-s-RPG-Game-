<#
.SYNOPSIS
    Copies canonical content data into the Godot project (Windows equivalent of
    tools/sync_data.sh).

.DESCRIPTION
    Godot cannot read files outside res://, but /data must remain the single
    source of truth because the Rust tests and the Python validator both read it
    directly. Rather than committing two copies of the JSON, it is copied as a
    build step. src/game/data/ is generated output and is gitignored.

.EXAMPLE
    pwsh -File tools/sync_data.ps1
#>

[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'

$root = Split-Path -Parent $PSScriptRoot
$source = Join-Path $root 'data'
$destination = Join-Path $root 'src/game/data'

if (-not (Test-Path $source)) {
    throw "Content directory not found: $source"
}

New-Item -ItemType Directory -Force -Path $destination | Out-Null
Get-ChildItem -Path $destination -Filter '*.json' -ErrorAction SilentlyContinue | Remove-Item -Force

$files = Get-ChildItem -Path $source -Filter '*.json'
foreach ($file in $files) {
    Copy-Item -Path $file.FullName -Destination $destination -Force
}

Write-Host ("Synced {0} content file(s) to {1}" -f $files.Count, $destination)
