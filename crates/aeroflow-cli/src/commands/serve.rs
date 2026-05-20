use aeroflow_core::SettingsManager;
use aeroflow_events::FileWatcher;
use aeroflow_skills::SkillsDb;
use tokio::signal;
use tracing::info;

pub async fn execute(port: u16) -> anyhow::Result<()> {
    let settings = SettingsManager::load();
    let workspace_dir: std::path::PathBuf = settings.settings.workspace_dir.clone().into();
    let import_dir = workspace_dir.join("import");

    println!("\n=== AeroFlow Agent — Server Mode ===\n");
    println!("  API:      http://0.0.0.0:{}", port);
    println!("  Watcher:  {:?}", import_dir);
    println!("  Workspace:{:?}", workspace_dir);
    println!();
    println!("  Endpoints:");
    println!("    GET  /api/health");
    println!("    POST /api/auth/login");
    println!("    POST /api/auth/register");
    println!("    GET  /api/cases");
    println!("    POST /api/cases");
    println!("    GET  /api/cases/{{id}}");
    println!("    POST /api/cases/{{id}}/run");
    println!("    GET  /api/users");
    println!("    GET  /api/events");
    println!();
    println!("  Press Ctrl+C to stop...");
    println!();

    // Connect DB
    let db = SkillsDb::connect(&settings.settings.database_url).await?;

    // Start file watcher for auto-import
    println!("  Starting file watcher on {:?}...", import_dir);
    let watcher = FileWatcher::new(import_dir, workspace_dir.clone())
        .with_db(db.clone());
    let _shutdown_tx = watcher.start().await;
    println!("  ✓ File watcher started");

    // Start REST API
    println!("  Starting API server on port {}...", port);
    unsafe { std::env::set_var("AEROFLOW_API_PORT", port.to_string()); }

    // Run API and watcher concurrently, exit gracefully on Ctrl+C
    let api_handle = tokio::spawn(async move {
        let api = aeroflow_api::WebApi::new();
        if let Err(e) = api.start().await {
            eprintln!("API server error: {}", e);
        }
    });

    // Wait for Ctrl+C
    signal::ctrl_c().await?;
    println!("\n  Shutting down gracefully...");
    info!("Server shutting down");

    // The file watcher will stop via its shutdown_rx when dropped
    api_handle.abort();

    println!("  ✓ Server stopped");
    Ok(())
}
