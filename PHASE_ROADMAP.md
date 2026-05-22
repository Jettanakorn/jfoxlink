# AeroFlow Agent — Phase Roadmap

> Developer: **Jettanakorn Pengsiri** by JFOX Aircraft Co., Ltd.
> Rust nightly | Edition 2024 | 14-crate workspace + Python viz | OpenFOAM + ParaView + VTK | PostgreSQL + Prometheus

---

## Big Picture — Phase Overview

| Phase | Title | What It Delivers |
|-------|-------|------------------|
| **P0** | 🏗 Scaffold | Workspace skeleton, core types, CLI skeleton, Docker setup, PostgreSQL schema |
| **P0.5** | ⚙️ Foundation | Settings system, user management, workspace manager, OpenFOAM binary format |
| **P1** | 🔧 Real Impl | Replace all stubs: real Docker/DB disk checks, STL ingestion, OpenFOAM execution |
| **P2** | 🔗 Pipeline | End-to-end autonomous CFD: STL in → report out, self-healing mesh loop |
| **P3** | 🌐 SaaS | Multi-tenant REST API, JWT auth, per-user quotas, case isolation |
| **P3.5** | 🧹 Polish | File watcher, auto-fix doctor, continuous monitoring, graceful shutdown |
| **P4** | 🧠 Skills | Real STL voxelization, Gaussian Process optimization, autonomous skill learning |
| **P5** | 🚀 Production | CI/CD, Prometheus metrics, K8s Helm chart, structured logging, docs |
| **P6** | 📊 Visualization | foamToVTK export + Python matplotlib plots (pressure/velocity/convergence) embedded in report |

---

## Phase Summary

| Phase | Focus | Status |
|-------|-------|--------|
| **P0** | Scaffold — workspace, core types, CLI, Docker, DB schema | ✅ Complete |
| **P0.5** | Settings, Users, Workspace, Binary Format | ✅ Complete |
| **P1** | Real Implementations — replace all stubs | ✅ Complete |
| **P2** | Pipeline Integration — end-to-end autonomous execution | ✅ Complete |
| **P3** | Multi-Tenant SaaS — auth, web API, quotas, isolation | ✅ Complete |
| **P3.5** | Polish — Docker health, file watcher, monitoring, alerts | ✅ Complete |
| **P4** | Advanced Skills — real STL voxelization, GP optimization | ✅ Complete |
| **P5** | Production — CI/CD, metrics, structured logging, Helm chart, scaling, docs | ✅ Complete |
| **P6** | Visualization — foamToVTK + Python matplotlib, report embedding | ✅ Complete |

---

## P0 — Scaffold (✅ Complete)

> Build the full project skeleton: workspace, crates, types, CLI, Docker, DB.

### Deliverables

- [x] **Workspace root** `Cargo.toml` — 14 crate members, all deps
- [x] **aeroflow-core** — types, config, errors, event bus
- [x] **aeroflow-cli** — clap CLI with 11 subcommands, ratatui TUI (3 tabs)
- [x] **aeroflow-pipeline** — PipelineOrchestrator state machine (8 phases + mesh quality loop)
- [x] **aeroflow-docker** — DockerClient, ContainerManager, StatsCollector stubs
- [x] **aeroflow-doctor** — 7-category health check runner (26 checks)
- [x] **aeroflow-skills** — SkillsDb (PostgreSQL), GeometryFingerprint, SkillMatcher
- [x] **aeroflow-mesh** — MeshGenerator (blockMeshDict/snappyHexMeshDict), MeshQualityEngine
- [x] **aeroflow-solver** — SolverConfigGen (solver/turbulence selection, controlDict)
- [x] **aeroflow-post** — ForceExtractor, PostReader, FieldExtractor stubs
- [x] **aeroflow-report** — ReportGenerator (Tera → HTML)
- [x] **aeroflow-monitor** — ResourceMonitor (sysinfo)
- [x] **aeroflow-learner** — RewardFunction, Optimizer, GaussianProcess stubs
- [x] **aeroflow-events** — FileWatcher (notify), WebApi stub
- [x] **Dockerfile** — Multi-stage: Rust builder + OpenFOAM runtime
- [x] **docker-compose.yml** — agent + PostgreSQL 16 Alpine with health check
- [x] **DB migrations** — `001_initial_schema.sql` (7 tables, indexes)
- [x] **Templates** — `report.html.tera` (dark-themed HTML report)
- [x] **Skills** — `openfoam-aerospace` skill (8 files, aerospace CFD expertise)

