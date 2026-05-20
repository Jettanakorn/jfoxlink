# Solver Selection Reference — OpenFOAM Aerospace

## Solver Decision Matrix

| Flow Type | Ma Range | Steady/Unsteady | Solver | Notes |
|-----------|----------|-----------------|--------|-------|
| Incompressible external aero | < 0.3 | Steady | **simpleFoam** | Most common for wing/airfoil |
| Incompressible external aero | < 0.3 | Unsteady | **pimpleFoam** | Wake, flutter, unsteady |
| Compressible subsonic | 0.3–0.8 | Steady | **rhoSimpleFoam** | Correct thermodynamics |
| Compressible subsonic | 0.3–0.8 | Unsteady | **rhoPimpleFoam** | Buffet, aeroelastic |
| Transonic | 0.8–1.2 | Steady | **rhoSimpleFoam** or **rhoCentralFoam** | Shock-capturing needed |
| Supersonic | > 1.2 | Steady | **rhoCentralFoam** | AUSM+/Kurganov flux |
| Hypersonic | > 5 | Unsteady | **rhoCentralFoam** + real gas | Requires custom EOS |
| Propeller/Fan (MRF) | any | Steady | **MRFSimpleFoam** | Rotating reference frame |
| Propeller (sliding mesh) | any | Unsteady | **pimpleFoam** + AMI | Blade-resolved unsteady |
| Heat transfer / CHT | any | Steady | **chtMultiRegionFoam** | Multi-region solver |
| Acoustics / noise | < 0.3 | Unsteady + post | **pimpleFoam** + FW-H | Ffowcs-Williams Hawkings |

---

## simpleFoam — Full controlDict Template

```cpp
/*--------------------------------*- C++ -*----------------------------------*\
  simpleFoam for NACA 0012, Re=6e6, AoA=5°
\*---------------------------------------------------------------------------*/
FoamFile { version 2.0; format ascii; class dictionary; object controlDict; }

application     simpleFoam;
startFrom       startTime;       // or latestTime (restart)
startTime       0;
stopAt          endTime;
endTime         3000;            // iterations for RANS steady-state
deltaT          1;               // iteration step (pseudo-time)
writeControl    timeStep;
writeInterval   500;             // write every 500 iterations
purgeWrite      3;               // keep only last 3 time directories
writeFormat     ascii;           // or binary (faster I/O)
writePrecision  8;
writeCompression off;
timeFormat      general;
timePrecision   6;
runTimeModifiable true;

functions
{
    #include "forceCoeffs"       // put forceCoeffs dict in system/
    #include "residuals"
    #include "yPlus"
}
```

### simpleFoam system/fvSolution
```cpp
FoamFile { version 2.0; format ascii; class dictionary; object fvSolution; }

solvers
{
    p
    {
        solver          GAMG;
        smoother        GaussSeidel;
        tolerance       1e-8;
        relTol          0.01;
    }
    U
    {
        solver          smoothSolver;
        smoother        symGaussSeidel;
        tolerance       1e-8;
        relTol          0.01;
    }
    "(k|omega|nuTilda)"
    {
        solver          smoothSolver;
        smoother        symGaussSeidel;
        tolerance       1e-8;
        relTol          0.01;
    }
}

SIMPLE
{
    nNonOrthogonalCorrectors 2;     // increase if high non-ortho mesh
    consistent               yes;   // SIMPLEC — better for aerospace
    residualControl
    {
        p       1e-5;
        U       1e-5;
        "(k|omega|nuTilda)" 1e-5;
    }
}

relaxationFactors
{
    fields      { p 0.3; }
    equations   { U 0.7; k 0.5; omega 0.5; nuTilda 0.7; }
}
```

---

## pimpleFoam — Unsteady Template

```cpp
application     pimpleFoam;
startFrom       latestTime;
endTime         0.5;             // physical seconds
deltaT          1e-4;            // Δt for Co<1 in resolved region
adjustTimeStep  yes;
maxCo           0.9;             // Courant number control
maxDeltaT       1e-3;
writeControl    adjustableRunTime;
writeInterval   0.01;            // write every 0.01 s
```

