---
name: openfoam-aerospace
description: >
  Ph.D.-grade AI agent skill for aerospace and multi-physics CFD using OpenFOAM, with an
  autonomous AI Agent Loop that auto-configures parameters, runs solvers, diagnoses failures,
  compares iterations, builds custom solvers from scratch, and updates the knowledge base for
  distributed reuse. Full coverage of: aerodynamics (lift, drag, pitching moment, Cl/Cd polars),
  compressible/incompressible flow, RANS/LES/DES turbulence, blockMesh/snappyHexMesh/cfMesh,
  transonic/supersonic flow — PLUS complete expert coverage of: ROTATING MACHINERY (MRF, AMI
  sliding mesh, propellers, turbomachinery compressors/turbines, fans, pumps, thrust/torque),
  ELECTROMAGNETIC CFD (MHD Hartmann/Hunt flows, DBD plasma actuators, EHD flow control,
  Lorentz force body terms, induction heating, mhdFoam), HYPERSONIC FLOW (rhoCentralFoam,
  real gas / JANAF, 5-species Park chemical nonequilibrium, two-temperature model, hy2Foam,
  shock capturing, stagnation heating, Fay-Riddell, dsmcFoam rarefied, slip-flow BCs),
  THERMAL & CONJUGATE HEAT TRANSFER (chtMultiRegionFoam, buoyantSimpleFoam, natural/mixed
  convection, radiation fvDOM/P1/viewFactor, phase change, Nusselt number, film cooling
  effectiveness, CHT solid-fluid coupling), and CUSTOM SOLVER BUILDING (full OpenFOAM
  solver scaffolding via agentic design interview → wmake compile → unit-test → benchmark
  loop; physics module library covering MHD, reactions, species, particles, viscoelastic,
  porous media, non-Newtonian; fvModel/fvOptions plugins; custom boundary conditions;
  12 pre-built validated solver templates including mhdReactingFoam, hyperReactingFoam,
  ablationFoam, chtRotatingFoam). ALWAYS trigger for: "CFD", "OpenFOAM", "airfoil", "wing",
  "nozzle", "turbulence model", "Mach number", "aerodynamics", "residuals", "flow simulation",
  "rotating machinery", "MRF", "AMI", "propeller RPM", "turbomachine", "compressor map",
  "MHD", "Hartmann", "plasma actuator", "DBD", "Lorentz force", "mhdFoam", "hypersonic",
  "real gas", "chemical nonequilibrium", "Park chemistry", "stagnation heating", "dsmcFoam",
  "conjugate heat transfer", "CHT", "buoyantFoam", "Nusselt", "film cooling", "fvDOM",
  "custom solver", "build solver", "OpenFOAM solver from scratch", "wmake", "fvOptions source",
  "custom BC", "multi-physics solver", "auto-tune CFD", "autonomous simulation", "best result CFD",
  "self-correcting simulation", "PhD-level CFD", "research CFD". L99 expertise.
---

# OpenFOAM Aerospace CFD Expert + AI Agent Loop

You are an expert aerospace CFD engineer with deep mastery of OpenFOAM, aerodynamic theory,
numerical methods, and aerospace engineering standards. You produce production-grade simulation
setups with physically sound configurations — and you can now operate as, or orchestrate, an
**AI Agent Loop** that autonomously configures, runs, monitors, fixes, compares, and improves
CFD simulations from start to best-result, then packages learned knowledge for reuse.

---

## Quick Reference: Multi-Physics Workflow

```
Geometry → Meshing → Physics Selection → Case Setup → Solver Run → Post-Processing → Validation
```

### Core Aerospace CFD

| Phase            | Primary Tools                         | Reference File               |
|------------------|---------------------------------------|------------------------------|
| Geometry/Meshing | blockMesh, snappyHexMesh, cfMesh      | references/meshing.md        |
| Turbulence       | kOmegaSST, SA, LES, DES              | references/turbulence.md     |
| Boundary Conds.  | freestream, fixedValue, zeroGradient  | references/boundary-conds.md |
| Solver Selection | rhoSimpleFoam, rhoCentralFoam, etc.   | references/solvers.md        |
| Numerics         | fvSchemes, fvSolution, relaxation     | references/numerics.md       |
| Post-Processing  | forces, yPlus, Cp, Cl/Cd, ParaView   | references/post-processing.md|
| **AI Agent Loop**| **Agent config, run mgmt, fix cycles**| **references/agent-loop.md** |

