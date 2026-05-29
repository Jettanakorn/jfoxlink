# AeroFlow Agent â€” Phase Roadmap

> Developer: **Jettanakorn Pengsiri** by JFOX Aircraft Co., Ltd.
> Rust nightly | Edition 2024 | 15-crate workspace + Python viz | OpenFOAM + ParaView + VTK | PostgreSQL + Prometheus

---

## Big Picture â€” Phase Overview

| Phase | Title | What It Delivers |
|-------|-------|------------------|
| **P0** | ðŸ— Scaffold | Workspace skeleton, core types, CLI skeleton, Docker setup, PostgreSQL schema |
| **P0.5** | âš™ï¸ Foundation | Settings system, user management, workspace manager, OpenFOAM binary format |
| **P1** | ðŸ”§ Real Impl | Replace all stubs: real Docker/DB disk checks, STL ingestion, OpenFOAM execution |
| **P2** | ðŸ”— Pipeline | End-to-end autonomous CFD: STL in â†’ report out, self-healing mesh loop |
| **P3** | ðŸŒ SaaS | Multi-tenant REST API, JWT auth, per-user quotas, case isolation |
| **P3.5** | ðŸ§¹ Polish | File watcher, auto-fix doctor, continuous monitoring, graceful shutdown |
| **P4** | ðŸ§  Skills | Real STL voxelization, Gaussian Process optimization, autonomous skill learning |
| **P5** | ðŸš€ Production | CI/CD, Prometheus metrics, K8s Helm chart, structured logging, docs |
| **P6** | ðŸ“Š Visualization | foamToVTK export + Python matplotlib plots (pressure/velocity/convergence) embedded in report |
| **P7** | **🧹 Code Quality** | Digital Wind Tunnel (DWT), zero clippy warnings, 77 new tests, code hardening, Dockerfile fixes |

---

## Phase Summary

| Phase | Focus | Status |
|-------|-------|--------|
| **P0** | Scaffold â€” workspace, core types, CLI, Docker, DB schema | âœ… Complete |
| **P0.5** | Settings, Users, Workspace, Binary Format | âœ… Complete |
| **P1** | Real Implementations â€” replace all stubs | âœ… Complete |
| **P2** | Pipeline Integration â€” end-to-end autonomous execution | âœ… Complete |
| **P3** | Multi-Tenant SaaS â€” auth, web API, quotas, isolation | âœ… Complete |
| **P3.5** | Polish â€” Docker health, file watcher, monitoring, alerts | âœ… Complete |
| **P4** | Advanced Skills â€” real STL voxelization, GP optimization | âœ… Complete |
| **P5** | Production â€” CI/CD, metrics, structured logging, Helm chart, scaling, docs | âœ… Complete |
| **P6** | Visualization â€” foamToVTK + Python matplotlib, report embedding | âœ… Complete |
| **P7** | Code Quality â€” DWT, zero warnings, tests, hardening, Dockerfile | âœ… Complete |

---

## P0 â€” Scaffold (âœ… Complete)

> Build the full project skeleton: workspace, crates, types, CLI, Docker, DB.

### Deliverables

- [x] **Workspace root** `Cargo.toml` â€” 14 crate members, all deps
- [x] **aeroflow-core** â€” types, config, errors, event bus
- [x] **aeroflow-cli** â€” clap CLI with 11 subcommands, ratatui TUI (3 tabs)
- [x] **aeroflow-pipeline** â€” PipelineOrchestrator state machine (8 phases + mesh quality loop)
- [x] **aeroflow-docker** â€” DockerClient, ContainerManager, StatsCollector stubs
- [x] **aeroflow-doctor** â€” 7-category health check runner (26 checks)
- [x] **aeroflow-skills** â€” SkillsDb (PostgreSQL), GeometryFingerprint, SkillMatcher
- [x] **aeroflow-mesh** â€” MeshGenerator (blockMeshDict/snappyHexMeshDict), MeshQualityEngine
- [x] **aeroflow-solver** â€” SolverConfigGen (solver/turbulence selection, controlDict)
- [x] **aeroflow-post** â€” ForceExtractor, PostReader, FieldExtractor stubs
- [x] **aeroflow-report** â€” ReportGenerator (Tera â†’ HTML)
- [x] **aeroflow-monitor** â€” ResourceMonitor (sysinfo)
- [x] **aeroflow-learner** â€” RewardFunction, Optimizer, GaussianProcess stubs
- [x] **aeroflow-events** â€” FileWatcher (notify), WebApi stub
- [x] **Dockerfile** â€” Multi-stage: Rust builder + OpenFOAM runtime
- [x] **docker-compose.yml** â€” agent + PostgreSQL 16 Alpine with health check
- [x] **DB migrations** â€” `001_initial_schema.sql` (7 tables, indexes)
- [x] **Templates** â€” `report.html.tera` (dark-themed HTML report)
- [x] **Skills** â€” `openfoam-aerospace` skill (8 files, aerospace CFD expertise)

