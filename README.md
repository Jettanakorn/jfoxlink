<p align="center">
  <h1 align="center">AeroFlow Agent</h1>
  <p align="center"><i>Autonomous CFD Analysis Orchestrator</i></p>
  <p align="center">
    <a href="https://github.com/Jettanakorn/aeroflow/actions"><img src="https://github.com/Jettanakorn/aeroflow/actions/workflows/ci.yml/badge.svg" alt="CI"></a>
    <a href="https://github.com/Jettanakorn/aeroflow/releases"><img src="https://img.shields.io/github/v/release/Jettanakorn/aeroflow" alt="Release"></a>
    <a href="https://github.com/Jettanakorn/aeroflow/blob/main/LICENSE"><img src="https://img.shields.io/github/license/Jettanakorn/aeroflow" alt="License"></a>
  </p>
</p>

**AeroFlow Agent** is a Rust-based autonomous CFD analysis system that takes an STL geometry
and produces a complete aerodynamic report — mesh quality, convergence history, and force
coefficients (Cl/Cd/Cm) — without manual intervention.

Built on OpenFOAM, PostgreSQL, and Gaussian Process optimization.
Deployable via Docker Compose or Kubernetes Helm chart.

---

## Features

- **End-to-end automation** — STL voxel fingerprint → 8-stage pipeline → HTML report
- **Self-healing mesh** — adaptive snappyHexMesh with up to 3 retries and relaxed parameters
- **Autonomous skill learning** — Gaussian Process optimizes solver parameters per (geometry, flow regime)
- **Real-time monitoring** — TUI dashboard, SSE event stream, Prometheus metrics
- **SaaS-ready** — JWT auth, multi-tenant REST API, file watcher auto-import
- **Production-grade** — Helm chart, HPA, multi-arch Docker images, structured JSON logging

---

## Quick Start

```bash
# Clone
git clone https://github.com/Jettanakorn/aeroflow.git
cd aeroflow

# Start stack (PostgreSQL + agent)
docker compose -f docker/docker-compose.yml up -d

# Create a case from STL
docker compose run agent aeroflow init my-wing
# (enter STL path when prompted)

# Run the CFD pipeline
docker compose run agent aeroflow run /data/cases/my-wing

# Generate report
docker compose run agent aeroflow report my-wing
# → Open reports/my-wing/index.html

# Or use Helm on Kubernetes
helm install aeroflow helm/aeroflow
```

---

## Pipeline Stages

```
STL → surfaceFeatureExtract → blockMesh + snappyHexMesh (adaptive, 3x retry)
  → checkMesh → controlDict / fvSchemes / fvSolution
  → simpleFoam / rhoCentralFoam (auto-selected by Mach)
  → forceCoeffs extraction → HTML report
```

Skills feedback loop: `case result → reward → GP update → next params → run again`

---

## Commands

| Command | Description |
|---------|-------------|
| `aeroflow init` | Ingest STL, fingerprint geometry, create case |
| `aeroflow run` | Execute full 8-stage CFD pipeline |
| `aeroflow status` | List all cases and their stage |
| `aeroflow report` | Generate HTML report from results |
| `aeroflow watch` | Auto-import STL files from directory |
| `aeroflow serve` | REST API + file watcher (combined) |
| `aeroflow doctor` | System health check + auto-fix |
| `aeroflow skills optimize` | Bayesian optimization via GP |
| `aeroflow user create` | Multi-tenant user management |
| `aeroflow tui` | Interactive terminal dashboard |

---

## Architecture

14 Rust crates in a workspace:

```text
aeroflow-cli         CLI + TUI
├── aeroflow-pipeline  8-stage orchestrator
│   ├── aeroflow-mesh     blockMesh + snappyHexMesh + quality
│   ├── aeroflow-solver   Config gen + subprocess launch
│   └── aeroflow-post     Force extraction
├── aeroflow-skills    PostgreSQL, STL fingerprint, user mgmt
├── aeroflow-api       REST API (axum), JWT, SSE
├── aeroflow-learner   Gaussian Process optimization
├── aeroflow-events    File watcher + event bus
├── aeroflow-doctor    Health checks (20+ checks, 7 categories)
├── aeroflow-core      Types, config, metrics, workspace
├── aeroflow-docker    Container management
├── aeroflow-report    Tera → HTML report
└── aeroflow-monitor   sysinfo resource monitoring
```

---

## Metrics (Prometheus)

Available at `http://localhost:8080/metrics`

`aeroflow_cases_created_total`, `aeroflow_cases_active`, `aeroflow_pipeline_duration_seconds`,
`aeroflow_solver_iterations`, `aeroflow_mesh_quality_failures_total`, and more.

---

## Deployment Options

| Method | Command |
|--------|---------|
| Docker Compose | `docker compose up -d` |
| Kubernetes | `helm install aeroflow helm/aeroflow` |
| Bare metal | `cargo build --release && ./target/release/aeroflow` |

---

## Documentation

- [Quickstart Guide](QUICKSTART.md)
- [Phase Roadmap](PHASE_ROADMAP.md)
- [GitHub Setup](GITHUB_SETUP.md)

---

## License

MIT © Jettanakorn Pengsiri — JFOX Aircraft Co., Ltd.

Developer: Jettanakorn Pengsiri
