use aeroflow_core::{
    CaseConfig, CaseId, CaseMeta, ForceCoefficients, IntakeConfig, MeshParams, MeshQualityMetrics,
    SolverStats, Stage, SystemEvent, EventBus, WindTunnelDomainSizer,
    create_event_bus,
};
use aeroflow_mesh::{GeoBounds, MeshGenerator, MeshQualityEngine};
use aeroflow_post::ForceExtractor;
use aeroflow_skills::SkillsDb;
use aeroflow_solver::{ProgressCallback, SolverLauncher, WindTunnelBcGenerator};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use tracing::{info, warn};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct PipelineResult {
    pub stage: Stage,
    pub forces: ForceCoefficients,
    pub mesh_metrics: MeshQualityMetrics,
    pub solver_stats: SolverStats,
}

pub struct PipelineOrchestrator {
    event_bus: EventBus,
    cases: HashMap<CaseId, CaseMeta>,
    active_cases: HashMap<CaseId, Stage>,
    max_concurrent: u32,
    data_dir: PathBuf,
    db: Option<SkillsDb>,
}

impl PipelineOrchestrator {
    pub fn new(data_dir: PathBuf, max_concurrent: u32) -> Self {
        Self {
            event_bus: create_event_bus(256),
            cases: HashMap::new(),
            active_cases: HashMap::new(),
            max_concurrent,
            data_dir,
            db: None,
        }
    }

    pub fn with_db(mut self, db: SkillsDb) -> Self {
        self.db = Some(db);
        self
    }

    pub fn event_bus(&self) -> &EventBus {
        &self.event_bus
    }

    pub fn register_case(&mut self, name: &str) -> CaseId {
        self.register_case_with_id(name, Uuid::new_v4())
    }

    pub fn register_case_with_id(&mut self, name: &str, id: CaseId) -> CaseId {
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
        mesh_params: Option<&MeshParams>,
        case_config: Option<&CaseConfig>,
    ) -> Result<PipelineResult, anyhow::Error> {
        if self.active_cases.len() as u32 > self.max_concurrent {
            anyhow::bail!("Max concurrent cases reached ({}).", self.max_concurrent);
        }

        let case_name = self.cases.get(&case_id)
            .map(|m| m.name.clone())
            .unwrap_or_else(|| case_path.to_string_lossy().to_string());

        info!("Starting pipeline for case '{}' ({})", case_name, case_id);
        self.emit(case_id, "pipeline", format!("Pipeline started for '{}'", case_name));

        // Ensure logs directory exists
        let _ = std::fs::create_dir_all(case_path.join("logs"));

        // Verify case directory
        if !case_path.join("constant").join("triSurface").exists() {
            anyhow::bail!("No STL geometry found in {:?}. Run 'aeroflow init' first.", case_path.join("constant/triSurface"));
        }

        // Phase 1: Import — STL already placed by init
        self.transition(case_id, Stage::Imported);
        self.write_initial_system_dicts(case_path)?;

        // Phase 2: Surface features
        self.transition(case_id, Stage::SurfacePrep);
        self.run_surface_features(case_path)?;

        // Phase 3: Generate background mesh
        self.transition(case_id, Stage::Meshing);
        // Remove any leftover mesh time dirs before starting fresh meshing
        self.remove_all_time_dirs(case_path);
        self.write_blockmesh_dict(case_path, case_config)?;
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

            // Reset to clean blockMesh background before each snappyHexMesh attempt
            // (snappyHexMesh fails with hexRef8 errors if run on an already-refined mesh)
            if attempt > 1 {
                let _ = std::fs::remove_dir_all(case_path.join("constant").join("polyMesh"));
                self.run_block_mesh(case_path)?;
            }

            // Write snappyHexMeshDict (with refinement regions from attempt 1)
            {
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
                    rotating: None,
                    hypersonic: None,
                    cht: None,
                    mhd: None,
                    pemfc: None,
                    wind_tunnel: None,
                    multiphase: None,
                    non_newtonian: None,
                    viscoelastic: None,
                    combustion: None,
                    cavitation: None,
                    spray: None,
                    phase_change: None,
                    particle: None,
                    porous: None,
                    aeroacoustic: None,
                    fsi: None,
                    wave: None,
                    wind_turbine: None,
                    electrostatic: None,
                    ablation: None,
                    propulsion: None,
                    nuclear: None,
                    marine: None,
                    ml_surrogate: None,
                };
                let stl_path = self.find_stl(case_path);
                let geo_bounds = stl_path.as_ref().and_then(|p| GeoBounds::from_stl(p));
                let stl_stem = stl_path.as_ref().and_then(|p| p.file_stem()).and_then(|s| s.to_str());
                let snappy_dict = mesh_gen.generate_adaptive_snappy_with_bounds(&dummy_config, &mesh_metrics, attempt, geo_bounds.as_ref(), stl_stem, mesh_params);
                std::fs::write(case_path.join("system").join("snappyHexMeshDict"), &snappy_dict)?;
                info!("  Written snappyHexMeshDict (attempt {})", attempt);
            }