### Files Created

```text
Cargo.toml                          â€” Workspace root (13 crate members)
crates/aeroflow-core/src/
  types.rs                          â€” All domain types (Stage, CaseMeta, IntakeConfig, etc.)
  config.rs                         â€” AeroFlowConfig, MeshQualityThresholds, ScoringWeights
  errors.rs                         â€” AeroFlowError enum
  events.rs                         â€” SystemEvent, EventBus
  lib.rs                            â€” Module exports
crates/aeroflow-cli/src/
  main.rs                           â€” CLI entry, 9 subcommands + Skills/User/Settings actions
  commands/init.rs                  â€” Guided Q&A case creation
  commands/run.rs                   â€” Pipeline execution stub
  commands/status.rs                â€” Case status listing
  commands/report.rs                â€” HTML report generation
  commands/watch.rs                 â€” STL directory watcher
  commands/serve.rs                 â€” Web dashboard stub
  commands/doctor.rs                â€” Health check runner
  commands/skills.rs                â€” Skills DB management
  commands/user.rs                  â€” User CRUD + auth
  commands/settings.rs              â€” Settings manager CLI
  tui/dashboard.rs                  â€” ratatui TUI (5 tabs)
crates/aeroflow-pipeline/src/orchestrator.rs
crates/aeroflow-docker/src/        â€” client.rs, container.rs, stats.rs
crates/aeroflow-doctor/src/        â€” lib.rs, checks/mod.rs
crates/aeroflow-skills/src/        â€” db.rs, fingerprint.rs, matcher.rs, user_manager.rs
crates/aeroflow-mesh/src/          â€” generator.rs, quality.rs
crates/aeroflow-solver/src/        â€” config_gen.rs, launcher.rs, monitor.rs
crates/aeroflow-post/src/          â€” forces.rs, extract.rs, reader.rs
crates/aeroflow-report/src/lib.rs  â€” Tera HTML report gen
crates/aeroflow-monitor/src/lib.rs â€” sysinfo resource monitor
crates/aeroflow-learner/src/       â€” reward.rs, optimizer.rs, gp.rs
crates/aeroflow-events/src/        â€” file_watcher.rs, api.rs
docker/Dockerfile                  â€” Multi-stage build
docker/docker-compose.yml          â€” Stack definition
db/migrations/001_initial_schema.sql
templates/report.html.tera
```

---

## P0.5 â€” Settings, Users, Workspace, Binary Format (âœ… Complete)

> User management, centralized settings, configurable workspace, and OpenFOAM binary saving.

### Deliverables

- [x] **Workspace Manager** â€” `WorkspaceManager` + `WorkspaceLayout` in aeroflow-core
  - Directory tree: `cases/`, `import/`, `reports/`, `skills/`, `settings/`, `temp/`, `logs/`
  - Disk space validation via `fs2`
  - `aeroflow settings init /path` CLI command
- [x] **User Management** â€” `UserManager` in aeroflow-skills
  - CRUD: `create_user`, `get_user`, `list_users`, `update_user`, `delete_user`
  - Auth: `authenticate`, `create_session` (SHA-256 hashing)
  - Roles: `admin`, `engineer`, `viewer`
  - Table migration: `password_hash`, `role`, `active`, `last_login` columns
  - CLI: `aeroflow user create|list|show|update|delete|login`
- [x] **Centralized Settings** â€” `AeroflowSettings` + `SettingsManager`
  - Load/save TOML config file
  - Merge: defaults â†’ TOML â†’ env vars (`AEROFLOW_*`)
  - CLI: `aeroflow settings show|set key=value|init|reset|path`
- [x] **Binary OpenFOAM Format** â€” `OpenFOAMFormat` enum (default: Binary)
  - `controlDict` generation uses `format binary`
  - `blockMeshDict` / `snappyHexMeshDict` use `format binary`
  - Saves 60-80% disk space on large cases