### Files Created

```text
Cargo.toml                          — Workspace root (13 crate members)
crates/aeroflow-core/src/
  types.rs                          — All domain types (Stage, CaseMeta, IntakeConfig, etc.)
  config.rs                         — AeroFlowConfig, MeshQualityThresholds, ScoringWeights
  errors.rs                         — AeroFlowError enum
  events.rs                         — SystemEvent, EventBus
  lib.rs                            — Module exports
crates/aeroflow-cli/src/
  main.rs                           — CLI entry, 9 subcommands + Skills/User/Settings actions
  commands/init.rs                  — Guided Q&A case creation
  commands/run.rs                   — Pipeline execution stub
  commands/status.rs                — Case status listing
  commands/report.rs                — HTML report generation
  commands/watch.rs                 — STL directory watcher
  commands/serve.rs                 — Web dashboard stub
  commands/doctor.rs                — Health check runner
  commands/skills.rs                — Skills DB management
  commands/user.rs                  — User CRUD + auth
  commands/settings.rs              — Settings manager CLI
  tui/dashboard.rs                  — ratatui TUI (5 tabs)
crates/aeroflow-pipeline/src/orchestrator.rs
crates/aeroflow-docker/src/        — client.rs, container.rs, stats.rs
crates/aeroflow-doctor/src/        — lib.rs, checks/mod.rs
crates/aeroflow-skills/src/        — db.rs, fingerprint.rs, matcher.rs, user_manager.rs
crates/aeroflow-mesh/src/          — generator.rs, quality.rs
crates/aeroflow-solver/src/        — config_gen.rs, launcher.rs, monitor.rs
crates/aeroflow-post/src/          — forces.rs, extract.rs, reader.rs
crates/aeroflow-report/src/lib.rs  — Tera HTML report gen
crates/aeroflow-monitor/src/lib.rs — sysinfo resource monitor
crates/aeroflow-learner/src/       — reward.rs, optimizer.rs, gp.rs
crates/aeroflow-events/src/        — file_watcher.rs, api.rs
docker/Dockerfile                  — Multi-stage build
docker/docker-compose.yml          — Stack definition
db/migrations/001_initial_schema.sql
templates/report.html.tera
```

---

## P0.5 — Settings, Users, Workspace, Binary Format (✅ Complete)

> User management, centralized settings, configurable workspace, and OpenFOAM binary saving.

### Deliverables

- [x] **Workspace Manager** — `WorkspaceManager` + `WorkspaceLayout` in aeroflow-core
  - Directory tree: `cases/`, `import/`, `reports/`, `skills/`, `settings/`, `temp/`, `logs/`
  - Disk space validation via `fs2`
  - `aeroflow settings init /path` CLI command
- [x] **User Management** — `UserManager` in aeroflow-skills
  - CRUD: `create_user`, `get_user`, `list_users`, `update_user`, `delete_user`
  - Auth: `authenticate`, `create_session` (SHA-256 hashing)
  - Roles: `admin`, `engineer`, `viewer`
  - Table migration: `password_hash`, `role`, `active`, `last_login` columns
  - CLI: `aeroflow user create|list|show|update|delete|login`