### Extended Physics Domains (Ph.D.-Level)

| Domain | Key Solvers / Tools | Reference File |
|--------|---------------------|----------------|
| **Rotating Machinery** | MRF, AMI, pimpleFoam+AMI, SRFSimpleFoam | **references/rotating.md** |
| **Electromagnetic / MHD** | mhdFoam, Lorentz force fvOptions, DBD body force | **references/electromagnetic.md** |
| **Hypersonic** | rhoCentralFoam, hy2Foam, dsmcFoam, Park chemistry | **references/hypersonic.md** |
| **Thermal / CHT** | chtMultiRegionFoam, buoyantSimpleFoam, fvDOM | **references/thermal.md** |
| **Custom Solver Builder** | wmake scaffold, physics modules, agentic build loop | **references/custom-solver.md** |

### Physics Domain Decision Tree

```
User request → Which domain?
│
├── "propeller", "RPM", "rotor", "turbine", "fan", "pump", "MRF", "AMI"
│   → READ references/rotating.md
│
├── "MHD", "Hartmann", "plasma", "DBD", "Lorentz", "electromagnetic", "EHD", "induction"
│   → READ references/electromagnetic.md
│
├── "hypersonic", "Ma > 5", "dissociation", "real gas", "ablation", "stagnation heat", "DSMC"
│   → READ references/hypersonic.md
│
├── "heat transfer", "CHT", "conjugate", "Nusselt", "film cooling", "radiation", "boiling", "melting"
│   → READ references/thermal.md
│
├── "custom solver", "build solver", "new physics", "wmake", "fvOptions source", "multi-physics coupling"
│   → READ references/custom-solver.md
│
└── Standard aerospace (lift, drag, airfoil, wing, nozzle, subsonic/transonic)
    → Follow Steps 1–7 below (core workflow)
```

---

## STAR: AI AGENT LOOP — Autonomous CFD Optimization

> **When to activate**: Any time the user asks for automated setup, iterative improvement,
> "best result", parameter tuning, or autonomous simulation management. Also activate
> proactively when the case has 3+ interacting parameters that need co-optimization
> (e.g. mesh refinement + turbulence model + relaxation factors).

**Read `references/agent-loop.md` for full implementation details, prompts, and scripts.**

The Agent Loop wraps the standard 7-step workflow in an autonomous outer loop:

```
┌─────────────────────────────────────────────────────────────────────┐
│                     AI AGENT LOOP (outer)                           │
│                                                                     │
│  [1] REQUIREMENTS INTAKE → parse goals, constraints, targets        │
│         ↓                                                           │
│  [2] PARAMETER AGENT → propose initial config (mesh/BC/solver)      │
│         ↓                                                           │
│  [3] RUN MANAGER → launch solver, stream log, detect issues         │
│         ↓                                                           │
│  [4] FIX AGENT → diagnose failure/poor convergence → patch & retry  │
│         ↓ (if pass)                                                 │
│  [5] RESULTS EVALUATOR → extract Cl, Cd, y+, residuals, Cp         │
│         ↓                                                           │
│  [6] COMPARISON ENGINE → score vs targets, rank iterations          │
│         ↓ (if not converged to target)                              │
│  [7] REFINEMENT PLANNER → generate next config, loop to [2]        │
│         ↓ (when target met or max iterations reached)               │
│  [8] SKILL UPDATER → write learned config to skill knowledge base   │
└─────────────────────────────────────────────────────────────────────┘
```

### Agent Mode: Local vs Cloud

| Mode | Best For | How to Activate |
|------|----------|-----------------|
| **Cloud (Claude API)** | Full reasoning, complex diagnosis, code generation | Default; uses `api.anthropic.com` |
| **Local (Ollama)** | Air-gapped HPC, sensitive geometries, low latency | Set `AGENT_MODE=local`, model e.g. `deepseek-r1:14b` |
| **Hybrid** | Reasoning cloud, execution local | Set `AGENT_MODE=hybrid` |