### pimpleFoam fvSolution
```cpp
PIMPLE
{
    nOuterCorrectors    2;       // 1 = PISO, 2-3 = PIMPLE
    nCorrectors         2;
    nNonOrthogonalCorrectors 1;
    pRefCell            0;
    pRefValue           0;
}
relaxationFactors
{
    // For PIMPLE (nOuterCorrectors > 1) apply relaxation
    equations { U 0.9; k 0.7; omega 0.7; }
    // For PISO (nOuterCorrectors = 1): NO relaxation needed
}
```

---

## rhoSimpleFoam — Compressible Steady

```cpp
application     rhoSimpleFoam;
endTime         5000;

functions { #include "forceCoeffs" #include "residuals" }
```

### rhoSimpleFoam fvSolution
```cpp
solvers
{
    p
    {
        solver          GAMG;
        smoother        GaussSeidel;
        tolerance       1e-8;
        relTol          0.01;
    }
    U { solver smoothSolver; smoother symGaussSeidel; tolerance 1e-8; relTol 0.01; }
    e { solver smoothSolver; smoother symGaussSeidel; tolerance 1e-8; relTol 0.01; }
    rho { solver diagonal; }
    "(k|omega)" { solver smoothSolver; smoother symGaussSeidel; tolerance 1e-8; relTol 0.01; }
}

SIMPLE
{
    nNonOrthogonalCorrectors 0;
    rhoMin  0.1;
    rhoMax  10.0;
    transonic yes;   // IMPORTANT: set yes for Ma > 0.5
    residualControl
    {
        p 1e-5; U 1e-5; e 1e-5; rho 1e-5;
    }
}

relaxationFactors
{
    fields      { p 0.3; rho 0.3; rhoU 0.3; rhoE 0.3; }
    equations   { U 0.5; e 0.5; k 0.4; omega 0.4; }
}
```

---

## rhoCentralFoam — High-Speed / Supersonic

Density-based solver with central-upwind (Kurganov-Tadmor) or AUSM scheme.
No SIMPLE loop — density-based time stepping.

```cpp
application     rhoCentralFoam;
startFrom       startTime;
endTime         0.01;            // physical time to reach quasi-steady
deltaT          1e-7;            // very small Δt for high-speed flows
adjustTimeStep  yes;
maxCo           0.4;             // lower Co for stability
writeInterval   0.001;
```

### rhoCentralFoam fvSchemes (critical for stability)
```cpp
fluxScheme      Kurganov;       // central-upwind, good for all Ma

ddtSchemes    { default         Euler; }
gradSchemes   { default         Gauss linear; }
divSchemes
{
    default                     none;
    div(tauMC)                  Gauss linear;
}
laplacianSchemes { default      Gauss linear corrected; }
interpolationSchemes
{
    default         linear;
    reconstruct(rho) vanLeer;   // TVD limiter for density
    reconstruct(U)  vanLeerV;
    reconstruct(T)  vanLeer;
}
snGradSchemes { default corrected; }
```

### rhoCentralFoam fvSolution
```cpp
solvers
{
    "(rho|rhoU|rhoE)"
    {
        solver          diagonal;  // explicit — no linear solve needed
    }
    "(U|e|k|omega)"
    {
        solver          smoothSolver;
        smoother        symGaussSeidel;
        tolerance       1e-10;
        relTol          0;
    }
}
```

---

## residuals Function Object

```cpp
// system/residuals (included from controlDict)
residuals
{
    type            residuals;
    libs            ("libutilityFunctionObjects.so");
    writeControl    timeStep;
    writeInterval   1;
    fields          (p U k omega nuTilda e T rho);
}
```

## yPlus Function Object

```cpp
yPlus
{
    type            yPlus;
    libs            ("libfieldFunctionObjects.so");
    writeControl    writeTime;
}
```