            self.run_snappy_hex_mesh(case_path, attempt)?;

            // SnappyHexMesh may write the refined mesh to a time directory;
            // ensure it's available in constant/polyMesh
            self.ensure_mesh_in_constant(case_path);

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

        // Clean stale time directories (keep 0/ with initial fields)
        self.clean_time_dirs(case_path);

        // Phase 5: Solver setup — copy controlDict, fvSchemes, fvSolution
        self.transition(case_id, Stage::Setup);
        self.setup_solver(case_path, solver_name, case_config)?;

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
        let forces = self.run_post_process(case_path, case_config)?;

        // Phase 8: Visualization — generate VTK export and rendered images
        self.transition(case_id, Stage::Visualization);
        let viz_images = self.run_visualization(case_path)?;

        // Phase 9: Generate report
        self.transition(case_id, Stage::Report);
        self.generate_report(case_path, &case_name, &mesh_metrics, &forces, &solver_stats, &viz_images)?;

        // Persist results to database if available
        self.persist_to_db(case_id, &mesh_metrics, &forces, &solver_stats);

        // Complete
        self.transition(case_id, Stage::Complete);
        info!("Pipeline complete for case '{}'", case_name);
        self.emit(case_id, "pipeline", format!("Pipeline complete for '{}'", case_name));

