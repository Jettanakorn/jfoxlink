# Hypersonic CFD — Full Reference

## Table of Contents
1. [Physics: High-Speed Flow Regimes](#1-physics)
2. [Solver Selection](#2-solver-selection)
3. [Real Gas Effects](#3-real-gas-effects)
4. [Chemical Nonequilibrium (Dissociation/Ionisation)](#4-chemical-nonequilibrium)
5. [Viscous Interaction & Rarefied Flow](#5-rarefied-flow)
6. [Shock Capturing](#6-shock-capturing)
7. [Boundary Conditions for Hypersonic](#7-boundary-conditions)
8. [Aerothermodynamics: Heat Flux, Stagnation Heating](#8-aerothermodynamics)
9. [Intake YAML Schema](#9-intake-schema)
10. [Validation References](#10-validation)
11. [Fix Library](#11-fix-library)

---

## 1. Physics

### Flow Regimes by Mach/Knudsen Number

| Mach | Regime | Key Physics |
|------|--------|------------|
| 1.2–5 | Supersonic | Oblique shocks, expansion fans, bow shock |
| 5–10 | Hypersonic (cold) | High stagnation enthalpy, thick shock layer |
| 10–25 | Hypersonic (hot) | Vibrational excitation, dissociation begins |
| > 25 | Extreme hypersonic | Ionisation, radiation, ablation |

**Mach-Reynolds independence**: For Ma > 5, force coefficients become approximately
independent of Mach number (but NOT of Reynolds or Knudsen number).

**Knudsen number regimes:**
```
Kn = λ / L                       ; λ = mean free path
Kn < 0.001   → continuum (N-S valid)
0.001-0.1    → slip flow (N-S + slip BCs)
0.1-10       → transition (DSMC hybrid)
> 10         → free molecular
```

### High-Enthalpy Effects

```
h0 = cp * T0 = U²/2 + cp*T     ; stagnation enthalpy
For Ma=8 at 30km: T0 ≈ 8200K → O2 dissociation (Td ≈ 2000K), N2 dissociation (Td ≈ 4000K)
```

---

## 2. Solver Selection

| Regime | Solver | Notes |
|--------|--------|-------|
| Ma 2–8, perfect gas | `rhoCentralFoam` | Kurganov-Tadmor flux; robust |
| Ma 2–8, perfect gas steady | `rhoSimpleFoam` | Requires `sonicCourantNo` control |
| Ma 5–15, thermally perfect | `rhoCentralFoam` + real gas EOS | Use `hPolynomial` cp(T) |
| Ma > 8, chemical nonequil. | `reactingFoam` or `hy2Foam` | Multi-species with chemistry |
| Ablating surface | `chtMultiRegionFoam` + surface recession | Custom ablation BC |
| Rarefied (Kn > 0.01) | DSMC via `dsmcFoam` | Particle-based |
| Slip flow (0.001 < Kn < 0.1) | `rhoCentralFoam` + Maxwell slip BCs | Modified wall BCs |

**Recommended external solver for hypersonic nonequilibrium:**
- **hy2Foam** (open-source, built on OpenFOAM) — thermochemical nonequilibrium,
  two-temperature model, Park's chemistry — install separately from GitHub

---

## 3. Real Gas Effects

### Thermally Perfect Gas (NASA 7-coefficient polynomials)

```cpp
// constant/thermophysicalProperties
thermoType
{
    type            hePsiThermo;
    mixture         pureMixture;
    transport       sutherland;
    thermo          janaf;           // JANAF tables (NASA polynomials)
    equationOfState perfectGas;
    specie          specie;
    energy          sensibleEnthalpy;
}

mixture
{
    specie      { molWeight 28.966; }
    thermodynamics
    {
        Tlow    200;  Thigh  6000;  Tcommon 1000;
        // 7 high-T coefficients (Tcommon to Thigh):
        highCpCoeffs  (3.697578 6.135197e-4 -1.26e-7 1.745e-11 -6.56e-16 -1233.93 2.05);
        // 7 low-T coefficients (Tlow to Tcommon):
        lowCpCoeffs   (3.298677 1.408240e-3 -3.96e-6 6.113e-9 -2.032e-12 -1020.90 3.95);
    }
    transport { As 1.458e-6; Ts 110.4; }  // Sutherland
}
```

### Equilibrium Real Gas (Cubic EOS)
For dense gas / cryogenic propellants:
```cpp
equationOfState  PengRobinson;
PengRobinsonCoeffs { Tc 154.6; Pc 5.043e6; omega 0.022; }   // O2 example
```

---

## 4. Chemical Nonequilibrium

### 5-Species Air Chemistry (Park 1990)

Species: N2, O2, NO, N, O

```cpp
// constant/chemistryProperties (using hy2Foam or reactingFoam)
chemistryType
{
    solver            ode;
    method            SIBS;           // or EulerImplicit for stiff chemistry
}

// Reactions (Arrhenius, 5-species Park model):
// N2 + M  ⇌  N + N + M    ; M = any third body
// O2 + M  ⇌  O + O + M
// NO + M  ⇌  N + O + M
// N2 + O  ⇌  NO + N
// NO + O  ⇌  O2 + N
```

### Two-Temperature Model (Park 1989)
```
T_tr : translational-rotational temperature  (N-S energy equation)
T_ve : vibrational-electronic temperature    (separate VE energy equation)
τ_VT : vibrational relaxation time (Millikan-White)
```
Use `hy2Foam` with `twoTemperatureModel on;` in thermophysicalProperties.

### Stiffness Note
Chemistry ODE is typically stiff for:
- Shock-heated flows (sudden temperature jump)
- Near-wall recombination

Use `SIBS` or `Rosenbrock` ODE solvers, NOT explicit Euler.

---

## 5. Rarefied Flow

### dsmcFoam (Direct Simulation Monte Carlo)
```yaml
dsmc_intake:
  geometry: "flat plate 0.1m x 0.01m"
  altitude_km: 80          # Kn ~ 0.1 → transition regime
  Ma: 8
  T_wall_K: 1000           # isothermal wall
  species: [N2, O2]
  particles_per_cell_target: 20
  time_steps: 500000
```

```cpp
// constant/dsmcProperties
nEquivalentParticles    1e11;      // real particles per simulated particle
```

### Slip Flow BCs (N-S regime, Kn = 0.001–0.1)
```cpp
// 0/U — Maxwell velocity slip BC
walls
{
    type    maxwellSlipU;
    Uwall   (0 0 0);
    accommodationCoeff 1.0;         // full accommodation
    thetaCoeff 0.0;
    value   uniform (0 0 0);
}

// 0/T — Smoluchowski temperature jump BC
walls
{
    type    smoluchowskiJumpT;
    Twall   uniform 1000;
    accommodationCoeff 1.0;
    value   uniform 1000;
}
```

---

## 6. Shock Capturing

### Kurganov-Tadmor (rhoCentralFoam default)
```cpp
// system/fvSchemes — for hypersonic
divSchemes
{
    div(phi,U)      Gauss limitedLinearV 1;    // TVD limiter
    div(phi,e)      Gauss limitedLinear 1;
    div(phi,K)      Gauss limitedLinear 1;
    div(phiv,p)     Gauss limitedLinear 1;
    div(phi,Ekp)    Gauss limitedLinear 1;
}

// Flux scheme selection — controls diffusion at shocks:
fluxScheme      Kurganov;           // or AUSM+ for better contact discontinuities
```

### Mesh Refinement at Shocks
```
For oblique shock: refine 3–5 cells across shock thickness
Bow shock stand-off distance δ ≈ 0.19 * R_nose * ρ_inf/ρ_2
snappyHexMesh: add refinement box aligned with expected shock location
```

**Carbuncle instability fix** (blunt body bow shock):
```cpp
// Add numerical viscosity via limitedLinear 1 on div(phi,U)
// Or use AUSM+ flux which is more stable on blunt bodies
// Increase mesh resolution at stagnation point (>20 cells in shock layer)
```

---

## 7. Boundary Conditions

| Field | Freestream (inflow) | Wall (cold) | Wall (hot / radiative) | Outlet |
|-------|---------------------|-------------|----------------------|--------|
| U | fixedValue (Ma direction) | fixedValue (0,0,0) | fixedValue (0,0,0) | zeroGradient |
| p | fixedValue (p_inf) | zeroGradient | zeroGradient | fixedValue |
| T | fixedValue (T_inf) | fixedValue (T_w) | mixedRadiation | zeroGradient |
| rho | fixedValue (ρ_inf) | zeroGradient | zeroGradient | zeroGradient |
| Yi (species) | fixedValue (air composition) | zeroGradient | fixedValue or catalytic | zeroGradient |

**Supersonic inlet (all char. incoming) — use `fixedValue` for ALL fields.**
**Supersonic outlet (all char. outgoing) — use `zeroGradient` for ALL fields.**

### Catalytic Wall BC (surface recombination)
```cpp
// For high-altitude ablative surfaces:
O { type catalyticWall; catalyticEfficiency 1.0; }   // fully catalytic
// or
O { type zeroGradient; }    // non-catalytic wall (lower heat flux, conservative)
```

---

## 8. Aerothermodynamics

### Stagnation Point Heating (Fay-Riddell)
```python
# Stagnation heat flux prediction (W/m²)
def fay_riddell(rho_inf, U_inf, R_nose, h_w, h_0):
    rho_s = 11 * rho_inf                        # shock density ratio at Ma>>1
    mu_s  = mu_ref * (T_s / T_ref)**0.7         # Sutherland at shock
    du_ds = (1/R_nose) * sqrt(2*(p_s-p_inf)/rho_s)   # velocity gradient
    q_w   = 0.763 * Pr**(-0.6) * (rho_s*mu_s)**0.5 * du_ds**0.5 * (h_0 - h_w)
    return q_w
```

### Heat Flux Post-Processing
```cpp
// system/controlDict functions block:
heatFlux
{
    type            wallHeatFlux;
    libs            ("libfieldFunctionObjects.so");
    patches         (wall);
    writeControl    timeStep;
    writeInterval   1;
}
```

### Radiative Heat Transfer (for Ma > 15)
```cpp
// Discrete Ordinates Method (DOM/fvDOM):
radiation
{
    radiationModel  fvDOM;
    absorptionEmissionModel constantAbsorptionEmission;
    constantAbsorptionEmissionCoeffs
    {
        absorptivity    absorptivity [0 -1 0 0 0 0 0] 0.5;
        emissivity      emissivity   [0 0 0 0 0 0 0]  0.5;
    }
    solverFreq      1;
}
```

---

## 9. Intake Schema

```yaml
hypersonic_intake:
  geometry: "sphere-cone, half-angle 10deg, nose R=0.05m"
  Ma_inf: 10.0
  altitude_km: 30
  alpha_deg: 0
  T_wall_K: 1500               # or "adiabatic"
  wall_catalysis: "noncatalytic"  # "fully_catalytic" | "noncatalytic" | "partial"
  real_gas: true
  chemistry: "5-species-Park"  # "none" | "5-species-Park" | "11-species-Park"
  two_temperature: true
  rarefied: false              # if Kn > 0.01, switch to dsmcFoam
  target_CD: 0.85
  target_peak_heat_flux_W_m2: 5e6
  convergence_residual: 1e-6
  hpc_cores: 64
```

---

## 10. Validation References

| Case | Reference | Metrics |
|------|-----------|---------|
| Blunt cone (Ma=10) | Holden (1978), AIAA 78-65 | CD, Cp, heat flux |
| Sphere (Ma=6-10) | Lobb (1964) | Bow shock stand-off |
| Flat plate (Ma=5 BL) | Van Driest (1956) | Cf, recovery factor |
| RAM-C II sphere-cone (Ma=14, ionised) | AIAA 72-689 | Electron density, attenuation |
| 5-species air chemistry | Park (1990), JTHT | Species mass fractions |

---

## 11. Fix Library

| Symptom | Cause | Fix |
|---------|-------|-----|
| Bow shock carbuncle (blunt body) | Kurganov flux on aligned mesh | Switch to AUSM+; refine stagnation region |
| Temperature → 0 (negative T) | Shockwave clipping EOS | Add `limitT` in controlDict; use `Gauss limitedLinear 1` |
| Chemistry ODE timeout | Stiff reactions near wall | Use SIBS solver; set `chemistrySolver EulerImplicit` |
| Species Y_i > 1 or < 0 | Interpolation overshoot | Add `limitSpecies yes;` in chemistryProperties |
| `rhoCentralFoam` diverges Ma>8 | CFL > 1 | Reduce `maxCo` to 0.3; add `meshCourantNo` monitoring |
| Two-temperature T_ve = T_tr everywhere | VE coupling too strong | Check VT relaxation `tau_VT` scaling; verify Park model coefficients |
| Huge heat flux at corner (numerical) | Mesh singularity | Round corners; refine + 3 prism layers |