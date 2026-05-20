use aeroflow_core::{IntakeConfig, MeshQualityMetrics, OpenFOAMFormat};

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

    pub fn generate_blockmesh_dict(&self, config: &IntakeConfig) -> String {
        let format = self.write_format.label();
        let _ = config;
        format!(
            r#"
FoamFile {{ version 2.0; format {}; class dictionary; object blockMeshDict; }}
scale 1.0;
vertices
(
    (-20 -15 -15)
    ( 40 -15 -15)
    ( 40  15 -15)
    (-20  15 -15)
    (-20 -15  15)
    ( 40 -15  15)
    ( 40  15  15)
    (-20  15  15)
);
blocks ( hex (0 1 2 3 4 5 6 7) (60 30 30) simpleGrading (1 1 1) );
"#, format)
    }

    pub fn generate_snappy_dict(&self, config: &IntakeConfig) -> String {
        let format = self.write_format.label();
        let _ = config;
        format!(
            r#"
FoamFile {{ version 2.0; format {}; class dictionary; object snappyHexMeshDict; }}
castellatedMesh true;
snap            true;
addLayers       true;
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
        let format = self.write_format.label();
        let _ = config;

        // Adaptive parameters: relax settings that were problematic
        let max_local_cells: u32 = if attempt > 2 { 500_000 } else { 200_000 };
        let min_refinement_cells: u32 = if prev_quality.n_failed_cells > 10 { 50 } else { 10 };
        let resolve_feature_angle: f64 = if prev_quality.max_skewness > 3.0 { 60.0 } else { 30.0 };
        let n_cells_between_levels: u32 = if prev_quality.max_non_orthogonality > 60.0 { 5 } else { 3 };
        let n_smooth_normals: u32 = if prev_quality.max_skewness > 3.0 { 5 } else { 3 };

        let mut dict = String::new();
        dict.push_str(&format!(
            r#"FoamFile {{ version 2.0; format {}; class dictionary; object snappyHexMeshDict; }}

castellatedMesh true;
snap            true;
addLayers       true;

// Adaptive parameters (attempt {})
refinementSurfaces
{{
    .*
    {{
        level (3 5);
    }}
}}

resolveFeatureAngle {:.0};

 castellatedMeshControls
 {{
     maxLocalCells        {};
     maxGlobalCells       20000000;
     minRefinementCells   {};
     nCellsBetweenLevels  {};
     features
     (
         {{ file "constant/constant/triSurface/geometry.fms"; level 3; }}
     );
 }}

 snapControls
 {{
     nSmoothPatchPoints   3;
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
     layers              {{ (".*" 3); }};
     expansionRatio      1.2;
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
 }}

meshQualityControls
{{
     maxNonOrtho          65;
     maxBoundaryNonOrtho 75;
     maxSkewness          4;
     maxDeterminant       0.001;
     minFaceWeight        0.05;
     minVolRatio          0.01;
     minTriangleTwist -1;
     minArea              -1;
     minTetQuality        -1;
     minVol               1e-13;
     minTerminalVol       1e-13;
     relaxed
     {{
         maxNonOrtho      75;
         maxBoundaryNonOrtho 85;
         maxSkewness       6;
     }}
 }}
"#,
            format, attempt, resolve_feature_angle,
            max_local_cells, min_refinement_cells,
            n_cells_between_levels, n_smooth_normals
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
