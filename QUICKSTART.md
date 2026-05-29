# AeroFlow Agent — User Manual

> Developer: **Jettanakorn Pengsiri** — JFOX Aircraft Co., Ltd.
> Rust nightly | 15-crate workspace | OpenFOAM + PostgreSQL + Prometheus
> 26 physics domains | 16 solver templates | 49 config generators | 21 post-processing extractors

---

## 1. Prerequisites

- Docker & Docker Compose
- OpenFOAM (v2312 or later)
- Rust nightly (for development; binary is statically linked)
- PostgreSQL 16+ (when using REST API / skills DB)

---

## 2. Getting Started

### Option A — Docker Compose

```bash
docker compose -f docker/docker-compose.yml up -d
docker compose -f docker/docker-compose.yml ps
```

### Option B — Bare Metal

```bash
cargo build --release
./target/release/aeroflow settings init /path/to/workspace
psql -U aeroflow -d aeroflow -f db/migrations/001_initial_schema.sql
```

### Option C — Kubernetes

```bash
helm upgrade --install aeroflow helm/aeroflow -f my-values.yaml
```

---

## 3. System Health Check

```bash
aeroflow doctor                     # Full health check
aeroflow doctor docker              # Check Docker service
aeroflow doctor database            # Check PostgreSQL connectivity
aeroflow doctor openfoam            # Verify OpenFOAM installation
aeroflow doctor --fix               # Auto-remediate common issues
aeroflow doctor --watch             # Continuous monitoring (30s interval)
```

---

## 4. Core Workflow: STL → Report

```bash
# Step 1 — Ingest geometry
aeroflow init my-wing
#   → prompts for STL path
#   → computes 64³ voxel fingerprint + SHA-256 hash
#   → deduplicates by database lookup
#   → creates case record + manifest.json

# Step 2 — Run the pipeline
aeroflow run cases/my-wing
#   → auto-selects solver by Mach number:
#        M < 0.3  → simpleFoam (incompressible)
#        M < 0.8  → rhoSimpleFoam (compressible subsonic)
#        M < 1.2  → rhoPimpleFoam (transonic)
#        M < 5.0  → rhoCentralFoam (supersonic)
#        M ≥ 5.0  → hy2Foam / dsmcFoam (hypersonic, auto-detected)
#   → generates controlDict, fvSchemes, fvSolution
#   → runs blockMesh + snappyHexMesh (adaptive, up to 3 retries)
#   → checkMesh with aerospace-grade thresholds
#   → solves forces / forceCoeffs
#   → HTML report with mesh quality, Cl/Cd/Cm, convergence

# With Bayesian optimization (N trials):
aeroflow run cases/my-wing --trials 20

# Step 3 — Monitor
aeroflow status                     # List all cases
aeroflow tui                        # Interactive TUI dashboard

# Step 4 — View results
aeroflow report my-wing
#   → reports/my-wing/index.html
#   → images/: pressure contour, velocity slice, convergence history
```

---

## 5. Physics Module Library

AeroFlow supports **26 physics domains**. Enable them via `IntakeConfig` opt-in fields.

### 5.1 Aerodynamics (default)

The baseline domain. Solver auto-selected by Mach number and Reynolds number. Turbulence models: SA, k-ω SST, DES, IDDES, LES, SAS, V2F, PANS.

### 5.2 Conjugate Heat Transfer (CHT)

```toml
[cht]
problem = "CHT"              # ForcedConvection | CHT | NaturalConvection | Radiation
solid_material = "Steel"     # Steel | Aluminum | Copper | Ceramic | CFRP | Inconel | Custom
solid_k = 16.0               # W/(m·K)
coolant_temp = 300.0         # K
film_cooling = true
phase_change = false
radiation_model = "FvDOM"    # None | P1 | FvDOM | ViewFactor | Rosseland
```