- [x] **Centralized Settings** — `AeroflowSettings` + `SettingsManager`
  - Load/save TOML config file
  - Merge: defaults → TOML → env vars (`AEROFLOW_*`)
  - CLI: `aeroflow settings show|set key=value|init|reset|path`
- [x] **Binary OpenFOAM Format** — `OpenFOAMFormat` enum (default: Binary)
  - `controlDict` generation uses `format binary`
  - `blockMeshDict` / `snappyHexMeshDict` use `format binary`
  - Saves 60-80% disk space on large cases
- [x] **TUI updated** — Users tab + Settings tab (5 tabs total)

### Key Types Added

```rust
// types.rs
User, UserRole, UserId, CreateUserRequest, UpdateUserRequest, Session
OpenFOAMFormat              // Ascii | Binary
WorkspaceLayout             // root, cases, import, reports, skills, settings, temp, logs

// config.rs
AeroflowSettings            // Aggregates all config categories
SettingsManager             // Load/save TOML, merge env vars
SolverDefaults              // Min/max iterations, write interval, relaxation factors
```

---

## P1 — Real Implementations (🔄 In Progress)

> Replace all P0 stubs with real functionality. Make the agent actually do CFD.

### ✅ `aeroflow doctor` — Real Connectivity Checks

**Goal:** Replace hardcoded health results with live checks.

| Check | Implementation |
|-------|---------------|
| Docker daemon | Real `bollard::Docker::ping()` with latency ms + version |
| DB connection | Real `sqlx::PgPool::connect()` + schema version query |
| OpenFOAM env | Checks `WM_PROJECT_VERSION`, `FOAM_INST_DIR`, executables on PATH |
| Disk space | Real `fs2::available_space()` on workspace dir |
| CPU/mem | Real `sysinfo::System::cpus()`, `total_memory()`, `used_memory()` |
| Skills DB | Real `SELECT COUNT(*)` from skills and parameter_trials tables |
| VTK/Post | Checks for `pvpython`, `vtkpython` on PATH; scans for VTU files |

**Status:** ✅ Complete — 20+ live checks across 7 categories, real data from bollard/sysinfo/fs2/sqlx.

**Files:** `crates/aeroflow-doctor/src/lib.rs` (rewritten), `crates/aeroflow-doctor/Cargo.toml` (+bollard, +sysinfo, +sqlx, +fs2, +which, +globwalk)

### ✅ `aeroflow skills` — Real Database

**Goal:** Connect to live PostgreSQL and query skill data.

- `SkillsDb::list_skills()` → real `SELECT` from skills table with fallback to demo data
- `SkillsDb::get_skill()` → real `JOIN geometries` query
- `SkillsDb::insert_skill()` → new — insert skill record
- `SkillsDb::get_trials()` → new — query top trials by reward
- `SkillsDb::insert_trial()` → new — insert trial record
- `Skills command` → connects to real DB, falls back gracefully to demo data

**Status:** ✅ Complete — SkillsDb extended with 3 new methods, skills CLI uses real DB with fallback.

**Files:** `crates/aeroflow-skills/src/db.rs` (+TrialSummary, +3 methods), `crates/aeroflow-cli/src/commands/skills.rs` (rewritten)

### Target: `aeroflow init` — STL Ingestion + DB Persistence

**Goal:** Accept real geometry files and persist to database.

- Prompt for STL file path → validate file exists
- Compute SHA-256 hash, store geometry metadata
- Insert into `geometries` table
- Create case record in `cases` table
- Write `case.json` manifest with intake config
- Save settings file in workspace

**Files:** `crates/aeroflow-cli/src/commands/init.rs`, `crates/aeroflow-skills/src/fingerprint.rs`

### Target: `aeroflow run` — Real Pipeline Execution

**Goal:** Move from `sleep(500ms)` to actual OpenFOAM toolchain calls.

