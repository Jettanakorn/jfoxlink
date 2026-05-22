use aeroflow_core::{IntakeConfig, MeshParams, MeshQualityMetrics, OpenFOAMFormat};
use std::path::Path;

#[derive(Debug, Clone)]
pub struct GeoBounds {
    pub min_x: f64,
    pub max_x: f64,
    pub min_y: f64,
    pub max_y: f64,
    pub min_z: f64,
    pub max_z: f64,
}

impl GeoBounds {
    pub fn from_stl(stl_path: &Path) -> Option<Self> {
        let output = std::process::Command::new("surfaceCheck")
            .arg(stl_path)
            .output().ok()?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        for line in stdout.lines() {
            if line.contains("Bounding Box") {
                if let Some(bbox) = line.split(':').nth(1) {
                    let parts: Vec<&str> = bbox.trim().split_whitespace().collect();
                    if parts.len() >= 6 {
                        let min_x = parts[0].trim_start_matches('(').parse().ok()?;
                        let min_y = parts[1].parse().ok()?;
                        let min_z = parts[2].trim_end_matches(')').parse().ok()?;
                        let max_x = parts[3].trim_start_matches('(').parse().ok()?;
                        let max_y = parts[4].parse().ok()?;
                        let max_z = parts[5].trim_end_matches(')').parse().ok()?;
                        return Some(Self { min_x, max_x, min_y, max_y, min_z, max_z });
                    }
                }
            }
        }
        None
    }
}

pub struct MeshGenerator {
    write_format: OpenFOAMFormat,
}

impl MeshGenerator {
    pub fn new() -> Self {
        Self {
            write_format: OpenFOAMFormat::Binary,
        }
    }

    pub fn with_format(format: OpenFOAMFormat) -> Self {
        Self { write_format: format }
    }

    pub fn generate_blockmesh_dict(&self, _config: &IntakeConfig) -> String {
        self.generate_blockmesh_with_bounds(None)
    }

    /// Generate blockMeshDict auto-sized around geometry bounding box.
    /// Uses padding proportional to geometry size, with grading toward center.
    pub fn generate_blockmesh_with_bounds(&self, bounds: Option<&GeoBounds>) -> String {
        let format = self.write_format.label();
        let (x_min, x_max, y_min, y_max, z_min, z_max, cells_x, cells_y, cells_z, grading) = match bounds {
            Some(b) => {
                let dx = (b.max_x - b.min_x).max(0.1);
                let dy = (b.max_y - b.min_y).max(0.1);
                let dz = (b.max_z - b.min_z).max(0.2);
                let pad_x = (dx * 5.0).max(1.0);
                let pad_y = (dy * 5.0).max(0.5);
                let pad_z = (dz * 2.0).max(0.5);
                let cx = (b.min_x + b.max_x) / 2.0;
                let cy = (b.min_y + b.max_y) / 2.0;
                let cz = (b.min_z + b.max_z) / 2.0;
                let domain_x = dx + 2.0 * pad_x;
                let domain_y = dy + 2.0 * pad_y;
                let domain_z = dz + 2.0 * pad_z;
                let cells_x = (domain_x * 30.0).ceil() as u32;
                let cells_y = (domain_y * 30.0).ceil() as u32;
                let cells_z = (domain_z * 30.0).ceil() as u32;
                (cx - domain_x/2.0, cx + domain_x/2.0,
                 cy - domain_y/2.0, cy + domain_y/2.0,
                 cz - domain_z/2.0, cz + domain_z/2.0,
                 cells_x.max(20), cells_y.max(20), cells_z.max(20),
                 "simpleGrading (1 1 1)")
            }
            None => {
                (-5.0, 10.0, -5.0, 5.0, -5.0, 5.0,
                 80, 50, 50, "simpleGrading (1 1 1)")
            }
        };
        format!(
            r#"
FoamFile {{ version 2.0; format {}; class dictionary; object blockMeshDict; }}
scale 1.0;
vertices
(
    ({} {} {})
    ({} {} {})
    ({} {} {})
    ({} {} {})
    ({} {} {})
    ({} {} {})
    ({} {} {})
    ({} {} {})
);
blocks ( hex (0 1 2 3 4 5 6 7) ({} {} {}) {} );
edges ();
boundary
(
    inlet
    {{
        type patch;
        faces ( (0 4 7 3) );
    }}
    outlet
    {{
        type patch;
        faces ( (1 2 6 5) );
    }}
    top
    {{
        type patch;
        faces ( (3 7 6 2) );
    }}
    bottom
    {{
        type patch;
        faces ( (0 1 5 4) );
    }}
    front
    {{
        type patch;
        faces ( (0 3 2 1) );
    }}
    back
    {{
        type patch;
        faces ( (4 5 6 7) );
    }}
);
mergePatchPairs ();
"#,
            format,
            x_min, y_min, z_min,
            x_max, y_min, z_min,
            x_max, y_max, z_min,
            x_min, y_max, z_min,
            x_min, y_min, z_max,
            x_max, y_min, z_max,
            x_max, y_max, z_max,
            x_min, y_max, z_max,
            cells_x, cells_y, cells_z, grading)
    }

