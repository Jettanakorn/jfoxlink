# Numerics Reference — fvSchemes & fvSolution

## Table of Contents
1. [Scheme Selection Philosophy](#1-scheme-philosophy)
2. [simpleFoam / RANS Steady fvSchemes](#2-rans-steady)
3. [pimpleFoam / LES / DES fvSchemes](#3-les-des)
4. [rhoSimpleFoam Compressible fvSchemes](#4-compressible)
5. [rhoCentralFoam High-Speed fvSchemes](#5-high-speed)
6. [Linear Solver Selection](#6-linear-solvers)
7. [Relaxation & Under-Relaxation](#7-relaxation)
8. [Troubleshooting Divergence](#8-divergence)

---

## 1. Scheme Selection Philosophy

### Accuracy vs Stability Triangle
```
High accuracy (2nd order linear)
        ↑
        |  Goal: stay here ←→ on structured, high-quality meshes
        |
        |
Low quality mesh → move toward limitedLinear or upwind
        ↓
Low accuracy but unconditionally stable (upwind 1st order)
```

**Rule of thumb for aerospace:**
- Use **Gauss linearUpwindV grad(U)** for momentum (bounded, 2nd order in smooth regions)
- Use **Gauss upwind** for turbulence quantities (k, ω) — they don't need high accuracy
- NEVER use pure upwind for final production runs (excessive diffusion → wrong Cl/Cd)

---

## 2. RANS Steady — simpleFoam / rhoSimpleFoam fvSchemes

```cpp
FoamFile { version 2.0; format ascii; class dictionary; object fvSchemes; }

ddtSchemes
{
    default         steadyState;    // pseudo-time for SIMPLE
}

gradSchemes
{
    default         Gauss linear;
    // For robustness on poor mesh cells near walls:
    grad(U)         cellLimited Gauss linear 1;
    grad(p)         Gauss linear;
}

divSchemes
{
    default         none;          // explicit — forces you to specify each term
    
    // Momentum advection — 2nd order bounded
    div(phi,U)      Gauss linearUpwindV grad(U);
    // Alternative (more stable on poor meshes):
    // div(phi,U)   Gauss limitedLinearV 1;
    
    // Turbulence — 1st order upwind (sufficient)
    div(phi,k)      Gauss upwind;
    div(phi,omega)  Gauss upwind;
    div(phi,nuTilda) Gauss upwind;
    div(phi,epsilon) Gauss upwind;
    
    // Compressible terms (rhoSimpleFoam)
    div(phi,e)      Gauss upwind;
    div(phi,h)      Gauss upwind;
    
    // Viscous stress
    div((nuEff*dev2(T(grad(U))))) Gauss linear;
    // Compressible:
    div(((rho*nuEff)*dev2(T(grad(U))))) Gauss linear;
}

laplacianSchemes
{
    default         Gauss linear corrected;     // corrected for non-orthogonal
    // If max non-ortho > 70°: use 'limited 0.333' or 'uncorrected'
    // default      Gauss linear limited 0.333;
}

interpolationSchemes
{
    default         linear;
}

snGradSchemes
{
    default         corrected;
    // If max non-ortho > 70°: limited 0.333
}

fluxRequired
{
    default         no;
    p               ;           // p fluxes needed for SIMPLE
    rho             ;           // (compressible only)
}
```

---

## 3. LES / DES / Unsteady (pimpleFoam) fvSchemes

```cpp
ddtSchemes
{
    default         backward;   // 2nd order implicit — required for LES
    // Euler is 1st order, only for startup/restart
}

gradSchemes
{
    default         Gauss linear;
}

divSchemes
{
    default             none;
    
    // LES momentum — use central/bounded linear (low dissipation)
    div(phi,U)          Gauss linear;            // pure central (LES ideal)
    // OR bounded variant:
    // div(phi,U)        Gauss linearUpwindV grad(U);  // more stable DES
    
    // SGS model terms
    div(phi,k)          Gauss upwind;
    div(phi,B)          Gauss linear;
    div(B)              Gauss linear;
    div(phi,nuTilda)    Gauss upwind;       // DES SA model
    div(phi,omega)      Gauss upwind;       // DES SST model
    
    // Viscous
    div((nuEff*dev(T(grad(U))))) Gauss linear;
}

laplacianSchemes
{
    default         Gauss linear corrected;
}

snGradSchemes
{
    default         corrected;
}
```

---

## 4. Compressible Subsonic fvSchemes (rhoSimpleFoam / rhoPimpleFoam)

Same as RANS steady but add enthalpy/energy terms:

```cpp
divSchemes
{
    div(phi,U)      Gauss linearUpwindV grad(U);
    div(phi,h)      Gauss upwind;           // enthalpy
    div(phi,e)      Gauss upwind;           // or internal energy
    div(phi,K)      Gauss upwind;           // kinetic energy term
    div(phid,p)     Gauss upwind;           // pressure-work term
    div(phi,k)      Gauss upwind;
    div(phi,omega)  Gauss upwind;
    div(((rho*nuEff)*dev2(T(grad(U))))) Gauss linear;
}
```

---

## 5. High-Speed / Supersonic fvSchemes (rhoCentralFoam)

```cpp
fluxScheme      Kurganov;   // OR AUSM — at top of fvSchemes

ddtSchemes    { default Euler; }
gradSchemes   { default Gauss linear; }

divSchemes
{
    default                     none;
    div(tauMC)                  Gauss linear;   // viscous stress
}

laplacianSchemes { default Gauss linear corrected; }

interpolationSchemes
{
    default             linear;
    // TVD reconstruction for shock-capturing:
    reconstruct(rho)    vanLeer;    // density — needs TVD at shocks
    reconstruct(U)      vanLeerV;   // velocity vector
    reconstruct(T)      vanLeer;    // temperature
    reconstruct(p)      vanLeer;    // pressure
}
snGradSchemes { default corrected; }
```

---

## 6. Linear Solver Selection

### GAMG (Generalised Algebraic Multigrid) — Pressure

```cpp
p
{
    solver              GAMG;
    smoother            GaussSeidel;
    nPreSweeps          0;
    nPostSweeps         2;
    cacheAgglomeration  on;
    agglomerator        faceAreaPair;
    nCellsInCoarsestLevel 10;
    mergeLevels         1;
    tolerance           1e-8;
    relTol              0.01;   // 0 for final iteration / unsteady
}
```

### smoothSolver — Velocity & Turbulence

```cpp
U
{
    solver          smoothSolver;
    smoother        symGaussSeidel;   // or GaussSeidel
    nSweeps         1;
    tolerance       1e-8;
    relTol          0.01;
}
```

### PCG / PBiCGStab — Alternative (sometimes faster)

```cpp
p
{
    solver          PCG;
    preconditioner  DIC;    // Diagonal Incomplete-Cholesky for symmetric
    tolerance       1e-8;
    relTol          0.01;
}
U
{
    solver          PBiCGStab;
    preconditioner  DILU;   // Diagonal Incomplete LU for asymmetric
    tolerance       1e-8;
    relTol          0.01;
}
```

---

## 7. Relaxation & Under-Relaxation

### Effect on Convergence Speed vs Stability
| relax factor | Stability | Convergence Speed | Use Case |
|-------------|-----------|-------------------|----------|
| p = 0.1–0.2 | High | Slow | Complex geom, first run |
| p = 0.3 | Good | Moderate | **Standard aerospace** |
| p = 0.5 | Moderate | Fast | Simple geometry, fine mesh |
| U = 0.5 | High | Slow | Diverging cases |
| U = 0.7 | Good | Moderate | **Standard aerospace** |
| U = 0.9 | Low | Fast | Nearly converged restart |

### SIMPLEC (consistent SIMPLE) — Recommended
```cpp
SIMPLE { consistent yes; }
// SIMPLEC allows higher relaxation factors
// Equivalent stability with p=0.5 instead of p=0.3
```

### Unsteady: No Under-Relaxation
For pimpleFoam with **nOuterCorrectors = 1** (pure PISO):
```cpp
relaxationFactors {}   // empty — no relaxation for unsteady PISO
```
For **nOuterCorrectors > 1** (PIMPLE iterations):
```cpp
relaxationFactors
{
    equations { U 0.9; k 0.7; omega 0.7; }  // only equations, not fields
}
```

---

## 8. Troubleshooting Divergence

### Symptom → Cause → Fix

| Symptom | Likely Cause | Fix |
|---------|-------------|-----|
| Divergence in first 10 iter | Cold start | Use `potentialFoam` init; lower relax to 0.1/0.3 |
| p diverges, U okay | Pressure-velocity decoupling | Add `nNonOrthogonalCorrectors 2`; lower p relax |
| Bounding k | Negative k due to instability | Lower k relax to 0.3; check mesh quality |
| `GAMG: Solving ... singular` | Floating reference pressure | Set `pRefCell 0; pRefValue 0` in SIMPLE/PIMPLE |
| NaN after N steps | High Co or bad cell | Check `checkMesh`; lower deltaT; inspect bad cells |
| Oscillating residuals (not converging) | Inherently unsteady flow | Switch to pimpleFoam; or acceptably oscillating |
| Converged residuals but wrong Cl/Cd | Wrong reference conditions | Check Aref, lRef, rhoInf, liftDir, dragDir |
| High divergence at outlet | Backflow at outlet | Use `inletOutlet` BC on velocity; extend domain |

### Emergency Stabilization Settings
```cpp
// Temporary — use to restart, then relax back to normal values
relaxationFactors
{
    fields      { p 0.1; rho 0.2; }
    equations   { U 0.3; k 0.2; omega 0.2; nuTilda 0.3; e 0.2; }
}
SIMPLE { nNonOrthogonalCorrectors 3; }
```