- Stage 1: Import → copy STL to `constant/triSurface/`
- Stage 2: Surface prep → `surfaceFeatureExtract`
- Stage 3: Meshing → `blockMesh` + `snappyHexMesh`
- Stage 4: Mesh quality → `checkMesh` with real output parsing
- Stage 5: Setup → write `controlDict`, `fvSchemes`, `fvSolution`
- Stage 6: Solve → `simpleFoam` / `rhoCentralFoam` with real output monitoring
- Stage 7: Post → `forces` function object, `fieldMinMax`, probe data
- Stage 8: Report → generate HTML from real data

**Files:** `crates/aeroflow-pipeline/src/orchestrator.rs`, `crates/aeroflow-pipeline/src/stages/mod.rs`

### Target: Solver — Real Launch & Monitor

**Goal:** Spawn OpenFOAM processes and track progress.

- `SolverLauncher::launch()` — spawn `simpleFoam` / `rhoCentralFoam` as child process
- `SolverMonitor::poll()` — read solver log, extract residuals, detect convergence/divergence
- `SolverConfigGen` — write binary `controlDict`, `fvSchemes`, `fvSolution` to case system dir

**Files:** `crates/aeroflow-solver/src/launcher.rs`, `crates/aeroflow-solver/src/monitor.rs`

### Target: Post-Processing — Real Force Extraction

**Goal:** Read OpenFOAM results and compute aerodynamic coefficients.

- `ForceExtractor::extract()` — parse `postProcessing/forces/` output
- `FieldExtractor::read_field()` — read VTK/vtkio or OpenFOAM field files
- `PostReader::read_case()` — aggregate solutions across time steps

**Files:** `crates/aeroflow-post/src/forces.rs`, `crates/aeroflow-post/src/extract.rs`, `crates/aeroflow-post/src/reader.rs`

### Target: Mesh — Real Quality Engine

**Goal:** Parse `checkMesh` output and compute aerospace-grade quality metrics.

- `MeshQualityEngine::check()` — parse `checkMesh` stdout for:
  - Max/average non-orthogonality
  - Max skewness
  - Min determinant
  - Max aspect ratio
  - Min volume
  - Number of failed cells
- Loop: up to 3 re-mesh attempts if quality thresholds not met

**Files:** `crates/aeroflow-mesh/src/quality.rs`, `crates/aeroflow-mesh/src/generator.rs`

### P1 File Change Summary

| File | Change |
|------|--------|
| `crates/aeroflow-doctor/src/lib.rs` | Replace hardcoded checks with real bollard/sqlx/sysinfo |
| `crates/aeroflow-skills/src/db.rs` | Add insert_skill, update_trial, get_trials |
| `crates/aeroflow-cli/src/commands/init.rs` | STL ingestion, DB persist, workspace setup |
| `crates/aeroflow-cli/src/commands/skills.rs` | Real DB queries instead of hardcoded data |
| `crates/aeroflow-pipeline/src/stages/mod.rs` | Real OpenFOAM stage implementations |
| `crates/aeroflow-pipeline/src/orchestrator.rs` | Connect stages, error handling, event emission |
| `crates/aeroflow-solver/src/launcher.rs` | Real process spawning |
| `crates/aeroflow-solver/src/monitor.rs` | Real log-based progress tracking |
| `crates/aeroflow-solver/src/config_gen.rs` | Write files to case system directory |
| `crates/aeroflow-post/src/forces.rs` | Parse OpenFOAM forces output |
| `crates/aeroflow-mesh/src/quality.rs` | Real checkMesh output parsing |

---

## P2 — Pipeline Integration

> End-to-end autonomous CFD: STL in → report out, with self-healing mesh loop.

### Milestones

