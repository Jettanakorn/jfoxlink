use std::path::Path;
use std::process::Command;
use tracing::{info, warn};

/// Generate visualization images from OpenFOAM simulation results.
///
/// Steps:
/// 1. Run `foamToVTK` to export results to VTK format
/// 2. Run Python visualization script to generate PNG images
/// 3. Returns list of generated image files
pub fn generate_visualization(case_path: &Path, report_dir: &Path) -> Result<Vec<String>, anyhow::Error> {
    let images_dir = report_dir.join("images");
    std::fs::create_dir_all(&images_dir)?;

    let mut generated = Vec::new();

    // Step 1: Run foamToVTK
    info!("  Exporting VTK...");
    let vtk_output = Command::new("foamToVTK")
        .args(["-case", &case_path.to_string_lossy(), "-latestTime", "-fields", "(p U)"])
        .output();

    match vtk_output {
        Ok(out) if out.status.success() => {
            info!("  ✓ VTK export complete");
        }
        Ok(out) => {
            let stderr = String::from_utf8_lossy(&out.stderr);
            warn!("foamToVTK exit code {:?}: {}", out.status.code(), stderr);
        }
        Err(e) => {
            warn!("foamToVTK not available: {}", e);
        }
    }

    // Step 2: Run Python visualization script
    let script_path = find_viz_script();
    if let Some(script) = script_path {
        info!("  Generating visualizations...");
        let py_output = Command::new("python3")
            .args([
                &script,
                "--case-path",
                &case_path.to_string_lossy(),
                "--output-dir",
                &images_dir.to_string_lossy(),
            ])
            .output();

        match py_output {
            Ok(out) => {
                let stdout = String::from_utf8_lossy(&out.stdout);
                let stderr = String::from_utf8_lossy(&out.stderr);
                if !stdout.is_empty() {
                    info!("  Viz output: {}", stdout.trim());
                }
                if out.status.success() {
                    info!("  ✓ Visualization complete");
                } else {
                    warn!("Python viz script failed: {}", stderr);
                }
            }
            Err(e) => {
                warn!("Failed to run Python viz script: {}", e);
            }
        }
    } else {
        warn!("Visualization script not found");
    }

    // Collect generated image files
    if let Ok(entries) = std::fs::read_dir(&images_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if let Some(ext) = path.extension()
                && ext == "png"
                    && let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                        generated.push(format!("images/{}", name));
                    }
        }
    }

    info!("  Generated {} visualization images", generated.len());
    Ok(generated)
}

fn find_viz_script() -> Option<String> {
    // Check several possible locations for the Python script
    let candidates = [
        "scripts/viz/generate_viz.py",
        "/usr/local/share/aeroflow/scripts/viz/generate_viz.py",
        "../scripts/viz/generate_viz.py",
    ];
    for path in &candidates {
        if Path::new(path).exists() {
            return Some(path.to_string());
        }
    }
    None
}
