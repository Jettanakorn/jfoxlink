use aeroflow_api::WebApi;
use std::path::PathBuf;
use tracing::info;

pub async fn execute(port: u16) -> anyhow::Result<()> {
    info!("Starting AeroFlow web workspace on port {}", port);

    unsafe { std::env::set_var("AEROFLOW_API_PORT", port.to_string()); }

    // Resolve frontend directory relative to binary or CWD
    let frontend_path = find_frontend_dir();

    let api = WebApi::with_frontend(frontend_path);
    api.start().await?;

    Ok(())
}

fn find_frontend_dir() -> PathBuf {
    let candidates = [
        "frontend",
        "../frontend",
        "/usr/local/share/aeroflow/frontend",
        "/build/frontend",
        "/data/frontend",
    ];

    for path in &candidates {
        let p = PathBuf::from(path);
        if p.join("index.html").exists() {
            info!("Found frontend at {:?}", p);
            return p;
        }
    }

    // Default fallback
    let fallback = PathBuf::from("frontend");
    info!("Frontend directory not found, using {:?}", fallback);
    fallback
}