- [x] **Full pipeline orchestration** — run all 8 stages sequentially with state persistence
- [x] **Mesh quality auto-loop** — up to 3 remesh attempts with parameter adjustment
- [x] **Convergence detection** — real-time residual monitoring, auto-stop on convergence
- [x] **Divergence recovery** — detect solver divergence, adjust relaxation, restart
- [x] **Event bus integration** — all stages emit events, TUI listens live
- [x] **Report generation** — real data from real cases, Tera → HTML
- [x] **`aeroflow status`** — query case progress from database
- [x] **`aeroflow report`** — generate report from completed case data

### Key Architectural Goals

```text
STL ──► Import ──► Surface ──► Mesh ──► Quality? ──► Setup ──► Solve ──► Post ──► Report
                 │             │        │ (loop 3x)
                 │             │        ▼
                 │             │     Adjust params
                 │             ▼
                 │         Remesh
                 ▼
             Re-import
```

---

## P3 — Multi-Tenant SaaS

> Web API, authentication, quotas, and case isolation.

### Milestones

- [ ] **REST API** — axum server with endpoints:
  - `POST /api/auth/login` — JWT-based authentication
  - `GET /api/cases` — list user's cases
  - `POST /api/cases` — create new case
  - `GET /api/cases/{id}` — case detail + progress
  - `GET /api/skills` — list available skills
  - `GET /api/health` — system health status
- [ ] **JWT sessions** — replace simple token with JWT, refresh tokens
- [ ] **Quota enforcement** — max concurrent cases, max cores, max memory per user
- [ ] **Case isolation** — each case runs in its own container/namespace
- [ ] **User preferences** — per-user settings stored in DB `preferences` JSONB
- [ ] **Admin endpoints** — user CRUD, system stats, audit log

### Phase Scope

```text
         ┌─────────────┐
         │  Auth Proxy  │
         └──────┬──────┘
                │
    ┌───────────┼───────────┐
    │           │           │
    ▼           ▼           ▼
┌───────┐ ┌───────┐ ┌───────┐
│User A │ │User B │ │User C │
│Case 1 │ │Case 1 │ │Case 1 │
│       │ │Case 2 │ │       │
└───────┘ └───────┘ └───────┘
    │           │           │
    └───────────┼───────────┘
                ▼
        ┌───────────────┐
        │  PostgreSQL   │
        │  (multi-tenant│
        │   row-level)  │
        └───────────────┘
```

---

## P3.5 — Polish & Hardening

> Monitoring, auto-repair, and reliability.

### Milestones

- [ ] **Real Docker health checks** — bollard ping with timeout, container stats
- [ ] **`aeroflow doctor --fix`** — auto-remediate: source OpenFOAM env, prune disk, restart DB
- [ ] **File watcher** — `aeroflow watch` using notify v7, auto-import STL files
- [ ] **Continuous monitoring** — `aeroflow doctor --watch` loops every 30s
- [ ] **Resource alerts** — disk < 10%, memory > 90%, CPU saturation → event bus warning
- [ ] **Error classification** — categorize failures (transient vs permanent), auto-retry
- [ ] **Graceful shutdown** — SIGTERM handler, save pipeline state, clean up containers

---

## P4 — Advanced Skills & Optimization

> Autonomous skill learning via Gaussian Process optimization.

### Milestones

- [x] **Real STL voxelization** — `stl-io` read → 64³ voxel grid → SHA-256 hash
- [x] **Geometry fingerprinting** — multi-resolution hashing: 8³, 32³, 64³
- [x] **Flow regime key** — compute from `(Mach, Re, flow_type, compressibility)`
- [x] **Skill matching** — find best skill for `(geometry_hash, flow_regime_key)`
- [x] **Gaussian Process** — real GP regression, not stub
  - Kernel: Matern 5/2 with automatic relevance determination (ARD)
  - Acquisition function: Expected Improvement (EI)
- [x] **Trial management** — `parameter_trials` table: insert, query best, prune worst
- [x] **Autonomous optimization** — `aeroflow skills optimize` runs N trials, learns
- [x] **Reward function** — composite score from Cl error, Cd excess, y+, residuals, mesh quality
- [x] **Skill export/import** — JSON serialization of skill + GP model for sharing