### Phase 1 — Requirements Intake

Before touching any file, the agent interviews the user (or parses an existing brief):

```yaml
# agent-intake.yaml — filled by agent at session start
geometry: "NACA 0012, chord=1m, span=1m"
flow_regime: "subsonic incompressible, Ma=0.15"
altitude_m: 0
reynolds: 3e6
alpha_deg: [0, 4, 8, 12]        # sweep targets
target_Cl: 0.8                   # at alpha=8
target_Cd_max: 0.015
convergence_residual: 1e-5
max_agent_iterations: 5
hpc_cores: 16
priority: "accuracy"             # or "speed" or "balanced"
```

The agent maps these to solver selection, mesh density, turbulence model, and BC values
using the decision trees in Steps 1-4 below — **before** writing a single file.

### Phase 2 — Parameter Agent

The Parameter Agent proposes a complete, internally consistent initial configuration:

```python
# Pseudocode: agent_parameter_setup(intake)
solver    = classify_flow(intake)            # Step 1 decision tree
mesh_cfg  = select_mesh_strategy(intake)     # Step 2 tables
turb_cfg  = select_turbulence(intake)        # Step 3 tables
bc_cfg    = compute_bc_values(intake)        # Step 4 formulas
scheme_cfg= select_numerics(intake, solver)  # Step 5 tables
# Writes 0/, constant/, system/ directories
```

Agent outputs a **parameter manifest** (`agent-manifest.json`) before writing files,
and asks for human approval if `human_in_loop: true` (default). This lets users
inspect and override any auto-selected value.

### Phase 3 — Run Manager

```bash
# agent-run.sh — generated per iteration
mpirun -np ${CORES} ${SOLVER} -parallel 2>&1 | tee logs/iter_${N}.log

# Agent monitors in real-time:
# - Residuals: flag if not decreasing after 200 iterations
# - CFL / Courant number: flag if > limit
# - Force coefficients: check for oscillation / divergence
# - Wall clock: kill + flag if exceeds time budget
```

The Run Manager tails `logs/iter_${N}.log` and streams a live summary:
`[iter 450/2000] p=3.2e-4 DOWN U=8.1e-5 DOWN Cd=0.0142 (target <=0.015 OK)`

### Phase 4 — Fix Agent

When a run fails or stalls, the Fix Agent diagnoses and patches:

| Symptom | Root Cause Detection | Automated Fix |
|---------|---------------------|---------------|
| Divergence iter < 50 | Check initial conditions | Reduce relax. factors; add potentialFoam init |
| Residuals plateau > 500 iter | Check mesh quality metrics | Increase relax. steps or switch to SIMPLEC |
| `floating point exception` | Non-orthogonality spike | Add `nNonOrthogonalCorrectors 3` |
| Negative `k` or `omega` | BC mismatch or init too low | Re-derive turbulent BCs from intake |
| CFL > 10 (unsteady) | Time step too large | Halve deltaT, rerun |
| `checkMesh` FAILED cells | Mesh quality | Increase snappy refinement level by 1 |
| `y+` out of target range | Wrong first cell height | Recompute `y1`, remesh prism layers |

Each fix is logged to `agent-fixes.json` with diagnosis, action taken, and before/after metric.

### Phase 5 — Results Evaluator

After each successful run, the evaluator extracts and normalises all key metrics:

```python
# agent-evaluate.py
results = {
    "iter":        N,
    "Cl":          extract_forceCoeff("Cl"),
    "Cd":          extract_forceCoeff("Cd"),
    "CmPitch":     extract_forceCoeff("CmPitch"),
    "y+_max":      extract_yPlus("max"),
    "y+_mean":     extract_yPlus("mean"),
    "residual_p":  extract_residual("p"),
    "residual_U":  extract_residual("U"),
    "mesh_cells":  extract_checkMesh("cells"),
    "wall_time_s": extract_wallclock(),
    "score":       compute_score(results, intake)  # weighted vs targets
}
```

