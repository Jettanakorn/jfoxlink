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
    // Built Vite output takes priority
    let candidates = [
        "frontend/dist",
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn find_frontend_dir_does_not_panic() {
        let path = find_frontend_dir();
        assert!(!path.as_os_str().is_empty());
    }

    #[test]
    fn find_frontend_dir_finds_frontend_in_mock_dir() {
        let tmp = std::env::temp_dir().join(format!("aeroflow_test_web_{}", std::process::id()));
        let frontend_dir = tmp.join("frontend").join("dist");
        std::fs::create_dir_all(&frontend_dir).expect("create mock frontend dir");
        std::fs::write(frontend_dir.join("index.html"), "<html></html>").expect("write mock index.html");

        let orig = std::env::current_dir().expect("get current dir");
        std::env::set_current_dir(&tmp).expect("change to temp dir");

        let found = find_frontend_dir();

        std::env::set_current_dir(&orig).expect("restore original dir");
        let _ = std::fs::remove_dir_all(&tmp);

        assert_eq!(found, PathBuf::from("frontend/dist"));
    }
}