Generates: `fluid/thermophysicalProperties`, `solid/thermophysicalProperties`, `radiationProperties`, coupled interface BCs, external wall BCs.

### 5.3 Rotating Machinery

```toml
[rotating]
approach = "MRF"             # MRF | AMI (sliding mesh)
rpm = 2500.0
axis = [1.0, 0.0, 0.0]
origin = [0.0, 0.0, 0.0]
cell_zone = "rotor"
n_blades = 6
diameter = 0.5
```

Generates: `MRFProperties` or `dynamicMeshDict`, `topoSetDict`, propeller forces function object.

### 5.4 Hypersonic Flow (Ma ≥ 5)

```toml
[hypersonic]
mach = 8.0
chemistry = "Park5Species"       # None | Park5Species | Park11Species
wall_catalysis = "FullyCatalytic" # NonCatalytic | FullyCatalytic | Partial
flux_scheme = "AUSMPlus"         # Kurganov | AUSMPlus
two_temperature = true
wall_temp = 500.0
```

Generates: JANAF thermo, chemistry properties, flux scheme, wall heat flux monitor. Automatically selects `hy2Foam` for two-temperature or chemistry cases, `rhoCentralFoam` otherwise.

### 5.5 MHD & Plasma

```toml
[mhd]
solver = "MhdFoam"             # MhdFoam | MagneticFoam | Custom
b_field = [0.0, 0.0, 1.0]     # Tesla
low_rm = true                  # Low magnetic Reynolds approximation
wall_conductivity = "Insulating" # Conducting | Insulating | Mixed
hartmann = 500.0               # Hartmann number (dimensionless)

[mhd.plasma_actuator]
model = "ShyyJayaraman"       # ShyyJayaraman | Suzen
applied_voltage = 5000.0      # V
actuator_width = 0.005        # m
dielectric_thickness = 0.001  # m
relative_permittivity = 4.0
```

Generates: `transportProperties` (MHD), `fvOptions` (Lorentz force), B field initial condition, DBD body force.

### 5.6 PEM Fuel Cells

```toml
[pemfc]
model = "NonIsothermal"          # SimplePolarization | Isothermal1D | NonIsothermal | TwoPhase
flow_field = "Serpentine"        # Parallel | Serpentine | Interdigitated | PinType
anode_pressure = 202650.0        # Pa
cathode_pressure = 202650.0      # Pa
cell_temperature = 353.0         # K
active_area = 0.01               # m²
membrane_thickness = 1.0e-4      # m
anode_stoich = 1.5
cathode_stoich = 2.0
reference_current_density = 10000.0  # A/m²
ecsa_initial = 100.0                 # m²/g
platinum_loading = 0.4               # mg/cm²

[pemfc.cycling]
profile = "Potentiodynamic"     # Potentiodynamic | Galvanodynamic | DriveCycle
cycles = 100
start_potential = 0.3           # V
end_potential = 1.2             # V
scan_rate = 0.05                # V/s

[pemfc.degradation]
model = "PtDissolution"        # None | PtDissolution | CarbonCorrosion | PinholeFormation | Combined
temperature = 353.0
relative_humidity = 0.9
potential_cycles = 10000
```

Generates 6 dictionary files: `pemfcProperties`, `electrochemistryProperties`, `membraneProperties`, `cyclingProperties`, `degradationProperties`, `blockMeshDict` (full 3D PEMFC mesh with boundary patches for anode/cathode inlet/outlet, bipolar plates).

Solver automatically selected by model:
- SimplePolarization / Isothermal1D → `pemfcFoam`
- NonIsothermal → `pemfcThermalFoam`
- TwoPhase → `pemfcTwoPhaseFoam`

### 5.7 Additional Physics Domains