### GP Optimization Loop

```text
┌──────────────────────────────────────────────────┐
│ 1. Match skill for (geometry, flow_regime)        │
│ 2. Suggest next parameters via GP acquisition fn  │
│ 3. Run CFD case with suggested parameters         │
│ 4. Compute reward (Cl, Cd, y+, res, mesh qual)    │
│ 5. Update GP model with new observation           │
│ 6. Repeat until budget exhausted or converged     │
│ 7. Update skill version with best parameters      │
└──────────────────────────────────────────────────┘
```

---

## P5 — Production Readiness

> Deploy, scale, and operate.

### Milestones

- [x] **CI/CD pipeline** — GitHub Actions: `cargo check` → test → lint → build → Docker push (multi-arch)
- [x] **Multi-arch builds** — `linux/amd64`, `linux/arm64` via QEMU + Docker Buildx
- [x] **Prometheus metrics** — 14 metrics: case throughput, queue depth, solver iterations, mesh failures, HTTP request count/duration, DB query duration, pipeline duration
- [x] **Grafana dashboards** — system health, case progress, skill improvement trends
- [x] **Database backups** — automated pg_dump to S3/MinIO, point-in-time recovery
- [x] **Log aggregation** — structured JSON logs (`--json-logs` flag), Loki integration
- [x] **Horizontal scaling** — multiple agent instances, shared DB, work queue
- [x] **Documentation** — CLI reference, API docs, architecture guide, deployment guide
- [x] **Helm chart** — `helm/aeroflow/` with Deployment, Service, Ingress, ConfigMap, Secrets, PVC, HPA, ServiceMonitor
- [x] **Auto-scaling** — HPA based on CPU/memory utilization
- [x] **Health probes** — liveness + readiness on `/api/health`
- [x] **ServiceMonitor** — Prometheus Operator integration for auto-scrape
- [x] **Structured JSON logging** — `aeroflow --json-logs` outputs structured JSON logs
- [x] **JWT secret management** — K8s Secret for JWT signing key
- [x] **PostgreSQL sidecar** — optional bundled PostgreSQL 16 Alpine in Helm chart

### P5 File Change Summary

| File | Change |
|------|--------|
| `crates/aeroflow-core/src/metrics.rs` | New — Prometheus metrics: 14 counters/gauges/histograms, gather_metrics() |
| `crates/aeroflow-api/src/server.rs` | New `/metrics` endpoint, metrics module import |
| `crates/aeroflow-cli/src/main.rs` | New `--json-logs` flag, structured JSON logging |
| `Cargo.toml` | Added `prometheus`, `lazy_static` deps; `json` feature for tracing-subscriber |
| `crates/aeroflow-core/Cargo.toml` | Added `prometheus`, `lazy_static` deps |
| `crates/aeroflow-api/Cargo.toml` | Added `prometheus` dep |
| `.github/workflows/ci.yml` | New — 8-job CI/CD pipeline |
| `helm/aeroflow/Chart.yaml` | New — Helm chart definition |
| `helm/aeroflow/values.yaml` | New — configurable values (40+ settings) |
| `helm/aeroflow/templates/*` | New — 9 K8s resource templates |

---

## P6 — Visualization & Report Enhancement (✅ Complete)

> foamToVTK export → Python VTK+matplotlib → images embedded in Tera HTML report.

### Milestones

- [x] **foamToVTK export** — pipeline exports VTK at `latestTime` with `(p, U)` fields
- [x] **Python visualization script** — `scripts/viz/generate_viz.py` generates 3 image types:
  - `pressure_surface.png` — pressure contour on blade surface (VTK `.vtp` → matplotlib)
  - `velocity_slice.png` — velocity magnitude at mid-plane slice (VTK `.vtu` → matplotlib)
  - `convergence.png` — Cd/Cl convergence history from `forceCoeffs` log
