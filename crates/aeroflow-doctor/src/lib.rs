pub mod checks;

use aeroflow_core::{HealthCategory, HealthStatus, SettingsManager};
use serde::Serialize;
use std::path::Path;

#[derive(Debug, Clone, Serialize)]
pub struct HealthCheckResult {
    pub name: String,
    pub category: HealthCategory,
    pub status: HealthStatus,
    pub message: String,
    pub detail: Option<String>,
    pub fix_message: Option<String>,
    pub has_fix: bool,
}

impl HealthCheckResult {
    pub fn category_label(&self) -> &'static str {
        match self.category {
            HealthCategory::Docker => "Docker",
            HealthCategory::Database => "Database",
            HealthCategory::OpenFOAM => "OpenFOAM",
            HealthCategory::FileSystem => "Filesystem",
            HealthCategory::System => "System",
            HealthCategory::Skills => "Skills",
            HealthCategory::PostProc => "VTK/Post",
        }
    }
}

fn pass(name: &str, category: HealthCategory, msg: String) -> HealthCheckResult {
    HealthCheckResult {
        name: name.to_string(),
        category,
        status: HealthStatus::Pass,
        message: msg,
        detail: None,
        fix_message: None,
        has_fix: false,
    }
}

fn warn(name: &str, category: HealthCategory, msg: String, fix: Option<String>) -> HealthCheckResult {
    let has = fix.is_some();
    HealthCheckResult {
        name: name.to_string(),
        category,
        status: HealthStatus::Warn,
        message: msg,
        detail: None,
        fix_message: fix,
        has_fix: has,
    }
}

fn fail(name: &str, category: HealthCategory, msg: String, fix: Option<String>) -> HealthCheckResult {
    let has = fix.is_some();
    HealthCheckResult {
        name: name.to_string(),
        category,
        status: HealthStatus::Fail,
        message: msg,
        detail: None,
        fix_message: fix,
        has_fix: has,
    }
}