| Domain | Config Key | Key Enums | Generates |
|--------|-----------|-----------|-----------|
| **Aeroacoustics** | `AeroacousticConfig` | FW-H sources (permeable/solid surface, receivers) | FW-H function object |
| **Ablation / TPS** | `AblationConfig` | SurfaceRecession, CharringMaterial, Pyrolysis | `ablationProperties` |
| **Cavitation** | `CavitationConfig` | Kunz, SchnerrSauer, Merkle | `cavitationProperties` |
| **Chemistry (JANAF)** | `JanafConfig` | N2, O2, NO, N, O, Ar, CO2, H2O, Custom | JANAF thermo dict |
| **Combustion** | `CombustionConfig` | EDC, LaminarFlamelet, PaSR, WSR | `chemistryProperties` |
| **Electrostatic** | `ElectrostaticConfig` | — | `transportProperties` |
| **FSI** | `FSIConfig` | LinearElastic, NonLinearGeometric, Plastic; DirichletNeumann, NeumannNeumann, RobinRobin | `mechanicalProperties` |
| **Marine** | `MarineConfig` | Hydrofoil, Propeller, Ship, Planing | `marineProperties` |
| **ML Surrogate** | `MlSurrogateConfig` | GPRBF, GPMatern, RF, XGB, LHS | `surrogateProperties` |
| **Multiphase** | `MultiphaseConfig` | VOF, EulerEuler, DriftFlux | `transportProperties` |
| **Non-Newtonian** | `NonNewtonianConfig` | PowerLaw, Cross, BirdCarreau, HerschelBulkley, Casson | `transportProperties` |
| **Nuclear** | `NuclearConfig` | Neutron, Photon, Coupled, RadiationHydro | `nuclearProperties` |
| **Particle** | `ParticleConfig` | Patch, Cone, Manual injection | Cloud properties + injection |
| **Phase Change** | `PhaseChangeConfig` | EnthalpyPorosity, LevelSet | `solidificationProperties` |
| **Porous Media** | `PorousZoneConfig` | Darcy, DarcyForchheimer | `fvOptions` |
| **Propulsion** | `PropulsionConfig` | Solid, Liquid, Hybrid, Scramjet | `propulsionProperties` |
| **Relaxation** | Priority enum | Speed, Balanced, Accuracy | `fvSchemes` + `fvSolution` |
| **Spray** | `SprayConfig` | ReitzDiwakar, KHRT, TAB, PilchErdman | `sprayProperties` |
| **Topology Opt.** | Rotating path | — | `topoSetDict` |
| **Viscoelastic** | `ViscoelasticConfig` | OldroydB, Giesekus | `transportProperties` |
| **Wave** | `WaveConfig` | StokesFirst/StokesFifth, Irregular, StreamFunction | `waveProperties` |
| **Wind Turbine** | `WindTurbineConfig` | Disc, Line, ALM actuator | `fvOptions` |

---

## 6. Solver Selection & Templates

The solver is selected through a decision tree that considers Mach, Reynolds, and enabled physics modules:

```
select_solver(mach, rotating, cht, mhd, pemfc)
  ├─ M ≥ 5.0  → hy2Foam (chem/two-temp) or rhoCentralFoam or dsmcFoam
  ├─ CHT       → chtMultiRegionSimpleFoam / buoyantSimpleFoam / ...
  ├─ MHD       → mhdFoam / magneticFoam
  ├─ PEMFC     → pemfcFoam / pemfcThermalFoam / pemfcTwoPhaseFoam
  ├─ AMI       → pimpleFoam / rhoPimpleFoam
  ├─ M > 0.3   → rhoSimpleFoam / rhoPimpleFoam / rhoCentralFoam
  └─ M ≤ 0.3   → simpleFoam (default)
```

### 16 Solver Templates for Custom Scaffold Generation

AeroFlow can generate complete OpenFOAM solver source code from 16 templates:

| Template | Base Solver | Physics Modules | Generated Files |
|---------|-------------|-----------------|-----------------|
| `MhdSimpleFoam` | simpleFoam | +EM, +Lorentz | UEqn, pEqn, createFields, Make/ |
| `MhdReactingFoam` | reactingFoam | +EM, +species, +reactions | +YEqn, +chemistry |
| `PlasmaActuatorFoam` | simpleFoam | +DBD body force fvOptions | +fvOptions |
| `HyperReactingFoam` | rhoCentralFoam | +species, +reactions, +real gas | +YEqn, +thermo |
| `ChtRotatingFoam` | chtMultiRegionSimpleFoam | +CHT, +MRF/AMI, +radiation | +MRFProps, +radiation, multi-region |
| `ViscoelasticHeatFoam` | simpleFoam | +viscoelastic, +heat | +EEqn, +constitutive |
| `BubblyReactingFoam` | reactingTwoPhaseEulerFoam | +two-phase, +reactions | +alphaEqn, +YEqn |
| `AblationFoam` | rhoCentralFoam | +ablation BC, +pyrolysis | +ablation BC, +char |
| `DsmcReactingFoam` | dsmcFoam | +DSMC, +chemistry | +DSMC chemistry |
| `MagneticConvectionFoam` | buoyantSimpleFoam | +magnetic, +buoyancy | +B field, +buoyancy |
| `RotorAeroFoam` | pimpleFoam | +AMI, +rotating | +dynamicMesh, +forces |
| `CoupledPlasmaFoam` | (custom) | +plasma, +E field | +plasma, +Poisson |
| `PemfcFoam` | simpleFoam | +membrane, +electrochemistry | +epsEqn, +YEqn (H2/O2/H2O) |
| `PemfcThermalFoam` | buoyantSimpleFoam | +PEMFC, +thermal | +epsEqn, +YEqn, +EEqn |
| `PemfcTwoPhaseFoam` | reactingTwoPhaseEulerFoam | +PEMFC, +two-phase | +epsEqn, +YEqn, +EEqn, +saturationEqn, +degradation |
| `Custom` | (user-defined) | Any module combination | Full scaffold from modules |

```bash
# Via the LLM agent (uses the generate_solver tool internally):
# Phase 2 of the agent loop auto-generates the solver scaffold based on
# the proposed config. Creates: Make/files, Make/options, solver.C,
# UEqn.H, pEqn.H, and domain-specific equations (EEqn.H, YEqn.H,
# epsEqn.H, saturationEqn.H, degradation.H, etc.)
```

---

## 7. Post-Processing Physics Extractors

21 specialized extractors for reading OpenFOAM field data and computing engineering metrics:

| Extractor | Methods | Domain |
|-----------|---------|--------|
| `PemfcExtractor` | current_density, membrane_potential_drop, water_crossover_flux, liquid_saturation, ecsa; nernst_potential, cell_voltage, power_density, efficiency, ohmic_loss, activation_loss, concentration_loss, ecsa_loss, membrane_lifetime | PEMFC |
| `HypersonicExtractor` | fay_riddell_heat_flux, shock_stand_off, stagnation_temperature, peak_heat_flux, aerothermal_aggregate | Hypersonic |
| `ChtExtractor` | heat_flux, max_solid_temp, film_cooling_effectiveness, nusselt_number, all_cht_metrics | CHT |
| `MhdExtractor` | hartmann_number, magnetic_reynolds, hartmann_layer, velocity_profile, induced_velocity, pressure_drop | MHD |
| `RotatingExtractor` | propeller_thrust, torque, efficiency, advance_ratio | Rotating |
| `PorousExtractor` | pressure_drop, darcy_number | Porous |
| `ParticleExtractor` | erosion_rate, deposition_rate | Particle |
| `MultiphaseExtractor` | phase_fraction, sauter_mean_diameter | Multiphase |
| `NonNewtonianExtractor` | apparent_viscosity, shear_rate | Non-Newtonian |
| `ViscoelasticExtractor` | normal_stress_diff, weissenberg_number | Viscoelastic |
| `FsiExtractor` | von_mises_stress, displacement, max_principal_stress | FSI |
| `CombustionExtractor` | flame_temperature, heat_release, mixture_fraction, damkohler_number | Combustion |
| `CavitationExtractor` | cavitation_number, vapor_fraction | Cavitation |
| `SprayExtractor` | sauter_mean_diameter | Spray |
| `AeroacousticExtractor` | sound_pressure_level, overall_spl, strouhal_number | Aeroacoustics |
| `WaveExtractor` | wave_height, ursell_number | Wave |
| `PhaseChangeExtractor` | liquid_fraction, stefan_number | Phase Change |
| `WindExtractor` | power_coefficient, thrust_coefficient | Wind |
| `ElectrostaticExtractor` | electric_field_magnitude | Electrostatic |
| `AblationExtractor` | recession_rate, char_depth, blowing_parameter | Ablation |
| `PropulsionExtractor` | specific_impulse, thrust_coefficient, characteristic_velocity, advance_coefficient | Propulsion |
| `NuclearExtractor` | neutron_flux, reaction_rate, multiplication_factor | Nuclear |
| `MarineExtractor` | wave_resistance, advance_coefficient, cavitation_inception_speed | Marine |
| `MlSurrogateExtractor` | expected_improvement, upper_confidence_bound, probability_of_improvement | ML Surrogate |