- [x] **TUI updated** â€” Users tab + Settings tab (5 tabs total)

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

## P1 â€” Real Implementations (ðŸ”„ In Progress)

> Replace all P0 stubs with real functionality. Make the agent actually do CFD.

### âœ… `aeroflow doctor` â€” Real Connectivity Checks

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

**Status:** âœ… Complete â€” 20+ live checks across 7 categories, real data from bollard/sysinfo/fs2/sqlx.

**Files:** `crates/aeroflow-doctor/src/lib.rs` (rewritten), `crates/aeroflow-doctor/Cargo.toml` (+bollard, +sysinfo, +sqlx, +fs2, +which, +globwalk)

### âœ… `aeroflow skills` â€” Real Database

**Goal:** Connect to live PostgreSQL and query skill data.

- `SkillsDb::list_skills()` â†’ real `SELECT` from skills table with fallback to demo data
- `SkillsDb::get_skill()` â†’ real `JOIN geometries` query
- `SkillsDb::insert_skill()` â†’ new â€” insert skill record
- `SkillsDb::get_trials()` â†’ new â€” query top trials by reward
- `SkillsDb::insert_trial()` â†’ new â€” insert trial record
- `Skills command` â†’ connects to real DB, falls back gracefully to demo data

**Status:** âœ… Complete â€” SkillsDb extended with 3 new methods, skills CLI uses real DB with fallback.

**Files:** `crates/aeroflow-skills/src/db.rs` (+TrialSummary, +3 methods), `crates/aeroflow-cli/src/commands/skills.rs` (rewritten)

### Target: `aeroflow init` â€” STL Ingestion + DB Persistence

**Goal:** Accept real geometry files and persist to database.

- Prompt for STL file path â†’ validate file exists
- Compute SHA-256 hash, store geometry metadata
- Insert into `geometries` table
- Create case record in `cases` table
- Write `case.json` manifest with intake config
- Save settings file in workspace

**Files:** `crates/aeroflow-cli/src/commands/init.rs`, `crates/aeroflow-skills/src/fingerprint.rs`

### Target: `aeroflow run` â€” Real Pipeline Execution

**Goal:** Move from `sleep(500ms)` to actual OpenFOAM toolchain calls.

- Stage 1: Import â†’ copy STL to `constant/triSurface/`
- Stage 2: Surface prep â†’ `surfaceFeatureExtract`
- Stage 3: Meshing â†’ `blockMesh` + `snappyHexMesh`
- Stage 4: Mesh quality â†’ `checkMesh` with real output parsing
- Stage 5: Setup â†’ write `controlDict`, `fvSchemes`, `fvSolution`
- Stage 6: Solve â†’ `simpleFoam` / `rhoCentralFoam` with real output monitoring
- Stage 7: Post â†’ `forces` function object, `fieldMinMax`, probe data
- Stage 8: Report â†’ generate HTML from real data

**Files:** `crates/aeroflow-pipeline/src/orchestrator.rs`, `crates/aeroflow-pipeline/src/stages/mod.rs`

### Target: Solver â€” Real Launch & Monitor

**Goal:** Spawn OpenFOAM processes and track progress.

- `SolverLauncher::launch()` â€” spawn `simpleFoam` / `rhoCentralFoam` as child process
- `SolverMonitor::poll()` â€” read solver log, extract residuals, detect convergence/divergence
- `SolverConfigGen` â€” write binary `controlDict`, `fvSchemes`, `fvSolution` to case system dir

**Files:** `crates/aeroflow-solver/src/launcher.rs`, `crates/aeroflow-solver/src/monitor.rs`

### Target: Post-Processing â€” Real Force Extraction

**Goal:** Read OpenFOAM results and compute aerodynamic coefficients.

- `ForceExtractor::extract()` â€” parse `postProcessing/forces/` output
- `FieldExtractor::read_field()` â€” read VTK/vtkio or OpenFOAM field files
- `PostReader::read_case()` â€” aggregate solutions across time steps

**Files:** `crates/aeroflow-post/src/forces.rs`, `crates/aeroflow-post/src/extract.rs`, `crates/aeroflow-post/src/reader.rs`

### Target: Mesh â€” Real Quality Engine

**Goal:** Parse `checkMesh` output and compute aerospace-grade quality metrics.

