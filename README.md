<p align="center">
  <h1 align="center">AeroFlow Agent</h1>
  <p align="center"><i>Autonomous CFD Analysis Orchestrator</i></p>
  <p align="center">
    <a href="https://github.com/Jettanakorn/aeroflow/actions"><img src="https://github.com/Jettanakorn/aeroflow/actions/workflows/ci.yml/badge.svg" alt="CI"></a>
    <a href="https://github.com/Jettanakorn/aeroflow/releases"><img src="https://img.shields.io/github/v/release/Jettanakorn/aeroflow" alt="Release"></a>
    <a href="https://github.com/Jettanakorn/aeroflow/blob/main/LICENSE"><img src="https://img.shields.io/github/license/Jettanakorn/aeroflow" alt="License"></a>
  </p>
</p>

**AeroFlow Agent** is a Rust-based autonomous multi-physics simulation orchestrator that takes
an STL geometry and produces a complete engineering report — covering **aerodynamics, CHT,
hypersonics, MHD, PEM fuel cells, rotating machinery, and 21 additional physics domains** —
without manual intervention. Includes a custom OpenFOAM solver scaffold generator with
**16 solver templates** and an **8-phase LLM-driven autonomous optimization loop**.

Built on OpenFOAM, PostgreSQL, and Gaussian Process regression.
Deployable via Docker Compose or Kubernetes Helm chart.

---

## Features

- **26 physics domains** — aerodynamics, CHT, hypersonic (Ma≥5, Park chemistry, JANAF), MHD/plasma, PEMFC (4 models), rotating machinery (MRF/AMI), combustion, multiphase, FSI, cavitation, spray, porous media, viscoelastic, non-Newtonian, particle, aeroacoustics, wave, phase change, wind, marine, propulsion, ablation, nuclear, electrostatic, ML surrogate, topology optimization
- **16 solver templates** — auto-generate complete OpenFOAM solver source code (Make/files, .C, UEqn, pEqn, EEqn, YEqn, epsEqn, saturationEqn, degradationEqn, createFields)
- **End-to-end automation** — STL voxel fingerprint → 9-stage pipeline → HTML report
- **Self-healing mesh** — adaptive snappyHexMesh with up to 3 retries and relaxed parameters
- **49 config generators** — physics-aware controlDict, fvSchemes, fvSolution, transport, thermo, radiation, MRF, AMI, BCs per domain
- **21 post-processing extractors** — domain-specific metrics (Fay-Riddell heat flux, PEMFC polarization, Hartmann number, Nusselt number, etc.)
- **8-phase LLM agent loop** — propose → scaffold → run → diagnose → evaluate → compare → refine → persist
- **Autonomous skill learning** — Gaussian Process optimizes solver parameters per (geometry, flow regime)
- **Real-time monitoring** — TUI dashboard, SSE event stream, Prometheus metrics
- **SaaS-ready** — JWT auth, multi-tenant REST API, file watcher auto-import
- **Visualization** — auto-generated pressure surface, velocity slice, and convergence plots (foamToVTK + Python matplotlib)
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
  → checkMesh → controlDict / fvSchemes / fvSolution (49 domain-aware generators)
  → solver (16 auto-selected by Mach + physics: simpleFoam, rhoCentralFoam, hy2Foam,
    chtMultiRegionFoam, mhdFoam, pemfcFoam, pemfcThermalFoam, pemfcTwoPhaseFoam, etc.)
  → 21 domain-specific physics extractors → foamToVTK
  → Python viz (pressure/velocity/convergence plots) → HTML report
```

Skills feedback loop: `case result → reward → GP update → next params → run again`

---

## Commands

| Command | Description |
|---------|-------------|
| `aeroflow init` | Ingest STL, fingerprint geometry, create case |
| `aeroflow run` | Execute full CFD pipeline (8 stages + visualization) |
| `aeroflow tui` | Interactive terminal dashboard |
| `aeroflow status` | List all cases and their stage |
| `aeroflow report` | Generate HTML report from results |
| `aeroflow watch` | Auto-import STL files from directory |
| `aeroflow serve` | REST API + file watcher (combined) |
| `aeroflow doctor` | System health check + auto-fix |
| `aeroflow skills` | Skills DB management (list/show/optimize/export/import) |
| `aeroflow user` | Multi-tenant user management (create/list/update/delete) |
| `aeroflow settings` | Configuration management (show/set/init/reset) |

---

## Architecture

15 Rust crates in a workspace + Python visualization scripts:

```text
aeroflow-cli            CLI + TUI
├── aeroflow-pipeline   9-stage orchestrator (+ Visualization)
│   ├── aeroflow-mesh     blockMesh + snappyHexMesh + quality
│   ├── aeroflow-solver   49 config generators, 16 solver templates, solver selection
│   └── aeroflow-post     21 domain-specific physics extractors
├── aeroflow-core        Core types, 26 physics domain configs
├── aeroflow-llm         12 LLM agent tools, 8-phase autonomous loop
├── aeroflow-skills      PostgreSQL, STL fingerprint, user mgmt
├── aeroflow-api         REST API (axum), JWT, SSE
├── aeroflow-learner     Gaussian Process optimization
├── aeroflow-events      File watcher + event bus
├── aeroflow-doctor      Health checks (20+ checks, 7 categories)
├── aeroflow-docker      Container management
├── aeroflow-report      Tera → HTML report (with viz images)
├── aeroflow-monitor     sysinfo resource monitoring
└── scripts/viz/         Python VTK + matplotlib visualization
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

## Version History

| Version | Date | Highlights |
|---------|------|------------|
| v0.1.0 | 2026-05-20 | P0 scaffold + P0.5 settings/users/workspace/binary |
| v0.2.0 | 2026-05-20 | P1–P2 real pipeline: end-to-end CFD, mesh quality loop, solver |
| v0.3.0 | 2026-05-20 | P3 SaaS: REST API, JWT auth, file watcher, multi-tenant |
| v0.4.0 | 2026-05-20 | P4–P5 production: GP optimization, CI/CD, Helm chart, metrics |
| **v0.5.0** | **2026-05-22** | **P6 visualization: foamToVTK export, Python matplotlib images (pressure/velocity/convergence), report embedding** |
| **v0.6.0** | **2026-05-29** | **P7 quality: Digital Wind Tunnel, zero warnings (179 fixed), 77 new tests, code hardening, Dockerfile fix** |

---

## License

MIT © Jettanakorn Pengsiri — JFOX Aircraft Co., Ltd.

Developer: Jettanakorn Pengsiri
