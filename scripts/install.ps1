[CmdletBinding()]
param([string]$InstallDir = (Join-Path $env:LOCALAPPDATA 'Programs\T-TUI'))
$ErrorActionPreference = 'Stop'
$projectDir = Split-Path $PSScriptRoot -Parent
& cargo build --release --locked --target x86_64-pc-windows-msvc --manifest-path (Join-Path $projectDir 'Cargo.toml')
if ($LASTEXITCODE -ne 0) { throw 'Release build failed.' }
$destination = [System.IO.Path]::GetFullPath($InstallDir)
New-Item -ItemType Directory -Path $destination -Force | Out-Null
Copy-Item -LiteralPath (Join-Path $projectDir 'target\x86_64-pc-windows-msvc\release\ttui.exe') -Destination (Join-Path $destination 'ttui.exe') -Force
Write-Host "Installed $destination\ttui.exe"
Write-Host 'Add this directory to your user PATH, or launch the executable directly. No system settings were changed.'