    pub fn generate_snappy_dict(&self, config: &IntakeConfig) -> String {
        let format = self.write_format.label();
        let _ = config;
        format!(
            r#"
FoamFile {{ version 2.0; format {}; class dictionary; object snappyHexMeshDict; }}
castellatedMesh true;
snap            true;
addLayers       false;
"#, format)
    }

    /// Generate snappyHexMeshDict with parameters adjusted based on
    /// the previous mesh quality metrics.  Relaxes settings that
    /// caused checkMesh failures.
    pub fn generate_adaptive_snappy_dict(
        &self,
        config: &IntakeConfig,
        prev_quality: &MeshQualityMetrics,
        attempt: u32,
    ) -> String {
        self.generate_adaptive_snappy_with_bounds(config, prev_quality, attempt, None, None, None)
    }

    /// Generate snappyHexMeshDict with optional refinement region around geometry.
    /// `stl_name`: the STL filename stem (without extension) for the features entry.
    pub fn generate_adaptive_snappy_with_bounds(
        &self,
        config: &IntakeConfig,
        prev_quality: &MeshQualityMetrics,
        attempt: u32,
        bounds: Option<&GeoBounds>,
        stl_name: Option<&str>,
        mesh_params: Option<&MeshParams>,
    ) -> String {
        let stl_name = stl_name.unwrap_or("geometry");
        let format = "ascii"; // snappyHexMesh requires ASCII format
        let _ = config;
        let mp = mesh_params.copied().unwrap_or_default();

        // Adaptive parameters: relax settings that were problematic
        let max_local_cells: u32 = if attempt > 2 { 200_000 } else { 100_000 };
        let min_refinement_cells: u32 = if prev_quality.n_failed_cells > 10 { 50 } else { 10 };
        let resolve_feature_angle: f64 = if prev_quality.max_skewness > 3.0 { 60.0 } else { 30.0 };
        // Use mesh_params override if provided (from optimizer), otherwise adapt based on quality
        let n_cells_between_levels: u32 = mp.n_cells_between_levels;
        let n_smooth_normals: u32 = if prev_quality.max_skewness > 3.0 { 5 } else { 3 };

        let (geometry_section, refinement_region, location_point) = match bounds {
            Some(b) => {
                let dx = (b.max_x - b.min_x).max(0.1);
                let dy = (b.max_y - b.min_y).max(0.1);
                let dz = (b.max_z - b.min_z).max(0.2);
                let cx = (b.min_x + b.max_x) / 2.0;
                let cy = (b.min_y + b.max_y) / 2.0;
                let cz = (b.min_z + b.max_z) / 2.0;
                let box_x = dx * 2.0;
                let box_y = dy * 4.0;
                let box_z = dz * 1.5;
                let loc = format!("({} {} {})", b.min_x - 0.05, b.max_y + 0.05, 0.0);
                let geo = format!(
                    "
geometry
{{
    {}.stl
    {{
        type triSurfaceMesh;
        name blade;
    }}

    geometryBox
    {{
        type searchableBox;
        min ({} {} {});
        max ({} {} {});
    }}
}}
",
                    stl_name,
                    cx - box_x, cy - box_y, cz - box_z,
                    cx + box_x, cy + box_y, cz + box_z
                );
                let reg = format!(
                    "
    refinementRegions
    {{
        geometryBox
        {{
            mode inside;
            levels (({} {}));
        }}
    }}
",
                    mp.region_min_level, mp.region_max_level
                );
                (geo, reg, loc)
            }
            None => (String::new(), String::new(), String::from("(0 0 0)")),
        };

        let mut dict = String::new();
        dict.push_str(&format!(
            r#"FoamFile {{ version 2.0; format {}; class dictionary; object snappyHexMeshDict; }}

castellatedMesh true;
snap            true;
addLayers       true;
mergeTolerance  1e-6;

// Adaptive parameters (attempt {})
{} castellatedMeshControls
{{
    maxLocalCells        {};
    maxGlobalCells       2000000;
    minRefinementCells   {};
    nCellsBetweenLevels  {};

    locationInMesh {};

    features ();

    refinementSurfaces
    {{
        blade
        {{
            level ({} {});
            patchInfo
            {{
                type wall;
                inGroups (bladeGroup);
            }}
        }}
    }}

    allowFreeStandingZoneFaces true;
    resolveFeatureAngle {:.0};
    {}
}}
"#,
            format, attempt,
            geometry_section,
            max_local_cells, min_refinement_cells,
            n_cells_between_levels, location_point,
            mp.surface_min_level, mp.surface_max_level,
            resolve_feature_angle,
            refinement_region
        ));

        dict.push_str(&format!(
            r#"
 snapControls
 {{
     nSmoothPatch         3;
     tolerance            2.0;
     nSolveIter          30;
     nRelaxIter           5;
     nFeatureSnapIter    10;
     implicitFeatureSnap true;
     explicitFeatureSnap  true;
     multiRegionFeatureSnap false;
 }}

 addLayersControls
 {{
     relativeSizes       true;
     layers
     {{
         ".*"
         {{
             nSurfaceLayers 2;
         }}
     }}
     expansionRatio      1.1;
     finalLayerThickness 0.5;
     minThickness        0.1;
     nGrow               1;
     featureAngle         60;
     nSmoothNormals       {};
     nSmoothSurfaceNormals 2;
     nSmoothThickness    2;
     maxFaceThicknessRatio 0.5;
     maxThicknessToMedialRatio 0.3;
      nRelaxIter          5;
      minMedialAxisAngle   90;
      nBufferCellsNoExtrude 0;
      nLayerIter           50;
  }}

meshQualityControls
{{
      maxNonOrtho          65;
      maxBoundarySkewness 20;
      maxInternalSkewness  4;
      maxConcave          80;
      minVol               1e-13;
      minTetQuality        1e-15;
      minArea              -1;
      minTwist             0.02;
      minDeterminant       0.001;
      minFaceWeight        0.05;
      minVolRatio          0.01;
      minTriangleTwist    -1;
      minEdgeLength       -1;
      errorReduction       0.75;
      nSmoothScale         4;
      relaxed
      {{
          maxNonOrtho      75;
          maxBoundaryNonOrtho 85;
          maxSkewness       6;
      }}
  }}
"#,
            n_smooth_normals
        ));
        dict
    }

    pub fn compute_first_cell_height(
        u_inf: f64,
        chord: f64,
        nu: f64,
        target_yplus: f64,
    ) -> f64 {
        let re = u_inf * chord / nu;
        let cf = 0.026 / re.powf(1.0 / 7.0);
        let u_tau = u_inf * (cf / 2.0).sqrt();
        target_yplus * nu / u_tau
    }
}