- `MeshQualityEngine::check()` â€” parse `checkMesh` stdout for:
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

## P2 â€” Pipeline Integration

> End-to-end autonomous CFD: STL in â†’ report out, with self-healing mesh loop.

### Milestones

- [x] **Full pipeline orchestration** â€” run all 8 stages sequentially with state persistence
- [x] **Mesh quality auto-loop** â€” up to 3 remesh attempts with parameter adjustment
- [x] **Convergence detection** â€” real-time residual monitoring, auto-stop on convergence
- [x] **Divergence recovery** â€” detect solver divergence, adjust relaxation, restart
- [x] **Event bus integration** â€” all stages emit events, TUI listens live
- [x] **Report generation** â€” real data from real cases, Tera â†’ HTML
- [x] **`aeroflow status`** â€” query case progress from database
- [x] **`aeroflow report`** â€” generate report from completed case data

### Key Architectural Goals

```text
STL â”€â”€â–º Import â”€â”€â–º Surface â”€â”€â–º Mesh â”€â”€â–º Quality? â”€â”€â–º Setup â”€â”€â–º Solve â”€â”€â–º Post â”€â”€â–º Report
                 â”‚             â”‚        â”‚ (loop 3x)
                 â”‚             â”‚        â–¼
                 â”‚             â”‚     Adjust params
                 â”‚             â–¼
                 â”‚         Remesh
                 â–¼
             Re-import
```

---

## P3 â€” Multi-Tenant SaaS

> Web API, authentication, quotas, and case isolation.

### Milestones

- [ ] **REST API** â€” axum server with endpoints:
  - `POST /api/auth/login` â€” JWT-based authentication
  - `GET /api/cases` â€” list user's cases
  - `POST /api/cases` â€” create new case
  - `GET /api/cases/{id}` â€” case detail + progress
  - `GET /api/skills` â€” list available skills
  - `GET /api/health` â€” system health status
- [ ] **JWT sessions** â€” replace simple token with JWT, refresh tokens
- [ ] **Quota enforcement** â€” max concurrent cases, max cores, max memory per user
- [ ] **Case isolation** â€” each case runs in its own container/namespace
- [ ] **User preferences** â€” per-user settings stored in DB `preferences` JSONB
- [ ] **Admin endpoints** â€” user CRUD, system stats, audit log

### Phase Scope

```text
         â”Œâ”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”
         â”‚  Auth Proxy  â”‚
         â””â”€â”€â”€â”€â”€â”€â”¬â”€â”€â”€â”€â”€â”€â”˜
                â”‚
    â”Œâ”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”¼â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”
    â”‚           â”‚           â”‚
    â–¼           â–¼           â–¼
â”Œâ”€â”€â”€â”€â”€â”€â”€â” â”Œâ”€â”€â”€â”€â”€â”€â”€â” â”Œâ”€â”€â”€â”€â”€â”€â”€â”
â”‚User A â”‚ â”‚User B â”‚ â”‚User C â”‚
â”‚Case 1 â”‚ â”‚Case 1 â”‚ â”‚Case 1 â”‚
â”‚       â”‚ â”‚Case 2 â”‚ â”‚       â”‚
â””â”€â”€â”€â”€â”€â”€â”€â”˜ â””â”€â”€â”€â”€â”€â”€â”€â”˜ â””â”€â”€â”€â”€â”€â”€â”€â”˜
    â”‚           â”‚           â”‚
    â””â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”¼â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”˜
                â–¼
        â”Œâ”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”
        â”‚  PostgreSQL   â”‚
        â”‚  (multi-tenantâ”‚
        â”‚   row-level)  â”‚
        â””â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”˜
```

---

## P3.5 â€” Polish & Hardening

> Monitoring, auto-repair, and reliability.

### Milestones

- [ ] **Real Docker health checks** â€” bollard ping with timeout, container stats
- [ ] **`aeroflow doctor --fix`** â€” auto-remediate: source OpenFOAM env, prune disk, restart DB
- [ ] **File watcher** â€” `aeroflow watch` using notify v7, auto-import STL files
- [ ] **Continuous monitoring** â€” `aeroflow doctor --watch` loops every 30s
- [ ] **Resource alerts** â€” disk < 10%, memory > 90%, CPU saturation â†’ event bus warning
- [ ] **Error classification** â€” categorize failures (transient vs permanent), auto-retry
- [ ] **Graceful shutdown** â€” SIGTERM handler, save pipeline state, clean up containers

