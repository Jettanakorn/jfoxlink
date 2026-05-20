use serde::{Deserialize, Serialize};
use uuid::Uuid;
use chrono::{DateTime, Utc};

pub type CaseId = Uuid;
pub type SkillId = Uuid;
pub type GeometryId = Uuid;
pub type UserId = Uuid;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum UserRole {
    Admin,
    Engineer,
    Viewer,
}

impl UserRole {
    pub fn label(&self) -> &'static str {
        match self {
            UserRole::Admin => "admin",
            UserRole::Engineer => "engineer",
            UserRole::Viewer => "viewer",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "admin" => Some(UserRole::Admin),
            "engineer" => Some(UserRole::Engineer),
            "viewer" => Some(UserRole::Viewer),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: UserId,
    pub name: String,
    pub email: String,
    pub role: UserRole,
    pub password_hash: Option<String>,
    pub active: bool,
    pub last_login: Option<DateTime<Utc>>,
    pub quota_max_concurrent: i32,
    pub quota_max_cores: i32,
    pub quota_max_memory_gb: i32,
    pub preferences: serde_json::Value,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateUserRequest {
    pub name: String,
    pub email: String,
    pub password: String,
    pub role: UserRole,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateUserRequest {
    pub name: Option<String>,
    pub email: Option<String>,
    pub password: Option<String>,
    pub role: Option<UserRole>,
    pub active: Option<bool>,
    pub quota_max_concurrent: Option<i32>,
    pub quota_max_cores: Option<i32>,
    pub quota_max_memory_gb: Option<i32>,
    pub preferences: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub token: String,
    pub user_id: UserId,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum OpenFOAMFormat {
    Ascii,
    Binary,
}

impl OpenFOAMFormat {
    pub fn label(&self) -> &'static str {
        match self {
            OpenFOAMFormat::Ascii => "ascii",
            OpenFOAMFormat::Binary => "binary",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceLayout {
    pub root: String,
    pub cases: String,
    pub import: String,
    pub reports: String,
    pub skills: String,
    pub settings: String,
    pub temp: String,
    pub logs: String,
}

impl WorkspaceLayout {
    pub fn new(root: &str) -> Self {
        Self {
            root: root.to_string(),
            cases: format!("{}/cases", root),
            import: format!("{}/import", root),
            reports: format!("{}/reports", root),
            skills: format!("{}/skills", root),
            settings: format!("{}/settings", root),
            temp: format!("{}/temp", root),
            logs: format!("{}/logs", root),
        }
    }

    pub fn create_all(&self) -> std::io::Result<()> {
        std::fs::create_dir_all(&self.cases)?;
        std::fs::create_dir_all(&self.import)?;
        std::fs::create_dir_all(&self.reports)?;
        std::fs::create_dir_all(&self.skills)?;
        std::fs::create_dir_all(&self.settings)?;
        std::fs::create_dir_all(&self.temp)?;
        std::fs::create_dir_all(&self.logs)?;
        Ok(())
    }

    pub fn case_dir(&self, case_name: &str) -> String {
        format!("{}/{}", self.cases, case_name)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub enum Stage {
    Created,
    Imported,
    SurfacePrep,
    Meshing,
    MeshQuality,
    Setup,
    Solving,
    Converged,
    Diverged,
    PostProcessing,
    Report,
    Complete,
    Failed,
    Cancelled,
}

impl Stage {
    pub fn label(&self) -> &'static str {
        match self {
            Stage::Created => "CREATED",
            Stage::Imported => "IMPORTED",
            Stage::SurfacePrep => "SURFACE",
            Stage::Meshing => "MESHING",
            Stage::MeshQuality => "QUALITY",
            Stage::Setup => "SETUP",
            Stage::Solving => "SOLVING",
            Stage::Converged => "CONVERGED",
            Stage::Diverged => "DIVERGED",
            Stage::PostProcessing => "POST-PROC",
            Stage::Report => "REPORT",
            Stage::Complete => "COMPLETE",
            Stage::Failed => "FAILED",
            Stage::Cancelled => "CANCELLED",
        }
    }

    pub fn is_terminal(&self) -> bool {
        matches!(self, Stage::Complete | Stage::Failed | Stage::Cancelled)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum HealthStatus {
    Pass,
    Warn,
    Fail,
    Skip,
    Info,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum HealthCategory {
    Docker,
    Database,
    OpenFOAM,
    FileSystem,
    System,
    Skills,
    PostProc,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum FlowType {
    External,
    Internal,
    ExternalWindTunnel,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Compressibility {
    Incompressible,
    Subsonic,
    Transonic,
    Supersonic,
    Hypersonic,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum AccuracyLevel {
    Draft,
    Standard,
    High,
    Aerospace,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Priority {
    Speed,
    Balanced,
    Accuracy,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaseMeta {
    pub id: CaseId,
    pub name: String,
    pub stage: Stage,
    pub user_id: Option<UserId>,
    pub workspace_root: Option<String>,
    pub flow_type: Option<FlowType>,
    pub compressibility: Option<Compressibility>,
    pub accuracy: Option<AccuracyLevel>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeshQualityMetrics {
    pub max_non_orthogonality: f64,
    pub avg_non_orthogonality: f64,
    pub max_skewness: f64,
    pub min_determinant: f64,
    pub max_aspect_ratio: f64,
    pub min_volume: f64,
    pub n_cells: u64,
    pub n_failed_cells: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForceCoefficients {
    pub cl: f64,
    pub cd: f64,
    pub cm: f64,
    pub cl_std: f64,
    pub cd_std: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SolverStats {
    pub iterations: u64,
    pub wall_time_s: f64,
    pub residual_p: f64,
    pub residual_u: f64,
    pub converged: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntakeConfig {
    pub geometry_description: String,
    pub geometry_file: Option<String>,
    pub case_class: Option<String>,
    pub workspace_root: Option<String>,
    pub user_id: Option<UserId>,
    pub altitude_m: f64,
    pub mach_number: f64,
    pub reynolds_number: f64,
    pub alpha_sweep_deg: Vec<f64>,
    pub freestream_turbulence_intensity: f64,
    pub target_cl: Option<f64>,
    pub target_cd_max: Option<f64>,
    pub target_yplus_max: f64,
    pub convergence_residual: f64,
    pub max_agent_iterations: u32,
    pub human_in_loop: bool,
    pub priority: Priority,
    pub hpc_cores: u32,
    pub time_budget_hours: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceUsage {
    pub cpu_percent: f64,
    pub memory_used_bytes: u64,
    pub memory_limit_bytes: u64,
    pub disk_read_bytes: u64,
    pub disk_write_bytes: u64,
    pub network_rx_bytes: u64,
    pub network_tx_bytes: u64,
    pub timestamp: DateTime<Utc>,
}
