use crate::OpenFOAMFormat;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Centralized AeroFlow settings — aggregates all configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AeroflowSettings {
    // Workspace
    pub workspace_dir: String,
    pub data_dir: String,

    // Database
    pub database_url: String,

    // Docker / Container
    pub docker_socket: String,
    pub openfoam_image: String,
    pub max_concurrent_cases: u32,
    pub default_cores: u32,
    pub default_memory_gb: u32,

    // OpenFOAM output format
    pub openfoam_format: OpenFOAMFormat,

    // Mesh quality
    pub mesh_quality: MeshQualityThresholds,

    // Scoring
    pub scoring: ScoringWeights,

    // Solver defaults
    pub solver: SolverDefaults,

    // User management
    pub session_ttl_hours: u32,
    pub allow_registration: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SolverDefaults {
    pub min_iterations: u32,
    pub max_iterations: u32,
    pub write_interval: u32,
    pub purge_write: u32,
    pub relaxation_factor_p: f64,
    pub relaxation_factor_u: f64,
    pub convergence_residual: f64,
}

impl Default for SolverDefaults {
    fn default() -> Self {
        Self {
            min_iterations: 100,
            max_iterations: 3000,
            write_interval: 500,
            purge_write: 3,
            relaxation_factor_p: 0.7,
            relaxation_factor_u: 0.3,
            convergence_residual: 1e-6,
        }
    }
}

impl Default for AeroflowSettings {
    fn default() -> Self {
        Self {
            workspace_dir: "/data".to_string(),
            data_dir: "/data".to_string(),
            database_url: "postgres://aeroflow@db:5432/aeroflow".to_string(),
            docker_socket: "/var/run/docker.sock".to_string(),
            openfoam_image: "microfluidica/openfoam:com".to_string(),
            max_concurrent_cases: 4,
            default_cores: 4,
            default_memory_gb: 8,
            openfoam_format: OpenFOAMFormat::Binary,
            mesh_quality: MeshQualityThresholds::default(),
            scoring: ScoringWeights::default(),
            solver: SolverDefaults::default(),
            session_ttl_hours: 24,
            allow_registration: false,
        }
    }
}

/// Settings manager — loads/saves TOML config, merges env vars
#[derive(Debug)]
pub struct SettingsManager {
    pub settings: AeroflowSettings,
    pub config_path: Option<PathBuf>,
}

impl SettingsManager {
    /// Load settings from default locations
    pub fn load() -> Self {
        let mut mgr = Self {
            settings: AeroflowSettings::default(),
            config_path: None,
        };
        mgr.merge_env();
        mgr
    }

    /// Load from a specific TOML file
    pub fn from_file(path: &std::path::Path) -> Result<Self, anyhow::Error> {
        let content = std::fs::read_to_string(path)?;
        let settings: AeroflowSettings = toml::from_str(&content)?;
        let mut mgr = Self {
            settings,
            config_path: Some(path.to_path_buf()),
        };
        mgr.merge_env();
        Ok(mgr)
    }

    /// Save settings to a TOML file
    pub fn save_to_file(&self, path: &std::path::Path) -> Result<(), anyhow::Error> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content = toml::to_string_pretty(&self.settings)?;
        std::fs::write(path, content)?;
        Ok(())
    }

    /// Save to the current config path
    pub fn save(&self) -> Result<(), anyhow::Error> {
        if let Some(path) = &self.config_path {
            self.save_to_file(path)
        } else {
            anyhow::bail!("No config path set. Use save_to_file() instead.");
        }
    }

    /// Merge environment variables (uppercase, underscore, AEROFLOW_ prefix)
    fn merge_env(&mut self) {
        if let Ok(v) = std::env::var("AEROFLOW_WORKSPACE_DIR") {
            self.settings.workspace_dir = v;
            self.settings.data_dir = self.settings.workspace_dir.clone();
        }
        if let Ok(v) = std::env::var("DATABASE_URL") {
            self.settings.database_url = v;
        }
        if let Ok(v) = std::env::var("AEROFLOW_DOCKER_SOCKET") {
            self.settings.docker_socket = v;
        }
        if let Ok(v) = std::env::var("AEROFLOW_OPENFOAM_IMAGE") {
            self.settings.openfoam_image = v;
        }
        if let Ok(v) = std::env::var("AEROFLOW_MAX_CONCURRENT") {
            if let Ok(n) = v.parse() {
                self.settings.max_concurrent_cases = n;
            }
        }
        if let Ok(v) = std::env::var("AEROFLOW_DEFAULT_CORES") {
            if let Ok(n) = v.parse() {
                self.settings.default_cores = n;
            }
        }
        if let Ok(v) = std::env::var("AEROFLOW_DEFAULT_MEMORY_GB") {
            if let Ok(n) = v.parse() {
                self.settings.default_memory_gb = n;
            }
        }
    }

    /// Get the workspace layout from the workspace dir
    pub fn workspace_layout(&self) -> crate::WorkspaceLayout {
        crate::WorkspaceLayout::new(&self.settings.workspace_dir)
    }

    /// Initialize workspace directory structure
    pub fn init_workspace(&self) -> std::io::Result<()> {
        let layout = self.workspace_layout();
        layout.create_all()
    }
}

/// Legacy config — preserves backward compat, delegates to AeroflowSettings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AeroFlowConfig {
    pub data_dir: String,
    pub database_url: String,
    pub docker_socket: String,
    pub openfoam_image: String,
    pub max_concurrent_cases: u32,
    pub default_cores: u32,
    pub default_memory_gb: u32,
}

impl Default for AeroFlowConfig {
    fn default() -> Self {
        Self {
            data_dir: "/data".to_string(),
            database_url: "postgres://aeroflow@db:5432/aeroflow".to_string(),
            docker_socket: "/var/run/docker.sock".to_string(),
            openfoam_image: "microfluidica/openfoam:com".to_string(),
            max_concurrent_cases: 4,
            default_cores: 4,
            default_memory_gb: 8,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeshQualityThresholds {
    pub max_non_orthogonality_warn: f64,
    pub max_non_orthogonality_fail: f64,
    pub max_skewness_warn: f64,
    pub max_skewness_fail: f64,
    pub min_determinant_warn: f64,
    pub min_determinant_fail: f64,
    pub max_aspect_ratio_warn: f64,
    pub max_aspect_ratio_fail: f64,
    pub min_volume_fail: f64,
}

impl Default for MeshQualityThresholds {
    fn default() -> Self {
        Self {
            max_non_orthogonality_warn: 60.0,
            max_non_orthogonality_fail: 70.0,
            max_skewness_warn: 2.0,
            max_skewness_fail: 4.0,
            min_determinant_warn: 0.01,
            min_determinant_fail: 0.001,
            max_aspect_ratio_warn: 500.0,
            max_aspect_ratio_fail: 1000.0,
            min_volume_fail: 1e-13,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoringWeights {
    pub w_cl: f64,
    pub w_cd: f64,
    pub w_yplus: f64,
    pub w_residual: f64,
}

impl Default for ScoringWeights {
    fn default() -> Self {
        Self {
            w_cl: 0.35,
            w_cd: 0.35,
            w_yplus: 0.15,
            w_residual: 0.15,
        }
    }
}
