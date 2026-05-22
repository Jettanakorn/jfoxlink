#!/usr/bin/env python3
"""Generate visualization images from OpenFOAM VTK output.

Usage:
    python3 generate_viz.py --case-path <case_path> --output-dir <output_dir>

Requires: numpy, matplotlib, vtk
"""

import argparse
import json
import os
import sys
from pathlib import Path

import numpy as np

# Try importing matplotlib with non-interactive backend
import matplotlib
matplotlib.use("Agg")
import matplotlib.pyplot as plt

# VTK imports
try:
    from vtk import vtkXMLPolyDataReader, vtkXMLUnstructuredGridReader, vtkXMLMultiBlockDataReader
    from vtk import vtkDataSetSurfaceFilter, vtkContourFilter, vtkCutter, vtkPlane
    from vtk import vtkThreshold, vtkGeometryFilter
    from vtk import vtkLookupTable, vtkScalarBarActor, vtkTextProperty
    from vtk.util import numpy_support
    VTK_AVAILABLE = True
except ImportError:
    VTK_AVAILABLE = False


def find_vtk_dir(case_path: Path, time_dir: str = None) -> Path:
    """Find the VTK directory containing the latest time results."""
    vtk_root = case_path / "VTK"
    if not vtk_root.exists():
        return None

    # Find the latest time directory
    time_dirs = []
    for p in vtk_root.iterdir():
        if p.is_dir():
            try:
                time_dirs.append((int(p.name.split("_")[-1]), p))
            except (ValueError, IndexError):
                pass

    if not time_dirs:
        return None

    latest = max(time_dirs, key=lambda x: x[0])[1]
    return latest


def read_vtp_polydata(filepath: Path):
    """Read a .vtp file and return point data arrays."""
    reader = vtkXMLPolyDataReader()
    reader.SetFileName(str(filepath))
    reader.Update()
    output = reader.GetOutput()

    points = output.GetPoints()
    n_points = points.GetNumberOfPoints()
    point_coords = np.array([points.GetPoint(i) for i in range(n_points)])

    data = {}
    pd = output.GetPointData()
    for i in range(pd.GetNumberOfArrays()):
        name = pd.GetArrayName(i)
        arr = pd.GetArray(i)
        if arr:
            ncomps = arr.GetNumberOfComponents()
            if ncomps == 1:
                data[name] = np.array([arr.GetValue(j) for j in range(arr.GetNumberOfTuples())])
            else:
                data[name] = np.array([arr.GetTuple(j) for j in range(arr.GetNumberOfTuples())])

    return point_coords, data


def read_vtu_mesh(filepath: Path):
    """Read a .vtu file and return unstructured grid data."""
    reader = vtkXMLUnstructuredGridReader()
    reader.SetFileName(str(filepath))
    reader.Update()
    output = reader.GetOutput()

    points = output.GetPoints()
    n_points = points.GetNumberOfPoints()
    point_coords = np.array([points.GetPoint(i) for i in range(n_points)])

    cells = output.GetCells()
    cell_connectivity = []
    if cells:
        ca = cells.GetConnectivityArray()
        offsets = cells.GetOffsetsArray()
        for i in range(offsets.GetNumberOfTuples()):
            start = offsets.GetValue(i - 1) if i > 0 else 0
            end = offsets.GetValue(i)
            cell = [ca.GetValue(j) for j in range(start, end)]
            cell_connectivity.append(cell)

    data = {}
    pd = output.GetPointData()
    for i in range(pd.GetNumberOfArrays()):
        name = pd.GetArrayName(i)
        arr = pd.GetArray(i)
        if arr:
            ncomps = arr.GetNumberOfComponents()
            if ncomps == 1:
                data[name] = np.array(
                    [arr.GetValue(j) for j in range(arr.GetNumberOfTuples())]
                )
            else:
                data[name] = np.array(
                    [arr.GetTuple(j) for j in range(arr.GetNumberOfTuples())]
                )

    return point_coords, cell_connectivity, data


def plot_surface_pressure(vtp_file: Path, output_path: Path, title: str = "Pressure on Blade Surface"):
    """Render pressure contour on the blade surface using matplotlib trimesh."""
    points, data = read_vtp_polydata(vtp_file)
    if points.shape[0] == 0:
        return False

    # Try to find pressure field
    p_field = None
    for key in ["p", "pressure", "Pressure"]:
        if key in data:
            p_field = data[key]
            break

    if p_field is None:
        # Use first scalar field
        for key, val in data.items():
            if val.ndim == 1:
                p_field = val
                break

    if p_field is None:
        return False

    # Project to 2D for visualization (mid-span slice, y=0)
    mid_indices = np.abs(points[:, 1]) < 0.01  # near y=0
    if np.sum(mid_indices) < 10:
        mid_indices = np.abs(points[:, 1]) < 0.1

    x = points[mid_indices, 0]
    z = points[mid_indices, 2]
    c = p_field[mid_indices]

    if len(x) < 3:
        return False

    fig, ax = plt.subplots(figsize=(10, 5))
    scatter = ax.scatter(x, z, c=c, cmap="RdBu_r", s=8, alpha=0.8)
    plt.colorbar(scatter, ax=ax, label="Pressure (Pa)")

    ax.set_xlabel("x (m)")
    ax.set_ylabel("z (m)")
    ax.set_title(title)
    ax.set_aspect("equal")
    ax.grid(True, alpha=0.3)

    fig.tight_layout()
    fig.savefig(str(output_path), dpi=150, bbox_inches="tight")
    plt.close(fig)
    return True