---

## P4 â€” Advanced Skills & Optimization

> Autonomous skill learning via Gaussian Process optimization.

### Milestones

- [x] **Real STL voxelization** â€” `stl-io` read â†’ 64Â³ voxel grid â†’ SHA-256 hash
- [x] **Geometry fingerprinting** â€” multi-resolution hashing: 8Â³, 32Â³, 64Â³
- [x] **Flow regime key** â€” compute from `(Mach, Re, flow_type, compressibility)`
- [x] **Skill matching** â€” find best skill for `(geometry_hash, flow_regime_key)`
- [x] **Gaussian Process** â€” real GP regression, not stub
  - Kernel: Matern 5/2 with automatic relevance determination (ARD)
  - Acquisition function: Expected Improvement (EI)
- [x] **Trial management** â€” `parameter_trials` table: insert, query best, prune worst
- [x] **Autonomous optimization** â€” `aeroflow skills optimize` runs N trials, learns
- [x] **Reward function** â€” composite score from Cl error, Cd excess, y+, residuals, mesh quality
- [x] **Skill export/import** â€” JSON serialization of skill + GP model for sharing

### GP Optimization Loop

```text
â”Œâ”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”
â”‚ 1. Match skill for (geometry, flow_regime)        â”‚
â”‚ 2. Suggest next parameters via GP acquisition fn  â”‚
â”‚ 3. Run CFD case with suggested parameters         â”‚
â”‚ 4. Compute reward (Cl, Cd, y+, res, mesh qual)    â”‚
â”‚ 5. Update GP model with new observation           â”‚
â”‚ 6. Repeat until budget exhausted or converged     â”‚
â”‚ 7. Update skill version with best parameters      â”‚
â””â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”˜
```

---

## P5 â€” Production Readiness

> Deploy, scale, and operate.

### Milestones

- [x] **CI/CD pipeline** â€” GitHub Actions: `cargo check` â†’ test â†’ lint â†’ build â†’ Docker push (multi-arch)
- [x] **Multi-arch builds** â€” `linux/amd64`, `linux/arm64` via QEMU + Docker Buildx
- [x] **Prometheus metrics** â€” 14 metrics: case throughput, queue depth, solver iterations, mesh failures, HTTP request count/duration, DB query duration, pipeline duration
- [x] **Grafana dashboards** â€” system health, case progress, skill improvement trends
- [x] **Database backups** â€” automated pg_dump to S3/MinIO, point-in-time recovery
- [x] **Log aggregation** â€” structured JSON logs (`--json-logs` flag), Loki integration
- [x] **Horizontal scaling** â€” multiple agent instances, shared DB, work queue
- [x] **Documentation** â€” CLI reference, API docs, architecture guide, deployment guide
- [x] **Helm chart** â€” `helm/aeroflow/` with Deployment, Service, Ingress, ConfigMap, Secrets, PVC, HPA, ServiceMonitor
- [x] **Auto-scaling** â€” HPA based on CPU/memory utilization
- [x] **Health probes** â€” liveness + readiness on `/api/health`
- [x] **ServiceMonitor** â€” Prometheus Operator integration for auto-scrape
- [x] **Structured JSON logging** â€” `aeroflow --json-logs` outputs structured JSON logs
- [x] **JWT secret management** â€” K8s Secret for JWT signing key
- [x] **PostgreSQL sidecar** â€” optional bundled PostgreSQL 16 Alpine in Helm chart

### P5 File Change Summary

| File | Change |
|------|--------|
| `crates/aeroflow-core/src/metrics.rs` | New â€” Prometheus metrics: 14 counters/gauges/histograms, gather_metrics() |
| `crates/aeroflow-api/src/server.rs` | New `/metrics` endpoint, metrics module import |
| `crates/aeroflow-cli/src/main.rs` | New `--json-logs` flag, structured JSON logging |
| `Cargo.toml` | Added `prometheus`, `lazy_static` deps; `json` feature for tracing-subscriber |
| `crates/aeroflow-core/Cargo.toml` | Added `prometheus`, `lazy_static` deps |
| `crates/aeroflow-api/Cargo.toml` | Added `prometheus` dep |
| `.github/workflows/ci.yml` | New â€” 8-job CI/CD pipeline |
| `helm/aeroflow/Chart.yaml` | New â€” Helm chart definition |
| `helm/aeroflow/values.yaml` | New â€” configurable values (40+ settings) |
| `helm/aeroflow/templates/*` | New â€” 9 K8s resource templates |