pub async fn run_checks(category_filter: Option<HealthCategory>) -> Vec<HealthCheckResult> {
    let settings = SettingsManager::load();
    let s = &settings.settings;
    let mut results = Vec::new();

    let should_run = |cat: HealthCategory| -> bool {
        category_filter.as_ref().is_none_or(|f| *f == cat)
    };

    // ── Docker checks ──────────────────────────────────────────
    if should_run(HealthCategory::Docker) {
        match bollard::Docker::connect_with_local_defaults() {
            Ok(docker) => {
                let start = std::time::Instant::now();
                match docker.ping().await {
                    Ok(_) => {
                        let elapsed = start.elapsed().as_millis();
                        results.push(pass(
                            "docker-daemon",
                            HealthCategory::Docker,
                            format!("Daemon reachable (bollard ping {} ms)", elapsed),
                        ));
                        if let Ok(v) = docker.version().await {
                            let ver = v.version.unwrap_or_default();
                            let api = v.api_version.unwrap_or_default();
                            results.push(pass(
                                "docker-version",
                                HealthCategory::Docker,
                                format!("Engine {} (API {})", ver, api),
                            ));
                        }
                    }
                    Err(e) => results.push(fail(
                        "docker-daemon",
                        HealthCategory::Docker,
                        format!("Docker ping failed: {}", e),
                        Some("Check Docker Desktop is running: `docker ps`".into()),
                    )),
                }
            }
            Err(e) => results.push(fail(
                "docker-daemon",
                HealthCategory::Docker,
                format!("Cannot connect to Docker socket: {}", e),
                Some("Ensure Docker Desktop is running and the socket is accessible".into()),
            )),
        }
    }

    // ── Database checks ────────────────────────────────────────
    if should_run(HealthCategory::Database) {
        let start = std::time::Instant::now();
        match sqlx::postgres::PgPoolOptions::new()
            .max_connections(1)
            .connect(&s.database_url)
            .await
        {
            Ok(pool) => {
                let elapsed = start.elapsed().as_millis();
                let version: Option<String> = sqlx::query_scalar(
                    "SELECT version FROM schema_migrations ORDER BY version DESC LIMIT 1"
                )
                .fetch_optional(&pool)
                .await
                .ok()
                .flatten();

                let version_str = version.unwrap_or_else(|| "unknown".to_string());
                results.push(pass(
                    "db-connect",
                    HealthCategory::Database,
                    format!("PostgreSQL reachable ({} ms, schema v{})", elapsed, version_str),
                ));

                if let Ok(count) = sqlx::query_scalar::<_, i64>(
                    "SELECT COUNT(*) FROM information_schema.tables WHERE table_schema = 'public'"
                )
                .fetch_one(&pool)
                .await
                {
                    results.push(pass(
                        "db-tables",
                        HealthCategory::Database,
                        format!("{} public tables", count),
                    ));
                }

                pool.close().await;
            }
            Err(e) => results.push(fail(
                "db-connect",
                HealthCategory::Database,
                format!("Cannot connect to PostgreSQL: {}", e),
                Some(format!(
                    "Ensure the database is running: `docker compose up -d db`. URL: {}",
                    s.database_url
                )),
            )),
        }
    }

    // ── OpenFOAM checks ───────────────────────────────────────
    if should_run(HealthCategory::OpenFOAM) {
        let wm = std::env::var("WM_PROJECT_VERSION");
        let foam = std::env::var("FOAM_INST_DIR");

        match (&wm, &foam) {
            (Ok(ver), Ok(dir)) => results.push(pass(
                "openfoam-env",
                HealthCategory::OpenFOAM,
                format!("OpenFOAM {} sourced ({})", ver, dir),
            )),
            (Ok(ver), _) => results.push(pass(
                "openfoam-env",
                HealthCategory::OpenFOAM,
                format!("OpenFOAM {} version set", ver),
            )),
            (_, _) => results.push(fail(
                "openfoam-env",
                HealthCategory::OpenFOAM,
                "WM_PROJECT_VERSION not set — OpenFOAM environment not sourced".into(),
                Some("Run `source /opt/openfoam*/etc/bashrc` or use the Docker container".into()),
            )),
        }

        for (name, exe) in [("simpleFoam", "simpleFoam"), ("blockMesh", "blockMesh"), ("checkMesh", "checkMesh")] {
            if which::which(exe).is_ok() {
                results.push(pass(
                    &format!("openfoam-{}", name),
                    HealthCategory::OpenFOAM,
                    format!("{} found on PATH", exe),
                ));
            } else {
                results.push(warn(
                    &format!("openfoam-{}", name),
                    HealthCategory::OpenFOAM,
                    format!("{} not on PATH", exe),
                    None,
                ));
            }
        }
    }

    // ── Filesystem checks ──────────────────────────────────────
    if should_run(HealthCategory::FileSystem) {
        let data_path = Path::new(&s.workspace_dir);
        match fs2::available_space(data_path) {
            Ok(bytes) => {
                let free_gb = bytes as f64 / 1_073_741_824.0;

                if free_gb < 1.0 {
                    results.push(fail(
                        "disk-space",
                        HealthCategory::FileSystem,
                        format!("Critical: {:.1} GB free", free_gb),
                        Some("Delete old cases, prune Docker: `docker system prune -af`".into()),
                    ));
                } else if free_gb < 5.0 {
                    results.push(warn(
                        "disk-space",
                        HealthCategory::FileSystem,
                        format!("Low: {:.1} GB free", free_gb),
                        Some("Consider cleaning old data or expanding storage".into()),
                    ));
                } else if free_gb < 50.0 {
                    results.push(pass(
                        "disk-space",
                        HealthCategory::FileSystem,
                        format!("Adequate: {:.1} GB free", free_gb),
                    ));
                } else {
                    results.push(pass(
                        "disk-space",
                        HealthCategory::FileSystem,
                        format!("Plenty: {:.1} GB free", free_gb),
                    ));
                }

                let layout = settings.workspace_layout();
                for (label, dir) in [
                    ("cases", &layout.cases),
                    ("import", &layout.import),
                    ("reports", &layout.reports),
                    ("skills", &layout.skills),
                    ("settings", &layout.settings),
                    ("temp", &layout.temp),
                    ("logs", &layout.logs),
                ] {
                    if Path::new(dir).exists() {
                        results.push(pass(
                            &format!("workspace-{}", label),
                            HealthCategory::FileSystem,
                            format!("{} directory exists", dir),
                        ));
                    } else {
                        results.push(warn(
                            &format!("workspace-{}", label),
                            HealthCategory::FileSystem,
                            format!("{} directory missing", dir),
                            Some("Run `aeroflow settings init <path>` to create workspace".into()),
                        ));
                    }
                }
            }
            Err(e) => results.push(fail(
                "disk-space",
                HealthCategory::FileSystem,
                format!("Cannot check disk space: {}", e),
                None,
            )),
        }
    }

    // ── System checks ──────────────────────────────────────────
    if should_run(HealthCategory::System) {
        let mut sys = sysinfo::System::new();
        sys.refresh_cpu_specifics(sysinfo::CpuRefreshKind::everything());
        sys.refresh_memory_specifics(sysinfo::MemoryRefreshKind::everything());

        let cpu_count = sys.cpus().len();
        let total_mem = sys.total_memory();
        let used_mem = sys.used_memory();
        let total_gb = total_mem as f64 / 1_073_741_824.0;
        let used_gb = used_mem as f64 / 1_073_741_824.0;
        let mem_pct = if total_mem > 0 { ((used_mem as f64 / total_mem as f64) * 100.0) as u32 } else { 0 };

        if cpu_count >= 2 {
            results.push(pass(
                "cpu-count",
                HealthCategory::System,
                format!("{} CPU cores (min 2 ✓)", cpu_count),
            ));
        } else {
            results.push(warn(
                "cpu-count",
                HealthCategory::System,
                format!("{} CPU core(s) — minimum 2 recommended", cpu_count),
                None,
            ));
        }

        results.push(pass(
            "memory",
            HealthCategory::System,
            format!("{:.1} GB total, {:.1} GB used ({}%)", total_gb, used_gb, mem_pct),
        ));

        results.push(pass(
            "os-info",
            HealthCategory::System,
            format!("{} {}", std::env::consts::OS, std::env::consts::ARCH),
        ));

        if let Ok(host) = std::env::var("HOSTNAME").or_else(|_| std::env::var("COMPUTERNAME")) {
            results.push(pass("hostname", HealthCategory::System, host));
        }
    }

    // ── Skills checks ──────────────────────────────────────────
    if should_run(HealthCategory::Skills) {
        match sqlx::postgres::PgPoolOptions::new()
            .max_connections(1)
            .connect(&s.database_url)
            .await
        {
            Ok(pool) => {
                match sqlx::query_scalar::<_, i64>(
                    "SELECT COUNT(*) FROM skills WHERE active = true"
                )
                .fetch_one(&pool)
                .await
                {
                    Ok(n) => results.push(pass(
                        "skills-count",
                        HealthCategory::Skills,
                        format!("{} active skills in database", n),
                    )),
                    Err(e) => results.push(warn(
                        "skills-count",
                        HealthCategory::Skills,
                        format!("Cannot query skills: {}", e),
                        Some("Ensure migration 001 has been applied".into()),
                    )),
                }

                if let Ok(n) = sqlx::query_scalar::<_, i64>(
                    "SELECT COUNT(*) FROM parameter_trials"
                )
                .fetch_one(&pool)
                .await
                {
                    results.push(pass(
                        "trials-count",
                        HealthCategory::Skills,
                        format!("{} parameter trials stored", n),
                    ));
                }

                pool.close().await;
            }
            Err(_) => {
                results.push(warn(
                    "skills-db",
                    HealthCategory::Skills,
                    "Database not reachable — skills check skipped".into(),
                    None,
                ));
            }
        }
    }

    // ── VTK/Post checks ────────────────────────────────────────
    if should_run(HealthCategory::PostProc) {
        let tools = [
            ("pvpython", "ParaView Python"),
            ("vtkpython", "VTK Python"),
            ("paraview", "ParaView GUI"),
        ];

        for (exe, label) in &tools {
            if which::which(exe).is_ok() {
                results.push(pass(
                    &format!("post-{}", exe.replace('-', "_")),
                    HealthCategory::PostProc,
                    format!("{} available on PATH", label),
                ));
            }
        }

        let has_vtk_files = globwalk::glob("**/*.vtu").map(|entries| entries.count()).unwrap_or(0) > 0;
        if has_vtk_files {
            results.push(pass(
                "vtkio",
                HealthCategory::PostProc,
                "VTU files found, vtkio read capable".into(),
            ));
        } else {
            results.push(pass(
                "vtkio",
                HealthCategory::PostProc,
                "vtkio crate ready (no VTU files found yet)".into(),
            ));
        }
    }

    results
}