---

## 8. LLM Agent Tools

The integrated LLM agent provides 12 tools for autonomous CFD optimization:

| Tool | Phase | Purpose |
|------|-------|---------|
| `get_case_detail` | Intake | Read simulation case metadata |
| `get_case_results` | Intake | Get current results (Cd, Cl, y+, convergence) |
| `propose_config` | 1 | Generate `agent-manifest.json` with full physics config (all 26 domains) |
| `generate_solver` | 2 | Generate custom OpenFOAM solver scaffold (16 templates) |
| `run_simulation` | 3 | Launch CFD solver pipeline with auto-detected solver |
| `diagnose_and_fix` | 4 | Diagnose failures → coarsen mesh, change schemes, reduce CFL, improve IC |
| `evaluate_results` | 5 | Extract forces, mesh quality, convergence metrics |
| `compare_iterations` | 6 | Score iteration vs engineering targets |
| `plan_refinement` | 7 | Generate improved config for next iteration |
| `update_skill` | 8 | Save winning config to skills database |
| `get_pipeline_status` | — | Get current pipeline stage |
| `get_skill_recommendations` | — | Query skills DB for similar past successes |

### 8-Phase Autonomous Agent Loop

```
Phase 1:  propose_config    → agent-manifest.json with physics config
Phase 2:  generate_solver   → custom solver scaffold (if needed)
Phase 3:  run_simulation    → launch solver with plateu detection
Phase 4:  diagnose_and_fix  → auto-fix divergence (14 diagnoses: F1-F14)
                ↻ loop back to Phase 3 until converged
Phase 5:  evaluate_results  → extract key metrics from OpenFOAM output
Phase 6:  compare_iterations → score vs targets (GCI, residual, y+, forces)
Phase 7:  plan_refinement   → propose improved config for next iteration
                ↻ loop entire cycle until target met or budget exhausted
Phase 8:  update_skill      → persist winning config to skills DB
```

---

## 9. Auto-Import Workflow

Watch a directory for new STL files:

```bash
aeroflow watch /data/import
# → auto-detects new .stl files via notify v7
# → computes fingerprint + SHA-256 hash
# → deduplicates by path + mtime
# → creates geometry + case records
```

---

## 10. REST API & Web Server

```bash
aeroflow serve
# => API on http://0.0.0.0:8080
# => File watcher on /data/import
# => Metrics on  http://0.0.0.0:8080/metrics
```

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| GET | `/api/health` | No | System health + version |
| GET | `/metrics` | No | Prometheus metrics |
| POST | `/api/auth/login` | No | JWT login |
| POST | `/api/auth/register` | No | User registration |
| GET | `/api/cases` | JWT | List user's cases |
| POST | `/api/cases` | JWT | Create new case |
| GET | `/api/cases/{id}` | JWT | Case detail |
| POST | `/api/cases/{id}/run` | JWT | Execute pipeline |
| GET | `/api/users` | Admin | List users |
| GET | `/api/events` | JWT | SSE event stream |

