<#
.SYNOPSIS
    DSH-UI Windows Build Script (x64)
#>
$ErrorActionPreference = "Stop"
$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Definition
$RootDir = (Resolve-Path "$ScriptDir\..").Path

Write-Host "========================================" -ForegroundColor Cyan
Write-Host "    Building DSH-UI for Windows (x64)   " -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan

Set-Location $RootDir

$env:PATH = "C:\Users\zhangpc\.cargo\bin;" + ($env:PATH -replace '"', '')

Write-Host "[1/2] Installing dependencies..." -ForegroundColor Yellow
npm install

Write-Host "[2/2] Compiling Windows release binary via Cargo & Tauri..." -ForegroundColor Yellow
cargo build --release --manifest-path "$RootDir\src-tauri\Cargo.toml"

$targetExe = "$RootDir\src-tauri\target\release\deepseek-harness.exe"
if (Test-Path $targetExe) {
    New-Item -ItemType Directory -Force -Path "$RootDir\release" | Out-Null
    Copy-Item $targetExe "$RootDir\release\DeepSeek-Harness-x64.exe" -Force
    Write-Host "BUILD SUCCESS! Output saved to: $RootDir\release\DeepSeek-Harness-x64.exe" -ForegroundColor Green
} else {
    Write-Error "Build output not found at $targetExe"
}
