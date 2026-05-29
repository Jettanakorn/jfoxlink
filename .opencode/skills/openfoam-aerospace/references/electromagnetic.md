# Electromagnetic CFD — Full Reference

## Table of Contents
1. [Physics: MHD, Plasma, EHD](#1-physics)
2. [OpenFOAM EM Solvers & Libraries](#2-solvers--libraries)
3. [MHD (Magnetohydrodynamics)](#3-mhd)
4. [Plasma Actuators (DBD)](#4-plasma-actuators-dbd)
5. [Electrohydrodynamic (EHD) Flow Control](#5-ehd-flow-control)
6. [Induction Heating & Electromagnetic Forming](#6-induction-heating)
7. [Lorentz Force Body Terms in N-S](#7-lorentz-force-body-terms)
8. [Boundary Conditions](#8-boundary-conditions)
9. [Validation References](#9-validation)
10. [Fix Library](#10-fix-library)

---

## 1. Physics

### MHD — Governing Equations

**Navier-Stokes with Lorentz body force:**
```
ρ(∂U/∂t + U·∇U) = -∇p + μ∇²U + J×B
J = σ(E + U×B)   ; Ohm's law in moving conductor
∂B/∂t = ∇×(U×B) + (1/σμ₀)∇²B   ; induction equation
∇·B = 0
```

**Magnetic Reynolds number:**
```
Rm = μ₀ σ U L
```
- `Rm << 1`: induced B negligible → low-Rm approximation (most liquid metal flows)
- `Rm ~ 1`: full induction equation required

**Hartmann number:**
```
Ha = B L √(σ / μ)   ; ratio of EM to viscous forces
```
- `Ha > 1`: EM effects dominant → Hartmann layers form on walls ⊥ to B

### DBD Plasma Actuator (phenomenological body force model)
```
f_body = C * ρ_charge * E      ; Shyy/Jayaraman model
ρ_charge = f(voltage, frequency, gap geometry)
E = -∇φ   ; from Poisson equation
```

---

## 2. Solvers & Libraries

| Domain | Solver / Library | Notes |
|--------|-----------------|-------|
| Low-Rm MHD | `mhdFoam` (OpenFOAM contrib) | Built-in; steady/unsteady |
| Full MHD | `magneticFoam` / custom | Requires induction eq. |
| Plasma actuator | `simpleFoam` + body force UDF | Body force from model |
| Electric arc / discharge | `chtMultiRegionFoam` + EM | Coupled thermal + EM |
| Electromagnetic heating | `electricalFoam` (FEniCS or custom OF) | Joule heating |
| Hall thruster plasma | `reactingFoam` + EM coupling | Needs `plasmaFoam` or custom |

### Enabling `mhdFoam` in OpenFOAM
```bash
# Available in OpenFOAM v2306+ in tutorials/electromagnetics/mhdFoam
# Requires: constant/transportProperties with sigma (electrical conductivity)
# and constant/physicalProperties with mu (permeability)
```

---

## 3. MHD

### Case Setup for Hartmann Channel Flow

```yaml
mhd_intake:
  geometry: "rectangular duct, 2H x 2H x 10H"
  fluid: "liquid sodium"          # or mercury, liquid steel
  sigma_S_m: 1.0e7                # electrical conductivity
  mu_Pa_s: 0.000234               # dynamic viscosity (liquid Na at 400K)
  rho_kg_m3: 920                  # density
  B0_T: 0.1                       # applied magnetic field (Tesla)
  Re: 5000
  Hartmann_Ha: null               # auto-computed
  target_velocity_profile: "Hartmann flattening expected"
```

### transportProperties for mhdFoam
```cpp
// constant/transportProperties
transportModel  Newtonian;
nu              nu [0 2 -1 0 0 0 0] 2.54e-7;   // kinematic viscosity

// EM properties
sigma           sigma [−1 -3 3 0 0 2 0] 1.0e7;  // electrical conductivity S/m
mu              mu [1 1 -2 0 0 -2 0] 1.2566e-6; // magnetic permeability H/m
```

### Key MHD Boundary Conditions
```cpp
// 0/U
walls { type fixedValue; value uniform (0 0 0); }

// 0/B (magnetic field)
walls_conducting  { type  fixedValue; value uniform (0 0.1 0); }   // conducting wall
walls_insulating  { type  zeroGradient; }                            // insulating wall

// 0/E (electric potential — if solving φ)
walls_conducting  { type  fixedValue; value uniform 0; }
walls_insulating  { type  zeroGradient; }
```

### Hartmann Layer Meshing
```
Hartmann layer thickness: δ_Ha ≈ L / Ha
First cell height target: y1 < δ_Ha / 5
For Ha=100, L=0.01m: δ_Ha = 0.0001m → y1 < 0.00002m
```

---

## 4. Plasma Actuators (DBD)

DBD (Dielectric Barrier Discharge) actuators add a body force to the near-wall
flow to delay separation or augment boundary layer.

### Shyy–Jayaraman Phenomenological Model

```cpp
// Custom fvOption source term in fvOptions:
bodyForceModel
{
    type            vectorSemiImplicitSource;
    volumeMode      absolute;
    selectionMode   cellZone;
    cellZone        actuatorZone;     // thin zone over actuator
    injectionRateSuSp
    {
        U (( 0.5 0 0 ) 0);           // body force vector (N/m³), tune from experiment
    }
}
```

**Spatially varying body force (more physical):**
```python
# Precompute body force field using Jayaraman model, write to 0/bodyForce as volVectorField
# Model: f_x(y) = C1 * exp(-C2 * y) * cos(theta)
# Typical C1 ~ 1-5 kN/m³, exponential decay over ~ 1mm from surface
# Write via foamDictionary or Python using fluidfoam library
```

---

## 5. EHD Flow Control

Electrohydrodynamic (space-charge-induced) flow for drag reduction / cooling.

```yaml
ehd_intake:
  electrode_gap_mm: 10
  voltage_kV: 20
  frequency_Hz: 5000
  fluid: "air"
  target_induced_velocity_m_s: 5.0
```

**Solver approach:**
1. Solve Laplace equation for electric field: `∇²φ = -ρ_e/ε`
2. Compute charge density `ρ_e` from corona model (empirical)
3. Inject body force `f = ρ_e * E` into N-S as `fvOptions` source
4. Iterate until coupled convergence

---

## 6. Induction Heating

Used for metal processing, plasma-assisted combustion pre-heating.

```cpp
// Joule heating source term added to energy equation:
// Q_joule = J·E = σ |E|²

// In chtMultiRegionFoam solid region:
// Add custom fvModel or use builtIn jouleHeatingSource if available (OF v2106+)
jouleHeating
{
    type            jouleHeatingSource;
    libs            ("libelectromagneticModels.so");
    selectionMode   all;
    sigma           1.0e7;          // or field sigma if non-uniform
}
```

---

## 7. Lorentz Force Body Terms in N-S

For **low-Rm MHD** (most practical cases), B is prescribed (not solved):

```cpp
// fvOptions — Lorentz force as explicit source
lorentzForce
{
    type            vectorExplicitSource;
    volumeMode      absolute;
    selectionMode   all;

    // J×B = σ(U×B)×B = σ[(U·B)B - B²U]  → linearise as -σB²U + σ(U·B)B
    // OpenFOAM mhdFoam handles this internally
    // For custom solver, add to UEqn:
    // UEqn += fvm::SuSp(-sigma * magSqr(B), U) + sigma * (U & B) * B;
}
```

---

## 8. Boundary Conditions

| Field | Conducting Wall | Insulating Wall | Inlet | Outlet |
|-------|----------------|-----------------|-------|--------|
| U | no-slip | no-slip | fixedValue | zeroGradient |
| B | fixedValue (applied B) | zeroGradient | fixedValue | zeroGradient |
| φ (electric) | fixedValue 0 | zeroGradient | — | — |
| j (current density) | calculated | zeroGradient | — | — |
| T (if coupled) | fixedValue or coupled | fixedValue | fixedValue | zeroGradient |

---

## 9. Validation References

| Case | Reference | Key Metric |
|------|-----------|-----------|
| Hartmann channel flow | Hartmann (1937), Müller & Bühler (2001) | Velocity profile flattening, δ_Ha |
| Hunt flow (conducting walls) | Hunt (1965) | Side layer thickness |
| MHD pipe flow | Moreau (1990) | Pressure drop vs Ha |
| DBD actuator | Suzen et al. (2005) | Induced wall-jet velocity |
| EHD channel | Atten & Malraison (1987) | Secondary flow pattern |

---

## 10. Fix Library

| Symptom | Cause | Fix |
|---------|-------|-----|
| `mhdFoam` diverges at Ha > 50 | Hartmann layer unresolved | Refine mesh normal to B: y1 < δ_Ha/5 |
| Oscillating B residuals | Non-solenoidal B (∇·B ≠ 0) | Add `divB` correction; use Biot-Savart initialisation |
| Body force too large → instability | Body force overestimated | Ramp force with `fvOptions` timeRamp; start with 10% |
| E field singularity at electrode tip | Sharp geometry | Round electrode tip; use `curvatureRefinement` in snappy |
| σ units mismatch | Dimension error in transportProperties | Verify SI: S/m = [kg⁻¹ m⁻³ s³ A²] = [-1 -3 3 0 0 2 0] |