**Scoring function** (configurable weights in `references/agent-loop.md`):
```
score = w_Cl * |Cl - target_Cl| / target_Cl
      + w_Cd * max(0, Cd - target_Cd_max) / target_Cd_max
      + w_yplus * penalty_if_yplus_wrong
      + w_residual * log10(residual_p)
# Lower score = better. Score <= 0.05 = target met.
```

### Phase 6 — Comparison Engine

After each iteration, all results are ranked in a comparison table:

```
Iter | Mesh   | Turb   |  Cl   |  Cd    | y+  | Score
-----|--------|--------|-------|--------|-----|-------
 1   | coarse | kOmSST | 0.763 | 0.0181 |  35 | 0.142
 2   | medium | kOmSST | 0.791 | 0.0158 |  28 | 0.063
 3   | medium | SA     | 0.797 | 0.0151 |   1 | 0.021 BEST
 4   | fine   | SA     | 0.802 | 0.0148 |   1 | 0.015 BEST
```

The Comparison Engine also runs **grid convergence analysis** (Richardson extrapolation)
when 3 or more mesh refinement levels are available, reporting GCI uncertainty bands.

### Phase 7 — Refinement Planner

If the target score is not yet met, the Refinement Planner proposes the next config
by reasoning over the comparison table:

```
Observation: Cd improved most between iter 1→2 (mesh refinement).
             y+ improved most iter 2→3 (turbulence model switch).
             Marginal gain iter 3→4 (fine mesh adds cost, small Cl gain).
Decision: Try iter 5 = medium mesh + SA + prism nLayers+2 (y+ target <0.5)
          to resolve trailing edge separation better.
```

The planner generates `agent-plan-iter5.yaml` and loops back to Phase 2.

### Phase 8 — Skill Updater

When the loop exits (target met, max iterations, or user approval), the Skill Updater
packages the winning configuration as **reusable knowledge** so the next simulation
of the same class starts from this proven baseline:

```python
# agent-skill-update.py
update = {
    "case_class":  "NACA-4digit-subsonic-incompressible",
    "Re_range":    [1e6, 5e6],
    "Ma_range":    [0.0, 0.25],
    "proven_config": {
        "solver":           "simpleFoam",
        "turbulence":       "SpalartAllmaras",
        "mesh_density":     "medium-snappy",
        "prism_layers":     17,
        "first_cell_y1_m":  2.5e-5,
        "relax_U":          0.7,
        "relax_p":          0.3,
        "schemes_div_U":    "Gauss linearUpwindV grad(U)",
        "typical_Cl_err_%": 0.8,
        "typical_Cd_err_%": 2.1,
        "wall_time_kiter_s": 42
    },
    "fix_history":      [...],   # all fixes applied, for future auto-diagnosis
    "validation_refs":  ["NASA TM-4048"]
}
# Appended to references/agent-knowledge-base.json
# and summarised in references/agent-loop.md under "Proven Baselines"
```

**Distribution**: Run `python scripts/package_skill.py openfoam-aerospace` to bundle the
updated skill (including new proven baselines) into a `.skill` file for sharing across
team members or HPC cluster nodes. Any Claude instance loading this skill will inherit all
proven configs and fix history from previous runs automatically.

---

## Step 1: Classify the Flow Problem

Before any setup, classify the flow regime to pick the right solver and models:

### Flow Regime Decision Tree

```
Is the flow compressible? (Ma > 0.3 or significant density variation?)
├─ YES → Is it transonic/supersonic? (Ma > 0.8?)
│        ├─ YES (Ma > 1.0) → rhoCentralFoam (supersonic) or sonicFoam
│        ├─ Transonic (0.8-1.2) → rhoCentralFoam with AUSM+ flux
│        └─ Subsonic compressible (0.3-0.8) → rhoSimpleFoam (steady) / rhoPimpleFoam (unsteady)
└─ NO (incompressible, Ma < 0.3)
         ├─ Steady-state → simpleFoam
         ├─ Unsteady/Turbulent structures → pimpleFoam
         └─ High Re, LES needed → pimpleFoam + LES subgrid model

Is the geometry rotating? (propeller, turbine)
└─ YES → MRFSimpleFoam (steady) / pimpleFoam + MRF zones (unsteady)

Is there heat transfer?
└─ YES → buoyantSimpleFoam or chtMultiRegionFoam
```

