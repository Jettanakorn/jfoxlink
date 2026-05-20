use aeroflow_core::{
    CaseId, CaseMeta, ForceCoefficients, IntakeConfig, MeshQualityMetrics, SolverStats,
    Stage, SystemEvent, EventBus, create_event_bus,
};
use aeroflow_mesh::{MeshGenerator, MeshQualityEngine};
use aeroflow_post::ForceExtractor;
use aeroflow_report::ReportGenerator;
use aeroflow_solver::{ProgressCallback, SolverLauncher};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use tracing::{info, warn};
use uuid::Uuid;

pub struct PipelineOrchestrator {
    event_bus: EventBus,
    cases: HashMap<CaseId, CaseMeta>,
    active_cases: HashMap<CaseId, Stage>,
    max_concurrent: u32,
    data_dir: PathBuf,
}

impl PipelineOrchestrator {
    pub fn new(data_dir: PathBuf, max_concurrent: u32) -> Self {
        Self {
            event_bus: create_event_bus(256),
            cases: HashMap::new(),
            active_cases: HashMap::new(),
            max_concurrent,
            data_dir,
        }
    }

    pub fn event_bus(&self) -> &EventBus {
        &self.event_bus
    }

    pub fn register_case(&mut self, name: &str) -> CaseId {
        let id = Uuid::new_v4();
        let meta = CaseMeta {
            id,
            name: name.to_string(),
            stage: Stage::Created,
            user_id: None,
            workspace_root: Some(self.data_dir.to_string_lossy().to_string()),
            flow_type: None,
            compressibility: None,
            accuracy: None,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };
        self.cases.insert(id, meta);
        self.active_cases.insert(id, Stage::Created);
        info!("Registered case {} ({})", name, id);
        id
    }

