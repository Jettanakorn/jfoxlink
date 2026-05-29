# Thermal & Conjugate Heat Transfer — Full Reference

## Table of Contents
1. [Physics: Heat Transfer Modes](#1-physics)
2. [Solver Selection](#2-solver-selection)
3. [Conjugate Heat Transfer (CHT)](#3-cht)
4. [Natural & Mixed Convection](#4-natural--mixed-convection)
5. [Turbulent Heat Transfer](#5-turbulent-heat-transfer)
6. [Radiation Models](#6-radiation-models)
7. [Phase Change (Boiling, Condensation)](#7-phase-change)
8. [Boundary Conditions](#8-boundary-conditions)
9. [Meshing for Thermal Problems](#9-meshing)
10. [Intake Schema](#10-intake-schema)
11. [Post-Processing: Nusselt, Effectiveness](#11-post-processing)
12. [Validation References](#12-validation)
13. [Fix Library](#13-fix-library)

---

## 1. Physics

### Heat Transfer Modes Summary

| Mode | Governing Equation | Key Dimensionless Group |
|------|-------------------|------------------------|
| Conduction | q = -k∇T | Biot Bi = hL/k |
| Convection (forced) | ρcpU·∇T = ∇·(k∇T) + Φ | Nu = hL/k, Re, Pr |
| Natural convection | + ρg β ΔT | Ra = Gr·Pr, Gr = gβΔTL³/ν² |
| Radiation | q_rad = εσ(T⁴-T_surr⁴) | emissivity ε, view factor F |
| Phase change | L_latent = dH/dt at interface | Ja = cp ΔT / L_lat |

### Energy Equation in OpenFOAM
```
ρ Dh/Dt = Dp/Dt + ∇·(k∇T) + τ:∇U + Q_vol
```
- `h` — specific enthalpy
- `Q_vol` — volumetric heat source (W/m³)
- `τ:∇U` — viscous dissipation (significant for Ma > 0.3 or high-viscosity fluids)

---

## 2. Solver Selection

| Problem | Solver | Notes |
|---------|--------|-------|
| Forced convection, incompressible | `buoyantBoussinesqSimpleFoam` | Boussinesq approx; ΔT < 20K |
| Natural/mixed convection | `buoyantSimpleFoam` | Full buoyancy; large ΔT |
| CHT steady | `chtMultiRegionSimpleFoam` | Solid + fluid regions |
| CHT unsteady | `chtMultiRegionFoam` | Transient multi-region |
| Compressible forced convection | `rhoSimpleFoam` / `rhoPimpleFoam` | Coupled p, T, ρ |
| Phase change (boiling/melting) | `icoReactingMultiphaseInterFoam` | Complex; see Section 7 |
| Radiation only | `buoyantSimpleFoam` + fvDOM | Standard combo |
| Cryogenic flows | `reactingFoam` + real gas EOS | Variable cp, k, μ |

---

## 3. Conjugate Heat Transfer (CHT)

CHT couples a fluid region and one or more solid regions, exchanging heat across
their shared interface.

### Directory Structure

```
CHT_case/
├── system/
│   ├── fluid/     (fvSchemes, fvSolution for fluid)
│   └── solid/     (fvSchemes, fvSolution for solid)
├── constant/
│   ├── fluid/     (thermophysicalProperties, turbulenceProperties)
│   └── solid/     (thermophysicalProperties — solid)
└── 0/
    ├── fluid/     (U, p, T, k, omega, ...)
    └── solid/     (T only)
```

### Interface Boundary Conditions (coupled)
```cpp
// 0/fluid/T — at fluid-solid interface
interface_fluid
{
    type        compressible::turbulentTemperatureCoupledBaffleMixed;
    Tnbr        T;            // field name in neighbouring region
    kappaMethod fluidThermo;  // thermal conductivity from thermophysics
    value       $internalField;
}

// 0/solid/T — at fluid-solid interface (same patch, opposite region)
interface_solid
{
    type        compressible::turbulentTemperatureCoupledBaffleMixed;
    Tnbr        T;
    kappaMethod solidThermo;
    value       $internalField;
}
```

### Solid thermophysicalProperties
```cpp
// constant/solid/thermophysicalProperties
thermoType { type heSolidThermo; mixture pureMixture; transport constIso;
             thermo eConst; equationOfState rhoConst; specie specie; energy sensibleInternalEnergy; }
mixture
{
    specie          { molWeight 55.85; }    // steel: 55.85 g/mol
    equationOfState { rho 7850; }           // density kg/m³
    thermodynamics  { Cv 502; Hf 0; }      // J/(kg·K)
    transport       { kappa 45.0; }         // W/(m·K) — thermal conductivity
}
```

---

## 4. Natural & Mixed Convection

### Boussinesq Approximation (ΔT < 20 K)
```cpp
// constant/thermophysicalProperties (buoyantBoussinesqSimpleFoam)
Prandtl         0.71;
beta            3.3e-3;        // thermal expansion coefficient 1/K (= 1/T_ref for ideal gas)
TRef            300;           // reference temperature K

// constant/g  (gravity)
dimensions [0 1 -2 0 0 0 0];
value       ( 0 -9.81 0 );
```

### Full Buoyancy (large ΔT, liquid metals, etc.)
```cpp
// constant/thermophysicalProperties — use hePsiThermo or heRhoThermo
// Density updated at every iteration from EOS
// Set in fvSolution: SIMPLE { pRefCell 0; pRefValue 0; }
```

### Key Dimensionless Numbers
```python
Re   = U * L / nu
Pr   = nu / alpha         ; alpha = k / (rho * cp)
Gr   = g * beta * dT * L**3 / nu**2
Ra   = Gr * Pr
Nu   = h * L / k          ; h from wall heat flux: h = q_wall / (T_wall - T_bulk)
```

**Natural convection validation:**
- Vertical plate: `Nu = 0.59 * Ra^0.25` (laminar, Ra 10⁴–10⁹)
- Enclosed cavity: see de Vahl Davis (1983) benchmark

---

## 5. Turbulent Heat Transfer

### Wall Treatment for Temperature
```cpp
// 0/T wall patch — high-Re (wall functions):
walls { type  compressible::alphatJayatillekeWallFunction;
        Prt   0.85;
        value $internalField; }

// 0/T wall patch — low-Re (resolve thermal BL, y+ < 1):
walls { type  fixedValue; value uniform 300; }   // or zeroGradient for adiabatic
```

### Turbulent Prandtl Number
```
Prt = 0.85–0.9   for air
Prt = 0.7        for liquid metals (Pr << 1, modified wall treatment needed)
Prt = 1.0        for heavy oils (Pr >> 1)
```

For liquid metals (Pr < 0.1), standard Jayatilleke wall function is inaccurate;
use Weigand–Ferguson–Crawford correlation or DNS-derived correction.

---

## 6. Radiation Models

| Model | Use Case | Cost |
|-------|----------|------|
| `viewFactor` | Enclosure radiation, opaque surfaces | Low |
| `fvDOM` (Discrete Ordinates) | Participating media, combustion gases | Medium |
| `P1` | Optically thick media | Low |
| `Rosseland` | Very optically thick (e.g., glass melt) | Very low |
| `WSGG` | Gas radiation (H2O, CO2) in combustion | Medium |

```cpp
// constant/radiationProperties
radiationModel  fvDOM;
fvDOMCoeffs
{
    nPhi    3;      // azimuthal directions (total = nPhi * nTheta * 4)
    nTheta  3;
    maxIter 4;
    tolerance 1e-3;
}
absorptionEmissionModel constantAbsorptionEmission;
scatterModel    none;
sootModel       none;
```

---

## 7. Phase Change

### Film Boiling / Nucleate Boiling
```cpp
// Use icoReactingMultiphaseInterFoam or Lee model:
phaseChangeModel Lee;
LeeCoeffs { rc 1e4; rv 1e3; }   // condensation/evaporation rate coefficients
```

### Melting / Solidification (Enthalpy-Porosity Method)
```cpp
// Custom fvOptions source (or use OpenFOAM solidificationMeltingSource):
solidificationMeltingSource
{
    type    solidificationMeltingSource;
    active  yes;
    solidificationMeltingSourceCoeffs
    {
        Tsol    1728;    // solidus temperature K (copper example)
        Tliq    1728;    // liquidus temperature K (pure metal)
        L       205000;  // latent heat J/kg
        relax   0.9;
    }
}
```

---

## 8. Boundary Conditions

| Condition | BC Type | Example |
|-----------|---------|---------|
| Fixed wall temperature | `fixedValue` | `T { type fixedValue; value uniform 350; }` |
| Adiabatic wall | `zeroGradient` | `T { type zeroGradient; }` |
| Convective (Robin) | `mixedFixed` or `externalWallHeatFlux` | See below |
| Fixed heat flux | `fixedGradient` or `externalWallHeatFluxTemperature` | `q = 5000 W/m²` |
| Radiation + convection | `externalWallHeatFluxTemperature` | Combined h+ε |

```cpp
// Convective outer wall BC (film cooling external side):
outer_wall
{
    type            externalWallHeatFluxTemperature;
    mode            coefficient;
    Ta              uniform 300;        // ambient temperature
    h               uniform 15;         // external heat transfer coeff W/(m²K)
    emissivity      0.85;               // surface emissivity
    thicknessLayers ( 0.002 );          // wall thickness m (for 1D conduction)
    kappaLayers     ( 16.0  );          // kappa of wall layers W/(m·K)
    value           uniform 300;
}
```

---

## 9. Meshing for Thermal Problems

**Thermal boundary layer thickness estimate:**
```
δ_T ≈ δ_u / Pr^(1/3)     ; for Pr > 0.6 (gases)
δ_T ≈ δ_u * Pr^(-1/2)    ; for Pr << 1 (liquid metals)
```

**CHT interface meshing requirement:**
- Fluid and solid meshes must share the same face topology at the interface
- In `chtMultiRegionSimpleFoam`, interface faces must be 1:1 conformal OR use AMI coupling

**y+ guidance for heat transfer:**
- Resolve thermal BL: `y+ <= 1` (mandatory for Pr >> 1 or Pr << 1)
- Wall functions adequate for `Pr ≈ 0.7` (air) at `y+ 30–300`

---

## 10. Intake Schema

```yaml
thermal_intake:
  problem_type: "CHT"            # "forced_convection" | "CHT" | "natural_convection" | "radiation"
  fluid: "air"                   # or "water", "liquid_sodium", custom
  solid_material: "steel"        # or "aluminum", "ceramic", "CFRP"
  T_inlet_K: 800                 # hot gas inlet temperature
  T_wall_ambient_K: 300          # external ambient
  U_inlet_m_s: 100
  Re: null                       # auto-computed if null
  Pr: 0.71
  heat_flux_target_W_m2: 5e5     # max allowable heat flux at hot surface
  Nu_target: null                # for validation cases
  radiation: true
  boiling: false
  max_T_solid_K: 1200            # design limit for solid
  convergence_residual: 1e-6
  hpc_cores: 32
```

---

## 11. Post-Processing: Nusselt, Effectiveness

```bash
# Wall heat flux
postProcess -func wallHeatFlux -case . -latestTime

# Nusselt number (from heat flux and bulk-to-wall dT):
python3 << 'EOF'
import numpy as np
q_wall = 5e5        # W/m² from wallHeatFlux output
T_bulk = 600        # K
T_wall = 900        # K
L_ref  = 0.05       # m (hydraulic diameter)
k_fluid = 0.057     # W/(m·K) at mean temperature
h = q_wall / (T_wall - T_bulk)
Nu = h * L_ref / k_fluid
print(f"h={h:.1f} W/(m²K), Nu={Nu:.1f}")
EOF
```

### Cooling Effectiveness (Film Cooling)
```
η = (T_aw - T_g) / (T_c - T_g)
; T_aw = adiabatic wall temperature, T_g = hot gas temp, T_c = coolant temp
```

---

## 12. Validation References

| Case | Reference | Metrics |
|------|-----------|---------|
| Backward-facing step (heated) | Vogel & Eaton (1985) | Nu(x), Stanton number |
| Turbulent channel CHT | Kim & Moin (1987) DNS | T profile, q_wall |
| Natural convection cavity | de Vahl Davis (1983) | Nu_avg vs Ra |
| Film cooling flat plate | Goldstein et al. (1974) | Effectiveness η |
| Pin-fin heat exchanger | VDI Heat Atlas (2010) | ΔP, Nu |
| Impingement cooling | Martin (1977) | Nu(r/D) |

---

## 13. Fix Library

| Symptom | Cause | Fix |
|---------|-------|-----|
| CHT interface T discontinuity | Mesh non-conformal; wrong coupled BC | Verify both patches use `turbulentTemperatureCoupledBaffleMixed` |
| Temperature blow-up in solid | kappa too low; heat accumulation | Check kappa units: W/(m·K); verify solid mesh is enclosed |
| Nu unrealistically high | y+ too high for Prt correction | Reduce y+ to <1 for high-Pr fluids; or use Kader wall function |
| Boussinesq fails | ΔT too large (>20 K for gases) | Switch to `buoyantSimpleFoam` (full density coupling) |
| Radiation non-convergence | fvDOM not enough ordinates | Increase nPhi, nTheta to 6; or switch to P1 for quick check |
| T oscillation at solid-fluid interface | Under-relaxation too high | Set T relaxation to 0.5 in fvSolution of both regions |
| `chtMultiRegionFoam` missing patch error | Region boundary patch names mismatch | Ensure patch names in constant/*/polyMesh/boundary are consistent |