- [x] **Report embedding** — images stored in `report/images/`, injected via updated Tera template
- [x] **Pipeline integration** — `Stage::Visualization` runs after post-processing
- [x] **Automatic fallback** — pipeline succeeds even if viz generation fails

### Key Design Decisions

| Decision | Rationale |
|----------|-----------|
| Python for viz (not Rust) | matplotlib + VTK Python bindings are mature; Rust VTK bindings (`vtkio`) are limited |
| foamToVTK export | OpenFOAM's built-in VTK exporter handles polyhedral/hex decomposition |
| Images embedded in report | Self-contained HTML, no external viewer needed; one `report/` directory per case |
| 3 standard views | Surface pressure (aerodynamic load), velocity slice (flow field), convergence (solver health) |

### Visualization Pipeline

```text
Post-processing (forces)
  → foamToVTK -latestTime -fields '(p U)'
  → generate_viz.py reads VTP/VTU
    ├── pressure_surface.png   (blade surface pressure contour)
    ├── velocity_slice.png     (mid-plane velocity magnitude)
    └── convergence.png        (Cd/Cl history from log)
  → report.html.tera embeds images in "Visualization" section
  → report/index.html with all 3 images
```

---

## Key Technical Decisions

| Decision | Rationale |
|----------|-----------|
| Rust binary + OpenFOAM in single Docker image | Portability, reproducible CFD env, avoids host install |
| CLI tools first, FFI via `cxx` later | Zero initial complexity, OpenFOAM CLI is mature |
| PostgreSQL for all persistence | SaaS-ready from day one, JSONB for flexible config |
| `vtkio` (pure Rust) for post first | Avoids C++ VTK dependency at compile time |
| Binary OpenFOAM format by default | 60-80% disk savings on large cases |
| STL voxel signature (64³) for geometry fingerprint | Enables shape-based skill matching without mesh dependency |
| GP per (geometry_hash, flow_regime_key) | Skills are specific to both shape AND flow conditions |
| Reward: weighted Cl + Cd + y+ + residual + mesh | Balances accuracy, stability, and mesh quality |
| Aerospace mesh thresholds | nonOrtho ≤60° warn, ≤70° fail; skewness ≤2 warn, ≤4 fail |

---

## Dependency Map

```text
aeroflow-cli
├── aeroflow-core          (types, config, events)
├── aeroflow-pipeline      (orchestration)
│   ├── aeroflow-mesh      (mesh gen + quality)
│   ├── aeroflow-solver    (solver config + launch)
│   └── aeroflow-post      (force extraction)
├── aeroflow-docker        (container management)
├── aeroflow-doctor        (health checks)
├── aeroflow-skills        (DB + fingerprint)
│   └── aeroflow-core
├── aeroflow-report        (Tera templates)
├── aeroflow-monitor       (sysinfo)
├── aeroflow-learner       (GP optimization)
└── aeroflow-events        (file watcher + web)
```

---

## Version History

| Version | Date | Changes |
|---------|------|---------|
| v0.1.0 | 2026-05-20 | P0 scaffold + P0.5 settings/users/workspace/binary |
| v0.1.1 | 2026-05-20 | P1 real impl — doctor, skills, init, pipeline, solver, post, mesh |
| v0.2.0 | 2026-05-20 | P2 pipeline + P3 API — end-to-end CFD, REST API, JWT auth, file watcher |
| v0.3.0 | 2026-05-20 | P4 skills — real STL voxelization, Gaussian Process, Bayesian optimizer |
| v0.4.0 | 2026-05-20 | P5 production — CI/CD, Prometheus metrics, Helm chart, structured logging |
| v0.5.0 | 2026-05-22 | P6 visualization — foamToVTK export, Python matplotlib, 3 image types, report embedding |

---

*This roadmap is a living document. Update status markers (`[x]` / `[ ]`) as phases are completed.*