    /// Run the full pipeline for a case directory.
    /// `case_path` must contain a valid OpenFOAM case structure
    /// (0/, constant/, system/ directories with constant/triSurface/).
    pub fn run_pipeline(
        &mut self,
        case_id: CaseId,
        case_path: &Path,
        solver_name: &str,
        cancel: Option<Arc<AtomicBool>>,
    ) -> Result<Stage, anyhow::Error> {
        if self.active_cases.len() as u32 > self.max_concurrent {
            anyhow::bail!("Max concurrent cases reached ({}).", self.max_concurrent);
        }

        let case_name = self.cases.get(&case_id)
            .map(|m| m.name.clone())
            .unwrap_or_else(|| case_path.to_string_lossy().to_string());

        info!("Starting pipeline for case '{}' ({})", case_name, case_id);
        self.emit(case_id, "pipeline", format!("Pipeline started for '{}'", case_name));

        // Verify case directory
        if !case_path.join("constant").join("triSurface").exists() {
            anyhow::bail!("No STL geometry found in {:?}. Run 'aeroflow init' first.", case_path.join("constant/triSurface"));
        }

        // Phase 1: Import — STL already placed by init
        self.transition(case_id, Stage::Imported);

        // Phase 2: Surface features
        self.transition(case_id, Stage::SurfacePrep);
        self.run_surface_features(case_path)?;

        // Phase 3: Generate background mesh
        self.transition(case_id, Stage::Meshing);
        self.run_block_mesh(case_path)?;

        // Phase 4: SnappyHexMesh + adaptive mesh quality loop (up to 3 attempts)
        let mut mesh_ok = false;
        let mut mesh_metrics = MeshQualityMetrics {
            max_non_orthogonality: 0.0, avg_non_orthogonality: 0.0,
            max_skewness: 0.0, min_determinant: 1.0, max_aspect_ratio: 0.0,
            min_volume: 0.0, n_cells: 0, n_failed_cells: 0,
        };

        for attempt in 1..=3 {
            self.transition(case_id, Stage::Meshing);

            if attempt > 1 {
                // Write adaptive snappy dict based on previous checkMesh results
                let mesh_gen = MeshGenerator::with_format(aeroflow_core::OpenFOAMFormat::Binary);
                let dummy_config = IntakeConfig {
                    geometry_description: String::new(), geometry_file: None,
                    case_class: None, workspace_root: None, user_id: None,
                    altitude_m: 0.0, mach_number: 0.0, reynolds_number: 0.0,
                    alpha_sweep_deg: vec![], freestream_turbulence_intensity: 0.0,
                    target_cl: None, target_cd_max: None, target_yplus_max: 1.0,
                    convergence_residual: 1e-6, max_agent_iterations: 3,
                    human_in_loop: false, priority: aeroflow_core::Priority::Balanced,
                    hpc_cores: 4, time_budget_hours: 24.0,
                };
                let snappy_dict = mesh_gen.generate_adaptive_snappy_dict(&dummy_config, &mesh_metrics, attempt);
                std::fs::write(case_path.join("system").join("snappyHexMeshDict"), &snappy_dict)?;
                info!("  Written adaptive snappyHexMeshDict (attempt {})", attempt);
            }

            self.run_snappy_hex_mesh(case_path, attempt)?;

            self.transition(case_id, Stage::MeshQuality);
            mesh_metrics = self.run_check_mesh(case_path)?;

            let engine = MeshQualityEngine::new();
            let thresholds = aeroflow_core::MeshQualityThresholds::default();
            let verdict = engine.check_mesh(&mesh_metrics, &thresholds);

            if verdict.passed {
                mesh_ok = true;
                info!("Mesh quality passed on attempt {}", attempt);
                break;
            }

            warn!(
                "Mesh quality check failed (attempt {}/3): {:?}",
                attempt, verdict.failures
            );
            self.emit(case_id, "mesh-quality",
                format!("Re-meshing attempt {} — {:?}", attempt, verdict.failures));
        }

        if !mesh_ok {
            self.transition(case_id, Stage::Failed);
            self.emit(case_id, "mesh-quality", format!(
                "Mesh failed after 3 attempts: nonOrth={:.1}°, skew={:.2}, failed={}",
                mesh_metrics.max_non_orthogonality, mesh_metrics.max_skewness, mesh_metrics.n_failed_cells));
            anyhow::bail!("Mesh quality check failed after 3 attempts");
        }

        // Phase 5: Solver setup — copy controlDict, fvSchemes, fvSolution
        self.transition(case_id, Stage::Setup);
        self.setup_solver(case_path, solver_name)?;

        // Phase 6: Solve (with plateau detection and event bus progress)
        self.transition(case_id, Stage::Solving);
        let solver_stats = self.run_solver(case_id, case_path, solver_name, cancel.clone())?;

        if solver_stats.converged {
            self.transition(case_id, Stage::Converged);
        } else {
            self.transition(case_id, Stage::Diverged);
            self.emit(case_id, "solver", "Solver did not converge — check residuals".to_string());
        }

        // Phase 7: Post-processing
        self.transition(case_id, Stage::PostProcessing);
        let forces = self.run_post_process(case_path)?;

        // Phase 8: Generate report
        self.transition(case_id, Stage::Report);
        self.generate_report(case_path, &case_name, &forces, &solver_stats)?;

        // Complete
        self.transition(case_id, Stage::Complete);
        info!("Pipeline complete for case '{}'", case_name);
        self.emit(case_id, "pipeline", format!("Pipeline complete for '{}'", case_name));

        Ok(Stage::Complete)
    }

    // ── Stage implementations ──

