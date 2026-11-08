<#
.SYNOPSIS
    Compila os binários em modo release e atualiza a pasta mikella/dist.
#>
[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'
$scriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$mikellaDir = Split-Path -Parent $scriptDir
$cargoToml = Join-Path $mikellaDir "Cargo.toml"
$distDir = Join-Path $mikellaDir "dist"

Write-Host "Compilando mikella-store-server e mikella-desktop-shell (release)..." -ForegroundColor Cyan
& cargo build --release --manifest-path $cargoToml -p mikella-store-server -p mikella-desktop-shell
if ($LASTEXITCODE -ne 0) {
    Write-Error "Falha na compilação do release."
}

$releaseServer = Join-Path $mikellaDir "target\release\mikella-server.exe"
$releaseApp = Join-Path $mikellaDir "target\release\mikella.exe"

Write-Host "Copiando binários atualizados para mikella\dist..." -ForegroundColor Cyan
try {
    Stop-Process -Name "mikella-server" -Force -ErrorAction SilentlyContinue
    Stop-Process -Name "mikella" -Force -ErrorAction SilentlyContinue
} catch {}

Copy-Item $releaseServer -Destination (Join-Path $distDir "mikella-server.exe") -Force
Copy-Item $releaseApp -Destination (Join-Path $distDir "mikella.exe") -Force

Write-Host "mikella/dist atualizado com sucesso!" -ForegroundColor Green
