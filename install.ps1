# Instalador Elagix — Windows nativo (specs.md §5.1, M8).
#
# v1 (2026-07-26): não builda cross-compilado sozinho (isso é feito no lado
# WSL/Linux com `cargo build --release --target x86_64-pc-windows-gnu` —
# ver MILESTONES.md M8). Este script assume que já existe um `elagix.exe`
# em um dos três lugares abaixo, ou que o `cargo` (toolchain Windows nativa)
# está disponível pra buildar na hora. Symlink não é usado de propósito —
# precisaria de modo desenvolvedor/admin no Windows; cópia simples funciona
# igual, porque o elagix decide o que filtrar pelo NOME do arquivo (argv[0]),
# não por ser link ou cópia.
$ErrorActionPreference = "Stop"

$ShimsDir = if ($env:ELAGIX_SHIMS_DIR) { $env:ELAGIX_SHIMS_DIR } else { Join-Path $env:USERPROFILE ".elagix\shims" }

# Mesma lista do install.sh (Camada A: git/pytest/cargo; Camada B:
# docker/npm/terraform) — manter as duas em sincronia se a lista mudar.
$DefaultCommands = @("git", "cargo", "pytest", "docker", "npm", "terraform")

function Find-ElagixExe {
    $candidates = @(
        (Join-Path $PSScriptRoot "target\release\elagix.exe"),
        (Join-Path $PSScriptRoot "target\x86_64-pc-windows-gnu\release\elagix.exe"),
        (Join-Path $PSScriptRoot "elagix.exe")
    )
    foreach ($c in $candidates) {
        if (Test-Path $c) { return $c }
    }
    return $null
}

$ElagixExe = Find-ElagixExe

if (-not $ElagixExe) {
    $cargo = Get-Command cargo -ErrorAction SilentlyContinue
    if ($cargo) {
        Write-Host "elagix: nao achei um elagix.exe pronto, compilando com cargo (toolchain Windows nativa)..."
        Push-Location $PSScriptRoot
        try { cargo build --release } finally { Pop-Location }
        $ElagixExe = Find-ElagixExe
    }
}

if (-not $ElagixExe) {
    Write-Error "elagix: nao achei elagix.exe (procurei target\release, target\x86_64-pc-windows-gnu\release e ao lado do script) e nao tem cargo disponivel pra compilar. Rode 'cargo build --release --target x86_64-pc-windows-gnu' no WSL primeiro, ou instale o Rust (https://rustup.rs) aqui."
    exit 1
}

Write-Host "elagix: usando binario em $ElagixExe"

New-Item -ItemType Directory -Force -Path $ShimsDir | Out-Null
foreach ($cmd in $DefaultCommands) {
    $dest = Join-Path $ShimsDir "$cmd.exe"
    Copy-Item -Path $ElagixExe -Destination $dest -Force
}
Write-Host "elagix: shims criados em $ShimsDir para: $($DefaultCommands -join ', ')"

$currentUserPath = [Environment]::GetEnvironmentVariable("Path", "User")
$pathEntries = @()
if ($currentUserPath) { $pathEntries = $currentUserPath.Split(";") }

if ($pathEntries -contains $ShimsDir) {
    Write-Host "elagix: PATH do usuario ja tem $ShimsDir (nada a fazer)"
} else {
    $newPath = if ($currentUserPath) { "$ShimsDir;$currentUserPath" } else { $ShimsDir }
    [Environment]::SetEnvironmentVariable("Path", $newPath, "User")
    Write-Host "elagix: $ShimsDir adicionado ao PATH do usuario (persistente)"
}

Write-Host ""
Write-Host "elagix: instalado. Abra um terminal NOVO pra pegar o PATH atualizado."
Write-Host "elagix: teste com 'git status | more' ou qualquer pipe -- se filtrar, funcionou."
