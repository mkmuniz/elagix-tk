# Schliffe installer — native Windows (specs.md §5.1, M8).
#
# v1 (2026-07-26): doesn't cross-compile on its own (that's done on the
# WSL/Linux side with `cargo build --release --target x86_64-pc-windows-gnu`
# — see MILESTONES.md M8). This script assumes an `schliffe.exe` already
# exists in one of the three locations below, or that `cargo` (a native
# Windows toolchain) is available to build it on the spot. Symlinks are
# deliberately not used — would need developer mode/admin on Windows; a
# plain copy works just as well, since schliffe decides what to filter by the
# file's NAME (argv[0]), not by whether it's a link or a copy.
$ErrorActionPreference = "Stop"

$ShimsDir = if ($env:SCHLIFFE_SHIMS_DIR) { $env:SCHLIFFE_SHIMS_DIR } else { Join-Path $env:USERPROFILE ".schliffe\shims" }

# Same list as install.sh (Layer A: git/pytest/cargo; Layer B:
# docker/npm/terraform) — keep both in sync if the list changes.
$DefaultCommands = @("git", "cargo", "pytest", "docker", "npm", "pnpm", "yarn", "pip", "pip3", "dotnet", "go", "terraform")

function Find-SchliffeExe {
    $candidates = @(
        (Join-Path $PSScriptRoot "target\release\schliffe.exe"),
        (Join-Path $PSScriptRoot "target\x86_64-pc-windows-gnu\release\schliffe.exe"),
        (Join-Path $PSScriptRoot "schliffe.exe")
    )
    foreach ($c in $candidates) {
        if (Test-Path $c) { return $c }
    }
    return $null
}

$SchliffeExe = Find-SchliffeExe

if (-not $SchliffeExe) {
    $cargo = Get-Command cargo -ErrorAction SilentlyContinue
    if ($cargo) {
        Write-Host "schliffe: no ready-made schliffe.exe found, building with cargo (native Windows toolchain)..."
        Push-Location $PSScriptRoot
        try { cargo build --release } finally { Pop-Location }
        $SchliffeExe = Find-SchliffeExe
    }
}

if (-not $SchliffeExe) {
    Write-Error "schliffe: couldn't find schliffe.exe (checked target\release, target\x86_64-pc-windows-gnu\release, and next to the script) and cargo isn't available to build it. Run 'cargo build --release --target x86_64-pc-windows-gnu' on WSL first, or install Rust (https://rustup.rs) here."
    exit 1
}

Write-Host "schliffe: using binary at $SchliffeExe"

New-Item -ItemType Directory -Force -Path $ShimsDir | Out-Null
foreach ($cmd in $DefaultCommands) {
    $dest = Join-Path $ShimsDir "$cmd.exe"
    Copy-Item -Path $SchliffeExe -Destination $dest -Force
}
Write-Host "schliffe: shims created in $ShimsDir for: $($DefaultCommands -join ', ')"

$currentUserPath = [Environment]::GetEnvironmentVariable("Path", "User")
$pathEntries = @()
if ($currentUserPath) { $pathEntries = $currentUserPath.Split(";") }

if ($pathEntries -contains $ShimsDir) {
    Write-Host "schliffe: user PATH already has $ShimsDir (nothing to do)"
} else {
    $newPath = if ($currentUserPath) { "$ShimsDir;$currentUserPath" } else { $ShimsDir }
    [Environment]::SetEnvironmentVariable("Path", $newPath, "User")
    Write-Host "schliffe: $ShimsDir added to the user PATH (persistent)"
}

Write-Host ""
Write-Host "schliffe: note - the Claude Code hook (remote MCP servers like Figma, image"
Write-Host "        resizing) is NOT installed: hook output replacement doesn't work on"
Write-Host "        native Windows. Use Schliffe from WSL to get it."
Write-Host "schliffe: installed. Open a NEW terminal to pick up the updated PATH."
Write-Host "schliffe: test with 'git status | more' or any pipe -- if it filters, it worked."