---

## P6 â€” Visualization & Report Enhancement (âœ… Complete)

> foamToVTK export â†’ Python VTK+matplotlib â†’ images embedded in Tera HTML report.

### Milestones

- [x] **foamToVTK export** â€” pipeline exports VTK at `latestTime` with `(p, U)` fields
- [x] **Python visualization script** â€” `scripts/viz/generate_viz.py` generates 3 image types:
  - `pressure_surface.png` â€” pressure contour on blade surface (VTK `.vtp` â†’ matplotlib)
  - `velocity_slice.png` â€” velocity magnitude at mid-plane slice (VTK `.vtu` â†’ matplotlib)
  - `convergence.png` â€” Cd/Cl convergence history from `forceCoeffs` log
- [x] **Report embedding** â€” images stored in `report/images/`, injected via updated Tera template
- [x] **Pipeline integration** â€” `Stage::Visualization` runs after post-processing
- [x] **Automatic fallback** â€” pipeline succeeds even if viz generation fails

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
  â†’ foamToVTK -latestTime -fields '(p U)'
  â†’ generate_viz.py reads VTP/VTU
    â”œâ”€â”€ pressure_surface.png   (blade surface pressure contour)
    â”œâ”€â”€ velocity_slice.png     (mid-plane velocity magnitude)
    â””â”€â”€ convergence.png        (Cd/Cl history from log)
  â†’ report.html.tera embeds images in "Visualization" section
  â†’ report/index.html with all 3 images
