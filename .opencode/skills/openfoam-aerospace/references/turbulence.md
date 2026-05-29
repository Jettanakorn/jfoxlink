# Turbulence Models Reference — Aerospace CFD

## Table of Contents
1. [RANS Models](#1-rans-models)
2. [DES / Hybrid RANS-LES](#2-des)
3. [LES Models](#3-les)
4. [Transition Models](#4-transition)
5. [Wall Functions](#5-wall-functions)
6. [Initialization Strategies](#6-initialization)

---

## 1. RANS Models

### 1.1 k-ω SST (Menter) — Recommended Default for Aerospace

Best for: adverse pressure gradients, mild separation, attached boundary layers over wings.

**`constant/turbulenceProperties`**
```cpp
FoamFile { version 2.0; format ascii; class dictionary; object turbulenceProperties; }
simulationType RAS;
RAS { RASModel kOmegaSST; turbulence on; printCoeffs on; }
```

**`0/k`**
```cpp
FoamFile { version 2.0; format ascii; class volScalarField; object k; }
dimensions [0 2 -2 0 0 0 0];
internalField uniform 0.00015;     // 0.5*(U*I)^2, I~0.5% for clean freestream
boundaryField
{
    inlet      { type fixedValue; value uniform 0.00015; }
    outlet     { type zeroGradient; }
    wing       { type kqRWallFunction; value uniform 0.00015; }   // wall function
    // OR for low-Re (y+≤1):
    // wing    { type fixedValue; value uniform 0; }
    farfield   { type inletOutlet; inletValue uniform 0.00015; value uniform 0.00015; }
    symmetry   { type symmetryPlane; }
}
```

**`0/omega`**
```cpp
FoamFile { version 2.0; format ascii; class volScalarField; object omega; }
dimensions [0 0 -1 0 0 0 0];
internalField uniform 1.0;       // k^0.5 / (Cmu^0.25 * L), L~chord
boundaryField
{
    inlet      { type fixedValue; value uniform 1.0; }
    outlet     { type zeroGradient; }
    wing       { type omegaWallFunction; value uniform 1.0; }  // wall function
    // OR for low-Re (y+≤1):
    // wing    { type omegaWallFunction; value uniform 1.0; }  // still use omegaWF at low y+
    farfield   { type inletOutlet; inletValue uniform 1.0; value uniform 1.0; }
    symmetry   { type symmetryPlane; }
}
```

**`0/nut`**
```cpp
FoamFile { version 2.0; format ascii; class volScalarField; object nut; }
dimensions [0 2 -1 0 0 0 0];
internalField uniform 1.5e-6;    // ~10× molecular viscosity (νt/ν ≈ 10)
boundaryField
{
    inlet      { type calculated; value uniform 1.5e-6; }
    outlet     { type calculated; value uniform 0; }
    wing       { type nutkWallFunction; value uniform 0; }
    farfield   { type calculated; value uniform 1.5e-6; }
    symmetry   { type symmetryPlane; }
}
```

### k-ω SST Coefficients (Default OpenFOAM)
These are correct for standard use — only modify for specific calibration:
```
alphaK1=0.85, alphaK2=1.0, alphaOmega1=0.5, alphaOmega2=0.856
beta1=0.075, beta2=0.0828, betaStar=0.09
gamma1=5/9, gamma2=0.44, a1=0.31, b1=1.0
```

---

### 1.2 Spalart-Allmaras (SA) — Industry Standard for Attached Flows

Best for: high-Re external aero, attached boundary layers, transonic conditions.
Not recommended for massive separation or adverse pressure gradients.

**`constant/turbulenceProperties`**
```cpp
simulationType RAS;
RAS { RASModel SpalartAllmaras; turbulence on; printCoeffs on; }
```

**`0/nuTilda`**
```cpp
FoamFile { version 2.0; format ascii; class volScalarField; object nuTilda; }
dimensions [0 2 -1 0 0 0 0];
internalField uniform 4.5e-5;    // 3× molecular viscosity: 3 × 1.5e-5
boundaryField
{
    inlet      { type fixedValue; value uniform 4.5e-5; }
    outlet     { type zeroGradient; }
    wing       { type fixedValue; value uniform 0; }      // wall: nuTilda=0
    farfield   { type inletOutlet; inletValue uniform 4.5e-5; value uniform 4.5e-5; }
    symmetry   { type symmetryPlane; }
}
```

**`0/nut`**
```cpp
boundaryField
{
    wing { type nutUSpaldingWallFunction; value uniform 0; }  // SA uses Spalding
}
```

---

### 1.3 k-ε Realizable — Wakes and Far-Field (Less Common in Aerospace)
Not recommended for wall-bounded aerospace flows. Use k-ω SST instead.
Acceptable for far-wake analysis or inlet-duct flows without significant adverse pressure gradient.

---

## 2. DES — Detached Eddy Simulation

Best for: bluff bodies, massive separation, landing gear, high-AoA stall, airbrakes.
Requires: **unsteady solver** (pimpleFoam), fine mesh in separated regions, LES-appropriate Δt.

**`constant/turbulenceProperties`**
```cpp
simulationType RAS;
RAS
{
    RASModel        kOmegaSSTDES;   // or SpalartAllmarasDES
    turbulence      on;
    printCoeffs     on;
}
```

### DES Mesh Requirements
- RANS region (boundary layer): y+ ≤ 1, standard prism layers
- LES region (separated area): **isotropic cells**, Δx ≈ Δy ≈ Δz
- Cell size in separated region: `Δ ≈ 0.01c` to `0.05c` (depending on resolved scales)

### DES Time Step
```
Co < 1 in LES region (Courant number)
Δt ≈ Δ_LES / U_convective
Typical: Δt = 1e-5 to 1e-4 s for aircraft-scale
```

### PIMPLE Settings for DES (system/fvSolution)
```cpp
PIMPLE
{
    nOuterCorrectors 1;      // 1 for pure PISO mode
    nCorrectors      2;
    nNonOrthogonalCorrectors 1;
    pRefCell 0; pRefValue 0;
}
```

---

## 3. LES Models

Best for: aeroacoustics, wake turbulence, detailed flow structures, transition to turbulence.
**Very expensive** — typically research, not production design.

**`constant/turbulenceProperties`**
```cpp
simulationType LES;
LES
{
    LESModel    WALE;       // Wall-Adapting Local Eddy-viscosity — best for wall flows
    // alternatives: Smagorinsky (simpler), dynamicKEqn (most accurate)
    turbulence  on;
    delta       cubeRootVol;
    printCoeffs on;
}
```

### LES Resolution Requirements
- **Filter width Δ**: automatically set to cube-root of cell volume
- **Resolved TKE > 80%** of total (Pope's criterion) — check with Q-criterion iso-surfaces
- **Mesh**: wall-resolved LES needs y+ ≤ 1, Δx+ ≤ 100, Δz+ ≤ 30
- **WMLES** (wall-modeled): y+ 30–300 with algebraic wall model

### LES Time Integration
```
Backward differencing (backward) in ddtSchemes
Co_max < 0.5 recommended for accuracy
```

---

## 4. Transition Models

### γ-Reθ (Gamma-ReTheta) Model — Laminar-Turbulent Transition

Best for: UAVs (low Re), compressor blades, laminar flow airfoils, low-Re wings.

**`constant/turbulenceProperties`**
```cpp
simulationType RAS;
RAS { RASModel gammaReTheta; turbulence on; printCoeffs on; }
```

Required additional fields: `0/gammaInt` (intermittency), `0/ReThetat`
```cpp
// 0/gammaInt
internalField uniform 1;
boundaryField
{
    inlet  { type fixedValue; value uniform 1; }
    outlet { type zeroGradient; }
    wing   { type zeroGradient; }
}

// 0/ReThetat
internalField uniform 100;
boundaryField
{
    inlet  { type fixedValue; value uniform 100; }  // compute from FSTI & chord Re
    outlet { type zeroGradient; }
    wing   { type zeroGradient; }
}
```

### k-kL-ω (Walters-Cokljat) — Alternative Transition
Simpler to set up than γ-Reθ, acceptable for many aerospace cases.

---

## 5. Wall Functions

### When to Use Wall Functions vs Low-Re
| y+ Range | Treatment | Models | Accuracy |
|----------|-----------|--------|----------|
| y+ ≤ 1 | Low-Reynolds (no wall function for k) | kOmegaSST, SA, LES | High (resolves viscous sublayer) |
| y+ 5–30 | Buffer layer — AVOID this range | Any | Poor — neither valid |
| y+ 30–300 | Standard wall functions | kOmegaSST + WF, k-ε | Moderate |
| y+ > 300 | Wall functions only | k-ε | Low (rough estimate) |

### Wall Function Dictionary (for y+ 30-300 regime)
```cpp
// 0/nut — wall boundaries
wing { type nutkWallFunction; value uniform 0; }

// 0/k — wall boundaries
wing { type kqRWallFunction; value uniform 0; }

// 0/omega — wall boundaries (use even for low-Re with kOmegaSST)
wing { type omegaWallFunction; value uniform 1.0; }

// 0/epsilon (if k-epsilon)
wing { type epsilonWallFunction; value uniform 0.1; }
```

---

## 6. Turbulence Initialization Strategies

### Option A: Uniform Freestream (Cold Start)
Risk: may diverge in first 50–200 iterations. Mitigate with low relaxation.
```
k_inf     = 1.5 × (U_inf × I)²    where I = 0.001 to 0.005
omega_inf = sqrt(k) / (Cmu^0.25 × L)
nut_inf   = k / omega_inf
```

### Option B: potentialFoam Initialization (Recommended for complex geometries)
```bash
potentialFoam -writePhi    # generates initial velocity field from potential flow
```
Then run RANS from this initial condition.

### Option C: Restart from Coarser Mesh / Lower Re
Map coarse mesh solution to fine mesh:
```bash
mapFields ../coarseCase -sourceTime latestTime
```

### Convergence Monitoring Script (sample)
```bash
# Watch residuals and force coefficients simultaneously
foamMonitor -l postProcessing/residuals/0/residuals.dat &
foamMonitor -l postProcessing/forces/0/forceCoeffs.dat
```