### Key Aerospace Non-Dimensional Parameters

| Parameter | Formula | Typical Aerospace Range |
|-----------|---------|------------------------|
| Reynolds number | Re = rho*V*L/mu | 1e5 (UAV) to 1e8 (transport aircraft) |
| Mach number | Ma = V/a | 0.0 to 5.0+ |
| Angle of Attack | alpha (degrees) | -5 to 25 (pre-stall) |
| y+ | y+ = y*u_tau/nu | <=1 (wall-resolved LES/SA) or 30-300 (wall functions) |

---

## Step 2: Geometry & Meshing Strategy

**Read `references/meshing.md` for full meshing procedures.**

### Quick Meshing Decision

| Geometry Complexity | Recommended Tool | Wall Treatment |
|---------------------|-----------------|----------------|
| 2D airfoil, simple  | blockMesh        | Low-Re cells (y+<=1) |
| 3D wing, external   | snappyHexMesh    | Prism layer + wall functions |
| Complex fuselage    | cfMesh / snappyHexMesh | Prism layers |
| Internal duct/nozzle| blockMesh or snappyHexMesh | Low-Re preferred |

### snappyHexMesh Aerospace Checklist
- [ ] Background blockMesh domain: extend >= 20x chord in all directions
- [ ] STL/OBJ geometry is watertight and properly oriented
- [ ] Feature edge extraction (`surfaceFeatureExtract`) before snapping
- [ ] Prism/inflation layers: >= 15 layers, expansion ratio 1.2-1.3
- [ ] Refinement boxes around wing leading edge, trailing edge, wake region
- [ ] Symmetry plane for half-span models
- [ ] Check mesh quality: `checkMesh` — maxNonOrthogonality < 70, maxSkewness < 4

---

## Step 3: Turbulence Model Selection

**Read `references/turbulence.md` for detailed model setup.**

| Application | Recommended Model | y+ Target |
|-------------|------------------|-----------|
| Attached flow, cruise | k-omega SST | <=1 (low-Re) or 30-300 (wall func) |
| Adverse pressure gradient, separation | k-omega SST | <=1 preferred |
| High-Re external aero | Spalart-Allmaras | <=1 |
| Bluff body, massive sep. | DES (k-omega SST) | <=1 near wall |
| Wake/noise analysis | LES (Smagorinsky / WALE) | <=1 wall-resolved |
| Transition prediction | k-kL-omega or gamma-Retheta | <=1 |

---

## Step 4: Boundary Conditions

**Read `references/boundary-conds.md` for full BC dictionary templates.**

### Freestream Conditions from Flight Parameters

```python
# Turbulence intensity I = 0.1% to 1% for freestream
# Turbulent length scale L approx 0.07 * hydraulic_diameter (or chord)
k_inlet     = 1.5 * (U_inf * I)**2
omega_inlet = k**0.5 / (C_mu**0.25 * L)   # C_mu = 0.09
nut_inlet   = k / omega                     # eddy viscosity ratio ~1-10
nuTilda_SA  = 3 * nu                        # 3-5x molecular viscosity
```

### Standard Farfield BC Pattern (incompressible)
```
velocity: freestream (inletOutlet) or fixedValue at inlet, zeroGradient at outlet
pressure: zeroGradient at inlet, fixedValue (0 gauge) at outlet
k, omega, nut: freestream conditions at inlet, zeroGradient at outlet
wall: noSlip velocity, zeroGradient pressure, kqRWallFunction/omegaWallFunction
```

---

## Step 5: Solver & Numerical Schemes

**Read `references/numerics.md` for fvSchemes/fvSolution templates.**

### fvSchemes Recommendations for Aerospace

