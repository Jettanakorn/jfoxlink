use aeroflow_core::{SettingsManager, CaseConfig, FlowType, WindTunnelConfig};
use aeroflow_pipeline::PipelineOrchestrator;
use std::path::Path;
use tracing::info;

fn determine_flow_type(flow_type_str: &str) -> FlowType {
    match flow_type_str {
        "External (Digital Wind Tunnel)" | "external_wind_tunnel" => FlowType::ExternalWindTunnel,
        _ => FlowType::External,
    }
}

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
    let velocity = manifest.get("velocity").and_then(|v| v.as_f64()).unwrap_or(9.15);
    let flow_type_str = manifest.get("flow_type").and_then(|s| s.as_str()).unwrap_or("External");
    let turb_model = manifest.get("turbulence_model").and_then(|s| s.as_str()).unwrap_or("kOmegaSST");

    // Build an optional WindTunnelConfig from manifest
    let wind_tunnel = manifest.get("wind_tunnel").map(|wt| {
        let upstream = wt.get("upstream").and_then(|v| v.as_f64()).unwrap_or(20.0);
        let downstream = wt.get("downstream").and_then(|v| v.as_f64()).unwrap_or(40.0);
        let vertical = wt.get("vertical").and_then(|v| v.as_f64()).unwrap_or(25.0);
        let lateral = wt.get("lateral").and_then(|v| v.as_f64()).unwrap_or(25.0);
        WindTunnelConfig {
            upstream,
            downstream,
            vertical,
            lateral,
            velocity_m_s: Some(velocity),
            ..WindTunnelConfig::default()
        }
    });

    let flow_type = determine_flow_type(flow_type_str);

    let case_config = CaseConfig {
        flow_type,
        velocity_m_s: velocity,
        turbulence_model: turb_model.to_string(),
        wind_tunnel: wind_tunnel.clone(),
        reference_length_m: None,
    };

    let settings = SettingsManager::load();
    let write_format = &settings.settings.openfoam_format;

    println!("\n=== AeroFlow Agent — Pipeline Execution ===\n");
    println!("  Case:    {}", case_name);
    println!("  Path:    {:?}", case_path);
    println!("  Solver:  {}", solver);
    println!("  Turb:    {}", turb_model);
    println!("  Trials:  {}", if trials > 0 { format!("{}", trials) } else { "single".into() });
    println!("  Format:  {} (binary)", write_format.label());
    if wind_tunnel.is_some() {
        println!("  Domain:  Digital Wind Tunnel");
    }
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
    match orchestrator.run_pipeline(case_id, case_path, solver, None, None, Some(&case_config)) {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn determine_flow_type_defaults_to_external() {
        assert!(matches!(determine_flow_type("External"), FlowType::External));
    }

    #[test]
    fn determine_flow_type_wind_tunnel_long() {
        assert!(matches!(determine_flow_type("External (Digital Wind Tunnel)"), FlowType::ExternalWindTunnel));
    }

    #[test]
    fn determine_flow_type_wind_tunnel_short() {
        assert!(matches!(determine_flow_type("external_wind_tunnel"), FlowType::ExternalWindTunnel));
    }

    #[test]
    fn determine_flow_type_unknown_falls_back() {
        assert!(matches!(determine_flow_type("Internal"), FlowType::External));
    }
}
