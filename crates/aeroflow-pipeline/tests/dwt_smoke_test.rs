use std::path::{Path, PathBuf};
use std::process::Command;

/// Docker smoke test for the Digital Wind Tunnel blockMesh pipeline.
///
/// Requires:
///   - Docker daemon running
///   - `AEROFLOW_DOCKER_TEST=1` env var set
///   - OpenFOAM image `microfluidica/openfoam:com` (pulled automatically)
///
/// Run:  $env:AEROFLOW_DOCKER_TEST=1; cargo test --test dwt_smoke_test -- --nocapture
const OPENFOAM_IMAGE: &str = "microfluidica/openfoam:com";

fn docker_available() -> bool {
    Command::new("docker")
        .args(["info"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn pull_image() -> bool {
    let status = Command::new("docker")
        .args(["pull", OPENFOAM_IMAGE])
        .status();
    matches!(status, Ok(s) if s.success())
}

fn make_case_dir() -> PathBuf {
    let dir = std::env::temp_dir().join(format!("aeroflow_dwt_test_{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(dir.join("constant/triSurface")).unwrap();
    std::fs::create_dir_all(dir.join("system")).unwrap();
    dir
}

/// Write a minimal STL file — a thin rectangular plate (0.1m chord, 0.04m span, 0.02m thick).
fn write_stl(path: &Path) {
    let stl = r#"solid test
  facet normal 0 0 -1
    outer loop
      vertex -0.05 -0.02 -0.01
      vertex  0.05 -0.02 -0.01
      vertex  0.05  0.02 -0.01
    endloop
  endfacet
  facet normal 0 0 -1
    outer loop
      vertex -0.05 -0.02 -0.01
      vertex  0.05  0.02 -0.01
      vertex -0.05  0.02 -0.01
    endloop
  endfacet
  facet normal 0 0 1
    outer loop
      vertex -0.05 -0.02  0.01
      vertex  0.05  0.02  0.01
      vertex  0.05 -0.02  0.01
    endloop
  endfacet
  facet normal 0 0 1
    outer loop
      vertex -0.05 -0.02  0.01
      vertex -0.05  0.02  0.01
      vertex  0.05  0.02  0.01
    endloop
  endfacet
endsolid test
"#;
    std::fs::write(path, stl).unwrap();
}

fn write_control_dict(path: &Path) {
    let cd = r#"/* AeroFlow DWT smoke test */
FoamFile {
    version     2.0;
    format      ascii;
    class       dictionary;
    object      controlDict;
}
application     blockMesh;
startFrom       startTime;
startTime       0;
stopAt          endTime;
endTime         1;
deltaT          1;
writeControl    timeStep;
writeInterval   1;
purgeWrite      0;
writeFormat     ascii;
writePrecision  6;
writeCompression off;
timeFormat      general;
timePrecision   6;
runTimeModifiable true;
"#;
    std::fs::write(path, cd).unwrap();
}

/// Manually compute GeoBounds from the known STL geometry (0.1m chord plate).
fn make_geo_bounds() -> aeroflow_mesh::GeoBounds {
    aeroflow_mesh::GeoBounds {
        min_x: -0.05, max_x: 0.05,
        min_y: -0.02, max_y: 0.02,
        min_z: -0.01, max_z: 0.01,
    }
}

fn write_blockmesh_dict(path: &Path, _case_dir: &Path) {
    use aeroflow_core::WindTunnelConfig;
    use aeroflow_mesh::MeshGenerator;
    use aeroflow_core::OpenFOAMFormat;

    let geo_bounds = make_geo_bounds();

    let config = WindTunnelConfig {
        velocity_m_s: Some(10.0),
        turbulence_intensity: 0.01,
        ..WindTunnelConfig::default()
    };

    let mesh_gen = MeshGenerator::with_format(OpenFOAMFormat::Ascii);
    let dict = mesh_gen.generate_wind_tunnel_blockmesh(&geo_bounds, Some(&config));

    let mut lines: Vec<&str> = dict.lines().collect();
    // Insert FoamFile header (blockMesh expects it in the file)
    let header = r#"/*--------------------------------*- C++ -*----------------------------------*\
  =========                 |
  \\      /  F ield         | OpenFOAM: The Open Source CFD Toolbox
   \\    /   O peration     |
    \\  /    A nd           | www.openfoam.com
     \\/     M anipulation  |
\*---------------------------------------------------------------------------*/
FoamFile {
    version     2.0;
    format      ascii;
    class       dictionary;
    object      blockMeshDict;
}
// * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * //
"#;
    lines.insert(0, header);
    let content = lines.join("\n");
    std::fs::write(path, content).unwrap();
}

fn run_in_docker(case_dir: &Path, cmd: &str, args: &[&str]) -> std::process::Output {
    let host_path = {
        let raw = case_dir.to_string_lossy();
        let s = raw.replace('\\', "/");
        // Handle Windows drive letter: C:/... -> /c/...
        if s.len() >= 2 && s.as_bytes()[1] == b':' {
            let drive = &s[..1].to_lowercase();
            format!("/{}{}", drive, &s[2..])
        } else {
            s
        }
    };

    let vol = format!("{}:/case", host_path);
    let mut docker_args = vec![
        "run", "--rm",
        "-v", &vol,
        "-w", "/case",
        OPENFOAM_IMAGE,
        cmd,
    ];
    docker_args.extend(args);

    Command::new("docker")
        .args(&docker_args)
        .output()
        .expect("Failed to execute Docker command")
}

#[test]
fn test_dwt_blockmesh_docker_smoke() {
    if std::env::var("AEROFLOW_DOCKER_TEST").is_err() {
        eprintln!("Skipping Docker smoke test — set AEROFLOW_DOCKER_TEST=1 to run");
        return;
    }

    if !docker_available() {
        eprintln!("Skipping Docker smoke test — Docker daemon not reachable");
        return;
    }

    if !pull_image() {
        panic!("Failed to pull OpenFOAM Docker image: {}", OPENFOAM_IMAGE);
    }

    let case_dir = make_case_dir();
    eprintln!("  Case dir: {:?}", case_dir);

    // Write STL geometry
    write_stl(&case_dir.join("constant/triSurface/airfoil.stl"));

    // Generate DWT blockMeshDict
    write_blockmesh_dict(&case_dir.join("system/blockMeshDict"), &case_dir);

    // Write controlDict
    write_control_dict(&case_dir.join("system/controlDict"));

    // Run blockMesh in Docker
    eprintln!("  Running blockMesh in Docker...");
    let output = run_in_docker(&case_dir, "blockMesh", &["-case", "/case"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    if !output.status.success() {
        eprintln!("  blockMesh stdout:\n{}", stdout);
        eprintln!("  blockMesh stderr:\n{}", stderr);
        panic!("blockMesh failed with exit code {:?}", output.status.code());
    }
    eprintln!("  ✓ blockMesh succeeded");

    // Verify mesh was written
    let points = case_dir.join("constant/polyMesh/points");
    assert!(points.exists(), "Mesh points file not found: {:?}", points);
    let n_cells_path = case_dir.join("constant/polyMesh/faces");
    assert!(n_cells_path.exists(), "Mesh faces file not found: {:?}", n_cells_path);

    // Write minimal fvSchemes and fvSolution (needed by checkMesh)
    let fv_schemes = r#"FoamFile { version 2.0; format ascii; class dictionary; object fvSchemes; }
ddtSchemes { default steadyState; }
gradSchemes { default Gauss linear; }
divSchemes { default Gauss linear; }
laplacianSchemes { default Gauss linear corrected; }
interpolationSchemes { default linear; }
snGradSchemes { default corrected; }
"#;
    std::fs::write(case_dir.join("system/fvSchemes"), fv_schemes).unwrap();

    let fv_solution = r#"FoamFile { version 2.0; format ascii; class dictionary; object fvSolution; }
solvers { p { solver PCG; preconditioner DIC; tolerance 1e-6; relTol 0; } }
SIMPLE { nNonOrthogonalCorrectors 0; }
"#;
    std::fs::write(case_dir.join("system/fvSolution"), fv_solution).unwrap();

    // Write minimal transportProperties for checkMesh
    let transport = r#"FoamFile { version 2.0; format ascii; class dictionary; object transportProperties; }
transportModel  Newtonian;
nu              nu [0 2 -1 0 0 0 0] 1e-05;
"#;
    std::fs::write(case_dir.join("constant/transportProperties"), transport).unwrap();

    // Run checkMesh for validation
    eprintln!("  Running checkMesh in Docker...");
    let check = run_in_docker(&case_dir, "checkMesh", &["-case", "/case", "-constant"]);
    let check_out = String::from_utf8_lossy(&check.stdout);
    let check_err = String::from_utf8_lossy(&check.stderr);
    let combined = format!("{}\n{}", check_out, check_err);

    if !check.status.success() && !combined.contains("Mesh OK") {
        eprintln!("  checkMesh output:\n{}", combined);
        panic!("checkMesh failed");
    }
    eprintln!("  ✓ checkMesh passed");

    // Extract mesh statistics from checkMesh output
    if let Some(line) = combined.lines().find(|l| l.contains("cells:")) {
        eprintln!("  Mesh stats: {}", line.trim());
    }
    if let Some(line) = combined.lines().find(|l| l.contains("Max non-orthogonality")) {
        eprintln!("  {}", line.trim());
    }

    // Cleanup
    let _ = std::fs::remove_dir_all(&case_dir);
    eprintln!("  ✓ Docker smoke test passed");
}