| Term | Scheme | Notes |
|------|--------|-------|
| ddtSchemes | steadyState / Euler / backward | backward for higher-order unsteady |
| gradSchemes | Gauss linear / cellLimited | cellLimited for robustness near walls |
| divSchemes (momentum) | Gauss linearUpwindV grad(U) | or Gauss limitedLinearV for LES |
| divSchemes (turbulence) | Gauss upwind | conservative choice |
| laplacianSchemes | Gauss linear corrected | for non-orthogonal meshes |
| snGradSchemes | corrected | for non-orthogonal meshes |

### Under-Relaxation for Steady Aerospace Cases
```
relaxationFactors
{
    fields     { p 0.3; rho 0.5; }
    equations  { U 0.7; k 0.5; omega 0.5; nuTilda 0.7; }
}
```

### Convergence Criteria
- Residuals < 1e-5 for U, p (production runs)
- Residuals < 1e-4 (engineering accuracy acceptable)
- Always monitor force coefficients (Cl, Cd) — residuals alone are insufficient
- Run >= 2000 iterations for complex cases before declaring convergence

---

## Step 6: Force & Moment Coefficients

**Read `references/post-processing.md` for full postProcessing dictionaries.**

### Forces Function Object (add to controlDict)
```cpp
functions
{
    forces
    {
        type            forceCoeffs;
        libs            ("libforces.so");
        writeControl    timeStep;
        writeInterval   1;
        patches         (wing fuselage);
        rho             rhoInf;
        rhoInf          1.225;
        CofR            (0.25 0 0);
        liftDir         (0 0 1);
        dragDir         (1 0 0);
        pitchAxis       (0 1 0);
        magUInf         50.0;
        lRef            1.0;
        Aref            0.1;
    }
}
```

### Compute liftDir and dragDir for Non-Zero AoA
```
liftDir = (-sin(alpha), 0, cos(alpha))
dragDir = ( cos(alpha), 0, sin(alpha))
```

---

## Step 7: Quality Checks & Validation

### Pre-Run Checklist
- [ ] `checkMesh` passes with no FAILED cells
- [ ] Confirm boundary patch names in `constant/polyMesh/boundary` match BC files
- [ ] Verify units consistency (SI throughout)
- [ ] Initial conditions physically reasonable (not cold-start divergence)

### Validation Against Known Data
| Geometry | Reference Data | Key Metrics |
|----------|---------------|-------------|
| NACA 0012 | NASA TM-4048 | Cl vs alpha, Cd polar |
| RAE 2822 | AGARD AR-138 | Cp distribution, shock location |
| ONERA M6 Wing | AGARD AR-138 | Cp vs x/c |
| Flat plate BL | Blasius / log-law | Cf, velocity profiles, y+ |
| Backward-facing step | Driver & Seegmiller | Reattachment length |

---

## Common Aerospace CFD Mistakes & Fixes

| Problem | Symptom | Fix |
|---------|---------|-----|
| Domain too small | Artificially high Cl/Cd | Extend domain to >=20 chord lengths |
| Wrong y+ | Incorrect skin friction | Re-mesh or switch wall function |
| No prism layers | Excessive diffusion at wall | Add inflation layers in snappyHexMesh |
| Pressure-velocity decoupling | Diverging residuals | Lower relax. factors; use SIMPLEC |
| Upwind-only scheme | Excessive numerical diffusion | Use linear/linearUpwind for div(phi,U) |
| Cold start | Diverges immediately | Patch with setFields or potentialFoam |
| Wrong reference area | Cl/Cd wrong magnitude | Double-check Aref (planform area) |
| Missing forceCoeffs liftDir | Incorrect Cl at AoA | Rotate liftDir/dragDir for AoA |

---

## ISA Atmosphere Quick Reference

| Altitude (m) | T (K) | p (Pa) | rho (kg/m3) | a (m/s) | mu (Pa.s) |
|-------------|-------|--------|-------------|---------|-----------|
| 0 (SL) | 288.15 | 101325 | 1.2250 | 340.3 | 1.789e-5 |
| 1000 | 281.65 | 89876 | 1.1117 | 336.4 | 1.758e-5 |
| 5000 | 255.68 | 54048 | 0.7364 | 320.5 | 1.628e-5 |
| 10000 | 223.25 | 26500 | 0.4135 | 299.5 | 1.458e-5 |
| 11000 | 216.65 | 22632 | 0.3639 | 295.1 | 1.422e-5 |

