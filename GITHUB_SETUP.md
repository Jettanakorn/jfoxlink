# AeroFlow Agent — GitHub Setup Guide

> How to publish, organize, and maintain the AeroFlow Agent repository.

---

## 1. Create the Repository

### Option A — Via GitHub CLI

```bash
gh repo create Jettanakorn/aeroflow --public --description "AeroFlow Agent — Autonomous CFD Analysis Orchestrator"
```

### Option B — Via Web

1. Go to https://github.com/new
2. Owner: `Jettanakorn`
3. Repository name: `aeroflow`
4. Description: `Autonomous CFD analysis using OpenFOAM + ParaView. Self-improving skills database with Gaussian Process optimization.`
5. Visibility: Public or Private
6. Do NOT initialize with README, .gitignore, or license (we have them)

---

## 2. Push Local Code

```bash
# From the project root (C:\home\project\AeroFlow Agent)
git init
git add .
git commit -m "Initial commit: AeroFlow Agent v0.4.0 — all 5 phases"

# Point to your remote
git remote add origin https://github.com/Jettanakorn/aeroflow.git
git branch -M main
git push -u origin main
```

### Add a .gitignore if one doesn't exist:

```gitignore
# Rust
target/
**/*.rs.bk
Cargo.lock

# IDE
.vscode/
.idea/
*.swp
*.swo

# OS
.DS_Store
Thumbs.db

# AeroFlow workspace (user data)
workspace/
cases/
reports/
temp/
logs/

# Environment
.env
*.local
```

---

## 3. GitHub Secrets (for CI/CD)

Navigate to: `Settings → Secrets and variables → Actions`

| Secret | Value | Used By |
|--------|-------|---------|
| `DOCKER_USERNAME` | Your Docker Hub / GHCR username | docker job |
| `DOCKER_PASSWORD` | GHCR token or Docker Hub PAT | docker job |
| `CARGO_REGISTRY_TOKEN` | crates.io API token (optional) | publish job |

For GHCR authentication, create a Personal Access Token:
`GitHub Settings → Developer settings → Personal access tokens → Fine-grained tokens`
- Scopes: `write:packages`, `read:packages`
- Expiration: 90 days

---

## 4. Branch Protection Rules

Navigate to: `Settings → Branches → Add rule`

**Branch:** `main`

- [x] Require pull request reviews (1 reviewer)
- [x] Dismiss stale reviews
- [x] Require status checks to pass:
  - `cargo check`
  - `cargo test`
  - `cargo clippy`
  - `cargo fmt`
  - `helm-lint`
- [x] Require branches to be up to date
- [x] Do not allow bypass
- [x] Linear history (no merge commits)

---

## 5. Repository Topics

Add these under `About → Topics`:

`cfd` `openfoam` `aerospace` `simulation` `rust` `optimization` `gaussian-processes` `kubernetes` `helm`

---

## 6. GitHub Labels

Create standard labels for issue tracking:

```bash
gh label create "bug" --color d73a4a --description "Something isn't working"
gh label create "enhancement" --color a2eeef --description "New feature or request"
gh label create "pipeline" --color 008672 --description "Pipeline stage issue"
gh label create "mesh" --color 7057ff --description "Meshing related"
gh label create "solver" --color ff8c00 --description "Solver related"
gh label create "skills" --color 00bfff --description "Skills DB / GP optimization"
gh label create "api" --color 3fb950 --description "REST API"
gh label create "docker" --color 0db7ed --description "Docker / containerization"
gh label create "docs" --color 6f42c1 --description "Documentation"
gh label create "good first issue" --color 7057ff --description "Good for newcomers"
```

---

## 7. Issue Templates

Create `.github/ISSUE_TEMPLATE/bug_report.md`:

```markdown
---
name: Bug report
about: Report a CFD pipeline or performance issue
---

**AeroFlow Version:** v0.x.x
**OpenFOAM Version:** (e.g., v2312, v2406)
**Geometry:** (STL file description)
**Flow Conditions:** (Mach, Re, alpha)

**Describe the bug**
A clear description of what goes wrong.

**Pipeline output** (last 20 lines of `aeroflow run`)

**Expected behavior**
What should happen instead.

**Environment:**
- OS: [e.g. Ubuntu 24.04, Windows 11]
- Docker: [e.g. 27.0]
- PostgreSQL: [e.g. 16]
```

Create `.github/ISSUE_TEMPLATE/feature_request.md`:

```markdown
---
name: Feature request
about: Suggest an enhancement
---

**Is your feature request related to a problem?**
A clear description of the limitation.

**Proposed solution**
How you'd like it to work.

**Alternative approaches**
What else have you considered?

**Relevant phase:**
- [ ] P0 Scaffold
- [ ] P1 Real Implementation
- [ ] P2 Pipeline
- [ ] P3 SaaS / API
- [ ] P4 Skills / GP
- [ ] P5 Production
```

---

## 8. Release Workflow

### Creating a Release

```bash
# 1. Update version in Cargo.toml
#    (workspace level and all crate Cargo.toml files)

# 2. Commit and tag
git commit -m "Release v0.5.0"
git tag -a v0.5.0 -m "AeroFlow Agent v0.5.0"
git push origin main --tags
```

### What the CI does on release:

1. **Docker** — builds `linux/amd64` and `linux/arm64`, pushes to GHCR with tags:
   - `ghcr.io/Jettanakorn/aeroflow:v0.5.0`
   - `ghcr.io/Jettanakorn/aeroflow:v0.5`
   - `ghcr.io/Jettanakorn/aeroflow:latest`
2. **Docs** — deploys `cargo doc` to GitHub Pages
3. **Release** — packages Helm chart and uploads artifacts

---