---

## 11. User Management

```bash
aeroflow user create          # Interactive user creation
aeroflow user list            # List all users
aeroflow user show email      # User details
aeroflow user update email    # Update user
aeroflow user delete email    # Delete user
aeroflow user login email     # Authenticate (JWT)
```

---

## 12. Skills & Bayesian Optimization

The skills database stores optimized CFD parameters per (geometry, flow regime) using Gaussian Process regression.

```bash
aeroflow skills list            # List skills
aeroflow skills show skill-name # Show skill details
aeroflow skills optimize name --trials 20  # GP optimization
aeroflow skills export name --format json  # Export skill
aeroflow skills import path     # Import skill
aeroflow skills reset name      # Reset to fresh state
```

### Optimization Loop

```
For each trial:
  1. GP suggests next parameters via Expected Improvement
  2. Pipeline runs with those parameters
  3. Reward = w₁·Cl_err + w₂·Cd_ex + w₃·y⁺ + w₄·residual + w₅·mesh_quality
  4. GP updated with new observation
  5. Repeat until budget exhausted
```

---

## 13. Settings

```bash
aeroflow settings show            # Current config
aeroflow settings set key=value   # Override setting
aeroflow settings init path       # Initialize workspace
aeroflow settings reset           # Reset to defaults
aeroflow settings path            # Config file location
```

Config precedence: `$WORKSPACE/settings/aeroflow-settings.toml` → `~/.config/aeroflow/settings.toml` → env vars

---

## 14. Production Deployment

### Prometheus Metrics

Available at `http://localhost:8080/metrics`:

| Metric | Type | Description |
|--------|------|-------------|
| `aeroflow_cases_created_total` | Counter | Cases created |
| `aeroflow_cases_completed_total` | Counter | Cases completed |
| `aeroflow_cases_failed_total` | Counter | Failed cases |
| `aeroflow_cases_active` | Gauge | Currently running |
| `aeroflow_queue_depth` | Gauge | Queued cases |
| `aeroflow_pipeline_duration_seconds` | Histogram | Pipeline runtime |
| `aeroflow_solver_iterations` | Histogram | Iterations per case |
| `aeroflow_http_request_duration_seconds` | Histogram | API latency |
| `aeroflow_db_query_duration_seconds` | Histogram | DB query latency |
| `aeroflow_mesh_quality_failures_total` | Counter | Mesh failures |
| `aeroflow_skill_trials_total` | Counter | GP trials run |

### Structured Logging

```bash
aeroflow serve --json-logs
RUST_LOG=aeroflow=debug aeroflow serve --json-logs
```

### Docker Compose

```yaml
services:
  aeroflow:
    build: .
    ports: ["8080:8080"]
    volumes: [/mnt/data/workspace:/workspace]
    environment:
      - AEROFLOW_DATABASE_URL=postgres://aeroflow:pass@postgres:5432/aeroflow
  postgres:
    image: postgres:16-alpine
```

### Kubernetes

```bash
helm upgrade --install aeroflow helm/aeroflow \
  --set postgresql.auth.password=secure-pass \
  --set ingress.enabled=true
kubectl scale deployment aeroflow --replicas=3
```

---

## 15. Architecture

15 Rust crates in a workspace:

