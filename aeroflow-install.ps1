<#
.SYNOPSIS
    AeroFlow Agent — Installation Script (Windows)
.DESCRIPTION
    Downloads or builds AeroFlow Agent, initializes workspace,
    verifies dependencies, and guides first-time setup.
#>

$ErrorActionPreference = "Stop"
$Version = "0.4.0"
$Repo = "ghcr.io/Jettanakorn/aeroflow"

Write-Host @"

  ╔══════════════════════════════════════════════╗
  ║        AeroFlow Agent v$Version              ║
  ║   Autonomous CFD Analysis — JFOX Aircraft    ║
  ╚══════════════════════════════════════════════╝
"@ -ForegroundColor Cyan

# ── 1. Check prerequisites ──
Write-Host "`n[1/6] Checking prerequisites..." -ForegroundColor Yellow

$missing = @()
if (-not (Get-Command docker -ErrorAction SilentlyContinue)) {
    $missing += "Docker (install from https://docker.com)"
}
if (-not (Get-Command rustc -ErrorAction SilentlyContinue)) {
    Write-Host "  ⚠ Rust not found — will use Docker image instead of building" -ForegroundColor DarkYellow
}
if (-not (Get-Command git -ErrorAction SilentlyContinue)) {
    $missing += "Git (install from https://git-scm.com)"
}

if ($missing.Count -gt 0) {
    Write-Host "  ❌ Missing prerequisites:" -ForegroundColor Red
    $missing | ForEach-Object { Write-Host "     - $_" }
    exit 1
}
Write-Host "  ✅ All prerequisites found" -ForegroundColor Green

# ── 2. Choose installation method ──
Write-Host "`n[2/6] Choosing installation method..." -ForegroundColor Yellow
$method = "docker"

if (Get-Command rustc -ErrorAction SilentlyContinue) {
    Write-Host "  Rust detected — building from source (recommended)"
    $method = "source"
} else {
    Write-Host "  Using Docker image (ghcr.io/Jettanakorn/aeroflow)"
}

# ── 3. Install / build ──
Write-Host "`n[3/6] Installing AeroFlow Agent..." -ForegroundColor Yellow

$InstallDir = "$env:USERPROFILE\.aeroflow"
New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null

if ($method -eq "source") {
    if (-not (Test-Path "$PSScriptRoot\Cargo.toml")) {
        Write-Host "  Cloning repository..."
        git clone https://github.com/Jettanakorn/aeroflow.git "$InstallDir\src" 2>&1 | Out-Null
        Set-Location "$InstallDir\src"
    } else {
        Write-Host "  Using local source at $PSScriptRoot"
        Set-Location $PSScriptRoot
    }

    Write-Host "  Building release binary (this may take 5-15 minutes)..."
    $build = cargo build --release 2>&1
    if ($LASTEXITCODE -ne 0) {
        Write-Host "  ❌ Build failed:" -ForegroundColor Red
        Write-Host $build
        exit 1
    }

    $BinPath = "target\release\aeroflow.exe"
    Copy-Item $BinPath "$InstallDir\aeroflow.exe" -Force
    Write-Host "  ✅ Binary built: $InstallDir\aeroflow.exe" -ForegroundColor Green

    # Add to PATH
    $userPath = [Environment]::GetEnvironmentVariable("Path", "User")
    if ($userPath -notlike "*$InstallDir*") {
        [Environment]::SetEnvironmentVariable("Path", "$userPath;$InstallDir", "User")
        $env:Path += ";$InstallDir"
        Write-Host "  ✅ Added $InstallDir to PATH" -ForegroundColor Green
    }
} else {
    Write-Host "  Pulling Docker image..."
    docker pull "$Repo`:$Version" 2>&1 | Out-Null
    if ($LASTEXITCODE -eq 0) {
        Write-Host "  ✅ Docker image pulled: $Repo`:$Version" -ForegroundColor Green
    } else {
        Write-Host "  ❌ Failed to pull Docker image" -ForegroundColor Red
        exit 1
    }
}

# ── 4. Pull Docker dependencies ──
Write-Host "`n[4/6] Pulling Docker dependencies..." -ForegroundColor Yellow
docker pull postgres:16-alpine 2>&1 | Out-Null
Write-Host "  ✅ PostgreSQL 16 Alpine ready" -ForegroundColor Green

# ── 5. Initialize workspace ──
Write-Host "`n[5/6] Initializing workspace..." -ForegroundColor Yellow

$defaultWorkspace = "$env:USERPROFILE\aeroflow-workspace"
$workspace = Read-Host "  Enter workspace path [$defaultWorkspace]"
if ([string]::IsNullOrWhiteSpace($workspace)) { $workspace = $defaultWorkspace }

if ($method -eq "source") {
    & "$InstallDir\aeroflow.exe" settings init "$workspace" 2>&1 | Out-Null
} else {
    docker run --rm -v "${workspace}:/workspace" "$Repo`:$Version" aeroflow settings init /workspace 2>&1 | Out-Null
}
Write-Host "  ✅ Workspace initialized at $workspace" -ForegroundColor Green

# ── 6. First-run health check ──
Write-Host "`n[6/6] Running first health check..." -ForegroundColor Yellow

if ($method -eq "source") {
    & "$InstallDir\aeroflow.exe" doctor
} else {
    docker run --rm -v "${workspace}:/workspace" --network host "$Repo`:$Version" aeroflow doctor
}

Write-Host @"

  ╔══════════════════════════════════════════════╗
  ║        AeroFlow Agent v$Version               ║
  ║           Installation Complete!              ║
  ╚══════════════════════════════════════════════╝

  Next steps:
    1. Run your first CFD simulation:
       aeroflow init my-first-wing
       aeroflow run .\cases\my-first-wing

    2. Launch the TUI dashboard:
       aeroflow tui

    3. Start the REST API server:
       aeroflow serve

  Documentation: https://github.com/Jettanakorn/aeroflow
"@ -ForegroundColor Cyan
