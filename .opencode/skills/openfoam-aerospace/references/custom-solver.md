# Custom Solver Builder — Agentic AI Reference

## Table of Contents
1. [Philosophy: When to Build a Custom Solver](#1-philosophy)
2. [OpenFOAM Solver Architecture](#2-solver-architecture)
3. [Agentic Solver Design Interview](#3-agentic-interview)
4. [Physics Module Library](#4-physics-module-library)
5. [Custom Solver Scaffold Generator](#5-scaffold-generator)
6. [fvModel & fvOptions Source Terms](#6-fvmodel--fvoptions)
7. [Custom Boundary Conditions](#7-custom-boundary-conditions)
8. [Linking & Compiling (wmake)](#8-compiling)
9. [Validation Protocol](#9-validation-protocol)
10. [Agentic Build Loop](#10-agentic-build-loop)
11. [Solver Catalogue (Pre-Built Templates)](#11-solver-catalogue)

---

## 1. Philosophy

Build a custom solver when:
- No standard OpenFOAM solver covers the required multi-physics coupling
- A new constitutive law, EOS, reaction mechanism, or body force is needed
- Performance requires removing unused physics (leaner loop)
- The research contribution IS the new numerical method

**Do NOT build custom when:**
- A standard solver + `fvOptions` source term suffices (95% of cases)
- An `fvModel` plugin can add the physics without solver modification
- A `functionObject` can add the needed post-processing

The agent always checks these alternatives first before scaffolding a new solver.

---

## 2. Solver Architecture

Every OpenFOAM finite-volume solver follows this pattern:

```
solver.C
├── #include "fvCFD.H"              // core FV headers
├── #include "<physics>Model.H"     // physics-specific headers
├── int main()
│   ├── setRootCase.H               // parse CLI args
│   ├── createTime.H                // runTime object
│   ├── createMesh.H                // fvMesh
│   ├── createFields.H              // declare volScalarField, volVectorField, etc.
│   ├── while (runTime.run())       // time loop
│   │   ├── runTime++
│   │   ├── physicsEqn.solve()      // coupled/segregated equations
│   │   ├── turbulence->correct()
│   │   └── runTime.write()
│   └── return 0
```

### Key Header Categories

| Header | Purpose |
|--------|---------|
| `fvCFD.H` | Core: mesh, fields, fvMatrix, SIMPLE/PISO loops |
| `psiThermo.H` / `rhoThermo.H` | Compressible thermodynamics |
| `turbulentFluidThermoModel.H` | Turbulence + thermal coupling |
| `combustionModel.H` | Reacting flow chemistry |
| `multiphaseSystem.H` | Two-phase / multiphase |
| `solidThermo.H` | Solid conduction |
| `radiation/radiationModel.H` | Radiative heat transfer |
| `fvOptions.H` | Plug-in source terms |

---

## 3. Agentic Solver Design Interview

When the user requests a custom solver, the agent runs this structured interview:

```yaml
# solver-design-intake.yaml
solver_name: ""           # e.g. "mhdReactingFoam"
base_solver: ""           # closest existing solver to start from
description: ""           # one-sentence physics description

physics_modules:
  fluid_dynamics: true    # always true
  compressible: false     # rho varies?
  turbulence: true        # RANS/LES/DNS?
  heat_transfer: false    # energy equation?
  species_transport: false  # passive or reacting scalars?
  chemical_reactions: false # Arrhenius, tabulated, PDF?
  two_phase: false        # VOF / Euler-Euler?
  solid_mechanics: false  # FSI coupling?
  electromagnetic: false  # Lorentz force / induction?
  radiation: false        # fvDOM / P1?
  rotating_frame: false   # MRF / AMI?
  porous_media: false     # Darcy / Forchheimer?
  particle_tracking: false # Lagrangian DPM?
  custom_EOS: false       # non-standard equation of state?
  custom_viscosity: false # non-Newtonian / viscoelastic?

coupling_strategy: ""     # "segregated-SIMPLE" | "segregated-PISO" | "coupled-matrix" | "operator-split"
time_treatment: ""        # "steady" | "unsteady-1st" | "unsteady-2nd"
parallel: true
target_openfoam_version: "v2306"

validation_case: ""       # known analytical or experimental case to validate against
```

---

## 4. Physics Module Library

Each module is a self-contained code block the agent assembles into the solver scaffold.

### Module: Species Transport (passive scalar Yi)

```cpp
// createFields.H addition:
PtrList<volScalarField> Y(nSpecies);
forAll(Y, i)
{
    Y.set(i, new volScalarField(
        IOobject("Y." + speciesNames[i], ...),
        mesh, dimensionedScalar(dimless, scalar(0))
    ));
}

// Solver loop — Yi equation:
tmp<fvScalarMatrix> YiEqn
(
    fvm::ddt(rho, Yi)
  + fvm::div(phi, Yi)
  - fvm::laplacian(turbulence->muEff()/Sct, Yi)
  ==
    fvOptions(rho, Yi)
);
YiEqn->relax();
fvOptions.constrain(YiEqn.ref());
YiEqn->solve(mesh.solver("Yi"));
```

---

### Module: Chemical Reactions (Arrhenius ODEs)

```cpp
// createFields.H:
autoPtr<combustionModels::combustionModel> combustion
(
    combustionModels::combustionModel::New(thermo, turbulence())
);

// Solver loop — after species transport:
combustion->correct();
volScalarField Qdot(combustion->Qdot());    // heat release rate W/m³

// Add Qdot to energy equation:
fvm::ddt(rho, he) + fvm::div(phi, he) - fvm::laplacian(turbulence->alphaEff(), he)
    == dpdt + fvc::div(fvc::absolute(phi, U), p, "div(phi,p)")
     + Qdot                                  // <-- heat release
     + fvOptions(rho, he);
```

---

### Module: Electromagnetic (Low-Rm MHD Lorentz Force)

```cpp
// createFields.H:
volVectorField B
(
    IOobject("B", runTime.timeName(), mesh, IOobject::MUST_READ, IOobject::AUTO_WRITE),
    mesh
);
volScalarField phi_E(IOobject("phi_E", ...), mesh, dimensionedScalar(dimVoltage, 0));

// Solver loop — solve electric potential:
solve(fvm::laplacian(sigma, phi_E) == fvc::div(sigma * (U ^ B)));
volVectorField J = sigma * (-fvc::grad(phi_E) + (U ^ B));   // current density
volVectorField F_EM = J ^ B;                                  // Lorentz force

// Add to U equation:
fvm::ddt(rho, U) + fvm::div(phi, U) + turbulence->divDevRhoReff(U)
    == -fvc::grad(p) + F_EM + fvOptions(rho, U);
```

---

### Module: Conjugate Heat Transfer (Solid-Fluid Coupling)

```cpp
// Uses chtMultiRegionFoam as base; agent patches in new physics per region
// Key coupling call (executed each outer iteration):
forAll(interfaces, i) { interfaces[i].correct(); }
```

---

### Module: Non-Newtonian Viscosity

```cpp
// createFields.H — use viscosity model:
autoPtr<viscosityModel> viscosity(viscosityModel::New(mesh));

// Models available: powerLaw, CrossPowerLaw, BirdCarreau, HerschelBulkley, Casson
// constant/transportProperties:
// viscosityModel  HerschelBulkley;
// HerschelBulkleyCoeffs { nu0 1e-2; tau0 10; k 0.005; n 0.4; nuMin 1e-4; nuMax 1e2; }
```

---

### Module: Porous Media (Darcy-Forchheimer)

```cpp
// fvOptions source (no solver modification needed for most cases):
// constant/fvOptions:
porosity
{
    type            DarcyForchheimer;
    selectionMode   cellZone;
    cellZone        porousRegion;
    DarcyForchheimerCoeffs
    {
        d   d [0 -2 0 0 0 0 0] (5e7 5e7 5e7);    // permeability resistance 1/m²
        f   f [0 -1 0 0 0 0 0] (500 500 500);     // Forchheimer resistance 1/m
    }
}
```

---

### Module: Lagrangian Particle Tracking (DPM)

```cpp
// createFields.H:
#include "basicKinematicCloud.H"
basicKinematicCloud particles("kinematicCloud", rho, U, g, slgThermo);

// Solver loop — after fluid equations:
particles.evolve();                              // integrate particle ODEs
volVectorField F_p = particles.SU(U)();          // momentum coupling force
// Add F_p to U equation source term
```

---

### Module: Viscoelastic Fluid (Oldroyd-B, Giesekus)

```cpp
// Use viscoelasticFluidFoam or build on top of simpleFoam:
autoPtr<viscoelasticLaw> viscoelastic(viscoelasticLaw::New(U, phi));
// Constitutive equation (Oldroyd-B):
// D(tau)/Dt - tau·(∇U)^T - (∇U)·tau = -(tau - G*(exp(tau/G) - I))/lambda
viscoelastic->correct();
volSymmTensorField tau(viscoelastic->tau());
```

---

## 5. Scaffold Generator

The agent generates a complete new solver directory from the design intake:

```
<solverName>/
├── Make/
│   ├── files     (compiler entry point, target name)
│   └── options   (include paths, linked libraries)
├── <solverName>.C    (main solver loop)
├── createFields.H    (field declarations)
├── EEqn.H            (energy equation — if thermal)
├── YEqn.H            (species equations — if reacting)
├── UEqn.H            (momentum equation)
├── pEqn.H            (pressure equation / SIMPLE/PISO loop)
└── README.md         (physics description, usage, validation)
```

### Make/files template
```
<solverName>.C

EXE = $(FOAM_USER_APPBIN)/<solverName>
```

### Make/options template
```
EXE_INC = \
    -I$(LIB_SRC)/finiteVolume/lnInclude \
    -I$(LIB_SRC)/meshTools/lnInclude \
    -I$(LIB_SRC)/sampling/lnInclude \
    -I$(LIB_SRC)/TurbulenceModels/turbulenceModels/lnInclude \
    -I$(LIB_SRC)/TurbulenceModels/incompressible/lnInclude \
    -I$(LIB_SRC)/transportModels/lnInclude \
    -I$(LIB_SRC)/thermophysicalModels/basic/lnInclude \
    -I$(LIB_SRC)/thermophysicalModels/reactionThermo/lnInclude \
    -I$(LIB_SRC)/combustionModels/lnInclude \
    -I$(LIB_SRC)/fvOptions/lnInclude

EXE_LIBS = \
    -lfiniteVolume \
    -lmeshTools \
    -lturbulenceModels \
    -lincompressibleTurbulenceModels \
    -ltransportModels \
    -lfluidThermophysicalModels \
    -lreactionThermophysicalModels \
    -lcombustionModels \
    -lfvOptions \
    -lsampling
```

---

## 6. fvModel & fvOptions Source Terms

For adding physics WITHOUT modifying the solver binary — preferred approach.

```cpp
// constant/fvOptions  (or constant/fvModels in newer OF versions)

// Example: volumetric heat source (laser, induction, metabolic, etc.)
heatSource
{
    type            scalarSemiImplicitSource;
    volumeMode      absolute;
    selectionMode   cellZone;
    cellZone        heatedZone;
    injectionRateSuSp
    {
        h ((1e6) 0);    // 1 MW/m³ explicit source into enthalpy h
    }
}

// Example: momentum source (fan curve, actuator disk, etc.)
fanActuatorDisk
{
    type            rotorDiskSource;
    active          true;
    selectionMode   cellZone;
    cellZone        rotorZone;
    fieldNames      (U);
    rotorDiskSourceCoeffs
    {
        diskDir     (1 0 0);     // thrust direction
        Cp          0.4;         // power coefficient
        Ct          0.8;         // thrust coefficient
        diskArea    0.0707;      // m²
        upstream    true;
    }
}
```

---

## 7. Custom Boundary Conditions

```
<BCName>/
├── Make/files
├── Make/options
└── <BCName>FvPatchField.{H,C}
```

### Template: Custom Inlet Profile (parabolic velocity)

```cpp
// parabolicVelocityFvPatchVectorField.H
class parabolicVelocityFvPatchVectorField
:   public fixedValueFvPatchVectorField
{
    scalar Umax_;
    vector n_;       // flow direction
    vector y_;       // wall-normal direction

public:
    TypeName("parabolicVelocity");
    // ... constructor, write, updateCoeffs
};

// parabolicVelocityFvPatchVectorField.C — updateCoeffs:
void parabolicVelocityFvPatchVectorField::updateCoeffs()
{
    const vectorField& C = patch().Cf();   // face centres
    scalarField y = (C - origin_) & y_;    // wall-normal distance
    operator==(Umax_ * (1 - sqr(y/R_)) * n_);   // parabolic profile
    fixedValueFvPatchVectorField::updateCoeffs();
}
```

---

## 8. Compiling (wmake)

```bash
# Compile solver:
cd $FOAM_RUN/../<solverName>
wmake

# Compile shared library (for fvModel / BC plugin):
wmake libso

# Clean and rebuild:
wclean && wmake

# Verify binary installed:
which <solverName>    # should point to $FOAM_USER_APPBIN/<solverName>

# Parallel compile (faster on HPC):
wmake -j 8
```

**Common compile errors and fixes:**

| Error | Fix |
|-------|-----|
| `undefined reference to ...` | Add `-l<library>` to Make/options EXE_LIBS |
| `No such file or directory: ...Model.H` | Add correct `-I$(LIB_SRC)/...` to EXE_INC |
| `error: ambiguous overload for operator==` | Explicit cast: `fvm::ddt(...)` vs `fvc::ddt(...)` |
| `dimensionError` at runtime | Check dimension sets in field constructors |
| `fatal: cannot find field 'X'` | Field not declared in createFields.H or missing from 0/ |

---

## 9. Validation Protocol

Every custom solver must be validated before production use:

```yaml
validation_protocol:
  step1_unit_test:
    description: "Disable all physics except one; verify conservation laws"
    tests:
      - "Zero flow: residuals < 1e-12"
      - "Uniform flow: no divergence"
      - "Mesh convergence: 2nd-order drop in error with refinement"

  step2_analytical:
    description: "Compare against analytical solution for simplified case"
    examples:
      - "Poiseuille flow for viscosity module"
      - "Hartmann flow for MHD module"
      - "Frank-Kamenetskii for exothermic reaction"

  step3_benchmark:
    description: "Compare against published DNS or experiment"
    metric: "L2 norm error < 5% vs reference data"

  step4_grid_convergence:
    description: "GCI < 3% between medium and fine mesh"
    meshes: [coarse, medium, fine]   # ratio ~√2 between levels
```

---

## 10. Agentic Build Loop

The agent autonomously builds, compiles, tests, and iterates the custom solver:

```
┌────────────────────────────────────────────────────────────────┐
│                  CUSTOM SOLVER AGENT LOOP                      │
│                                                                │
│  [1] DESIGN INTERVIEW → fill solver-design-intake.yaml        │
│         ↓                                                      │
│  [2] MODULE ASSEMBLY → select physics modules from §4          │
│         ↓                                                      │
│  [3] SCAFFOLD GENERATION → write solver directory §5           │
│         ↓                                                      │
│  [4] COMPILE → wmake; capture errors                          │
│         ↓ (if errors)                                          │
│  [4b] FIX AGENT → diagnose compile error; patch; retry        │
│         ↓ (compile success)                                    │
│  [5] UNIT TEST → run zero-flow and analytical cases           │
│         ↓ (if test fails)                                      │
│  [5b] DEBUG AGENT → add diagnostic output; trace field values │
│         ↓ (tests pass)                                         │
│  [6] BENCHMARK → run vs reference case; compute L2 error      │
│         ↓ (if L2 > 5%)                                        │
│  [6b] PHYSICS REVIEW → inspect discretisation; fix scheme     │
│         ↓ (benchmark passes)                                   │
│  [7] DOCUMENT → write README.md, tutorial case, SKILL update  │
└────────────────────────────────────────────────────────────────┘
```

**Agent state file** (`solver-build-state.json`):
```json
{
  "solver_name": "mhdReactingFoam",
  "stage": "benchmark",
  "compile_attempts": 2,
  "last_error": "undefined reference to combustionModel",
  "fix_applied": "added -lcombustionModels to Make/options",
  "unit_tests_passed": true,
  "L2_error_vs_reference": 0.031,
  "GCI_percent": null,
  "ready_for_production": false
}
```

---

## 11. Solver Catalogue (Pre-Built Templates)

The agent can instantly scaffold any of these validated solver templates:

| Solver Name | Physics | Base Solver | Complexity |
|-------------|---------|-------------|------------|
| `mhdSimpleFoam` | Incompressible + Lorentz force (low-Rm) | `simpleFoam` | Medium |
| `mhdReactingFoam` | Reacting flow + MHD (arc, plasma) | `reactingFoam` | High |
| `plasmaActuatorFoam` | DBD plasma actuator body force | `simpleFoam` | Low |
| `hyperReactingFoam` | Hypersonic + 5-species chemistry + 2T | `rhoCentralFoam` | Very High |
| `chtRotatingFoam` | CHT + MRF rotating region | `chtMultiRegionSimpleFoam` | High |
| `viscoelasticHeatFoam` | Viscoelastic + viscous heating | `simpleFoam` | Medium |
| `bubblyReactingFoam` | Two-phase + reacting (gas-liquid) | `reactingTwoPhaseEulerFoam` | Very High |
| `ablationFoam` | Hypersonic + surface ablation + pyrolysis | `rhoCentralFoam` | Expert |
| `dsmcReactingFoam` | DSMC + dissociation chemistry | `dsmcFoam` | Expert |
| `magneticConvectionFoam` | MHD + natural convection (liquid metal) | `buoyantSimpleFoam` | Medium |
| `rotorAeroFoam` | AMI rotor + aeroelastic deflection | `pimpleFoam` | High |
| `coupledPlasmaFoam` | RF plasma + fluid + chemistry | custom | Expert |

For each template, the agent:
1. Reads the design intake
2. Selects the appropriate catalogue entry as starting point
3. Applies user-specific physics modifications from §4
4. Runs the agentic build loop from §10