def plot_slice_velocity(vtu_file: Path, output_path: Path, title: str = "Velocity Magnitude at Mid-Plane"):
    """Extract a slice plane and plot velocity magnitude."""
    # Read the vtu file
    reader = vtkXMLUnstructuredGridReader()
    reader.SetFileName(str(vtu_file))
    reader.Update()
    unstructured = reader.GetOutput()

    # Create a slice plane at y=0
    plane = vtkPlane()
    plane.SetOrigin(0, 0, 0)
    plane.SetNormal(0, 1, 0)

    cutter = vtkCutter()
    cutter.SetInputData(unstructured)
    cutter.SetCutFunction(plane)
    cutter.Update()
    slice_output = cutter.GetOutput()

    if slice_output.GetNumberOfPoints() == 0:
        return False

    # Extract points and data
    points = slice_output.GetPoints()
    n_pts = points.GetNumberOfPoints()
    coords = np.array([points.GetPoint(i) for i in range(n_pts)])

    # Find velocity magnitude
    pd = slice_output.GetPointData()
    vel_mag = None
    for name in ["U", "velocity", "Velocity"]:
        arr = pd.GetArray(name)
        if arr and arr.GetNumberOfComponents() == 3:
            vel_mag = np.array(
                [np.sqrt(sum(arr.GetTuple(j)[c] ** 2 for c in range(3)))
                 for j in range(arr.GetNumberOfTuples())]
            )
            break
        elif arr and arr.GetNumberOfComponents() == 1:
            vel_mag = np.array([arr.GetValue(j) for j in range(arr.GetNumberOfTuples())])
            break

    if vel_mag is None:
        return False

    x = coords[:, 0]
    z = coords[:, 2]
    y = coords[:, 1]

    fig, ax = plt.subplots(figsize=(10, 6))
    scatter = ax.scatter(x, z, c=vel_mag, cmap="plasma", s=3, alpha=0.7)
    cbar = plt.colorbar(scatter, ax=ax, label="Velocity Magnitude (m/s)")
    cbar.ax.tick_params()

    ax.set_xlabel("x (m)")
    ax.set_ylabel("z (m)")
    ax.set_title(title)
    ax.set_aspect("equal")
    ax.grid(True, alpha=0.3)

    fig.tight_layout()
    fig.savefig(str(output_path), dpi=150, bbox_inches="tight")
    plt.close(fig)
    return True


def plot_convergence(forces_path: Path, output_path: Path, title: str = "Force Coefficient Convergence"):
    """Plot Cd and Cl convergence history from forceCoeffs output."""
    if not forces_path.exists():
        return False

    data = np.loadtxt(str(forces_path), comments="#")
    if data.ndim == 1 or data.shape[0] < 2:
        return False

    time = data[:, 0]
    cd = data[:, 1]
    cl = data[:, 4]

    fig, (ax1, ax2) = plt.subplots(2, 1, figsize=(10, 7), sharex=True)

    ax1.plot(time, cd, "r-", linewidth=1.2, label="Cd")
    ax1.set_ylabel("Drag Coefficient Cd")
    ax1.legend()
    ax1.grid(True, alpha=0.3)

    ax2.plot(time, cl, "b-", linewidth=1.2, label="Cl")
    ax2.set_xlabel("Iteration / Time")
    ax2.set_ylabel("Lift Coefficient Cl")
    ax2.legend()
    ax2.grid(True, alpha=0.3)

    fig.suptitle(title)
    fig.tight_layout()
    fig.savefig(str(output_path), dpi=150, bbox_inches="tight")
    plt.close(fig)
    return True


def main():
    parser = argparse.ArgumentParser(description="Generate CFD visualization images")
    parser.add_argument("--case-path", required=True, help="Path to OpenFOAM case directory")
    parser.add_argument("--output-dir", required=True, help="Output directory for images")
    args = parser.parse_args()

    case_path = Path(args.case_path)
    output_dir = Path(args.output_dir)
    output_dir.mkdir(parents=True, exist_ok=True)

    if not VTK_AVAILABLE:
        print('{"status":"error","message":"VTK Python module not available"}')
        sys.exit(1)

    vtk_dir = find_vtk_dir(case_path)
    if vtk_dir is None:
        print('{"status":"error","message":"No VTK output found"}')
        sys.exit(1)

    results = {}

    # 1. Blade surface pressure
    blade_vtp = vtk_dir / "boundary" / "blade.vtp"
    if blade_vtp.exists():
        success = plot_surface_pressure(blade_vtp, output_dir / "pressure_surface.png")
        results["pressure_surface"] = success
        print(f"  pressure_surface.png: {'✓' if success else '✗'}")

    # 2. Velocity slice at mid-plane
    internal_vtu = vtk_dir / "internal.vtu"
    if internal_vtu.exists():
        success = plot_slice_velocity(internal_vtu, output_dir / "velocity_slice.png")
        results["velocity_slice"] = success
        print(f"  velocity_slice.png: {'✓' if success else '✗'}")

    # 3. Convergence plot
    forces_dir = case_path / "postProcessing" / "forceCoeffs" / "0"
    coefficient_file = forces_dir / "coefficient.dat"
    if coefficient_file.exists():
        success = plot_convergence(coefficient_file, output_dir / "convergence.png")
        results["convergence"] = success
        print(f"  convergence.png: {'✓' if success else '✗'}")
    else:
        # Try alternative locations
        for alt in [forces_dir / "forceCoeffs.dat", forces_dir / "forces.dat"]:
            if alt.exists():
                success = plot_convergence(alt, output_dir / "convergence.png")
                results["convergence"] = success
                print(f"  convergence.png: {'✓' if success else '✗'}")
                break

    status = "ok" if any(results.values()) else "error"
    print(f'\n{{"status":"{status}","images":{json.dumps(results)}}}')


if __name__ == "__main__":
    main()
