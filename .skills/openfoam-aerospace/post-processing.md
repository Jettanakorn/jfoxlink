# Post-Processing Reference — Aerospace OpenFOAM

## Table of Contents
1. [Force & Moment Coefficients](#1-force-coefficients)
2. [Pressure Coefficient Cp](#2-pressure-coefficient)
3. [y+ Distribution](#3-yplus)
4. [Velocity Profiles & Boundary Layer](#4-velocity-profiles)
5. [Wake Survey / Sampling Lines](#5-wake-survey)
6. [ParaView Post-Processing](#6-paraview)
7. [Aeroacoustics (FW-H)](#7-acoustics)
8. [Python Post-Processing Scripts](#8-python)

---

## 1. Force & Moment Coefficients

### Full forceCoeffs Dictionary
Save as `system/forceCoeffs` and `#include` from `controlDict`.

```cpp
forceCoeffs
{
    type            forceCoeffs;
    libs            ("libforces.so");
    writeControl    timeStep;
    writeInterval   1;
    log             true;
    
    // --- GEOMETRY ---
    patches         (wing);              // list of wall patches
    
    // --- REFERENCE FRAME ---
    // For AoA = α degrees, flow along x:
    // liftDir = (-sin(α), 0, cos(α))
    // dragDir = ( cos(α), 0, sin(α))
    // α = 5°:
    liftDir         (-0.0872  0  0.9962);   // z-component lift at 5° AoA
    dragDir         ( 0.9962  0  0.0872);
    pitchAxis       (0 1 0);
    CofR            (0.25 0 0);           // quarter-chord moment reference
    
    // --- REFERENCE CONDITIONS ---
    rho             rhoInf;              // incompressible: use rhoInf
    rhoInf          1.225;               // [kg/m³] ISA sea level
    magUInf         50.0;                // [m/s] freestream speed
    lRef            1.0;                 // [m] reference chord
    Aref            0.1;                 // [m²] reference area (span × chord for 2D: 1×chord)
    
    // For compressible (rho already in field):
    // rho          rho;                 // use field density
}
```

### Output Location & Format
```
postProcessing/forceCoeffs/0/forceCoeffs.dat
```
Columns: `Time  Cm  Cd  Cl  Cl(f)  Cl(r)`

### Extract Final Converged Values (bash)
```bash
# Get last 100 iterations, average Cl and Cd
tail -100 postProcessing/forceCoeffs/0/forceCoeffs.dat | \
  awk '{sum_Cd+=$3; sum_Cl+=$4; n++} END {print "Cd="sum_Cd/n, "Cl="sum_Cl/n}'
```

### AoA Sweep: Multiple Cases Script
```bash
for aoa in 0 2 4 6 8 10 12; do
    mkdir -p case_aoa${aoa}
    cp -r case_base/. case_aoa${aoa}/
    # Modify 0/U with correct velocity components
    python set_aoa.py case_aoa${aoa} ${aoa}
    cd case_aoa${aoa}
    simpleFoam > log.simpleFoam 2>&1
    cd ..
done
```

---

## 2. Pressure Coefficient Cp

### Function Object for Cp on Surface
```cpp
// system/Cp
Cp
{
    type            pressure;
    libs            ("libfieldFunctionObjects.so");
    writeControl    writeTime;
    mode            staticCoeff;     // Cp = (p - p_inf) / (0.5 rho U²)
    
    // Reference conditions
    pRef            101325;          // [Pa] — compressible, or 0 for incompressible
    rhoRef          1.225;           // [kg/m³]
    Uref            50.0;            // [m/s]
    
    result          Cp;              // field name in output
}
```

For **incompressible** (p = p/ρ is kinematic):
```cpp
// Cp = (p - p_ref) / (0.5 * U_ref²)
// where p is kinematic pressure from simpleFoam
mode    staticCoeff;
pRef    0;       // reference gauge pressure
rhoRef  1;       // divide out the density (since p is kinematic)
Uref    50.0;
```

### Sample Cp on Airfoil Surface (spanwise strip)
```cpp
// system/sampleSurface
sampleSurface
{
    type            surfaces;
    libs            ("libsampling.so");
    writeControl    writeTime;
    
    fields          (p Cp U);
    
    surfaces
    (
        wingSection
        {
            type        cuttingPlane;
            planeType   pointAndNormal;
            pointAndNormalDict { point (0 0.5 0); normal (0 1 0); }  // cut at mid-span
            interpolate true;
        }
    );
}
```

---

## 3. y+ Distribution

### yPlus Function Object
```cpp
yPlus
{
    type            yPlus;
    libs            ("libfieldFunctionObjects.so");
    writeControl    writeTime;
    patches         (wing);     // optional: compute on specific patches only
}
```

Post-run, view y+ field in ParaView: `postProcessing/yPlus/*/yPlus.vtk`

### Command-Line y+ (after simulation)
```bash
yPlus -latestTime       # OpenFOAM v6 and earlier
postProcess -func yPlus -latestTime   # OpenFOAM v7+
```

### Check Average y+ on Patch
```bash
postProcess -func "patchIntegrate(name=wing,field=yPlus)" -latestTime
```

---

## 4. Velocity Profiles & Boundary Layer

### Sample Line Through Boundary Layer
```cpp
// system/sampleDict (or include in functions{})
sampleLines
{
    type            sets;
    libs            ("libsampling.so");
    writeControl    writeTime;
    fields          (U k omega nut);
    interpolationScheme cellPointFace;
    
    sets
    (
        BL_x0p5c   // boundary layer profile at x/c = 0.5
        {
            type    face;
            axis    z;              // normal direction (wall-normal)
            start   (0.5 0.5 0);   // start at wall
            end     (0.5 0.5 0.3); // extend 0.3m wall-normal
            nPoints 100;
        }
    );
}
```

### Log-Law Verification
```
y+ range | Expected u+ | Notes
< 5      | y+          | viscous sublayer: u+ = y+
5-30     | transition  | buffer layer
30-300   | 1/κ ln(y+)+B| log-law: u+ = 2.44 ln(y+) + 5.2
```
Plot u+ vs log(y+) to verify wall treatment.

---

## 5. Wake Survey

### Downstream Sampling Planes
```cpp
wakePlanes
{
    type        surfaces;
    libs        ("libsampling.so");
    writeControl writeTime;
    fields      (U p k TotalPressure);
    
    surfaces
    (
        wake_1c    // 1 chord downstream
        {
            type        cuttingPlane;
            planeType   pointAndNormal;
            pointAndNormalDict { point (2 0 0); normal (1 0 0); }
            interpolate true;
        }
        wake_5c    // 5 chords downstream
        {
            type        cuttingPlane;
            planeType   pointAndNormal;
            pointAndNormalDict { point (6 0 0); normal (1 0 0); }
            interpolate true;
        }
    );
}
```

### Total Pressure Loss
```cpp
// Compute total pressure field
TotalPressure
{
    type            totalPressure;
    libs            ("libfieldFunctionObjects.so");
    p               p;
    U               U;
    writeControl    writeTime;
}
```

---

## 6. ParaView Post-Processing

### Essential Filters for Aerospace CFD in ParaView

**Pressure Coefficient visualization:**
1. Open `case.foam` in ParaView
2. Apply filter: `Programmable Filter`
```python
# Compute Cp from pressure and reference conditions
import numpy as np
p = inputs[0].PointData['p']     # kinematic pressure
U_inf = 50.0
Cp = p / (0.5 * U_inf**2)       # incompressible
output.PointData.append(Cp, 'Cp')
```

**Q-criterion for vortex visualization (LES/DES):**
1. Apply `Gradient of Unstructured DataSet` filter on U
2. Apply `Programmable Filter`:
```python
import numpy as np
grad = inputs[0].PointData['Gradients']
# grad is 9-component tensor: reshape to 3x3
# Q = 0.5*(||Omega||^2 - ||S||^2)
# Use built-in Q-criterion filter in ParaView 5.x+
```
3. Or use: `Filters → Common → Q-Criterion`

**Streamlines from inlet:**
- Source: `Line` from inlet plane
- Filter: `Streamtracer`

### ParaView State File (Python script batch)
```python
# batch_postprocess.py — run with: pvpython batch_postprocess.py
from paraview.simple import *
reader = OpenFOAMReader(FileName='case.foam')
reader.MeshRegions = ['internalMesh']
reader.CellArrays = ['U', 'p', 'Cp']
Show(reader)
view = GetActiveViewOrCreate('RenderView')
view.ViewSize = [1920, 1080]
SaveScreenshot('Cp_surface.png', view, ImageResolution=[1920,1080])
```

---

## 7. Aeroacoustics (Ffowcs-Williams Hawkings)

For airframe noise / propeller noise:
```cpp
// Add to controlDict functions{}
FWH
{
    type            FfowcsWilliamsHawkings;
    libs            ("libacoustics.so");
    patches         (wing flap);           // noise source surfaces
    
    // Observer microphone positions
    observers
    (
        mic1 { position (0 -5 0); }        // below wing
        mic2 { position (10 0 0); }        // downstream
    );
    
    c0              340.0;    // speed of sound
    rhoRef          1.225;
    pRef            101325;
    
    timeStart       0.1;     // skip initial transient
}
```

---

## 8. Python Post-Processing

### Read OpenFOAM force coefficients
```python
import numpy as np
import matplotlib.pyplot as plt

# Load forceCoeffs output
data = np.loadtxt('postProcessing/forceCoeffs/0/forceCoeffs.dat',
                  comments='#', usecols=(0,1,2,3))
time, Cm, Cd, Cl = data.T

# Plot convergence
fig, (ax1, ax2) = plt.subplots(2, 1, figsize=(10, 6))
ax1.plot(time, Cl, 'b-', label='CL')
ax1.set_ylabel('Lift Coefficient CL')
ax1.legend(); ax1.grid(True)
ax2.plot(time, Cd, 'r-', label='CD')
ax2.set_ylabel('Drag Coefficient CD')
ax2.set_xlabel('Iteration'); ax2.legend(); ax2.grid(True)
plt.tight_layout()
plt.savefig('force_convergence.png', dpi=150)

# Averaged final values (last 20%)
n_avg = int(0.2 * len(Cl))
print(f"CL = {np.mean(Cl[-n_avg:]):.4f} ± {np.std(Cl[-n_avg:]):.4f}")
print(f"CD = {np.mean(Cd[-n_avg:]):.4f} ± {np.std(Cd[-n_avg:]):.4f}")
print(f"L/D = {np.mean(Cl[-n_avg:])/np.mean(Cd[-n_avg:]):.2f}")
```

### Read Cp Distribution on Airfoil
```python
import pandas as pd
import matplotlib.pyplot as plt

# Load sampled Cp data (from postProcessing/sampleSurface/)
df = pd.read_csv('postProcessing/sampleSurface/latestTime/Cp_wingSection.csv')
# Sort by x/c
df['xc'] = df['x'] / chord_length
df_sorted = df.sort_values('xc')

plt.figure(figsize=(10, 5))
plt.plot(df_sorted['xc'], df_sorted['Cp'], 'b-o', markersize=2)
plt.gca().invert_yaxis()   # Cp convention: lower is "up"
plt.xlabel('x/c'); plt.ylabel('Cp')
plt.title('Pressure Coefficient Distribution')
plt.grid(True); plt.savefig('Cp_distribution.png', dpi=150)
```

### Drag Polar (AoA sweep)
```python
import glob, os
import numpy as np
import matplotlib.pyplot as plt

results = []
for case_dir in sorted(glob.glob('case_aoa*')):
    aoa = int(case_dir.split('aoa')[1])
    fc_file = f'{case_dir}/postProcessing/forceCoeffs/0/forceCoeffs.dat'
    if os.path.exists(fc_file):
        data = np.loadtxt(fc_file, comments='#', usecols=(2,3))
        Cd, Cl = np.mean(data[-100:], axis=0)
        results.append((aoa, Cl, Cd))

aoa_arr, Cl_arr, Cd_arr = zip(*sorted(results))
fig, axes = plt.subplots(1, 2, figsize=(12, 5))
axes[0].plot(aoa_arr, Cl_arr, 'bo-')
axes[0].set_xlabel('AoA [°]'); axes[0].set_ylabel('CL'); axes[0].grid(True)
axes[1].plot(Cd_arr, Cl_arr, 'ro-')
axes[1].set_xlabel('CD'); axes[1].set_ylabel('CL'); axes[1].grid(True)
axes[1].set_title('Drag Polar')
plt.tight_layout(); plt.savefig('drag_polar.png', dpi=150)
```