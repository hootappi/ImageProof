$ErrorActionPreference = "Stop"

$projectRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
Set-Location $projectRoot

function Ensure-Command {
    param(
        [Parameter(Mandatory = $true)][string]$Name,
        [Parameter(Mandatory = $true)][string]$InstallHint
    )

    if (-not (Get-Command $Name -ErrorAction SilentlyContinue)) {
        throw "$Name is required. $InstallHint"
    }
}

function Ensure-WasmPack {
    $wasmPackPath = Join-Path $env:USERPROFILE ".cargo\bin\wasm-pack.exe"
    if (Test-Path $wasmPackPath) {
        return
    }

    $cargoPath = Join-Path $env:USERPROFILE ".cargo\bin\cargo.exe"
    if (-not (Test-Path $cargoPath)) {
        throw "cargo.exe is required to install wasm-pack. Install Rust via winget: winget install -e --id Rustlang.Rustup"
    }

    Write-Host "Installing wasm-pack (one-time)..." -ForegroundColor Yellow
    & $cargoPath install wasm-pack
}

function Ensure-WasmTarget {
    $rustupPath = Join-Path $env:USERPROFILE ".cargo\bin\rustup.exe"
    if (-not (Test-Path $rustupPath)) {
        throw "rustup.exe not found at $rustupPath"
    }

    & $rustupPath target add wasm32-unknown-unknown
}

function Ensure-VisualCppLinker {
    $linker = Get-ChildItem "${env:ProgramFiles(x86)}\Microsoft Visual Studio\2022\BuildTools\VC\Tools\MSVC" -Filter link.exe -Recurse -ErrorAction SilentlyContinue |
        Where-Object { $_.FullName -like "*\bin\Hostx64\x64\link.exe" } |
        Select-Object -First 1

    if ($linker) {
        return
    }

    Write-Host "Installing Visual Studio Build Tools C++ workload (one-time)..." -ForegroundColor Yellow
    winget install -e --id Microsoft.VisualStudio.2022.BuildTools --override "--quiet --wait --norestart --nocache --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended" --accept-source-agreements --accept-package-agreements
}

Ensure-Command -Name "node" -InstallHint "Install Node.js LTS from https://nodejs.org"
Ensure-Command -Name "npm" -InstallHint "Install Node.js LTS from https://nodejs.org"
Ensure-Command -Name "winget" -InstallHint "Install Windows App Installer from Microsoft Store"

$env:Path = "$env:USERPROFILE\.cargo\bin;$env:Path"

if (-not (Get-Command rustup -ErrorAction SilentlyContinue)) {
    Write-Host "Installing Rust toolchain (one-time)..." -ForegroundColor Yellow
    winget install -e --id Rustlang.Rustup --accept-source-agreements --accept-package-agreements
    $env:Path = "$env:USERPROFILE\.cargo\bin;$env:Path"
}

Ensure-VisualCppLinker
Ensure-WasmPack
Ensure-WasmTarget

Set-Location (Join-Path $projectRoot "web")

if (-not (Test-Path "node_modules")) {
    Write-Host "Installing npm dependencies (one-time)..." -ForegroundColor Yellow
    npm install
}

Write-Host "Running one-command verification build..." -ForegroundColor Cyan
npm run check

Write-Host "Starting web app on http://127.0.0.1:4173/" -ForegroundColor Green
npm run dev -- --host 127.0.0.1 --port 4173