        Ok(PipelineResult {
            stage: Stage::Complete,
            forces,
            mesh_metrics,
            solver_stats,
        })
    }

    // ── Stage implementations ──

    /// Write minimal system dicts so pre-solver tools (surfaceFeatureExtract, blockMesh, snappyHexMesh) can start.
    /// The solver phase will overwrite these with real ones.
    fn write_initial_system_dicts(&self, case_path: &Path) -> Result<(), anyhow::Error> {
        let system_dir = case_path.join("system");
        std::fs::create_dir_all(&system_dir)?;
        let files: Vec<(&str, &str)> = vec![
            ("controlDict", "application surfaceFeatureExtract;\nstartFrom startTime;\nstartTime 0;\nstopAt endTime;\nendTime 1;\ndeltaT 1;\nwriteControl timeStep;\nwriteInterval 1;\n"),
            ("surfaceFeatureExtractDict", "surfaces (\"*.stl\");\nextractionMethod extractFromSurface;\nextractFromSurfaceCoeffs { includedAngle 150; writeObj true; }\n"),
            ("fvSchemes", "ddtSchemes { default steadyState; }\ngradSchemes { default Gauss linear; }\ndivSchemes { default Gauss linear; }\nlaplacianSchemes { default Gauss linear corrected; }\ninterpolationSchemes { default linear; }\nsnGradSchemes { default corrected; }\n"),
            ("fvSolution", "solvers { p { solver PCG; preconditioner DIC; tolerance 1e-06; relTol 0; } U { solver smoothSolver; smoother GaussSeidel; tolerance 1e-06; relTol 0; nSweeps 1; } } SIMPLE { nNonOrthogonalCorrectors 0; consistent yes; } relaxationFactors { p 0.3; U 0.7; } \n"),
        ];
        for (name, content) in &files {
            let path = system_dir.join(name);
            if !path.exists() {
                let dict = format!(
                    "FoamFile {{ version 2.0; format ascii; class dictionary; object {name}; }}\n{content}"
                );
                std::fs::write(&path, dict.as_bytes())?;
            }
        }
        Ok(())
    }

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

    fn find_stl(&self, case_path: &Path) -> Option<std::path::PathBuf> {
        let tri_surface = case_path.join("constant").join("triSurface");
        if tri_surface.is_dir() {
            for entry in std::fs::read_dir(&tri_surface).ok()? {
                let entry = entry.ok()?;
                let path = entry.path();
                let ext = path.extension()?.to_string_lossy().to_lowercase();
                if ext == "stl" || ext == "stlb" {
                    return Some(path);
                }
            }
        }
        None
    }

    fn write_blockmesh_dict(&self, case_path: &Path, case_config: Option<&CaseConfig>) -> Result<(), anyhow::Error> {
        let stl_path = self.find_stl(case_path);
        let geo_bounds = stl_path.as_ref().and_then(|p| GeoBounds::from_stl(p));

        // If wind_tunnel config is present, use chord-based asymmetric blockMesh
        if let Some(cfg) = case_config.and_then(|c| c.wind_tunnel.as_ref()) {
            info!("  Writing digital wind tunnel blockMeshDict...");
            let bounds = geo_bounds.as_ref().ok_or_else(|| {
                anyhow::anyhow!("Cannot generate wind tunnel blockMesh without STL bounds")
            })?;
            let mesh_gen = MeshGenerator::with_format(aeroflow_core::OpenFOAMFormat::Binary);
            let dict = mesh_gen.generate_wind_tunnel_blockmesh(bounds, Some(cfg));
            std::fs::write(case_path.join("system").join("blockMeshDict"), dict.as_bytes())?;
            return Ok(());
        }

        // Fallback to existing uniform-padding blockMesh
        info!("  Writing blockMeshDict (uniform padding)...");
        if geo_bounds.is_some() {
            info!("    Auto-sized from STL bounding box");
        } else {
            info!("    Using default domain (STL bounds unavailable)");
        }
        let mesh_gen = MeshGenerator::with_format(aeroflow_core::OpenFOAMFormat::Binary);
        let dict = mesh_gen.generate_blockmesh_with_bounds(geo_bounds.as_ref());
        std::fs::write(case_path.join("system").join("blockMeshDict"), dict.as_bytes())?;
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
        let args = vec!["-case".to_string(), case_path.to_string_lossy().to_string(), "-overwrite".to_string()];
        let output = Command::new("snappyHexMesh")
            .args(&args)
            .output()?;
        // Save log for debugging
        let log_path = case_path.join("logs").join("snappyHexMesh.log");
        let combined = format!("{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr));
        let _ = std::fs::write(&log_path, &combined);
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
            .args(["-case", &case_path.to_string_lossy(), "-constant"])
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

    /// Remove stale time directories (keep 0/ for initial fields).
    /// Preserves directories containing a polyMesh (snappyHexMesh may write the
    /// refined mesh to a time directory rather than constant/polyMesh).
    fn clean_time_dirs(&self, case_path: &Path) {
        if let Ok(entries) = std::fs::read_dir(case_path) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir()
                    && let Some(name) = path.file_name().and_then(|n| n.to_str())
                        && let Ok(time) = name.parse::<f64>()
                            && time > 0.0 && !path.join("polyMesh").exists() {
                                let _ = std::fs::remove_dir_all(&path);
                            }
            }
        }
        // Clean postProcessing from previous runs
        let pp = case_path.join("postProcessing");
        if pp.exists() {
            let _ = std::fs::remove_dir_all(&pp);
        }
    }

    /// Remove ALL time directories (except 0/) before re-meshing.
    /// Meshing is always restarted from blockMesh, so old mesh time dirs are stale.
    fn remove_all_time_dirs(&self, case_path: &Path) {
        if let Ok(entries) = std::fs::read_dir(case_path) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir()
                    && let Some(name) = path.file_name().and_then(|n| n.to_str())
                        && let Ok(time) = name.parse::<f64>()
                            && time > 0.0 {
                                let _ = std::fs::remove_dir_all(&path);
                            }
            }
        }
    }

    /// After snappyHexMesh, ensure the refined mesh is in constant/polyMesh.
    /// Some OpenFOAM versions write to a time directory even with -overwrite.
    fn ensure_mesh_in_constant(&self, case_path: &Path) {
        if case_path.join("constant").join("polyMesh").join("points").exists() {
            return; // mesh already in constant
        }
        // Find latest time directory with a polyMesh and copy to constant/
        let latest = self.find_latest_mesh_time(case_path);
        if let Some(t) = latest {
            let src = case_path.join(&t).join("polyMesh");
            let dst = case_path.join("constant").join("polyMesh");
            if src.exists() && !dst.join("points").exists() {
                let _ = std::fs::create_dir_all(&dst);
                if let Ok(entries) = std::fs::read_dir(&src) {
                    for entry in entries.flatten() {
                        let name = entry.file_name();
                        let _ = std::fs::copy(entry.path(), dst.join(&name));
                    }
                }
                info!("  Copied mesh from time {} to constant/polyMesh", t);
            }
        }
    }

    fn setup_solver(&self, case_path: &Path, solver_name: &str, case_config: Option<&CaseConfig>) -> Result<(), anyhow::Error> {
        info!("  Setting up solver configuration...");

        // Determine magUInf and reference values from case_config or defaults
        let mag_u_inf = case_config
            .and_then(|c| c.wind_tunnel.as_ref())
            .and_then(|wt| wt.velocity_m_s)
            .or_else(|| case_config.map(|c| c.velocity_m_s))
            .unwrap_or(9.15);
        let l_ref = case_config
            .and_then(|c| c.reference_length_m)
            .unwrap_or(0.3);
        let a_ref = l_ref * l_ref;

        // Write controlDict
        let control_dict = format!(
            r#"FoamFile {{ version 2.0; format binary; class dictionary; object controlDict; }}

application     {};
startFrom       latestTime;
startTime       0;
stopAt          endTime;
endTime         1000;
deltaT          1;
writeControl    timeStep;
writeInterval   100;
purgeWrite      3;
writeFormat     binary;
writePrecision  8;
writeCompression on;
timeFormat      general;
timePrecision   6;
runTimeModifiable true;
functions
{{
    forceCoeffs
    {{
        type            forceCoeffs;
        libs            (forces);
        patches         (blade);
        rho             rhoInf;
        rhoInf          1.225;
        CofR            (0 0 0);
        liftDir         (0 0 1);
        dragDir         (1 0 0);
        pitchAxis       (0 1 0);
        magUInf         {};
        lRef            {};
        Aref            {};
        writeControl    timeStep;
        writeInterval   10;
    }}
}}
"#, solver_name, mag_u_inf, l_ref, a_ref);

        let system_dir = case_path.join("system");
        std::fs::create_dir_all(&system_dir)?;
        std::fs::write(system_dir.join("controlDict"), &control_dict)?;

        // Write fvSchemes (default second-order)
        let fv_schemes = r#"FoamFile { version 2.0; format binary; class dictionary; object fvSchemes; }

ddtSchemes { default         steadyState; }
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
wallDist
{
    method meshWave;
}
"#;
        std::fs::write(system_dir.join("fvSchemes"), fv_schemes)?;

        // Write fvSolution (PIMPLE with defaults)
        let fv_solution = r#"FoamFile { version 2.0; format binary; class dictionary; object fvSolution; }

solvers
{
    p
    {
        solver          GAMG;
        tolerance       1e-6;
        relTol          0.01;
        smoother        GaussSeidel;
    }
    U
    {
        solver          smoothSolver;
        smoother        symGaussSeidel;
        tolerance       1e-6;
        relTol          0;
        nSweeps         3;
    }
    k
    {
        $U;
        tolerance       1e-6;
        relTol          0;
        nSweeps         3;
    }
    omega
    {
        $U;
        tolerance       1e-6;
        relTol          0;
        nSweeps         3;
    }
}

SIMPLE
{
    nNonOrthogonalCorrectors 2;
    residualControl
    {
        U              1e-4;
        p              1e-4;
        k              1e-4;
        omega          1e-4;
    }
    pRefPoint       (0 0 0);
    pRefValue       0;
}

relaxationFactors
{
    fields
    {
        p               0.3;
    }
    equations
    {
        U               0.7;
        k               0.7;
        omega           0.7;
    }
}
"#.to_string();
        std::fs::write(system_dir.join("fvSolution"), fv_solution)?;

        // Write constant/transportProperties
        let constant_dir = case_path.join("constant");
        std::fs::create_dir_all(&constant_dir)?;
        let transport = r#"FoamFile { version 2.0; format ascii; class dictionary; object transportProperties; }

phase (air);
transportModel  Newtonian;
nu              nu [0 2 -1 0 0 0 0] 1.5e-05;
rho             rho [1 -3 0 0 0 0 0] 1.225;
"#;
        std::fs::write(constant_dir.join("transportProperties"), transport)?;

        // Write constant/turbulenceProperties
        let turb_model = case_config
            .map(|c| c.turbulence_model.as_str())
            .unwrap_or("kOmegaSST");
        let turb = format!(
            r#"FoamFile {{ version 2.0; format ascii; class dictionary; object turbulenceProperties; }}

simulationType  RAS;
RAS
{{
    RASModel        {};
    turbulence      on;
    printCoeffs     on;
}}
"#, turb_model);
        std::fs::write(constant_dir.join("turbulenceProperties"), turb)?;

        // Write initial field files (0/U, 0/p, 0/k, 0/omega, 0/nut)
        let zero_dir = case_path.join("0");
        std::fs::create_dir_all(&zero_dir)?;

        // Use WindTunnelBcGenerator if wind_tunnel config is present
        if let Some(wt_cfg) = case_config.and_then(|c| c.wind_tunnel.as_ref()) {
            let wall_patches = vec!["blade".to_string()];
            let files = WindTunnelBcGenerator::generate_all(wt_cfg, &wall_patches);
            for (name, content) in &files {
                let path = zero_dir.join(name);
                if !path.exists() {
                    std::fs::write(&path, content.as_bytes())?;
                }
            }
        } else {
            // Legacy hardcoded field files
            let fields: Vec<(&str, &str, &str)> = vec![
                ("U", "volVectorField", "uniform (9.15 0 0)"),
                ("p", "volScalarField", "uniform 0"),
                ("k", "volScalarField", "uniform 0.314"),
                ("omega", "volScalarField", "uniform 100"),
                ("nut", "volScalarField", "uniform 0"),
            ];
            for (name, class, internal) in &fields {
                let path = zero_dir.join(name);
                if !path.exists() {
                    let dim = match *name {
                        "U" => "[0 1 -1 0 0 0 0]",
                        "p" => "[0 2 -2 0 0 0 0]",
                        "k" => "[0 2 -2 0 0 0 0]",
                        "omega" => "[0 0 -1 0 0 0 0]",
                        "nut" => "[0 2 -1 0 0 0 0]",
                        _ => "[0 0 0 0 0 0 0]",
                    };
                    let bc_body = if *name == "U" {
                        r#"blade   { type fixedValue; value uniform (0 0 0); }
    inlet    { type fixedValue; value $internalField; }
    outlet   { type zeroGradient; }
    top      { type zeroGradient; }
    bottom   { type zeroGradient; }
    front    { type zeroGradient; }
    back     { type zeroGradient; }
    ".*"     { type zeroGradient; }"#
                    } else if *name == "p" {
                        r#"blade   { type zeroGradient; }
    inlet    { type zeroGradient; }
    outlet   { type fixedValue; value uniform 0; }
    top      { type zeroGradient; }
    bottom   { type zeroGradient; }
    front    { type zeroGradient; }
    back     { type zeroGradient; }
    ".*"     { type zeroGradient; }"#
                    } else if *name == "nut" {
                        r#"blade   { type nutUSpaldingWallFunction; value uniform 0; }
    inlet    { type calculated; value $internalField; }
    outlet   { type calculated; value $internalField; }
    top      { type calculated; value $internalField; }
    bottom   { type calculated; value $internalField; }
    front    { type calculated; value $internalField; }
    back     { type calculated; value $internalField; }
    ".*"     { type calculated; value $internalField; }"#
                    } else if *name == "k" || *name == "omega" {
                        r#"blade   { type zeroGradient; }
    inlet    { type fixedValue; value $internalField; }
    outlet   { type zeroGradient; }
    top      { type zeroGradient; }
    bottom   { type zeroGradient; }
    front    { type zeroGradient; }
    back     { type zeroGradient; }
    ".*"     { type zeroGradient; }"#
                    } else {
                        r#"blade   { type zeroGradient; }
    inlet    { type zeroGradient; }
    outlet   { type zeroGradient; }
    top      { type zeroGradient; }
    bottom   { type zeroGradient; }
    front    { type zeroGradient; }
    back     { type zeroGradient; }
    ".*"     { type zeroGradient; }"#
                    };
                    let content = format!(
                        r#"FoamFile {{ version 2.0; format ascii; class {}; object {}; }}
dimensions {};
internalField {};
boundaryField {{
    {}
}}
"#, class, name, dim, internal, bc_body
                    );
                    std::fs::write(&path, content.as_bytes())?;
                }
            }
        }

        // Copy initial fields from 0/ to the latest time directory (snappyHexMesh may have written
        // mesh to a later time that has no field files yet).
        let latest_mesh_time = self.find_latest_mesh_time(case_path);
        if let Some(ref t) = latest_mesh_time
            && t != "0" {
                let zero_dir = case_path.join("0");
                let target_dir = case_path.join(t);
                for entry in std::fs::read_dir(&zero_dir)? {
                    let entry = entry?;
                    let name = entry.file_name();
                    let src_path = zero_dir.join(&name);
                    if src_path.is_file() {
                        let dst_path = target_dir.join(&name);
                        if dst_path.exists() {
                            // Mesh may have changed between runs — overwrite stale fields
                            let _ = std::fs::remove_file(&dst_path);
                        }
                        std::fs::copy(&src_path, &dst_path)?;
                    }
                }
            }

        info!("  ✓ Solver configuration written");
        Ok(())
    }

    /// Find the latest time directory that contains a polyMesh (i.e. the mesh was written there).
    fn find_latest_mesh_time(&self, case_path: &Path) -> Option<String> {
        use std::cmp::Ordering;
        let dirs: Vec<_> = std::fs::read_dir(case_path).ok()?
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
            .map(|e| e.file_name().to_string_lossy().to_string())
            .filter(|n| n.parse::<f64>().is_ok()) // numeric directories only
            .filter(|n| case_path.join(n).join("polyMesh").exists())
            .collect();
        if dirs.is_empty() { return None; }
        dirs.into_iter()
            .max_by(|a, b| {
                let a: f64 = a.parse().unwrap_or(0.0);
                let b: f64 = b.parse().unwrap_or(0.0);
                a.partial_cmp(&b).unwrap_or(Ordering::Equal)
            })
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

    fn run_post_process(&self, case_path: &Path, case_config: Option<&CaseConfig>) -> Result<ForceCoefficients, anyhow::Error> {
        info!("  Running postProcessing (forceCoeffs)...");

        // Run postProcess utility to compute force coefficients
        let output = Command::new("postProcess")
            .args([
                "-case", &case_path.to_string_lossy(),
                "-func", "forceCoeffs",
            ])
            .output();

        // Compute and persist wind tunnel results if DWT is active
        if let Some(cfg) = case_config.and_then(|c| c.wind_tunnel.as_ref()) {
            let stl_path = self.find_stl(case_path);
            let geo_bounds = stl_path.as_ref().and_then(|p| {
                let mg = GeoBounds::from_stl(p)?;
                Some(aeroflow_core::types::GeoBounds {
                    min_x: mg.min_x, max_x: mg.max_x,
                    min_y: mg.min_y, max_y: mg.max_y,
                    min_z: mg.min_z, max_z: mg.max_z,
                })
            });
            if let Some(ref bounds) = geo_bounds {
                let chord = WindTunnelDomainSizer::chord_from_bounds(bounds);
                let forces = ForceExtractor::extract_from_case(&case_path.to_string_lossy()).ok();
                let (cl, cd) = forces.map(|f| (f.cl, f.cd)).unwrap_or((0.0, 0.0));
                let result = WindTunnelDomainSizer::compute_result(bounds, chord, Some(cfg), cl, cd);
                if let Ok(json) = serde_json::to_string(&result) {
                    let _ = std::fs::write(case_path.join("wind_tunnel_result.json"), &json);
                    info!("  ✓ Wind tunnel result written to wind_tunnel_result.json");
                }
            } else {
                warn!("  Skipping wind tunnel result: could not read STL bounds");
            }
        }

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
                } else if let Ok(out) = output
                    && !out.status.success() {
                        let stderr = String::from_utf8_lossy(&out.stderr);
                        warn!("postProcess warnings: {}", stderr);
                    }
                warn!("Could not extract forces: {}. Returning zeros.", e);
                Ok(ForceCoefficients {
                    cl: 0.0, cd: 0.0, cm: 0.0, cl_std: 0.0, cd_std: 0.0,
                })
            }
        }
    }

    fn run_visualization(&self, case_path: &Path) -> Result<Vec<String>, anyhow::Error> {
        let report_dir = case_path.join("report");
        std::fs::create_dir_all(&report_dir)?;
        match aeroflow_post::generate_visualization(case_path, &report_dir) {
            Ok(images) => {
                info!("  ✓ Generated {} visualization images", images.len());
                Ok(images)
            }
            Err(e) => {
                warn!("Visualization failed (non-fatal): {}", e);
                Ok(Vec::new())
            }
        }
    }

    fn generate_report(
        &self,
        case_path: &Path,
        case_name: &str,
        mesh: &MeshQualityMetrics,
        forces: &ForceCoefficients,
        solver_stats: &SolverStats,
        viz_images: &[String],
    ) -> Result<(), anyhow::Error> {
        let report_dir = case_path.join("report");
        std::fs::create_dir_all(&report_dir)?;

        let _report_path = report_dir.join("index.html");

        let engine = aeroflow_report::ReportGenerator::new()?;
        engine.generate_html_report(
            case_name, "", "",
            mesh, forces, solver_stats, viz_images,
            &report_dir,
        )?;

        info!("  ✓ Report: {:?}/index.html", report_dir);
        Ok(())
    }

    // ── Helpers ──

    /// Persist pipeline results to the database (fire-and-forget on failure).
    fn persist_to_db(
        &self,
        case_id: CaseId,
        mesh: &MeshQualityMetrics,
        forces: &ForceCoefficients,
        solver: &SolverStats,
    ) {
        let db = match self.db.as_ref() {
            Some(db) => db.clone(),
            None => return,
        };
        let forces_json = serde_json::json!({
            "cl": forces.cl, "cd": forces.cd, "cm": forces.cm,
            "cl_std": forces.cl_std, "cd_std": forces.cd_std,
        });
        let mesh_json = serde_json::json!({
            "max_non_orthogonality": mesh.max_non_orthogonality,
            "avg_non_orthogonality": mesh.avg_non_orthogonality,
            "max_skewness": mesh.max_skewness,
            "min_determinant": mesh.min_determinant,
            "max_aspect_ratio": mesh.max_aspect_ratio,
            "min_volume": mesh.min_volume,
            "n_cells": mesh.n_cells,
            "n_failed_cells": mesh.n_failed_cells,
        });
        let solver_json = serde_json::json!({
            "iterations": solver.iterations,
            "wall_time_s": solver.wall_time_s,
            "residual_p": solver.residual_p,
            "residual_u": solver.residual_u,
            "converged": solver.converged,
        });
        let status = if solver.converged { "complete" } else { "diverged" };

        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            let _ = handle.block_on(async {
                db.update_case_results(case_id, status, &forces_json, &mesh_json, &solver_json).await
            });
        }
    }

    fn transition(&mut self, case_id: CaseId, stage: Stage) {
        if let Some(meta) = self.cases.get_mut(&case_id) {
            let prev = meta.stage.label();
            meta.stage = stage;
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
            .map(|(id, s)| (*id, *s))
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

    /// Extract the first f64 value from a string that may have trailing text.
    fn parse_first_f64(s: &str) -> Option<f64> {
        let s = s.trim();
        // Find the first contiguous numeric segment (handles "4.99, 4 highly skew..." etc.)
        let mut num = String::new();
        for c in s.chars() {
            if c.is_ascii_digit() || c == '.' || c == '-' || c == '+' || c == 'e' || c == 'E' {
                num.push(c);
            } else if !num.is_empty() {
                break;
            }
        }
        num.parse().ok()
    }

    for line in output.lines() {
        // Cells: "cells:   1847321"
        if line.trim().starts_with("cells:")
            && let Some(val) = line.split_whitespace().nth(1)
                && let Ok(n) = val.replace(',', "").parse::<u64>() {
                    metrics.n_cells = n;
                }

        // Non-orthogonality: "Mesh non-orthogonality Max: 54.3 average: 5.95"
        if line.contains("non-orthogonality") || line.contains("Non-orthogonality") {
            if let Some(max_part) = line.split("Max: ").nth(1).or_else(|| line.split("Maximum = ").nth(1))
                && let Some(v) = parse_first_f64(max_part) {
                    metrics.max_non_orthogonality = metrics.max_non_orthogonality.max(v);
                }
            if let Some(avg_part) = line.split("average: ").nth(1).or_else(|| line.split("average = ").nth(1))
                && let Some(v) = parse_first_f64(avg_part) {
                    metrics.avg_non_orthogonality = v;
                }
        }

        // Skewness: "Max skewness = 4.9958392, 4 highly skew faces..."
        if line.contains("skewness")
            && let Some(val) = line.split('=').nth(1)
                && let Some(v) = parse_first_f64(val) {
                    metrics.max_skewness = metrics.max_skewness.max(v);
                }

        // Determinant: "minimum = 0.12"
        if (line.contains("determinant") || line.contains("Determinant"))
            && let Some(val) = line.split('=').nth(1)
                && let Some(v) = parse_first_f64(val) {
                    metrics.min_determinant = metrics.min_determinant.min(v);
                }

        // Aspect ratio: "Max aspect ratio = 6.19 OK."
        if (line.contains("aspect ratio") || line.contains("Aspect ratio"))
            && let Some(val) = line.split('=').nth(1)
                && let Some(v) = parse_first_f64(val) {
                    metrics.max_aspect_ratio = metrics.max_aspect_ratio.max(v);
                }

        // Failed checks: "Failed 1 mesh checks."
        if line.contains("Failed") && line.contains("mesh checks")
            && let Some(val) = line.split_whitespace().nth(1)
                && let Ok(n) = val.parse::<u64>() {
                    failed_cells = n;
                }

        // Min volume: "Min volume = 2.4e-11. Max volume = ..."
        if (line.contains("Min volume") || line.contains("minimum volume"))
            && let Some(val) = line.split('=').nth(1)
                && let Some(v) = parse_first_f64(val) {
                    metrics.min_volume = v;
                }
    }

    metrics.n_failed_cells = failed_cells;
    metrics
}
