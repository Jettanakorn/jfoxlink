# Rotating Machinery — Full Reference

## Table of Contents
1. [Physics & Governing Equations](#1-physics)
2. [Solver Selection](#2-solver-selection)
3. [MRF (Multiple Reference Frame)](#3-mrf)
4. [Sliding Mesh (AMI)](#4-sliding-mesh-ami)
5. [Propeller / Rotor Setup](#5-propeller--rotor-setup)
6. [Turbomachinery (Compressor / Turbine)](#6-turbomachinery)
7. [Fan & Pump Cases](#7-fan--pump)
8. [Boundary Conditions for Rotating Zones](#8-boundary-conditions)
9. [Post-Processing: Thrust, Torque, Efficiency](#9-post-processing)
10. [Fix Library for Rotating Cases](#10-fix-library)

---

## 1. Physics

Rotating reference frame adds Coriolis and centrifugal source terms to N-S:

```
∂(ρU)/∂t + ∇·(ρUU) + 2ρ(Ω×U) + ρΩ×(Ω×r) = -∇p + ∇·τ
```

- `Ω` — angular velocity vector (rad/s)
- `2ρ(Ω×U)` — Coriolis force
- `ρΩ×(Ω×r)` — centrifugal force
- `r` — position vector from rotation axis

**Tip speed ratio (TSR)** for wind turbines / propellers:
```
TSR = Ω * R / U_inf
```

**Advance ratio J** (propellers):
```
J = V_inf / (n * D)    ; n = rps, D = diameter
```

---

## 2. Solver Selection

| Case | Solver | Notes |
|------|--------|-------|
| Steady MRF (fan, impeller) | `simpleFoam` + MRF | Fast; no time dependency |
| Transient AMI rotor | `pimpleFoam` + AMI | Captures blade-to-blade interaction |
| Compressible turbomachine | `rhoSimpleFoam` + MRF | Ma > 0.3 stages |
| Transient compressible | `rhoPimpleFoam` + AMI | Full unsteady compressible |
| Wind turbine, full unsteady | `pimpleFoam` + AMI | Tower shadow, wake |
| Pump (cavitation) | `interPhaseChangeFoam` | Two-phase |

---

## 3. MRF (Multiple Reference Frame)

MRF is the fastest approach — no mesh motion. The rotating zone is treated in a
rotating reference frame while the rest stays stationary.

```cpp
// constant/MRFProperties
MRF1
{
    cellZone        rotatingZone;     // volume cell zone encompassing rotor
    active          yes;
    nonRotatingPatches ();            // patches that DON'T rotate (e.g. hub if separate)
    origin          (0 0 0);
    axis            (0 0 1);          // rotation axis (here: Z)
    omega           [0 0 -1 0 1 0 0] 104.72;  // rad/s (= 1000 RPM)
}
```

**MRF checklist:**
- [ ] `rotatingZone` cell set created in `topoSetDict`
- [ ] MRF zone boundary patches defined as `cyclicAMI` if periodic, else `wall`
- [ ] Interface between rotating/stationary zones must be a conformal internal face set
- [ ] omega correctly signed (right-hand rule around axis vector)
- [ ] For incompressible: use `SRFSimpleFoam` for single rotating frame cases

---

## 4. Sliding Mesh (AMI)

AMI (Arbitrary Mesh Interface) allows physically rotating mesh regions. More
accurate than MRF for cases with strong rotor-stator interaction.

```cpp
// system/blockMeshDict — create inner (rotor) and outer (stator) cylinders
// Inner cylinder: rotatingPatch (cyclic AMI pair)
// Outer cylinder: stationaryPatch (cyclic AMI pair)

// constant/dynamicMeshDict
dynamicFvMesh   dynamicMotionSolverFvMesh;

motionSolverLibs ("libfvMotionSolvers.so");

solver          solidBody;
solidBodyMotionFunction  rotatingMotion;
rotatingMotionCoeffs
{
    origin      (0 0 0);
    axis        (0 0 1);
    omega       104.72;     // rad/s
}
```

**AMI patch definition in boundary file:**
```cpp
rotorAMI
{
    type            cyclicAMI;
    matchTolerance  0.0001;
    neighbourPatch  statorAMI;
    transform       noOrdering;
}
```

**AMI time step rule:**
```
deltaT <= (blade_pitch_angle_rad / omega) / 20   ; 20 steps per blade passage min
```

---

## 5. Propeller / Rotor Setup

### Intake Parameters

```yaml
propeller:
  diameter_m: 0.3
  blades: 3
  RPM: 5000
  advance_ratio_J: 0.6       # V_inf / (n * D)
  pitch_angle_deg: 15
  airfoil_sections: [NACA4412, NACA4412]
  hub_diameter_m: 0.06
target_CT: 0.12              # thrust coefficient
target_CP_max: 0.08          # power coefficient
target_eta_min: 0.75         # propulsive efficiency
```

### Thrust & Torque Coefficients

```python
n  = RPM / 60            # rps
CT = T  / (rho * n**2 * D**4)
CQ = Q  / (rho * n**2 * D**5)
CP = 2*pi * CQ
eta = CT * J / CP        # propulsive efficiency
```

### Forces Function Object for Propellers

```cpp
propellerForces
{
    type          forceCoeffs;
    libs          ("libforces.so");
    patches       (blade1 blade2 blade3);
    rho           rhoInf;
    rhoInf        1.225;
    CofR          (0 0 0);
    liftDir       (0 0 1);    // thrust direction = rotation axis
    dragDir       (1 0 0);    // torque resolved direction
    pitchAxis     (0 1 0);
    magUInf       50.0;
    lRef          0.3;        // diameter
    Aref          0.0707;     // pi*D^2/4
}
```

---

## 6. Turbomachinery

### Compressor Stage Setup

```yaml
compressor:
  type: "axial"           # or "centrifugal"
  stages: 3
  inlet_total_pressure_Pa: 101325
  inlet_total_temp_K: 288.15
  pressure_ratio_target: 3.2
  mass_flow_kg_s: 10.0
  rotational_speed_RPM: 15000
  tip_clearance_m: 0.0005
```

**Boundary conditions for compressor inlet/outlet:**
```cpp
// 0/p (total pressure inlet)
inlet { type totalPressure; p0 uniform 101325; }

// 0/T
inlet { type totalTemperature; T0 uniform 288.15; }

// 0/p (static pressure outlet)
outlet { type fixedValue; value uniform 323000; }   // target PR=3.2 * 101325 * (~approx)
```

**Non-dimensionalization:**
```
ψ = ΔH0 / (U_tip²)              // head coefficient
φ = Cm / U_tip                   // flow coefficient
η_tt = ψ_ideal / ψ_actual        // total-to-total efficiency
```

### Performance Map Generation (Agent Script)
```python
# Sweep flow coefficient φ for each RPM:
for RPM in [12000, 13500, 15000, 16500]:
    for mdot in linspace(0.7*mdot_design, 1.1*mdot_design, 8):
        run_case(RPM, mdot)
        extract_PR_eta()
# Agent auto-detects surge line (dp/dmdot > 0 → unstable)
```

---

## 7. Fan & Pump

**Fan**: Low-pressure ratio, incompressible or weakly compressible.
- Solver: `simpleFoam` + MRF (steady) or `pimpleFoam` + AMI (transient)
- Key metric: fan total pressure rise ΔPt, flow rate Q, efficiency η

**Pump** (hydraulic):
- Solver: `simpleFoam` (incompressible, single-phase) or `interPhaseChangeFoam` (cavitation)
- Dimensionless groups: `ψ = ΔH / (n²D²)`, `φ = Q / (nD³)`, `σ = NPSH / H` (cavitation number)

```cpp
// Pump performance extraction (postProcessing)
functions
{
    pressureRise
    {
        type            fieldValueDelta;
        libs            ("libfieldFunctionObjects.so");
        operation       subtract;
        region1         { type patch; patches (outlet); fields (p); }
        region2         { type patch; patches (inlet); fields (p); }
    }
}
```

---

## 8. Boundary Conditions for Rotating Zones

### Wall (rotating solid boundary)
```cpp
// For MRF: wall velocity is ZERO in absolute frame but rotates in relative frame
// Use movingWallVelocity only if mesh actually moves (AMI)

walls_MRF   { type  fixedValue;        value uniform (0 0 0); }  // stationary walls
walls_AMI   { type  movingWallVelocity; value uniform (0 0 0); }  // moving mesh walls
rotor_hub   { type  rotatingWallVelocity; origin (0 0 0); axis (0 0 1); omega 104.72; }
```

### Periodic (cyclic sector)
```cpp
periodic1
{
    type        cyclicAMI;
    neighbourPatch periodic2;
    transform   rotational;
    rotationAxis (0 0 1);
    rotationCentre (0 0 0);
    matchTolerance 0.001;
}
```

---

## 9. Post-Processing: Thrust, Torque, Efficiency

```bash
# Extract thrust (axial force) and torque from force function objects
postProcess -func forces -case . -latestTime

# Parse forceCoeffs output:
python3 << 'EOF'
import numpy as np
data = np.loadtxt("postProcessing/forces/0/forceCoeffs.dat", comments='#')
# columns: time Cd Cl CmRoll CmPitch CmYaw
CT = data[-100:, 2].mean()   # Cl column = thrust direction
CP = data[-100:, 1].mean() * 2 * np.pi   # Cd → torque → CP
J  = 0.6   # from intake
eta = CT * J / CP
print(f"CT={CT:.4f}  CP={CP:.4f}  eta={eta:.3f}")
EOF
```

---

## 10. Fix Library for Rotating Cases

| Symptom | Cause | Fix |
|---------|-------|-----|
| AMI non-matching faces warning | Rotor/stator interface not conformal | Increase `matchTolerance` to 0.005 |
| MRF divergence near interface | Cell zone boundary not flush with faces | Rebuild `topoSet` with tighter face matching |
| Negative p near blade tip | Tip vortex → pressure undershoot | Refine mesh in tip clearance region; add tip box |
| Oscillating Cd / CT | Under-resolved blade wake | Refine wake cell zone; reduce deltaT (AMI) |
| `omega` in turbulence near wall → 0 | Missing `omegaWallFunction` on rotating patches | Add `omegaWallFunction` to all wall patches in MRF zone |
| Thrust over-prediction | MRF interface leakage (non-flush) | Ensure MRF zone boundary exactly coincides with internal faces |
| Convergence plateau > 500 iter | MRF + SIMPLE needs frozen rotor BCs | Switch from SRF to MRF formulation or use periodic sector |