## 9. GitHub Pages (Documentation)

The CI auto-deploys Rust documentation to Pages on every release.

Navigate to: `Settings → Pages`
- Source: `GitHub Actions`

The workflow in `.github/workflows/ci.yml` job `deploy-docs` handles this automatically.

---

## 10. Repository Structure (Final)

```text
aeroflow/
├── .github/
│   ├── workflows/
│   │   └── ci.yml                  # 8-job CI/CD pipeline
│   └── ISSUE_TEMPLATE/
│       ├── bug_report.md
│       └── feature_request.md
├── scripts/
│   └── viz/                        # Python VTK + matplotlib viz
│       ├── generate_viz.py         # 3 image types
│       └── inject_report.py        # Embed images in HTML
├── crates/
│   ├── aeroflow-core/              # Types, config, events, metrics
│   ├── aeroflow-cli/               # CLI + TUI
│   ├── aeroflow-pipeline/          # 9-stage orchestrator
│   ├── aeroflow-docker/            # Container management
│   ├── aeroflow-doctor/            # Health checks
│   ├── aeroflow-skills/            # PostgreSQL + fingerprint
│   ├── aeroflow-mesh/              # blockMesh/snappyHexMesh
│   ├── aeroflow-solver/            # Solver config + launch
│   ├── aeroflow-post/              # Force extraction
│   ├── aeroflow-report/            # HTML report generation
│   ├── aeroflow-monitor/           # Resource monitoring
│   ├── aeroflow-learner/           # GP optimization
│   ├── aeroflow-events/            # File watcher
│   └── aeroflow-api/               # REST API
├── docker/
│   ├── Dockerfile                  # Multi-stage build
│   └── docker-compose.yml          # Stack definition
├── db/migrations/
│   └── 001_initial_schema.sql      # 7-table PostgreSQL schema
├── helm/aeroflow/
│   ├── Chart.yaml
│   ├── values.yaml
│   └── templates/                  # 9 K8s templates
├── templates/
│   └── report.html.tera            # HTML report template
├── .skills/openfoam-aerospace/     # 8-file expert skill
├── QUICKSTART.md
├── GITHUB_SETUP.md
├── PHASE_ROADMAP.md
├── Cargo.toml                      # Workspace root
├── aeroflow-install.sh             # Unix install script
├── aeroflow-install.ps1            # Windows install script
└── README.md                       # See Section 11
```

---

## 11. README.md

Create or update `README.md`:

```markdown
<p align="center">
  <h1 align="center">AeroFlow Agent</h1>
  <p align="center"><i>Autonomous CFD Analysis Orchestrator</i></p>
  <p align="center">
    <a href="https://github.com/Jettanakorn/aeroflow/actions"><img src="https://github.com/Jettanakorn/aeroflow/actions/workflows/ci.yml/badge.svg" alt="CI"></a>
    <a href="https://github.com/Jettanakorn/aeroflow/releases"><img src="https://img.shields.io/github/v/release/Jettanakorn/aeroflow" alt="Release"></a>
    <a href="https://github.com/Jettanakorn/aeroflow/blob/main/LICENSE"><img src="https://img.shields.io/github/license/Jettanakorn/aeroflow" alt="License"></a>
  </p>
</p>

**AeroFlow Agent** is a Rust-based autonomous CFD analysis system that takes an STL geometry and produces a complete aerodynamic report — mesh quality, convergence history, and force coefficients (Cl/Cd/Cm) — without manual intervention.

Built on OpenFOAM, PostgreSQL, and Gaussian Process optimization. Deployable via Docker Compose or Kubernetes Helm chart.

## Features

- **End-to-end automation:** STL → voxel fingerprint → 8-stage pipeline → HTML report
- **Self-healing mesh:** adaptive snappyHexMesh with up to 3 retries
- **Autonomous skill learning:** Gaussian Process optimizes solver parameters per (geometry, flow regime)
- **Real-time monitoring:** TUI dashboard, SSE event stream, Prometheus metrics
- **SaaS-ready:** JWT auth, multi-tenant, REST API, file watcher auto-import
- **Production-grade:** Helm chart, HPA, multi-arch Docker images, structured JSON logging

## Quick Start

```bash
docker compose -f docker/docker-compose.yml up -d
docker compose run agent aeroflow init my-wing
docker compose run agent aeroflow run /data/cases/my-wing
docker compose run agent aeroflow report my-wing
```

Or with Helm: `helm install aeroflow helm/aeroflow`

## Documentation

- [Quickstart Guide](QUICKSTART.md)
- [Phase Roadmap](PHASE_ROADMAP.md)
- [GitHub Setup](GITHUB_SETUP.md)

## Architecture

14 Rust crates in a workspace, connected via event bus:

```
STL → surfaceFeatureExtract → blockMesh + adaptive snappyHexMesh → checkMesh
  → controlDict/fvSchemes/fvSolution → simpleFoam/rhoCentralFoam
  → forceCoeffs extraction → HTML report
```

Skills feedback loop: `case result → reward → GP update → next parameters → run again`

## License

MIT © Jettanakorn Pengsiri — JFOX Aircraft Co., Ltd.
```

---

## 12. Post-Push Checklist

```markdown
After pushing to GitHub, verify:

[ ] GitHub Actions CI runs all 8 jobs successfully
[ ] Docker image builds and pushes to GHCR
[ ] Helm chart can be installed (`helm install aeroflow helm/aeroflow`)
[ ] Issue templates appear under "New Issue"
[ ] Branch protection rules are active
[ ] Repository topics are visible
[ ] GitHub Pages is configured for docs deployment
[ ] Labels are created for issue tracking
[ ] Secrets are configured for CI/CD
[ ] License is displayed on repo page
```