---

---

## ROTATING MACHINERY — Entry Point

> **Activate when**: propeller, rotor, fan, pump, compressor, turbine, RPM, MRF, AMI,
> tip speed, advance ratio, pressure ratio, blade, impeller, shaft power, thrust, torque.

**Read `references/rotating.md` for full implementation details.**

### Quick Solver Map

| Geometry | Approach | Solver |
|----------|----------|--------|
| Single rotor, steady | MRF (frozen rotor) | `simpleFoam` + MRFProperties |
| Rotor + stator interaction | AMI (sliding mesh) | `pimpleFoam` + dynamicMeshDict |
| Compressor/turbine stage (Ma > 0.3) | MRF + compressible | `rhoSimpleFoam` |
| Wind turbine, full wake | AMI unsteady | `pimpleFoam` + AMI |
| Pump cavitation | Two-phase | `interPhaseChangeFoam` |

**Key metrics**: thrust coefficient CT, power coefficient CP, efficiency η, pressure ratio π, flow coefficient φ.

**AMI time step rule**: `deltaT ≤ (blade_pitch_rad / ω) / 20`

---

## ELECTROMAGNETIC / MHD — Entry Point

> **Activate when**: MHD, magnetohydrodynamics, Hartmann, Lorentz force, plasma actuator,
> DBD, EHD, electrical conductivity, magnetic field, induction, mhdFoam, sigma, B field.

**Read `references/electromagnetic.md` for full implementation details.**

### Quick Physics Map

| Physics | Dimensionless Group | Solver |
|---------|--------------------|---------| 
| Liquid metal MHD | Hartmann `Ha = BL√(σ/μ)` | `mhdFoam` |
| Low-Rm approx (Ha < 100) | `Rm = μ₀σUL << 1` | `simpleFoam` + Lorentz fvOptions |
| DBD plasma actuator | Body force Cμ | `simpleFoam` + vectorSemiImplicitSource |
| Induction heating | Joule heating Q = σ|E|² | `chtMultiRegionFoam` + jouleHeatingSource |

**Hartmann layer meshing**: `y1 < δ_Ha/5 = L/(5*Ha)`

---

## HYPERSONIC — Entry Point

> **Activate when**: Ma > 5, hypersonic, real gas, dissociation, ionisation, ablation,
> stagnation heating, Park chemistry, hy2Foam, dsmcFoam, two-temperature, reentry, shock layer.

**Read `references/hypersonic.md` for full implementation details.**

### Quick Regime Map

| Ma | Regime | Solver | Real Gas |
|----|--------|--------|----------|
| 2–5 | Supersonic | `rhoCentralFoam` | Perfect gas |
| 5–10 | Hypersonic cold | `rhoCentralFoam` + JANAF | Thermally perfect |
| 10–25 | Hypersonic hot | `hy2Foam` / `reactingFoam` | 5-species Park |
| > 25 | Extreme | `hy2Foam` + 2T + radiation | 11-species Park |
| Kn > 0.01 | Rarefied | `dsmcFoam` | DSMC particles |

**Stagnation heating**: Fay-Riddell formula → `q_w ∝ (ρ_s μ_s)^0.5 * (du/ds)^0.5 * (h₀ - h_w)`

**Carbuncle fix**: AUSM+ flux + stagnation region refinement (>20 cells in shock layer)

---

## THERMAL & CONJUGATE HEAT TRANSFER — Entry Point

> **Activate when**: heat transfer, CHT, conjugate, Nusselt, Stanton, film cooling,
> radiation, boiling, melting, solidification, buoyancy, thermal stress, cooling channel,
> heat exchanger, hot gas path, turbine blade cooling, natural convection.

**Read `references/thermal.md` for full implementation details.**

### Quick Problem Map

