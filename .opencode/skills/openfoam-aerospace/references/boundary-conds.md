# Boundary Conditions Reference — Aerospace OpenFOAM

## Table of Contents
1. [Incompressible External Aero (simpleFoam / pimpleFoam)](#1-incompressible)
2. [Compressible Subsonic (rhoSimpleFoam)](#2-compressible-subsonic)
3. [Transonic / Supersonic (rhoCentralFoam)](#3-transonic-supersonic)
4. [Inlet Turbulence Specification](#4-turbulence-inlet)
5. [Symmetry & Periodic BCs](#5-symmetry-periodic)
6. [Moving Wall / Rotating Domain](#6-moving-wall)

---

## 1. Incompressible External Aerodynamics

### Flow conditions: U=50 m/s, AoA=5°, Re=3.3×10⁶, ν=1.5×10⁻⁵ m²/s

**`0/U`** (velocity)
```cpp
FoamFile { version 2.0; format ascii; class volVectorField; object U; }
dimensions [0 1 -1 0 0 0 0];

// Decompose into x and z for angle of attack α=5°
// U = (U×cos(α), 0, U×sin(α))
// cos(5°)=0.9962, sin(5°)=0.08716
internalField uniform (49.81 0 4.358);

boundaryField
{
    inlet
    {
        type        fixedValue;
        value       uniform (49.81 0 4.358);
    }
    outlet
    {
        type        inletOutlet;           // handles backflow
        inletValue  uniform (49.81 0 4.358);
        value       uniform (49.81 0 4.358);
    }
    wing
    {
        type        noSlip;               // no-slip wall
    }
    top
    {
        type        slip;                 // or fixedValue if inlet-like
    }
    bottom
    {
        type        slip;
    }
    farfield
    {
        type        freestreamVelocity;
        freestreamValue uniform (49.81 0 4.358);
    }
    symmetry
    {
        type        symmetryPlane;
    }
    frontAndBack          // 2D cases — empty patches
    {
        type        empty;
    }
}
```

**`0/p`** (pressure — kinematic, Pa/ρ for incompressible)
```cpp
FoamFile { version 2.0; format ascii; class volScalarField; object p; }
dimensions [0 2 -2 0 0 0 0];   // kinematic pressure [m²/s²]
internalField uniform 0;

boundaryField
{
    inlet      { type zeroGradient; }
    outlet     { type fixedValue; value uniform 0; }   // reference pressure = 0
    wing       { type zeroGradient; }
    top        { type zeroGradient; }
    bottom     { type zeroGradient; }
    farfield   { type freestreamPressure; }
    symmetry   { type symmetryPlane; }
    frontAndBack { type empty; }
}
```

---

## 2. Compressible Subsonic (rhoSimpleFoam / rhoPimpleFoam)

### Flow conditions: Ma=0.5, T∞=288.15K, p∞=101325 Pa, AoA=3°

**`0/U`**
```cpp
// a = sqrt(γRT) = sqrt(1.4 × 287 × 288.15) = 340.3 m/s
// U = Ma × a = 0.5 × 340.3 = 170.2 m/s
// Ux = 170.2 × cos(3°) = 170.0, Uz = 170.2 × sin(3°) = 8.91
internalField uniform (170.0 0 8.91);
boundaryField
{
    inlet    { type fixedValue; value uniform (170.0 0 8.91); }
    outlet   { type inletOutlet; inletValue uniform (170.0 0 8.91); value uniform (170.0 0 8.91); }
    wing     { type noSlip; }
    farfield { type freestreamVelocity; freestreamValue uniform (170.0 0 8.91); }
}
```

**`0/p`** (static pressure, Pa for compressible)
```cpp
FoamFile { version 2.0; format ascii; class volScalarField; object p; }
dimensions [1 -1 -2 0 0 0 0];   // [Pa] for compressible
internalField uniform 101325;
boundaryField
{
    inlet    { type fixedValue; value uniform 101325; }  // total or static depending on BC type
    outlet   { type fixedValue; value uniform 101325; }
    wing     { type zeroGradient; }
    farfield { type freestream; freestreamValue uniform 101325; }
}
```

**`0/T`** (temperature)
```cpp
FoamFile { version 2.0; format ascii; class volScalarField; object T; }
dimensions [0 0 0 1 0 0 0];
internalField uniform 288.15;
boundaryField
{
    inlet    { type fixedValue; value uniform 288.15; }
    outlet   { type inletOutlet; inletValue uniform 288.15; value uniform 288.15; }
    wing     { type zeroGradient; }               // adiabatic wall
    // wing  { type fixedValue; value uniform 300; }   // isothermal wall
    farfield { type freestream; freestreamValue uniform 288.15; }
}
```

**`0/rho`** (density — computed from p and T via thermophysics, but may need IC)
```cpp
internalField uniform 1.225;   // ρ = p/(RT) = 101325/(287×288.15)
boundaryField
{
    inlet    { type fixedValue; value uniform 1.225; }
    outlet   { type inletOutlet; inletValue uniform 1.225; value uniform 1.225; }
    wing     { type zeroGradient; }
    farfield { type freestream; freestreamValue uniform 1.225; }
}
```

**`constant/thermophysicalProperties`** (for compressible solvers)
```cpp
thermoType
{
    type            hePsiThermo;
    mixture         pureMixture;
    transport       sutherland;      // Sutherland law — best for aerospace
    thermo          janaf;           // or hConst for constant Cp
    equationOfState perfectGas;
    specie          specie;
    energy          sensibleEnthalpy;
}
mixture
{
    specie      { nMoles 1; molWeight 28.97; }
    thermodynamics
    {
        Tlow    200; Thigh 6000; Tcommon 1000;
        // JANAF coefficients for air (standard)
        highCpCoeffs (3.5 0 0 0 0 -1047.5 4.36);
        lowCpCoeffs  (3.5 0 0 0 0 -1047.5 4.36);
    }
    transport
    {
        As  1.458e-6;    // Sutherland coefficient
        Ts  110.4;       // Sutherland temperature [K]
    }
}
```

**`constant/transportProperties`** (incompressible only)
```cpp
nu [0 2 -1 0 0 0 0] 1.5e-5;   // kinematic viscosity air at sea level
```

---

## 3. Transonic / Supersonic (rhoCentralFoam)

### Flow conditions: Ma=1.5 (supersonic), p∞=101325 Pa, T∞=288.15K

**`0/U`** — All-Dirichlet at inlet, no-gradient at outlet (supersonic outflow)
```cpp
internalField uniform (510.5 0 0);   // Ma=1.5 × a=340.3
boundaryField
{
    inlet    { type fixedValue; value uniform (510.5 0 0); }
    outlet   { type zeroGradient; }   // supersonic: no upstream influence
    wing     { type noSlip; }
}
```

**`0/p`** — for supersonic, all quantities specified at inlet
```cpp
boundaryField
{
    inlet    { type fixedValue; value uniform 101325; }
    outlet   { type zeroGradient; }   // supersonic outflow
    wing     { type zeroGradient; }
}
```

### rhoCentralFoam system/fvSchemes (essential for stability)
```cpp
fluxScheme    Kurganov;    // or AUSM — central-upwind for high-speed flows
ddtSchemes    { default Euler; }
gradSchemes   { default Gauss linear; }
divSchemes    { default Gauss linear; }   // advection handled by flux scheme
laplacianSchemes { default Gauss linear corrected; }
```

---

## 4. Turbulence Inlet Specification

### Compute from Freestream Conditions
| Quantity | Formula | Typical Range |
|----------|---------|---------------|
| Turbulence Intensity I | 0.001–0.05 | 0.1% (freestream) to 5% (wind tunnel) |
| k | 1.5 × (U × I)² | varies |
| ω | k^0.5 / (Cmu^0.25 × L) | L = 0.07×chord (or duct diameter) |
| ε | Cmu^0.75 × k^1.5 / L | Cmu = 0.09 |
| nuTilda (SA) | 3–5 × ν_molecular | ~5e-5 m²/s for air at sea level |

### turbulentIntensityKineticEnergyInlet (alternative)
```cpp
// 0/k — inlet
{
    type        turbulentIntensityKineticEnergyInlet;
    intensity   0.005;    // 0.5%
    value       uniform 0.00015;
}
```

---

## 5. Symmetry & Periodic BCs

### Symmetry Plane (half-span model)
```cpp
// 0/U, 0/p, 0/k, etc. — symmetry patch
symmetryPlane { type symmetryPlane; }
// In system/createPatchDict, ensure patch type = symmetryPlane in polyMesh
```

### Cyclic (Periodic) — span-periodic cases
```cpp
// In blockMeshDict or createPatchDict:
periodicLeft  { type cyclic; neighbourPatch periodicRight; }
periodicRight { type cyclic; neighbourPatch periodicLeft; }

// 0/U on these patches:
periodicLeft  { type cyclic; }
periodicRight { type cyclic; }
```

### AMI (Arbitrary Mesh Interface) — rotating/sliding interfaces
```cpp
// For MRF or rotating mesh:
interface_rotating { type cyclicAMI; neighbourPatch interface_static; }
interface_static   { type cyclicAMI; neighbourPatch interface_rotating; }
```

---

## 6. Moving Wall / Rotating Domain

### Stationary Reference Frame with MRF (propeller, fan)
```cpp
// constant/MRFProperties
MRF1
{
    cellZone    rotatingZone;       // must be defined in mesh
    active      yes;
    nonRotatingPatches (inlet outlet farfield);
    origin      (0 0 0);
    axis        (1 0 0);            // rotation axis (x-axis for propeller)
    omega       523.6;              // rad/s = 5000 RPM × 2π/60
}
```

**`0/U` for rotating wall** (use if not MRF but explicit wall motion)
```cpp
propellerHub { type movingWallVelocity; value uniform (0 0 0); }
```

### Actuator Disk (simpler propeller model)
```cpp
// fvOptions
disk1
{
    type            actuationDiskSource;
    active          true;
    selectionMode   cellZone;
    cellZone        diskZone;
    diskDir         (1 0 0);
    Cp              0.1;
    Ct              0.5;
    diskArea        1.0;
    upstreamPoint   (-0.1 0 0);
}
```