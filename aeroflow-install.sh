#!/usr/bin/env bash
set -euo pipefail

# AeroFlow Agent — Installation Script (Unix)
# Developer: Jettanakorn Pengsiri — JFOX Aircraft Co., Ltd.

VERSION="0.4.0"
REPO="ghcr.io/Jettanakorn/aeroflow"

echo ""
echo "  ╔══════════════════════════════════════════════╗"
echo "  ║        AeroFlow Agent v${VERSION}              ║"
echo "  ║   Autonomous CFD Analysis — JFOX Aircraft    ║"
echo "  ╚══════════════════════════════════════════════╝"
echo ""

# ── 1. Check prerequisites ──
echo "[1/6] Checking prerequisites..."

MISSING=()
command -v docker >/dev/null 2>&1 || MISSING+=("Docker (install from https://docker.com)")
command -v git >/dev/null 2>&1     || MISSING+=("Git (install from https://git-scm.com)")

if ! command -v rustc >/dev/null 2>&1; then
    echo "  ⚠ Rust not found — will use Docker image"
fi

if [ ${#MISSING[@]} -gt 0 ]; then
    echo "  ❌ Missing prerequisites:"
    for m in "${MISSING[@]}"; do echo "     - $m"; done
    exit 1
fi
echo "  ✅ All prerequisites found"

# ── 2. Choose method ──
echo ""
echo "[2/6] Choosing installation method..."

METHOD="docker"
if command -v rustc >/dev/null 2>&1; then
    echo "  Rust detected — building from source (recommended)"
    METHOD="source"
else
    echo "  Using Docker image ($REPO)"
fi

# ── 3. Install / build ──
echo ""
echo "[3/6] Installing AeroFlow Agent..."

INSTALL_DIR="${HOME}/.aeroflow"
mkdir -p "$INSTALL_DIR"

if [ "$METHOD" = "source" ]; then
    if [ ! -f "$PWD/Cargo.toml" ]; then
        echo "  Cloning repository..."
        git clone https://github.com/Jettanakorn/aeroflow.git "${INSTALL_DIR}/src"
        cd "${INSTALL_DIR}/src"
    else
        echo "  Using local source at $PWD"
    fi

    echo "  Building release binary (5-15 minutes)..."
    cargo build --release

    cp target/release/aeroflow "$INSTALL_DIR/aeroflow"
    echo "  ✅ Binary built: ${INSTALL_DIR}/aeroflow"

    # Add to PATH
    if [[ ":$PATH:" != *":$INSTALL_DIR:"* ]]; then
        SHELL_CONFIG="${HOME}/.bashrc"
        if [ -n "$ZSH_VERSION" ]; then SHELL_CONFIG="${HOME}/.zshrc"; fi
        echo "export PATH=\"\$PATH:$INSTALL_DIR\"" >> "$SHELL_CONFIG"
        export PATH="$PATH:$INSTALL_DIR"
        echo "  ✅ Added $INSTALL_DIR to PATH in $SHELL_CONFIG"
    fi
else
    echo "  Pulling Docker image..."
    docker pull "${REPO}:${VERSION}"
    echo "  ✅ Docker image pulled: ${REPO}:${VERSION}"
fi

# ── 4. Pull Docker dependencies ──
echo ""
echo "[4/6] Pulling Docker dependencies..."
docker pull postgres:16-alpine
echo "  ✅ PostgreSQL 16 Alpine ready"

# ── 5. Initialize workspace ──
echo ""
echo "[5/6] Initializing workspace..."

DEFAULT_WORKSPACE="${HOME}/aeroflow-workspace"
read -rp "  Enter workspace path [${DEFAULT_WORKSPACE}]: " WORKSPACE
WORKSPACE="${WORKSPACE:-$DEFAULT_WORKSPACE}"

if [ "$METHOD" = "source" ]; then
    "$INSTALL_DIR/aeroflow" settings init "$WORKSPACE"
else
    docker run --rm -v "${WORKSPACE}:/workspace" "${REPO}:${VERSION}" aeroflow settings init /workspace
fi
echo "  ✅ Workspace initialized at $WORKSPACE"

# ── 6. First-run health check ──
echo ""
echo "[6/6] Running first health check..."

if [ "$METHOD" = "source" ]; then
    "$INSTALL_DIR/aeroflow" doctor
else
    docker run --rm -v "${WORKSPACE}:/workspace" --network host "${REPO}:${VERSION}" aeroflow doctor
fi

echo ""
echo "  ╔══════════════════════════════════════════════╗"
echo "  ║        AeroFlow Agent v${VERSION}               ║"
echo "  ║           Installation Complete!              ║"
echo "  ╚══════════════════════════════════════════════╝"
echo ""
echo "  Next steps:"
echo "    1. Run your first CFD simulation:"
echo "       aeroflow init my-first-wing"
echo "       aeroflow run ./cases/my-first-wing"
echo ""
echo "    2. Launch the TUI dashboard:"
echo "       aeroflow tui"
echo ""
echo "    3. Start the REST API server:"
echo "       aeroflow serve"
echo ""
echo "  Documentation: https://github.com/Jettanakorn/aeroflow"
echo ""
