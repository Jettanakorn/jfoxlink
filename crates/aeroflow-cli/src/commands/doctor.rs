use aeroflow_core::{HealthCategory, HealthStatus};
use tracing::info;

pub async fn execute(
    category_str: Option<&str>,
    fix: bool,
    json: bool,
    watch: bool,
) -> anyhow::Result<()> {
    info!("Running health check (fix={}, json={}, watch={})", fix, json, watch);

    let category_filter: Option<HealthCategory> = category_str.and_then(|c| match c {
        "docker" => Some(HealthCategory::Docker),
        "database" => Some(HealthCategory::Database),
        "openfoam" => Some(HealthCategory::OpenFOAM),
        "filesystem" => Some(HealthCategory::FileSystem),
        "system" => Some(HealthCategory::System),
        "skills" => Some(HealthCategory::Skills),
        "postproc" => Some(HealthCategory::PostProc),
        _ => {
            println!("Unknown category: {}. Available: docker, database, openfoam, filesystem, system, skills, postproc", c);
            None
        }
    });

    if category_str.is_some() && category_filter.is_none() {
        return Ok(());
    }

    println!("\n═══════════════════════════════════════════════");
    println!("  AeroFlow Agent — System Health Report");
    println!("  Developer: Jettanakorn Pengsiri by JFOX Aircraft Co., Ltd.");
    println!("═══════════════════════════════════════════════\n");

    if watch {
        println!("  Continuous monitoring mode (Ctrl+C to stop)\n");
    }

    let results = aeroflow_doctor::run_checks(category_filter).await;

    for result in &results {
        let icon = match result.status {
            HealthStatus::Pass => "✓",
            HealthStatus::Warn => "⚠",
            HealthStatus::Fail => "✗",
            HealthStatus::Skip => "–",
            HealthStatus::Info => "ℹ",
        };
        println!("  {} {} — {}", icon, result.category_label(), result.message);
    }

    let pass = results.iter().filter(|r| r.status == HealthStatus::Pass).count();
    let warn = results.iter().filter(|r| r.status == HealthStatus::Warn).count();
    let fail = results.iter().filter(|r| r.status == HealthStatus::Fail).count();
    let skip = results.iter().filter(|r| r.status == HealthStatus::Skip).count();

    println!("\n  Results: {} PASS, {} WARN, {} FAIL, {} SKIP", pass, warn, fail, skip);

    if fix {
        println!("\n  Auto-fix mode enabled. Attempting fixes...\n");
        for result in &results {
            if result.status == HealthStatus::Fail || result.status == HealthStatus::Warn {
                println!("    Fixing {}... (not yet implemented in P0)", result.category_label());
            }
        }
    }

    if json {
        let json_output = serde_json::json!({
            "results": &results,
            "summary": {
                "pass": pass,
                "warn": warn,
                "fail": fail,
                "skip": skip,
            }
        });
        println!("\n{}", serde_json::to_string_pretty(&json_output)?);
    }

    if watch {
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(30)).await;
        }
    }

    Ok(())
}
