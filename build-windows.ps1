<#
.SYNOPSIS
  Build KimchiCraft for Windows with the Korean interface.

.DESCRIPTION
  The interface translations live in storytold/craft-i18n, not in this repository. Without them
  the build still succeeds but every menu is in English. This script checks out craft-i18n beside
  this repository (..\craft-i18n) at the commit pinned in craft-i18n.rev, points CRAFT_I18N_DIR at
  it and builds the desktop app in release mode.

  Result: target\release\wordcraft.exe

.EXAMPLE
  .\build-windows.ps1          # build
  .\build-windows.ps1 -Run     # build, then start the app
#>
param(
  [switch] $Run
)
$ErrorActionPreference = 'Stop'
$Root = $PSScriptRoot
$I18n = Join-Path (Split-Path $Root -Parent) 'craft-i18n'
$Rev = (Get-Content (Join-Path $Root 'craft-i18n.rev') -TotalCount 1).Trim()

function Invoke-Native([string] $What, [scriptblock] $Block) {
  Write-Output "==> $What"
  & $Block
  if ($LASTEXITCODE -ne 0) { throw "$What failed with exit code $LASTEXITCODE" }
}

if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
  throw 'cargo not found: install Rust from https://rustup.rs (MSVC toolchain) and the Visual Studio C++ build tools'
}

if (-not (Test-Path (Join-Path $I18n '.git'))) {
  Invoke-Native "clone craft-i18n into $I18n" { git clone https://github.com/storytold/craft-i18n.git $I18n }
}
$Have = (git -C $I18n rev-parse HEAD).Trim()
if ($Have -ne $Rev) {
  Invoke-Native "fetch craft-i18n" { git -C $I18n fetch origin }
  Invoke-Native "check out craft-i18n $Rev" { git -C $I18n checkout --quiet $Rev }
}

$env:CRAFT_I18N_DIR = $I18n
# Fail instead of quietly building an English-only app.
$env:CRAFT_I18N_REQUIRED = '1'

Push-Location $Root
try {
  Invoke-Native 'cargo build --release -p wordcraft' { cargo build --release -p wordcraft }
} finally {
  Pop-Location
}

$Exe = Join-Path $Root 'target\release\wordcraft.exe'
Write-Output "Built: $Exe"
if ($Run) { Start-Process $Exe }
