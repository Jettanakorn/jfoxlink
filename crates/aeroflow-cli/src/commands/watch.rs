use aeroflow_core::SettingsManager;
use aeroflow_events::FileWatcher;
use aeroflow_skills::SkillsDb;
use std::path::Path;
use tokio::signal;
use tracing::info;

pub async fn execute(path: &Path) -> anyhow::Result<()> {
    let watch_dir = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()?.join(path)
    };

    let settings = SettingsManager::load();
    let workspace_dir: std::path::PathBuf = settings.settings.workspace_dir.clone().into();

    println!("\n=== AeroFlow Agent — File Watcher ===\n");
    println!("  Watching:  {:?}", watch_dir);
    println!("  Workspace: {:?}", workspace_dir);
    println!();
    println!("  Auto-imports new .stl files into the AeroFlow pipeline.");
    println!("  Press Ctrl+C to stop.");

    let db = match SkillsDb::connect(&settings.settings.database_url).await {
        Ok(d) => {
            info!("Database connected for auto-import");
            Some(d)
        }
        Err(e) => {
            println!("  ⚠ No database — running in dry-run mode ({})", e);
            None
        }
    };

    let watcher = FileWatcher::new(watch_dir, workspace_dir);
    let watcher = if let Some(ref db) = db {
        watcher.with_db(db.clone())
    } else {
        watcher
    };

    let _shutdown_tx = watcher.start().await;

    println!("  ✓ Watcher started");
    println!();

    // Wait for Ctrl+C
    signal::ctrl_c().await?;

    println!("\n  Shutting down watcher...");
    info!("File watcher terminated by user");

    Ok(())
}