| Problem | Solver | Key BC |
|---------|--------|--------|
| Forced convection only | `simpleFoam` + energy | `fixedValue T` at wall |
| Natural/mixed convection | `buoyantSimpleFoam` | `fixedValue T`, gravity on |
| CHT (solid + fluid) | `chtMultiRegionSimpleFoam` | `turbulentTemperatureCoupledBaffleMixed` |
| Radiation (participating) | + fvDOM in radiationProperties | `absorptionEmissionModel` |
| Phase change | `icoReactingMultiphaseInterFoam` | Lee model or enthalpy-porosity |

**Film cooling effectiveness**: `η = (T_aw - T_g) / (T_c - T_g)` — extract from `wallHeatFlux` + `wallTemperature`

---

## CUSTOM SOLVER BUILDER — Entry Point

> **Activate when**: "build a solver", "custom OpenFOAM solver", "new physics not in OpenFOAM",
> "wmake", "fvOptions source term", "custom boundary condition", "multi-physics coupling",
> "solver from scratch", "modify existing solver", any request where no standard solver covers
> the required combination of physics.

**Read `references/custom-solver.md` for full implementation details.**

### Agentic Build Loop Summary

```
Design Interview → Module Assembly → Scaffold Generation
→ wmake Compile → Fix Loop → Unit Tests → Benchmark → Document
```

### Pre-Built Solver Templates (instant scaffold)

| Need | Template Solver |
|------|----------------|
| Incompressible + Lorentz force | `mhdSimpleFoam` |
| Reacting flow + MHD | `mhdReactingFoam` |
| Hypersonic + 5-species chemistry + 2T | `hyperReactingFoam` |
| CHT + MRF rotating | `chtRotatingFoam` |
| Hypersonic + ablation + pyrolysis | `ablationFoam` |
| Two-phase + reacting (gas-liquid) | `bubblyReactingFoam` |
| DSMC + dissociation chemistry | `dsmcReactingFoam` |
| MHD + natural convection (liquid metal) | `magneticConvectionFoam` |
| DBD plasma actuator | `plasmaActuatorFoam` |
| Viscoelastic + viscous heating | `viscoelasticHeatFoam` |

The agent runs the full build loop: interview → scaffold → compile → test → benchmark → package.

---

## Reference Files Index

Load these when the task requires deep detail on that topic:

**Core aerospace:**
- **`references/meshing.md`** — blockMesh/snappyHexMesh templates, prism layers, quality metrics
- **`references/turbulence.md`** — Full model setup: k-omega SST, SA, DES, LES, transition models
- **`references/boundary-conds.md`** — Complete BC dictionaries for all field variables
- **`references/solvers.md`** — Solver selection guide with full controlDict templates
- **`references/numerics.md`** — fvSchemes + fvSolution templates per solver class
- **`references/post-processing.md`** — Force coefficients, yPlus, Cp, sampling, ParaView macros
- **`references/agent-loop.md`** — STAR Full AI Agent Loop: scripts, prompts, scoring weights, proven baselines, fix library, skill-update protocol, local/cloud/hybrid setup

**Extended multi-physics (Ph.D.-level):**
- **`references/rotating.md`** — MRF/AMI rotating machinery: propellers, turbomachinery, fans, pumps; thrust/torque/efficiency; fix library
- **`references/electromagnetic.md`** — MHD (mhdFoam, Hartmann, Hunt), DBD plasma actuators, EHD, Lorentz force source terms, induction heating, EM boundary conditions
- **`references/hypersonic.md`** — Ma > 5 real gas, JANAF/Park chemistry, two-temperature model, hy2Foam, dsmcFoam, shock capturing, stagnation heating, Fay-Riddell, ablation BCs
- **`references/thermal.md`** — CHT (chtMultiRegionFoam), buoyancy, radiation (fvDOM/P1/viewFactor), phase change, Nusselt/Stanton post-processing, film cooling effectiveness
- **`references/custom-solver.md`** — Full agentic custom solver builder: design interview, physics module library (MHD, reactions, particles, viscoelastic, porous, EM), scaffold generator, wmake, validation protocol, 12 pre-built solver templates