    fn run_surface_features(&self, case_path: &Path) -> Result<(), anyhow::Error> {
        info!("  Running surfaceFeatureExtract...");
        let output = Command::new("surfaceFeatureExtract")
            .args(["-case", &case_path.to_string_lossy()])
            .output()?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            // surfaceFeatureExtract often warns but still succeeds; only fail on fatal
            if stderr.contains("FOAM FATAL") || stderr.contains("--> FOAM FATAL") {
                anyhow::bail!("surfaceFeatureExtract failed: {}", stderr);
            }
        }
        info!("  ✓ surfaceFeatureExtract complete");
        Ok(())
    }

    fn run_block_mesh(&self, case_path: &Path) -> Result<(), anyhow::Error> {
        info!("  Running blockMesh...");
        let output = Command::new("blockMesh")
            .args(["-case", &case_path.to_string_lossy()])
            .output()?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("blockMesh failed: {}", stderr);
        }
        info!("  ✓ blockMesh complete");
        Ok(())
    }

    fn run_snappy_hex_mesh(&self, case_path: &Path, attempt: u32) -> Result<(), anyhow::Error> {
        info!("  Running snappyHexMesh (attempt {})...", attempt);
        // For re-mesh attempts > 1, override mesh quality settings to relax
        let mut args = vec!["-case".to_string(), case_path.to_string_lossy().to_string()];
        if attempt > 1 {
            args.push("-overwrite".to_string());
        }
        let output = Command::new("snappyHexMesh")
            .args(&args)
            .output()?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            if stderr.contains("FOAM FATAL") || stderr.contains("--> FOAM FATAL") {
                anyhow::bail!("snappyHexMesh failed: {}", stderr);
            }
            warn!("snappyHexMesh may have partial output — continuing");
        }
        info!("  ✓ snappyHexMesh complete");
        Ok(())
    }

    fn run_check_mesh(&self, case_path: &Path) -> Result<MeshQualityMetrics, anyhow::Error> {
        info!("  Running checkMesh...");
        let output = Command::new("checkMesh")
            .args(["-case", &case_path.to_string_lossy()])
            .output()?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        let combined = format!("{}\n{}", stdout, stderr);

        if !output.status.success() && !combined.contains("Mesh OK") && !combined.contains("Failed 0 mesh checks") {
            anyhow::bail!("checkMesh execution failed");
        }

        // Parse checkMesh output
        let metrics = parse_checkmesh_output(&combined);
        info!(
            "  Mesh: {} cells, nonOrth={:.1}°, skewness={:.2}, failed={}",
            metrics.n_cells, metrics.max_non_orthogonality,
            metrics.max_skewness, metrics.n_failed_cells
        );
        Ok(metrics)
    }

    fn setup_solver(&self, case_path: &Path, solver_name: &str) -> Result<(), anyhow::Error> {
        info!("  Setting up solver configuration...");

        // Write controlDict
        let control_dict = format!(
            r#"FoamFile {{ version 2.0; format binary; class dictionary; object controlDict; }}

application     {};
startFrom       latestTime;
startTime       0;
stopAt          endTime;
endTime         3000;
deltaT          1;
writeControl    timeStep;
writeInterval   500;
purgeWrite      3;
writeFormat     binary;
"#, solver_name);

        let system_dir = case_path.join("system");
        std::fs::create_dir_all(&system_dir)?;
        std::fs::write(system_dir.join("controlDict"), &control_dict)?;

        // Write fvSchemes (default second-order)
        let fv_schemes = r#"FoamFile { version 2.0; format binary; class dictionary; object fvSchemes; }

ddtSchemes { default         Euler; }
gradSchemes { default         Gauss linear; }
divSchemes {
    default         none;
    div(phi,U)      Gauss linearUpwindV grad(U);
    div(phi,k)      Gauss upwind;
    div(phi,omega)  Gauss upwind;
    div((nuEff*dev2(T(grad(U))))) Gauss linear;
}
laplacianSchemes { default Gauss linear corrected; }
interpolationSchemes { default linear; }
snGradSchemes { default corrected; }
"#;
        std::fs::write(system_dir.join("fvSchemes"), fv_schemes)?;

        // Write fvSolution (PIMPLE with defaults)
        let fv_solution = format!(r#"FoamFile {{ version 2.0; format binary; class dictionary; object fvSolution; }}

solvers
{{
    p
    {{
        solver          GAMG;
        tolerance       1e-6;
        relTol          0.01;
        smoother        GaussSeidel;
    }}
    U
    {{
        solver          smoothSolver;
        smoother        symGaussSeidel;
        tolerance       1e-6;
        relTol          0;
        nSweeps         1;
    }}
    k
    {{
        $U;
        tolerance       1e-6;
        relTol          0;
    }}
    omega
    {{
        $U;
        tolerance       1e-6;
        relTol          0;
    }}
}}

PIMPLE
{{
    nOuterCorrectors 1;
    nCorrectors     2;
    nNonOrthogonalCorrectors 0;
    pRefCell        0;
    pRefValue       0;
}}

relaxationFactors
{{
    fields
    {{
        p               0.3;
    }}
    equations
    {{
        U               0.7;
        k               0.7;
        omega           0.7;
    }}
}}
"#);
        std::fs::write(system_dir.join("fvSolution"), fv_solution)?;

        info!("  ✓ Solver configuration written");
        Ok(())
    }

    fn run_solver(
        &self,
        case_id: CaseId,
        case_path: &Path,
        solver_name: &str,
        cancel: Option<Arc<AtomicBool>>,
    ) -> Result<SolverStats, anyhow::Error> {
        info!("  Running {}...", solver_name);

        let eb = self.event_bus.clone();
        let progress: Option<ProgressCallback> = Some(Box::new(move |iter, p_res, u_res| {
            let _ = eb.send(SystemEvent::info(
                Some(case_id),
                "solver",
                format!("Iter {}: p={:.2e}, U={:.2e}", iter, p_res, u_res),
            ));
        }));

        let stats = SolverLauncher::run_and_monitor(
            case_path, solver_name, cancel, progress, 500, 0.05,
        )?;

        if stats.converged {
            info!("  ✓ {} converged in {} iterations ({:.1}s)",
                solver_name, stats.iterations, stats.wall_time_s);
        } else {
            warn!("  {} finished (no conv): p={:.2e}, U={:.2e}",
                solver_name, stats.residual_p, stats.residual_u);
        }
        Ok(stats)
    }

    fn run_post_process(&self, case_path: &Path) -> Result<ForceCoefficients, anyhow::Error> {
        info!("  Running postProcessing (forceCoeffs)...");

        // Run postProcess utility to compute force coefficients
        let output = Command::new("postProcess")
            .args([
                "-case", &case_path.to_string_lossy(),
                "-func", "forceCoeffs",
            ])
            .output();

        // Try parsing existing force data (postProcess may have already run or data may exist)
        match ForceExtractor::extract_from_case(&case_path.to_string_lossy()) {
            Ok(forces) => {
                info!("  ✓ Forces: Cl={:.4}, Cd={:.4}, Cm={:.4}", forces.cl, forces.cd, forces.cm);
                Ok(forces)
            }
            Err(e) => {
                // If postProcess failed and no data exists, return zeros
                if let Err(p_err) = output {
                    warn!("postProcess command failed: {}. Using zero forces.", p_err);
                } else if let Ok(out) = output {
                    if !out.status.success() {
                        let stderr = String::from_utf8_lossy(&out.stderr);
                        warn!("postProcess warnings: {}", stderr);
                    }
                }
                warn!("Could not extract forces: {}. Returning zeros.", e);
                Ok(ForceCoefficients {
                    cl: 0.0, cd: 0.0, cm: 0.0, cl_std: 0.0, cd_std: 0.0,
                })
            }
        }
    }

    fn generate_report(
        &self,
        case_path: &Path,
        case_name: &str,
        forces: &ForceCoefficients,
        solver: &SolverStats,
    ) -> Result<(), anyhow::Error> {
        info!("  Generating report...");

        let report_dir = case_path.join("report");
        let template_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent().unwrap()
            .parent().unwrap()
            .join("templates");

        let report_gen = ReportGenerator::new(&template_dir)?;

        let case_meta = CaseMeta {
            id: Uuid::nil(),
            name: case_name.to_string(),
            stage: Stage::Complete,
            user_id: None,
            workspace_root: None,
            flow_type: None,
            compressibility: None,
            accuracy: None,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };

        let mesh_metrics = MeshQualityMetrics {
            max_non_orthogonality: 0.0,
            avg_non_orthogonality: 0.0,
            max_skewness: 0.0,
            min_determinant: 0.0,
            max_aspect_ratio: 0.0,
            min_volume: 0.0,
            n_cells: 0,
            n_failed_cells: 0,
        };

        report_gen.generate_html_report(
            &case_meta,
            &mesh_metrics,
            forces,
            solver,
            &report_dir,
        )?;

        info!("  ✓ Report: {:?}/index.html", report_dir);
        Ok(())
    }

    // ── Helpers ──

    fn transition(&mut self, case_id: CaseId, stage: Stage) {
        if let Some(meta) = self.cases.get_mut(&case_id) {
            let prev = meta.stage.label();
            meta.stage = stage.clone();
            meta.updated_at = chrono::Utc::now();
            tracing::info!("Case {}: {} → {}", case_id, prev, stage.label());
        }
        self.active_cases.insert(case_id, stage);
    }

    fn emit(&self, case_id: CaseId, stage: &str, msg: String) {
        let _ = self.event_bus.send(SystemEvent::info(Some(case_id), stage, msg));
    }

    pub fn get_case(&self, case_id: &CaseId) -> Option<&CaseMeta> {
        self.cases.get(case_id)
    }

    pub fn list_cases(&self) -> Vec<&CaseMeta> {
        self.cases.values().collect()
    }

    pub fn list_active(&self) -> Vec<(CaseId, Stage)> {
        self.active_cases
            .iter()
            .filter(|(_, s)| !s.is_terminal())
            .map(|(id, s)| (*id, s.clone()))
            .collect()
    }
}

/// Parse checkMesh stdout into structured metrics.
fn parse_checkmesh_output(output: &str) -> MeshQualityMetrics {
    let mut metrics = MeshQualityMetrics {
        max_non_orthogonality: 0.0,
        avg_non_orthogonality: 0.0,
        max_skewness: 0.0,
        min_determinant: 1.0,
        max_aspect_ratio: 0.0,
        min_volume: 0.0,
        n_cells: 0,
        n_failed_cells: 0,
    };

    let mut failed_cells = 0u64;

    for line in output.lines() {
        // Cells: "cells:   1847321"
        if line.starts_with("cells:") {
            if let Some(val) = line.split_whitespace().nth(1) {
                if let Ok(n) = val.replace(',', "").parse::<u64>() {
                    metrics.n_cells = n;
                }
            }
        }

        // Non-orthogonality: "Maximum = 62.3"
        if line.contains("non-orthogonality") || line.contains("Non-orthogonality") {
            if let Some(max_part) = line.split("Maximum = ").nth(1) {
                if let Some(val) = max_part.split_whitespace().next() {
                    if let Ok(v) = val.parse::<f64>() {
                        metrics.max_non_orthogonality = metrics.max_non_orthogonality.max(v);
                    }
                }
            }
            if let Some(avg_part) = line.split("average = ").nth(1) {
                if let Some(val) = avg_part.split_whitespace().next() {
                    if let Ok(v) = val.parse::<f64>() {
                        metrics.avg_non_orthogonality = v;
                    }
                }
            }
        }

        // Skewness: "Max skewness = 3.2"
        if line.contains("skewness") {
            if let Some(val) = line.split('=').nth(1) {
                if let Ok(v) = val.trim().parse::<f64>() {
                    metrics.max_skewness = metrics.max_skewness.max(v);
                }
            }
        }

        // Determinant: "minimum = 0.12"
        if line.contains("determinant") || line.contains("Determinant") {
            if let Some(val) = line.split('=').nth(1) {
                if let Ok(v) = val.trim().parse::<f64>() {
                    metrics.min_determinant = metrics.min_determinant.min(v);
                }
            }
        }

        // Aspect ratio: "Maximum aspect ratio = 87"
        if line.contains("aspect ratio") || line.contains("Aspect ratio") {
            if let Some(val) = line.split('=').nth(1) {
                if let Ok(v) = val.trim().parse::<f64>() {
                    metrics.max_aspect_ratio = metrics.max_aspect_ratio.max(v);
                }
            }
        }

        // Failed cells: "Failed 1 mesh checks"
        if line.contains("Failed") && line.contains("mesh checks") {
            if let Some(val) = line.split_whitespace().nth(1) {
                if let Ok(n) = val.parse::<u64>() {
                    failed_cells = n;
                }
            }
        }

        // Min volume: "Min volume = 2.4e-11"
        if line.contains("Min volume") || line.contains("minimum volume") {
            if let Some(val) = line.split('=').nth(1) {
                if let Ok(v) = val.trim().parse::<f64>() {
                    metrics.min_volume = v;
                }
            }
        }
    }

    metrics.n_failed_cells = failed_cells;
    metrics
}
