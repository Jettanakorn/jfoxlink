# AI Agent Loop — Full Reference

This file documents the complete implementation of the autonomous AI Agent Loop embedded in
the `openfoam-aerospace` skill. It covers setup, scripts, scoring, fix library, proven
baselines, and the skill-update protocol.

---

## Table of Contents

1. [Agent Modes: Local / Cloud / Hybrid](#1-agent-modes)
2. [Session Bootstrap](#2-session-bootstrap)
3. [Requirements Intake Schema](#3-requirements-intake-schema)
4. [Parameter Agent — Config Mapping Rules](#4-parameter-agent)
5. [Run Manager — Monitoring Logic](#5-run-manager)
6. [Fix Agent — Diagnosis Library](#6-fix-agent)
7. [Results Evaluator — Extraction Scripts](#7-results-evaluator)
8. [Comparison Engine — Scoring Weights](#8-comparison-engine)
9. [Refinement Planner — Strategy Rules](#9-refinement-planner)
10. [Skill Updater — Knowledge Base Protocol](#10-skill-updater)
11. [Proven Baselines (grows over time)](#11-proven-baselines)
12. [Distribution & Packaging](#12-distribution--packaging)

---

## 1. Agent Modes

### Cloud Mode (default)
Uses the Anthropic Claude API for all reasoning steps (parameter selection, diagnosis,
planning). Requires `api.anthropic.com` network access from the compute node.

```bash
export AGENT_MODE=cloud
export ANTHROPIC_API_KEY=<your-key>     # only if running outside Claude.ai
```

All agent prompts call `claude-sonnet-4-20250514` with `max_tokens=2000`.
Reasoning steps use extended thinking (`budget_tokens=8000`) for diagnosis.

### Local Mode
Uses a locally-served LLM via Ollama for air-gapped HPC environments or sensitive geometry.
Recommended models: `deepseek-r1:14b` (best reasoning), `qwen2.5:14b` (fast).

```bash
export AGENT_MODE=local
export OLLAMA_HOST=http://localhost:11434
export OLLAMA_MODEL=deepseek-r1:14b
ollama serve &
```

### Hybrid Mode
Cloud handles reasoning (parameter selection, diagnosis, planning).
Local handles execution (file writes, log parsing, metrics extraction).

```bash
export AGENT_MODE=hybrid
```

---

## 2. Session Bootstrap

```bash
# scripts/agent-bootstrap.sh
#!/bin/bash
# Run once at start of a new CFD project
set -e

CASE_DIR=${1:-.}
AGENT_MODE=${AGENT_MODE:-cloud}

echo "=== OpenFOAM AI Agent Loop Bootstrap ==="
echo "Case directory: $CASE_DIR"
echo "Agent mode:     $AGENT_MODE"

mkdir -p $CASE_DIR/agent-workspace/{logs,results,plans,fixes}
cp scripts/agent-intake-template.yaml $CASE_DIR/agent-workspace/intake.yaml

echo "Edit agent-workspace/intake.yaml then run: python scripts/agent-run-loop.py $CASE_DIR"
```

```python
# scripts/agent-run-loop.py
# Main entry point for the autonomous agent loop
import yaml, json, os, subprocess, sys
from agent_lib import (
    ParameterAgent, RunManager, FixAgent,
    ResultsEvaluator, ComparisonEngine,
    RefinementPlanner, SkillUpdater
)

def main(case_dir):
    intake = yaml.safe_load(open(f"{case_dir}/agent-workspace/intake.yaml"))
    history = []
    best_score = float("inf")
    best_iter = None

    for iteration in range(1, intake["max_agent_iterations"] + 1):
        print(f"\n=== AGENT ITERATION {iteration}/{intake['max_agent_iterations']} ===")

        # Phase 2: Parameter Agent
        config = ParameterAgent(intake, history).propose(iteration)
        if intake.get("human_in_loop", True):
            confirm = input(f"Approve config for iter {iteration}? [y/n]: ")
            if confirm.lower() != "y":
                print("Skipping iteration.")
                continue

        # Phase 3: Run Manager
        run_result = RunManager(case_dir, config, iteration).run()

        if not run_result["success"]:
            # Phase 4: Fix Agent
            fix = FixAgent(run_result, intake).diagnose_and_fix(config, case_dir)
            run_result = RunManager(case_dir, fix["patched_config"], iteration).run()

        if not run_result["success"]:
            print(f"[WARNING] Iter {iteration} failed after fix attempt. Skipping.")
            continue

        # Phase 5: Results Evaluator
        results = ResultsEvaluator(case_dir, iteration, intake).evaluate()
        history.append({"iter": iteration, "config": config, "results": results})

        # Phase 6: Comparison Engine
        score = ComparisonEngine(history, intake).score(results)
        results["score"] = score
        print(ComparisonEngine(history, intake).render_table())

        if score < best_score:
            best_score = score
            best_iter = iteration

        if score <= 0.05:
            print(f"[AGENT] Target met at iteration {iteration}! Score={score:.4f}")
            break

        # Phase 7: Refinement Planner (only if more iterations remain)
        if iteration < intake["max_agent_iterations"]:
            plan = RefinementPlanner(history, intake).next_plan()
            intake["_plan_override"] = plan  # passed to ParameterAgent next iter

    # Phase 8: Skill Updater
    print(f"\n=== AGENT LOOP COMPLETE. Best iter={best_iter}, score={best_score:.4f} ===")
    SkillUpdater(history, intake, best_iter).update_knowledge_base()
    print("Skill knowledge base updated. Run package_skill.py to distribute.")

if __name__ == "__main__":
    main(sys.argv[1] if len(sys.argv) > 1 else ".")
```

---

## 3. Requirements Intake Schema

```yaml
# agent-intake-template.yaml
# ── Geometry ──────────────────────────────────────────────────
geometry_description: "NACA 0012, chord=1m, span=1m (2D slice)"
geometry_file: ""              # path to STL/OBJ (leave blank for blockMesh cases)
case_class: ""                 # auto-detected if blank; or set manually e.g. "NACA-4digit"

# ── Flow Conditions ───────────────────────────────────────────
altitude_m: 0
mach_number: 0.15
reynolds_number: 3.0e6
alpha_sweep_deg: [0, 4, 8, 12]
freestream_turbulence_intensity: 0.001   # 0.1%

# ── Performance Targets ───────────────────────────────────────
target_Cl: 0.8                 # at alpha = 8 deg
target_Cd_max: 0.015
target_CmPitch: null           # null = not constrained
target_yplus_max: 1.0          # for wall-resolved approach
convergence_residual: 1.0e-5

# ── Agent Settings ────────────────────────────────────────────
max_agent_iterations: 5
human_in_loop: true            # pause for approval before each run
priority: "accuracy"           # "accuracy" | "speed" | "balanced"
hpc_cores: 16
time_budget_hours: 4.0

# ── Scoring Weights (must sum to 1.0) ─────────────────────────
scoring:
  w_Cl:       0.35
  w_Cd:       0.35
  w_yplus:    0.15
  w_residual: 0.15

# ══════════════════════════════════════════════════════════════
# EXTENDED PHYSICS (leave null/false for standard aero cases)
# ══════════════════════════════════════════════════════════════

# ── Rotating Machinery ────────────────────────────────────────
rotating:
  enabled: false
  approach: null              # "MRF" | "AMI"
  omega_rad_s: null           # angular velocity
  axis: [0, 0, 1]
  origin: [0, 0, 0]
  RPM: null                   # alternative to omega_rad_s
  rotating_patches: []        # list of patch names in rotor zone
  target_CT: null             # thrust coefficient
  target_CP_max: null         # power coefficient
  target_eta_min: null        # efficiency
  target_pressure_ratio: null # for turbomachinery

# ── Electromagnetic / MHD ─────────────────────────────────────
electromagnetic:
  enabled: false
  model: null                 # "low-Rm-MHD" | "full-MHD" | "DBD-actuator" | "EHD" | "induction-heating"
  sigma_S_m: null             # electrical conductivity
  B0_T: null                  # applied magnetic field magnitude
  B0_direction: [0, 1, 0]     # direction of applied B
  wall_conductivity: "insulating"  # "insulating" | "conducting"
  actuator_voltage_kV: null   # for DBD
  actuator_frequency_Hz: null

# ── Hypersonic ────────────────────────────────────────────────
hypersonic:
  enabled: false
  real_gas: false
  chemistry: null             # "none" | "5-species-Park" | "11-species-Park"
  two_temperature: false
  rarefied: false             # if Kn > 0.01, switches to dsmcFoam
  T_wall_K: null              # null = adiabatic
  wall_catalysis: "noncatalytic"   # "noncatalytic" | "fully_catalytic"
  target_CD: null
  target_peak_heat_flux_W_m2: null

# ── Thermal / CHT ─────────────────────────────────────────────
thermal:
  enabled: false
  problem_type: null          # "forced_convection" | "CHT" | "natural_convection" | "radiation"
  fluid: "air"
  solid_material: null        # e.g. "steel", "aluminum", "ceramic" — for CHT
  T_inlet_K: null
  T_wall_K: null
  T_ambient_K: 300
  heat_flux_W_m2: null
  radiation: false
  radiation_model: "fvDOM"    # "fvDOM" | "P1" | "viewFactor"
  boiling: false
  max_T_solid_K: null         # design limit — agent flags if exceeded
  target_Nu: null
  target_effectiveness: null  # for film cooling

# ── Custom Solver ─────────────────────────────────────────────
custom_solver:
  build_new: false
  solver_name: null           # e.g. "mhdReactingFoam"
  base_solver: null           # closest existing solver
  physics_modules: []         # from custom-solver.md §4 module library
  coupling_strategy: null     # "segregated-SIMPLE" | "segregated-PISO" | "operator-split"
  time_treatment: "steady"
  validation_case: null
```

---

## 4. Parameter Agent — Config Mapping Rules

The Parameter Agent converts the intake into a full OpenFOAM configuration.
Rules are applied in order; later rules can override earlier ones.

```
RULE P1 — Solver Selection
  Ma < 0.3  → simpleFoam (steady) or pimpleFoam (unsteady)
  0.3-0.8   → rhoSimpleFoam / rhoPimpleFoam
  0.8-1.2   → rhoCentralFoam (AUSM+ flux)
  Ma > 1.2  → rhoCentralFoam + sonicFoam fallback
  Ma > 5.0  → rhoCentralFoam + real gas EOS (JANAF)
  Ma > 8.0 + chemistry → hy2Foam or reactingFoam
  Kn > 0.01 → dsmcFoam (rarefied, override all above)

RULE P1b — Extended Physics Solver Override
  rotating.enabled=true + AMI  → base solver + dynamicMeshDict (pimpleFoam)
  rotating.enabled=true + MRF  → base solver + MRFProperties (simpleFoam)
  electromagnetic.model=low-Rm-MHD → mhdFoam (incompressible) or custom Lorentz fvOptions
  electromagnetic.model=DBD-actuator → base solver + vectorSemiImplicitSource fvOptions
  thermal.problem_type=CHT     → chtMultiRegionSimpleFoam or chtMultiRegionFoam
  thermal.problem_type=natural_convection → buoyantSimpleFoam
  custom_solver.build_new=true → STOP: read references/custom-solver.md, run build loop

RULE P2 — Turbulence Model
  Re > 1e6, attached flow, priority=speed  → Spalart-Allmaras
  Re > 1e6, attached flow, priority=accuracy → k-omega SST (low-Re)
  Separated flow or adverse pressure gradient → k-omega SST (low-Re)
  Ma > 0.5 → k-omega SST (compressible formulation)
  Wake/noise required → LES WALE (pimpleFoam)
  rotating.enabled=true → k-omega SST preferred (handles adverse dp/dx in blade BL)
  hypersonic.enabled=true + Ma > 8 → Disable RANS; use laminar or 2-eq in shock layer

RULE P3 — Mesh Density
  priority=speed  → coarse: ~50k cells (2D), ~2M cells (3D)
  priority=balanced → medium: ~150k cells (2D), ~8M cells (3D)
  priority=accuracy → fine: ~400k cells (2D), ~25M cells (3D)
  Re > 5e6 or Ma > 0.5 → bump one density level up
  electromagnetic + Ha > 10 → add Hartmann layer cells: y1 < L/(5*Ha)
  hypersonic.enabled=true → add shock refinement box; 3–5 cells across shock; stagnation y1 < 1e-5m
  rotating.AMI=true → add 5-cell refinement in tip clearance gap

RULE P4 — First Cell Height (y1)
  target_yplus_max <= 1 →  y1 = (target_yplus * nu) / u_tau
                            u_tau = U_inf * sqrt(0.5 * Cf)
                            Cf = 0.026 / Re^(1/7)  [turbulent flat plate estimate]
  target_yplus 30-300  →  y1 = 30 * nu / u_tau
  MHD Hartmann layer  →  y1 < δ_Ha / 5 = L / (5 * Ha)  [OVERRIDES standard y+ rule]
  thermal CHT + Pr > 5 →  y1 such that y+ <= 1 [MANDATORY for high-Pr heat transfer]

RULE P5 — Relaxation Factors
  priority=speed    → U:0.8, p:0.4, k:0.6, omega:0.6
  priority=balanced → U:0.7, p:0.3, k:0.5, omega:0.5
  priority=accuracy → U:0.5, p:0.2, k:0.4, omega:0.4
  If iter > 1 and previous run diverged → reduce all by 0.1
  CHT: add T relaxation 0.5 in both solid and fluid fvSolution
  electromagnetic: add phi_E relaxation 0.7

RULE P6 — Numerical Schemes
  Ma < 0.3 → Gauss linearUpwindV for div(phi,U); Gauss upwind for turbulence
  Ma > 0.5 → Gauss limitedLinear for all; van Leer flux limiter
  LES      → Gauss filteredLinear2 for div(phi,U)
```

---

## 5. Run Manager — Monitoring Logic

```python
# agent_lib/run_manager.py (key methods)

DIVERGENCE_THRESHOLD = 1e3     # residual spike = divergence
PLATEAU_WINDOW       = 300     # iterations without residual drop = stall
CFL_LIMIT            = 10.0    # for unsteady cases
FORCE_OSCILLATION_WINDOW = 100 # iterations to check Cl/Cd oscillation

class RunManager:
    def monitor_log(self, log_path):
        for line in tail_file(log_path):
            if "FOAM FATAL ERROR" in line or "Floating point" in line:
                return {"status": "crash", "line": line}
            res = parse_residual(line)
            if res and res["value"] > DIVERGENCE_THRESHOLD:
                return {"status": "divergence", "field": res["field"]}
            if self.iterations_since_improvement() > PLATEAU_WINDOW:
                return {"status": "stall"}
            if self.is_unsteady and self.current_Co() > CFL_LIMIT:
                return {"status": "high_cfl", "Co": self.current_Co()}
        return {"status": "running"}
```

---

## 6. Fix Agent — Diagnosis Library

```
FIX F1 — Divergence at iter < 50 (cold start)
  Cause: Initial field far from solution
  Fix:   1. Run potentialFoam to initialise p, U
         2. Lower relaxation: U→0.5, p→0.2
         3. Set nNonOrthogonalCorrectors 2

FIX F2 — Residual plateau (stall)
  Cause: Under-relaxation too aggressive or mesh quality
  Fix:   1. Switch simpleFoam to SIMPLEC (consistent=yes)
         2. Increase nNonOrthogonalCorrectors 1→3
         3. If plateau persists, enable linearUpwind (from upwind)

FIX F3 — Floating point exception
  Cause: Non-orthogonal faces or zero-gradient BC mismatch
  Fix:   1. Add nNonOrthogonalCorrectors 3
         2. Verify all patch names in 0/ match boundary file
         3. Set limiters: limitedLinear 1 for div schemes

FIX F4 — Negative k / omega
  Cause: Turbulent BC init too low or incompatible wall function
  Fix:   1. Re-derive k/omega from intake using Rule P4 formulas
         2. Set lowerBound for k: 1e-15; for omega: 1e-6
         3. Ensure nutWallFunction not mixed with low-Re cells

FIX F5 — High CFL (unsteady)
  Cause: Time step too large
  Fix:   1. Halve deltaT
         2. Set adjustTimeStep yes; maxCo 0.8 in controlDict

FIX F6 — checkMesh FAILED cells
  Cause: snappyHexMesh over-aggressive refinement or bad STL
  Fix:   1. Reduce maxNonOrtho from 70 to 65 in snappyHexMesh
         2. Add one snappy refinement level in wake region
         3. Re-run surfaceFeatureExtract and snappy

FIX F7 — y+ out of target range
  Cause: First cell height y1 incorrect
  Fix:   1. Recompute y1 from Rule P4 using actual wall shear
             (use yPlus from previous run if available)
         2. Rebuild prism layer in snappyHexMesh
         3. If y+ too high by factor >3: switch to wall functions

FIX F8 — AMI non-matching faces / interface warning (ROTATING)
  Cause: Rotor-stator interface mesh not sufficiently conformal
  Fix:   1. Increase matchTolerance in cyclicAMI patch to 0.005
         2. Ensure inner and outer cylinder radii are identical in blockMesh
         3. Re-run snappyHexMesh with conformal interface layer

FIX F9 — mhdFoam diverges at high Hartmann number (EM/MHD)
  Cause: Hartmann boundary layer unresolved (y1 too large)
  Fix:   1. Compute δ_Ha = L/Ha; set y1 < δ_Ha/5
         2. Rebuild mesh with geometric progression toward Hartmann walls
         3. Lower relaxation factors for B field to 0.5

FIX F10 — Bow shock carbuncle instability (HYPERSONIC)
  Cause: Kurganov flux on mesh aligned with normal shock
  Fix:   1. Switch fluxScheme to AUSM+
         2. Refine stagnation region: >= 20 cells in shock layer
         3. Add limitedLinear 1 to div(phi,U)

FIX F11 — Temperature blow-up in solid (CHT)
  Cause: kappa too low or solid region not fully enclosed
  Fix:   1. Verify kappa units in solid thermophysicalProperties: W/(m·K)
         2. Confirm solid mesh boundary is closed; no open patches
         3. Reduce T relaxation to 0.3 in solid fvSolution

FIX F12 — CHT interface T discontinuity
  Cause: Incorrect coupled BC type on interface patches
  Fix:   1. Verify BOTH sides use turbulentTemperatureCoupledBaffleMixed
         2. Check kappaMethod: fluidThermo (fluid) vs solidThermo (solid)
         3. Ensure interface patches are 1:1 conformal (no AMI unless specified)

FIX F13 — Chemistry ODE timeout (HYPERSONIC / REACTING)
  Cause: Stiff reactions near shock or cold-wall recombination zone
  Fix:   1. Switch chemistrySolver to SIBS or Rosenbrock (not EulerImplicit)
         2. Set maxDeltaT in chemistryProperties to 1e-9 near shock
         3. Consider partitioned chemistry: solve in hot cells only

FIX F14 — Custom solver compile error (CUSTOM SOLVER)
  Cause: Missing library link or include path
  Fix:   1. Parse linker error: identify undefined symbol → find owning library
         2. Add -l<library> to Make/options EXE_LIBS
         3. Add -I$(LIB_SRC)/<module>/lnInclude to EXE_INC
         4. Run: wclean && wmake; verify binary in $FOAM_USER_APPBIN
```

---

## 7. Results Evaluator — Extraction Scripts

```bash
# scripts/agent-extract-results.sh
CASE=$1
ITER=$2
OUTFILE="agent-workspace/results/iter_${ITER}.json"

# Extract force coefficients (last 100 iter average)
Cl=$(foamPostProcess -func forceCoeffs -case $CASE | awk '/Cl/{sum+=$NF;n++} END{print sum/n}')
Cd=$(foamPostProcess -func forceCoeffs -case $CASE | awk '/Cd/{sum+=$NF;n++} END{print sum/n}')

# Extract y+ statistics
yplus_max=$(postProcess -func yPlus -case $CASE -latestTime 2>&1 | grep "max" | awk '{print $NF}')
yplus_mean=$(postProcess -func yPlus -case $CASE -latestTime 2>&1 | grep "mean" | awk '{print $NF}')

# Extract final residuals
res_p=$(tail -200 $CASE/logs/iter_${ITER}.log | grep "Solving for p" | tail -1 | awk -F'residual = ' '{print $2}' | awk '{print $1}' | tr -d ',')

# Mesh statistics
cells=$(checkMesh -case $CASE 2>&1 | grep "cells:" | awk '{print $2}')
walltime=$(grep "ExecutionTime" $CASE/logs/iter_${ITER}.log | tail -1 | awk '{print $3}')

cat > $OUTFILE << EOF
{
  "iter": $ITER, "Cl": $Cl, "Cd": $Cd,
  "yplus_max": $yplus_max, "yplus_mean": $yplus_mean,
  "residual_p": $res_p, "mesh_cells": $cells, "wall_time_s": $walltime
}
EOF
echo "Results saved to $OUTFILE"
```

---

## 8. Comparison Engine — Scoring Weights

Default weights (from intake.yaml `scoring` block):

| Metric | Default Weight | Penalty Function |
|--------|---------------|-----------------|
| Cl error | 0.35 | \|Cl - target_Cl\| / target_Cl |
| Cd excess | 0.35 | max(0, Cd - target_Cd_max) / target_Cd_max |
| y+ violation | 0.15 | 0 if in range; (y+/target)^0.5 - 1 if over |
| Residual | 0.15 | log10(residual_p) / log10(target_residual) |

**Grid Convergence Index (GCI)** is computed when 3+ mesh levels exist:
```
r = (N_fine / N_coarse)^(1/3)           # mesh refinement ratio
p_order = log((f3-f2)/(f2-f1)) / log(r) # observed order of convergence
GCI_fine = 1.25 * |e21| / (r^p - 1)    # grid convergence index
```
Results with GCI < 5% are flagged as mesh-converged.

---

## 9. Refinement Planner — Strategy Rules

```
STRATEGY S1 — Poor Cl (score dominated by Cl error)
  → Try: increase mesh refinement by 1 level (better wake resolution)
  → Or:  switch turbulence model (kOmSST → SA or vice versa)
  → Or:  add angle-of-attack refinement box (AoA > 8 deg)

STRATEGY S2 — Poor Cd (score dominated by Cd excess)
  → Try: increase prism layer count by 2 (better boundary layer)
  → Or:  lower first cell height (target y+ 0.5 instead of 1.0)
  → Or:  check if domain is < 20 chord — extend it

STRATEGY S3 — Poor y+ (wrong range)
  → Recompute y1 from actual wall shear stress (from last run yPlus field)
  → Adjust expansion ratio of prism layers (1.2 → 1.15 for finer near-wall)

STRATEGY S4 — Stagnation (score not improving over 2 iterations)
  → Reset to medium mesh + kOmSST (known-good baseline)
  → Then try 1 parameter change at a time

STRATEGY S5 — Diminishing returns (score < 0.05 but not < 0.02)
  → Run with fine mesh + same turbulence model
  → Accept if GCI < 5%
```

---

## 10. Skill Updater — Knowledge Base Protocol

When the agent loop completes, the Skill Updater appends a new entry to
`references/agent-knowledge-base.json` using this schema:

```json
{
  "case_class": "NACA-4digit-subsonic-incompressible",
  "Re_range": [1000000, 5000000],
  "Ma_range": [0.0, 0.25],
  "date_added": "2025-05-02",
  "proven_config": {
    "solver": "simpleFoam",
    "turbulence": "SpalartAllmaras",
    "mesh_density": "medium-snappy",
    "prism_layers": 17,
    "first_cell_y1_m": 0.000025,
    "relax_U": 0.7,
    "relax_p": 0.3,
    "schemes_div_U": "Gauss linearUpwindV grad(U)",
    "iterations_to_converge": 1850,
    "wall_time_per_kiter_s": 42
  },
  "achieved_score": 0.019,
  "achieved_Cl": 0.797,
  "achieved_Cd": 0.0151,
  "achieved_yplus_max": 0.98,
  "fix_history": [
    {"iter": 1, "symptom": "stall", "fix": "F2", "action": "SIMPLEC + nNonOrth 3"}
  ],
  "GCI_percent": 2.3,
  "validation_refs": ["NASA TM-4048"]
}
```

The Skill Updater also appends a one-line summary to Section 11 (Proven Baselines) of
this file for quick human-readable lookup.

### Knowledge Base Lookup (used by Parameter Agent)

When the Parameter Agent starts a new session, it queries `agent-knowledge-base.json`
for any entry whose `case_class`, `Re_range`, and `Ma_range` overlap with the intake.
If found, it uses `proven_config` as the starting point for iteration 1 instead of
deriving from scratch — dramatically reducing iterations needed.

---

## 11. Proven Baselines

*(This section is auto-appended by the Skill Updater after each completed agent loop)*

| Date | Case Class | Re | Ma | Best Config | Score | Cl | Cd |
|------|-----------|----|----|-------------|-------|----|----|
| *(none yet — first entry will appear after your first run)* | | | | | | | |

---

## 12. Distribution & Packaging

After the agent loop completes and the knowledge base is updated:

```bash
# Package the entire updated skill (includes agent-knowledge-base.json)
python scripts/package_skill.py /path/to/openfoam-aerospace

# This produces: openfoam-aerospace.skill
# Share with team or copy to HPC cluster node
# Any Claude instance loading this .skill file will have:
#   - All original CFD knowledge
#   - All proven baselines accumulated so far
#   - All fix history for faster future diagnosis
```

### HPC Cluster Deployment

```bash
# On each compute node (or shared filesystem):
cp openfoam-aerospace.skill ~/.claude/skills/
# Claude on that node will automatically pick up the skill

# For air-gapped HPC with local agent mode:
export AGENT_MODE=local
export OLLAMA_HOST=http://<head-node-ip>:11434
# Ollama runs on head node; worker nodes route reasoning through it
```

### Version Control

Commit `references/agent-knowledge-base.json` to your project git repository.
This creates a shared, versioned record of all proven CFD configurations for the project,
enabling reproducibility and team knowledge sharing.