```text
aeroflow-cli            CLI + TUI
├── aeroflow-pipeline   9-stage orchestrator
│   ├── aeroflow-mesh     blockMesh + snappyHexMesh
│   ├── aeroflow-solver   49 config generators, 16 solver templates, solver selection
│   └── aeroflow-post     21 physics extractors
├── aeroflow-core        Core types, 26 physics domain configs
├── aeroflow-llm         12 LLM agent tools, 8-phase autonomous loop
├── aeroflow-skills      PostgreSQL, STL fingerprint, user management
├── aeroflow-api         REST API (axum), JWT, SSE
├── aeroflow-learner     Gaussian Process optimization
├── aeroflow-events      File watcher + event bus
├── aeroflow-doctor      20+ health checks, 7 categories
├── aeroflow-docker      Container management
├── aeroflow-report      Tera → HTML report generator
└── aeroflow-monitor     sysinfo resource monitoring
```

---

## 16. End-to-End Example: PEMFC Fuel Cell

```bash
# 1. Verify system
aeroflow doctor

# 2. Initialize workspace
aeroflow settings init ~/aeroflow-workspace

# 3. Create PEMFC case (interactive — enter config when prompted)
aeroflow init my-pemfc
# → Enter STL path (or skip for PEMFC mesh generation)
# → PEMFC config: model=NonIsothermal, flow_field=Serpentine
# → Case created

# 4. Run with PEMFC solver (auto-selects pemfcThermalFoam)
aeroflow run ~/aeroflow-workspace/cases/my-pemfc
# → Generates pemfcProperties, electrochemistryProperties,
#   membraneProperties, cyclingProperties, degradationProperties,
#   blockMeshDict (3D single-channel mesh)
# → Solves epsEqn + YEqn (H2/O2/H2O) + EEqn (Joule + reaction enthalpy)
# → Post-processes current density, cell voltage, power density

# 5. Monitor
aeroflow tui

# 6. View polarization curve and degradation report
aeroflow report my-pemfc
```

### End-to-End Example: Hypersonic Re-entry

```bash
aeroflow init reentry-vehicle
# → mach=8.0, chemistry=Park5Species, flux=AUSMPlus
aeroflow run cases/reentry-vehicle
# → auto-selects hy2Foam
# → generates JANAF thermo, chemistry properties, radiation (fvDOM)
# → post-processes stagnation heating (Fay-Riddell), shock stand-off
```

### End-to-End Example: CHT Turbine Blade

```bash
aeroflow init turbine-blade
# → cht problem=CHT, solid=Inconel, radiation=FvDOM
# → rotating approach=AMI, rpm=15000
aeroflow run cases/turbine-blade --trials 10
# → auto-selects chtRotatingFoam
# → conjugate heat transfer with film cooling
# → post-processes heat flux, max solid temp, film cooling effectiveness
```

---

## Quick Reference

```text
aeroflow                            # CLI tool
├── init [name]                     # Start a case (interactive)
├── run <case> [--trials N]         # Execute pipeline
├── status                          # List cases
├── report <case>                   # Generate report
├── watch [path]                    # Auto-import STL files
├── serve [port]                    # REST API + watcher
├── doctor [category]               # Health checks
│   ├── --fix                       # Auto-remediate
│   ├── --json                      # JSON output
│   └── --watch                     # Continuous monitoring
├── skills                          # Skills management
│   ├── list                        # List skills
│   ├── show <name>                 # Skill details
│   ├── optimize <name>             # Bayesian optimization
│   ├── export <name>               # Export skill
│   ├── import <path>               # Import skill
│   └── reset <name>                # Reset skill
├── user                            # User management
│   ├── create                      # Create user
│   ├── list                        # List users
│   ├── show <email>                # User details
│   ├── update <email>              # Update user
│   ├── delete <email>              # Delete user
│   └── login <email>               # Authenticate
├── settings                        # Configuration
│   ├── show                        # Current settings
│   ├── set <key>=<val>             # Override setting
│   ├── init [path]                 # Initialize workspace
│   ├── reset                       # Reset to defaults
│   └── path                        # Config file location
├── tui                             # Interactive dashboard
└── --json-logs                     # Structured JSON logs
```
