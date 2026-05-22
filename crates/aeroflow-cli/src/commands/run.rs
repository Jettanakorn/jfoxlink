use aeroflow_core::SettingsManager;
use aeroflow_pipeline::PipelineOrchestrator;
use std::path::Path;
use tracing::info;

pub async fn execute(case_path: &Path, trials: u32) -> anyhow::Result<()> {
    info!("Running pipeline for case: {:?} (trials={})", case_path, trials);

    if !case_path.exists() {
        anyhow::bail!("Case directory not found: {:?}", case_path);
    }

    let manifest_path = case_path.join("manifest.json");
    let manifest: serde_json::Value = if manifest_path.exists() {
        let content = std::fs::read_to_string(&manifest_path)?;
        serde_json::from_str(&content)?
    } else {
        serde_json::json!({})
    };

    let solver = manifest.get("solver")
        .and_then(|s| s.as_str())
        .unwrap_or("simpleFoam");
    let case_name = manifest.get("name")
        .and_then(|s| s.as_str())
        .unwrap_or("unnamed");

    let settings = SettingsManager::load();
    let write_format = &settings.settings.openfoam_format;

    println!("\n=== AeroFlow Agent — Pipeline Execution ===\n");
    println!("  Case:    {}", case_name);
    println!("  Path:    {:?}", case_path);
    println!("  Solver:  {}", solver);
    println!("  Trials:  {}", if trials > 0 { format!("{}", trials) } else { "single".into() });
    println!("  Format:  {} (binary)", write_format.label());
    println!();

    for tool in &["blockMesh", "snappyHexMesh", "checkMesh", solver] {
        if which::which(tool).is_err() {
            anyhow::bail!("OpenFOAM tool '{}' not found in PATH. Are you inside the AeroFlow container?", tool);
        }
    }

    let data_dir = settings.settings.data_dir.clone();
    let max_conc = settings.settings.max_concurrent_cases;

    // Build orchestrator and register the case synchronously
    let mut orchestrator = PipelineOrchestrator::new(data_dir.into(), max_conc);
    let case_id = orchestrator.register_case(case_name);
    info!("Registered case {} with id {}", case_name, case_id);

    // Run pipeline (synchronous, uses std::process::Command for OpenFOAM tools)
    match orchestrator.run_pipeline(case_id, case_path, solver, None, None) {
        Ok(result) => {
            println!("\n✓ Pipeline complete for '{}'", case_name);
            println!("  Final stage: {:?}", result.stage);
            println!("  Report: {:?}/report/index.html", case_path);
            println!("  Cd={:.4}, Cl={:.4}, {} iters",
                result.forces.cd, result.forces.cl, result.solver_stats.iterations);
        }
        Err(e) => {
            println!("\n✗ Pipeline failed for '{}': {}", case_name, e);
            println!("  Check logs in {:?}/logs/", case_path);
            return Err(e);
        }
    }

    Ok(())
}