```

---

## P7 — Code Quality & Hardening (✅ Complete)

> Zero warnings, zero clippy violations, 77 new tests, Digital Wind Tunnel (DWT) feature, and production Dockerfile fixes.

### Milestones

- [x] **Digital Wind Tunnel (DWT)** — full feature across 14+ files:
  - `WindTunnelDomainSizer` — chord detection, domain math, blockage correction
  - `WindTunnelBlockMesh` — asymmetric 2-block grading (upstream + downstream)
  - `WindTunnelBcGenerator` — freestreamVelocity/freestreamPressure BCs
  - `WindTunnelExtractor` — blockage-corrected force extraction
  - Agent integration: `propose_config` → `run_simulation` → `evaluate_results` → `diagnose_and_fix` → `compare_iterations`
  - CLI prompts (`init`/`run`), config persistence, DWT smoke test
- [x] **Zero clippy warnings** — 179 eliminated (149 auto-fix, 30 manual): redundant closures, `&PathBuf` → `&Path`, `if_same_then_else`, `manual_checked_ops`, `ptr_arg`, `needless_range_loop`
- [x] **Code hardening** — `partial_cmp` → `total_cmp`, `mutex.unwrap()` → `.expect("poisoned")`, anyhow context added, serde infallible unwraps documented
- [x] **77 new tests** — across 4 previously untested crates (+22% increase):
  - `aeroflow-llm`: 28 tests (types JSON roundtrip, prompt building, tool defs, executor state)
  - `aeroflow-api`: 16 tests (JWT roundtrip + tamper, admin auth, file parsing)
  - `aeroflow-cli`: 17 tests (settings path, frontend path, force parsing, flow type detection)
  - `aeroflow-skills`: 16 tests (flow regime keys, password hash, fingerprint Hamming, pack_bits, ray-triangle)
- [x] **Dead code cleanup** — removed unused `reward_fn` field from `Optimizer`, documented `StatsCollector`/`ContainerManager`/`point_inside_mesh` as P2 stubs
- [x] **Dockerfile fixes** — removed `|| true` from build step (was silently swallowing failures), removed duplicate `COPY scripts/viz`
- [x] **Configuration bug fix** — `config_gen.rs:171-172` identical `if/else` branches in `dragDir` generation (rotation axis unit vector)

### P7 File Change Summary

| File | Change |
|------|--------|
| `crates/aeroflow-core/src/domain.rs` | New `WindTunnelDomainSizer` |
| `crates/aeroflow-core/src/types.rs` | New `WindTunnelConfig`, `WindTunnelResult`, `GeoBounds`, `CaseConfig`, `InletWallConfig` |
| `crates/aeroflow-mesh/src/wind_tunnel.rs` | New `WindTunnelBlockMesh` |
| `crates/aeroflow-solver/src/boundary_conditions.rs` | New `WindTunnelBcGenerator` |
| `crates/aeroflow-solver/src/config_gen.rs` | Fix `dragDir` rotation axis (lines 171-172) |
| `crates/aeroflow-post/src/physics.rs` | New `WindTunnelExtractor` |
| `crates/aeroflow-pipeline/src/orchestrator.rs` | DWT mesh/BC gen pipeline integration |
| `crates/aeroflow-llm/src/tools.rs` | `propose_config`, `run_simulation`, `evaluate_results` DWT handlers |
| `crates/aeroflow-llm/src/prompts.rs` | DWT expertise in agent system prompt |
| `crates/aeroflow-cli/src/commands/init.rs` | Wind tunnel init prompts |
| `crates/aeroflow-cli/src/commands/run.rs` | DWT config + `CaseConfig` builder |
| `crates/aeroflow-cli/src/commands/report.rs` | `parse_force_coefficients` (extracted for testing) |
| `crates/aeroflow-pipeline/tests/dwt_smoke_test.rs` | Docker smoke test (gated) |
| `docker/Dockerfile` | Fix: remove `|| true`, remove duplicate COPY |

---

## Key Technical Decisions

| Decision | Rationale |
|----------|-----------|
| Rust binary + OpenFOAM in single Docker image | Portability, reproducible CFD env, avoids host install |
| CLI tools first, FFI via `cxx` later | Zero initial complexity, OpenFOAM CLI is mature |
| PostgreSQL for all persistence | SaaS-ready from day one, JSONB for flexible config |
| `vtkio` (pure Rust) for post first | Avoids C++ VTK dependency at compile time |
| Binary OpenFOAM format by default | 60-80% disk savings on large cases |
| STL voxel signature (64Â³) for geometry fingerprint | Enables shape-based skill matching without mesh dependency |
| GP per (geometry_hash, flow_regime_key) | Skills are specific to both shape AND flow conditions |
| Reward: weighted Cl + Cd + y+ + residual + mesh | Balances accuracy, stability, and mesh quality |
| Aerospace mesh thresholds | nonOrtho â‰¤60Â° warn, â‰¤70Â° fail; skewness â‰¤2 warn, â‰¤4 fail |

---

## Dependency Map

```text
aeroflow-cli
â”œâ”€â”€ aeroflow-core          (types, config, events)
â”œâ”€â”€ aeroflow-pipeline      (orchestration)
â”‚   â”œâ”€â”€ aeroflow-mesh      (mesh gen + quality)
â”‚   â”œâ”€â”€ aeroflow-solver    (solver config + launch)
â”‚   â””â”€â”€ aeroflow-post      (force extraction)
â”œâ”€â”€ aeroflow-docker        (container management)
â”œâ”€â”€ aeroflow-doctor        (health checks)
â”œâ”€â”€ aeroflow-skills        (DB + fingerprint)
â”‚   â””â”€â”€ aeroflow-core
â”œâ”€â”€ aeroflow-report        (Tera templates)
â”œâ”€â”€ aeroflow-monitor       (sysinfo)
â”œâ”€â”€ aeroflow-learner       (GP optimization)
â””â”€â”€ aeroflow-events        (file watcher + web)
```

---

## Version History

| Version | Date | Changes |
|---------|------|---------|
| v0.1.0 | 2026-05-20 | P0 scaffold + P0.5 settings/users/workspace/binary |
| v0.1.1 | 2026-05-20 | P1 real impl â€” doctor, skills, init, pipeline, solver, post, mesh |
| v0.2.0 | 2026-05-20 | P2 pipeline + P3 API â€” end-to-end CFD, REST API, JWT auth, file watcher |
| v0.3.0 | 2026-05-20 | P4 skills â€” real STL voxelization, Gaussian Process, Bayesian optimizer |
| v0.4.0 | 2026-05-20 | P5 production â€” CI/CD, Prometheus metrics, Helm chart, structured logging |
| v0.5.0 | 2026-05-22 | P6 visualization â€” foamToVTK export, Python matplotlib, 3 image types, report embedding |
| **v0.6.0** | **2026-05-29** | **P7 quality â€” DWT, zero warnings, 77 new tests, code hardening** |

---

*This roadmap is a living document. Update status markers (`[x]` / `[ ]`) as phases are completed.*
