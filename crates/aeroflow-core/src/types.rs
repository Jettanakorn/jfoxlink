use serde::{Deserialize, Serialize};
use std::str::FromStr;
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

}

impl FromStr for UserRole {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "admin" => Ok(UserRole::Admin),
            "engineer" => Ok(UserRole::Engineer),
            "viewer" => Ok(UserRole::Viewer),
            _ => Err(format!("unknown role: {s}")),
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
    Visualization,
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
            Stage::Visualization => "VISUALIZATION",
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

#[derive(Debug, Clone)]
pub struct GeoBounds {
    pub min_x: f64,
    pub max_x: f64,
    pub min_y: f64,
    pub max_y: f64,
    pub min_z: f64,
    pub max_z: f64,
}

impl GeoBounds {
    pub fn center(&self) -> (f64, f64, f64) {
        (
            (self.min_x + self.max_x) / 2.0,
            (self.min_y + self.max_y) / 2.0,
            (self.min_z + self.max_z) / 2.0,
        )
    }

    pub fn span_x(&self) -> f64 { self.max_x - self.min_x }
    pub fn span_y(&self) -> f64 { self.max_y - self.min_y }
    pub fn span_z(&self) -> f64 { self.max_z - self.min_z }
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

#[derive(Debug, Clone)]
pub struct TrialOutput {
    pub cd: f64,
    pub cl: f64,
    pub converged: bool,
    pub runtime_s: f64,
    pub n_cells: u64,
    pub n_failed_cells: u64,
}

impl TrialOutput {
    pub fn failed() -> Self {
        Self { cd: 1.0, cl: 0.0, converged: false, runtime_s: 0.0, n_cells: 0, n_failed_cells: u64::MAX }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct MeshParams {
    pub surface_min_level: u32,
    pub surface_max_level: u32,
    pub region_min_level: u32,
    pub region_max_level: u32,
    pub n_cells_between_levels: u32,
}

impl Default for MeshParams {
    fn default() -> Self {
        Self {
            surface_min_level: 3,
            surface_max_level: 4,
            region_min_level: 1,
            region_max_level: 1,
            n_cells_between_levels: 3,
        }
    }
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
    /// Rotating machinery parameters (None for non-rotating cases)
    pub rotating: Option<RotatingConfig>,
    /// Hypersonic flow parameters (None for non-hypersonic cases)
    pub hypersonic: Option<HypersonicConfig>,
    /// Conjugate Heat Transfer parameters (None for non-CHT cases)
    pub cht: Option<ChtConfig>,
    /// MHD / Electromagnetic parameters (None for non-MHD cases)
    pub mhd: Option<MhdConfig>,
    /// PEMFC / Fuel Cell parameters (None for non-fuel-cell cases)
    pub pemfc: Option<PemfcConfig>,
    /// Digital Wind Tunnel configuration (None for non-external-aero cases or to use defaults)
    pub wind_tunnel: Option<WindTunnelConfig>,
}

/// Rotating machinery configuration (MRF / AMI / propeller / turbomachinery)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RotatingConfig {
    /// Rotational speed in RPM
    pub rpm: f64,
    /// Rotation axis direction vector (x, y, z)
    pub axis: [f64; 3],
    /// Rotation origin point (x, y, z)
    pub origin: [f64; 3],
    /// Rotation approach: "MRF" (steady) or "AMI" (unsteady sliding mesh)
    pub approach: RotatingApproach,
    /// Number of blades (propeller/impeller)
    pub num_blades: u32,
    /// Diameter of the rotor/propeller (m)
    pub diameter_m: Option<f64>,
    /// Hub diameter (m)
    pub hub_diameter_m: Option<f64>,
    /// Tip clearance (m) — for turbomachinery
    pub tip_clearance_m: Option<f64>,
    /// Advance ratio J = V_inf / (n * D) — for propellers
    pub advance_ratio: Option<f64>,
    /// Target thrust coefficient CT
    pub target_ct: Option<f64>,
    /// Target power coefficient CP
    pub target_cp_max: Option<f64>,
    /// Target propulsive efficiency
    pub target_eta_min: Option<f64>,
    /// Mass flow rate (kg/s) — for pumps/compressors
    pub mass_flow_kg_s: Option<f64>,
    /// Pressure ratio target — for compressors
    pub pressure_ratio_target: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum RotatingApproach {
    MRF,
    AMI,
}

/// Chemistry model for hypersonic chemical nonequilibrium
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ChemistryModel {
    /// Frozen chemistry (perfect gas)
    None,
    /// 5-species Park model: N2, O2, NO, N, O
    Park5Species,
    /// 11-species Park model: adds N+, O+, NO+, e-
    Park11Species,
}

/// Wall catalysis model for hypersonic surfaces
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum WallCatalysis {
    NonCatalytic,
    FullyCatalytic,
    Partial(f64),
}

/// Flux scheme for hypersonic shock capturing
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum FluxScheme {
    Kurganov,
    AUSMPlus,
}

/// Heat transfer problem classification
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum HeatTransferProblem {
    ForcedConvection,
    CHT,
    NaturalConvection,
    Radiation,
}

/// Common solid materials for CHT
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SolidMaterial {
    Steel,
    Aluminum,
    Copper,
    Ceramic,
    CFRP,
    Inconel,
    Custom(f64, f64, f64), // (density, cp, kappa)
}

impl SolidMaterial {
    pub fn thermal_properties(&self) -> (f64, f64, f64) {
        match self {
            SolidMaterial::Steel => (7850.0, 502.0, 45.0),
            SolidMaterial::Aluminum => (2700.0, 897.0, 237.0),
            SolidMaterial::Copper => (8960.0, 385.0, 401.0),
            SolidMaterial::Ceramic => (3600.0, 880.0, 2.5),
            SolidMaterial::CFRP => (1550.0, 900.0, 0.5),
            SolidMaterial::Inconel => (8190.0, 435.0, 11.4),
            SolidMaterial::Custom(rho, cp, kappa) => (*rho, *cp, *kappa),
        }
    }
}

/// Radiation model selection for thermal problems
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum RadiationModel {
    None,
    P1,
    FvDOM,
    ViewFactor,
    Rosseland,
}

/// Conjugate Heat Transfer configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChtConfig {
    /// Problem classification
    pub problem_type: HeatTransferProblem,
    /// Fluid medium (e.g., "air", "water", "liquid_sodium")
    pub fluid: String,
    /// Solid material for CHT regions
    pub solid_material: SolidMaterial,
    /// Inlet / hot gas temperature (K)
    pub t_inlet_k: f64,
    /// Ambient / external temperature (K)
    pub t_ambient_k: f64,
    /// Inlet velocity (m/s)
    pub u_inlet_m_s: Option<f64>,
    /// Reynolds number (auto-computed if None)
    pub re: Option<f64>,
    /// Prandtl number
    pub pr: f64,
    /// Target heat flux at hot wall (W/m²)
    pub heat_flux_target_w_m2: Option<f64>,
    /// Enable radiation modeling
    pub radiation: bool,
    /// Specific radiation model (if radiation is true)
    pub radiation_model: RadiationModel,
    /// Enable phase change (boiling / melting)
    pub phase_change: bool,
    /// Maximum allowable solid temperature (K) — design limit
    pub max_t_solid_k: Option<f64>,
    /// Thickness of solid wall (m)
    pub wall_thickness_m: Option<f64>,
    /// External heat transfer coefficient (W/m²K) — for externalWallHeatFluxTemp
    pub external_h_w_m2k: Option<f64>,
    /// Surface emissivity
    pub emissivity: Option<f64>,
}

/// Hypersonic / high-Mach flow configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HypersonicConfig {
    /// Freestream Mach number (Ma > 5)
    pub mach_inf: f64,
    /// Altitude in km (for atmospheric properties)
    pub altitude_km: f64,
    /// Wall temperature (K) — None means adiabatic
    pub wall_temperature_k: Option<f64>,
    /// Wall catalysis model
    pub wall_catalysis: WallCatalysis,
    /// Enable real gas effects (JANAF / NASA polynomials)
    pub real_gas: bool,
    /// Chemical nonequilibrium model
    pub chemistry: ChemistryModel,
    /// Enable two-temperature model (vibrational-electronic)
    pub two_temperature: bool,
    /// Rarefied flow — switch to dsmcFoam if Kn > 0.01
    pub rarefied: bool,
    /// Nose radius (m) — for stagnation heating
    pub nose_radius_m: Option<f64>,
    /// Flux scheme for shock capturing
    pub flux_scheme: FluxScheme,
    /// Target peak heat flux (W/m²)
    pub target_peak_heat_flux_w_m2: Option<f64>,
}

/// MHD solver selection
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum MhdSolver {
    MhdFoam,
    MagneticFoam,
    Custom,
}

/// Wall conductivity type for MHD
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum MhdWallConductivity {
    Conducting,
    Insulating,
    Mixed,
}

/// Plasma actuator body force model
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum PlasmaModel {
    ShyyJayaraman,
    Suzen,
}

/// Plasma actuator configuration for DBD flow control
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlasmaActuatorConfig {
    /// Applied voltage (kV)
    pub voltage_kv: f64,
    /// AC frequency (Hz)
    pub frequency_hz: f64,
    /// Body force magnitude (N/m³)
    pub body_force_n_m3: f64,
    /// Actuator width along flow direction (m)
    pub actuator_width_m: f64,
    /// Phenomenological model
    pub model: PlasmaModel,
}

/// MHD / Electromagnetic flow configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MhdConfig {
    /// Applied magnetic field strength (Tesla)
    pub b0_tesla: f64,
    /// Electrical conductivity (S/m)
    pub sigma_s_m: f64,
    /// Magnetic permeability (H/m)
    pub mu_permeability_h_m: f64,
    /// MHD solver type
    pub solver: MhdSolver,
    /// Low magnetic Reynolds number approximation (Rm << 1)
    pub low_rm: bool,
    /// Hartmann number (auto-computed if None)
    pub hartmann_number: Option<f64>,
    /// Wall electrical conductivity type
    pub wall_conductivity: MhdWallConductivity,
    /// DBD plasma actuator config (None if no actuator)
    pub plasma_actuator: Option<PlasmaActuatorConfig>,
}

/// Pre-built solver template catalogue
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SolverTemplate {
    MhdSimpleFoam,
    MhdReactingFoam,
    PlasmaActuatorFoam,
    HyperReactingFoam,
    ChtRotatingFoam,
    ViscoelasticHeatFoam,
    BubblyReactingFoam,
    AblationFoam,
    DsmcReactingFoam,
    MagneticConvectionFoam,
    RotorAeroFoam,
    CoupledPlasmaFoam,
    /// PEMFC fuel cell solver (isothermal / simple polarization)
    PemfcFoam,
    /// PEMFC fuel cell solver with thermal effects
    PemfcThermalFoam,
    /// PEMFC fuel cell solver with two-phase water transport
    PemfcTwoPhaseFoam,
    Custom,
}

/// Coupling strategy for segregated solvers
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum CouplingStrategy {
    SegregatedSIMPLE,
    SegregatedPISO,
    CoupledMatrix,
    OperatorSplit,
}

/// Time treatment
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TimeTreatment {
    Steady,
    UnsteadyFirstOrder,
    UnsteadySecondOrder,
}

/// Physics modules that can be assembled into a custom solver
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum PhysicsModule {
    FluidDynamics,
    Compressible,
    Turbulence,
    HeatTransfer,
    SpeciesTransport,
    ChemicalReactions,
    TwoPhase,
    SolidMechanics,
    Electromagnetic,
    Radiation,
    RotatingFrame,
    PorousMedia,
    ParticleTracking,
    CustomEOS,
    CustomViscosity,
}

/// Solver design intake from the agentic interview
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SolverDesign {
    /// Solver name (e.g., "mhdReactingFoam")
    pub solver_name: String,
    /// Base solver or template
    pub template: SolverTemplate,
    /// One-line description
    pub description: String,
    /// Enabled physics modules
    pub modules: Vec<PhysicsModule>,
    /// Coupling strategy
    pub coupling: CouplingStrategy,
    /// Time treatment
    pub time_treatment: TimeTreatment,
    /// Target OpenFOAM version
    pub openfoam_version: String,
    /// Validation case description
    pub validation_case: Option<String>,
}

/// Agent state tracking for the build loop
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SolverBuildState {
    pub solver_name: String,
    pub stage: String,
    pub compile_attempts: u32,
    pub last_error: Option<String>,
    pub fix_applied: Option<String>,
    pub unit_tests_passed: bool,
    pub l2_error_vs_reference: Option<f64>,
    pub gci_percent: Option<f64>,
    pub ready_for_production: bool,
}

/// Non-Newtonian viscosity model selection
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ViscosityModel {
    Newtonian,
    PowerLaw,
    CrossPowerLaw,
    BirdCarreau,
    HerschelBulkley,
    Casson,
}

/// Non-Newtonian fluid configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NonNewtonianConfig {
    pub model: ViscosityModel,
    pub nu0: f64,
    pub nu_inf: f64,
    pub k: f64,
    pub n: f64,
    pub tau0: f64,
    pub nu_min: f64,
    pub nu_max: f64,
}

impl Default for NonNewtonianConfig {
    fn default() -> Self {
        Self {
            model: ViscosityModel::Newtonian,
            nu0: 1e-2,
            nu_inf: 1e-6,
            k: 0.005,
            n: 0.4,
            tau0: 10.0,
            nu_min: 1e-4,
            nu_max: 1e2,
        }
    }
}

/// Porous media model
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum PorousModel {
    Darcy,
    DarcyForchheimer,
}

/// Porous media zone configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PorousZoneConfig {
    pub model: PorousModel,
    pub cell_zone: String,
    pub d_coeffs: [f64; 3],
    pub f_coeffs: [f64; 3],
}

/// Lagrangian particle injection type
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ParticleInjection {
    PatchInjection,
    ConeInjection,
    ManualInjection,
}

/// Lagrangian particle configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParticleConfig {
    pub injection: ParticleInjection,
    pub diameter_m: f64,
    pub mass_flow_kg_s: f64,
    pub velocity_m_s: f64,
    pub temperature_k: f64,
    pub material_density_kg_m3: f64,
}

/// Viscoelastic constitutive model
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ViscoelasticModel {
    OldroydB,
    Giesekus,
}

/// Viscoelastic fluid configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ViscoelasticConfig {
    pub model: ViscoelasticModel,
    pub relaxation_time_s: f64,
    pub solvent_viscosity_ratio: f64,
    pub mobility_factor: f64,
}

/// Multiphase flow model
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum MultiphaseModel {
    VOF,
    EulerEuler,
    DriftFlux,
}

/// Multiphase configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultiphaseConfig {
    pub model: MultiphaseModel,
    pub n_phases: u32,
    pub surface_tension_n_m: f64,
    pub phase_names: Vec<String>,
}

/// NASA 7-coefficient JANAF polynomial species
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum JanafSpecies {
    N2,
    O2,
    NO,
    N,
    O,
    Ar,
    CO2,
    H2O,
    Custom(String, [f64; 7], [f64; 7]),
}

/// JANAF thermochemistry configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JanafConfig {
    pub species: Vec<JanafSpecies>,
    pub t_low: f64,
    pub t_mid: f64,
    pub t_high: f64,
}

impl Default for JanafConfig {
    fn default() -> Self {
        Self {
            species: vec![JanafSpecies::N2, JanafSpecies::O2, JanafSpecies::NO, JanafSpecies::N, JanafSpecies::O],
            t_low: 200.0,
            t_mid: 1000.0,
            t_high: 6000.0,
        }
    }
}

/// Combustion model selection
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum CombustionModel {
    EDC,
    LaminarFlamelet,
    PaSR,
    WellStirredReactor,
}

/// Combustion configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CombustionConfig {
    pub model: CombustionModel,
    pub c_eps: f64,
    pub c_mu: f64,
    pub oxidizer: String,
    pub fuel: String,
}

impl Default for CombustionConfig {
    fn default() -> Self {
        Self {
            model: CombustionModel::EDC,
            c_eps: 2.1377,
            c_mu: 0.09,
            oxidizer: "O2".into(),
            fuel: "CH4".into(),
        }
    }
}

/// Cavitation mass transfer model
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum CavitationModel {
    Kunz,
    SchnerrSauer,
    Merkle,
}

/// Cavitation configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CavitationConfig {
    pub model: CavitationModel,
    pub p_vap_kpa: f64,
    pub rho_liquid_kg_m3: f64,
    pub rho_vapor_kg_m3: f64,
}

impl Default for CavitationConfig {
    fn default() -> Self {
        Self {
            model: CavitationModel::SchnerrSauer,
            p_vap_kpa: 2.34,
            rho_liquid_kg_m3: 1000.0,
            rho_vapor_kg_m3: 0.0258,
        }
    }
}

/// Spray breakup model
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SprayBreakupModel {
    ReitzDiwakar,
    KHRT,
    TAB,
    PilchErdman,
}

/// Spray evaporation model
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SprayEvaporationModel {
    Standard,
    FuchsKnudsen,
}

/// Spray configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SprayConfig {
    pub breakup: SprayBreakupModel,
    pub evaporation: SprayEvaporationModel,
    pub collision: bool,
    pub parcel_per_second: f64,
    pub injection_velocity_m_s: f64,
    pub cone_angle_deg: f64,
}

impl Default for SprayConfig {
    fn default() -> Self {
        Self {
            breakup: SprayBreakupModel::KHRT,
            evaporation: SprayEvaporationModel::Standard,
            collision: true,
            parcel_per_second: 1e5,
            injection_velocity_m_s: 50.0,
            cone_angle_deg: 15.0,
        }
    }
}

/// Extended turbulence model selection
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TurbulenceModel {
    KEpsilon,
    RealizableKEpsilon,
    KOmegaSST,
    KOmegaSSTSAS,
    V2F,
    LES,
    LESigma,
    KEqnLES,
    PANS,
    SpalartAllmarasDES,
    SpalartAllmarasIDDES,
}

/// Fluid-structure interaction model
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum FSIModel {
    LinearElastic,
    NonLinearGeometric,
    Plastic,
}

/// FSI boundary coupling type
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum FSICoupling {
    DirichletNeumann,
    NeumannNeumann,
    RobinRobin,
}

/// Fluid-structure interaction configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FSIConfig {
    pub model: FSIModel,
    pub coupling: FSICoupling,
    pub youngs_modulus_gpa: f64,
    pub poisson_ratio: f64,
    pub density_kg_m3: f64,
    pub mesh_relaxation: f64,
    pub n_subcycles: u32,
}

impl Default for FSIConfig {
    fn default() -> Self {
        Self {
            model: FSIModel::LinearElastic,
            coupling: FSICoupling::DirichletNeumann,
            youngs_modulus_gpa: 200.0,
            poisson_ratio: 0.3,
            density_kg_m3: 7800.0,
            mesh_relaxation: 0.5,
            n_subcycles: 5,
        }
    }
}

/// Aeroacoustic FW-H configuration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum FWHSource {
    PermeableSurface,
    SolidSurface,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AeroacousticConfig {
    pub fwh_source: FWHSource,
    pub receiver_positions: Vec<(f64, f64, f64)>,
    pub start_time: f64,
    pub density_far: f64,
    pub speed_of_sound: f64,
}

impl Default for AeroacousticConfig {
    fn default() -> Self {
        Self {
            fwh_source: FWHSource::SolidSurface,
            receiver_positions: vec![(0.0, 0.0, 1.0)],
            start_time: 0.0,
            density_far: 1.225,
            speed_of_sound: 340.0,
        }
    }
}

/// Free surface wave model
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum WaveModel {
    StokesFirst,
    StokesFifth,
    Irregular,
    StreamFunction,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WaveConfig {
    pub model: WaveModel,
    pub wave_height_m: f64,
    pub wave_period_s: f64,
    pub water_depth_m: f64,
    pub direction_deg: f64,
    pub relaxation_zone_length_m: f64,
}

impl Default for WaveConfig {
    fn default() -> Self {
        Self {
            model: WaveModel::StokesFirst,
            wave_height_m: 2.0,
            wave_period_s: 8.0,
            water_depth_m: 20.0,
            direction_deg: 0.0,
            relaxation_zone_length_m: 10.0,
        }
    }
}

/// Phase change / melting-solidification model
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum PhaseChangeModel {
    EnthalpyPorosity,
    LevelSet,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhaseChangeConfig {
    pub model: PhaseChangeModel,
    pub t_solidus_k: f64,
    pub t_liquidus_k: f64,
    pub latent_heat_j_kg: f64,
    pub mushy_constant: f64,
}

impl Default for PhaseChangeConfig {
    fn default() -> Self {
        Self {
            model: PhaseChangeModel::EnthalpyPorosity,
            t_solidus_k: 800.0,
            t_liquidus_k: 900.0,
            latent_heat_j_kg: 2.5e5,
            mushy_constant: 1e5,
        }
    }
}

/// Wind turbine actuator model
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ActuatorModel {
    Disc,
    Line,
    ALM,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindTurbineConfig {
    pub actuator: ActuatorModel,
    pub thrust_coefficient: f64,
    pub power_coefficient: f64,
    pub rotor_diameter_m: f64,
    pub hub_height_m: f64,
    pub wind_speed_ref_m_s: f64,
    pub rated_power_mw: f64,
}

impl Default for WindTurbineConfig {
    fn default() -> Self {
        Self {
            actuator: ActuatorModel::Disc,
            thrust_coefficient: 0.8,
            power_coefficient: 0.45,
            rotor_diameter_m: 126.0,
            hub_height_m: 90.0,
            wind_speed_ref_m_s: 10.0,
            rated_power_mw: 5.0,
        }
    }
}

/// Electrostatic / charge transport configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ElectrostaticConfig {
    pub potential_v: f64,
    pub permittivity_f_m: f64,
    pub space_charge_c_m3: f64,
    pub ion_mobility_m2_vs: f64,
}

impl Default for ElectrostaticConfig {
    fn default() -> Self {
        Self {
            potential_v: 1e4,
            permittivity_f_m: 8.854e-12,
            space_charge_c_m3: 0.0,
            ion_mobility_m2_vs: 2.0e-4,
        }
    }
}

/// Thermal protection system ablation model
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum AblationModel {
    SurfaceRecession,
    CharringMaterial,
    Pyrolysis,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AblationConfig {
    pub model: AblationModel,
    pub char_conductivity_w_mk: f64,
    pub virgin_conductivity_w_mk: f64,
    pub pyrolysis_gas_enthalpy_j_kg: f64,
    pub recession_rate_coeff: f64,
    pub emissivity: f64,
}

impl Default for AblationConfig {
    fn default() -> Self {
        Self {
            model: AblationModel::SurfaceRecession,
            char_conductivity_w_mk: 0.5,
            virgin_conductivity_w_mk: 2.0,
            pyrolysis_gas_enthalpy_j_kg: 5e6,
            recession_rate_coeff: 1e-4,
            emissivity: 0.85,
        }
    }
}

/// Propulsion / rocket chamber configuration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum PropulsionModel {
    SolidRocket,
    LiquidRocket,
    HybridRocket,
    Scramjet,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PropulsionConfig {
    pub model: PropulsionModel,
    pub chamber_pressure_bar: f64,
    pub chamber_temp_k: f64,
    pub exit_pressure_bar: f64,
    pub mass_flow_rate_kg_s: f64,
    pub throat_area_m2: f64,
    pub exit_area_m2: f64,
}

impl Default for PropulsionConfig {
    fn default() -> Self {
        Self {
            model: PropulsionModel::LiquidRocket,
            chamber_pressure_bar: 70.0,
            chamber_temp_k: 3500.0,
            exit_pressure_bar: 0.1,
            mass_flow_rate_kg_s: 100.0,
            throat_area_m2: 0.01,
            exit_area_m2: 0.05,
        }
    }
}

/// Nuclear / radiation transport configuration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum NuclearModel {
    NeutronTransport,
    PhotonTransport,
    Coupled,
    RadiationHydro,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NuclearConfig {
    pub model: NuclearModel,
    pub n_energy_groups: u32,
    pub cross_sections: Vec<f64>,
    pub source_strength_m3: f64,
    pub temperature_k: f64,
}

impl Default for NuclearConfig {
    fn default() -> Self {
        Self {
            model: NuclearModel::NeutronTransport,
            n_energy_groups: 2,
            cross_sections: vec![0.1, 0.5],
            source_strength_m3: 1e10,
            temperature_k: 600.0,
        }
    }
}

/// Marine / hydrodynamics configuration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum MarineModel {
    Hydrofoil,
    PropellerOpenWater,
    ShipResistance,
    PlaningHull,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarineConfig {
    pub model: MarineModel,
    pub speed_knots: f64,
    pub depth_m: f64,
    pub cavitation_margin: f64,
    pub propeller_rpm: f64,
    pub thrust_coefficient: f64,
}

impl Default for MarineConfig {
    fn default() -> Self {
        Self {
            model: MarineModel::Hydrofoil,
            speed_knots: 20.0,
            depth_m: 5.0,
            cavitation_margin: 1.5,
            propeller_rpm: 1200.0,
            thrust_coefficient: 0.6,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[derive(Default)]
pub enum MlSurrogateModel {
    #[default]
    GpRbf,
    GpMatern,
    Rf,
    Xgb,
    Lhs,
}


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MlSurrogateConfig {
    pub model: MlSurrogateModel,
    pub n_samples: u32,
    pub n_varies: u32,
    pub exploration_rate: f64,
    pub acquisition: String,
    pub rho_default: f64,
    pub length_scale_default: f64,
}

impl Default for MlSurrogateConfig {
    fn default() -> Self {
        Self {
            model: MlSurrogateModel::default(),
            n_samples: 200,
            n_varies: 4,
            exploration_rate: 0.1,
            acquisition: "expected_improvement".into(),
            rho_default: 1.0,
            length_scale_default: 1.0,
        }
    }
}

impl MlSurrogateConfig {
    pub fn label(&self) -> &'static str {
        match self.model {
            MlSurrogateModel::GpRbf => "gp_rbf",
            MlSurrogateModel::GpMatern => "gp_matern",
            MlSurrogateModel::Rf => "random_forest",
            MlSurrogateModel::Xgb => "xgboost",
            MlSurrogateModel::Lhs => "lhs",
        }
    }
}

// ── PEMFC Fuel Cell Types ──────────────────────────────────────

/// PEMFC model complexity level
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[derive(Default)]
pub enum PemfcModel {
    SimplePolarization,
    #[default]
    Isothermal1D,
    NonIsothermal,
    TwoPhase,
}


/// Flow-field plate channel pattern
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[derive(Default)]
pub enum PemfcFlowField {
    Parallel,
    #[default]
    Serpentine,
    Interdigitated,
    PinType,
}


/// Operating cycle / load profile for transient simulation
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[derive(Default)]
pub enum PemfcCycleProfile {
    #[default]
    Potentiodynamic,
    Galvanodynamic,
    DriveCycle,
}


/// Degradation / aging model
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[derive(Default)]
pub enum PemfcDegradationModel {
    #[default]
    None,
    PtDissolution,
    CarbonCorrosion,
    PinholeFormation,
    Combined,
}


/// Full PEMFC configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PemfcConfig {
    // Model tier
    pub model: PemfcModel,
    // Operating conditions
    pub t_cell_k: f64,
    pub p_anode_bar: f64,
    pub p_cathode_bar: f64,
    pub lambda_anode: f64,
    pub lambda_cathode: f64,
    pub stoich_anode: f64,
    pub stoich_cathode: f64,
    // Electrochemistry
    pub i_ref_a_m2: f64,
    pub alpha_anode: f64,
    pub alpha_cathode: f64,
    pub exchange_i_anode_a_m2: f64,
    pub exchange_i_cathode_a_m2: f64,
    // Membrane
    pub membrane_thickness_um: f64,
    pub membrane_conductivity_s_m: f64,
    pub eod_coefficient: f64,
    pub water_uptake_max: f64,
    // GDL / CL
    pub gdl_thickness_um: f64,
    pub gdl_porosity: f64,
    pub gdl_permeability_m2: f64,
    pub cl_thickness_um: f64,
    pub cl_porosity: f64,
    // Flow field geometry
    pub flow_field: PemfcFlowField,
    pub channel_width_mm: f64,
    pub rib_width_mm: f64,
    pub channel_depth_mm: f64,
    pub n_passes: u32,
    pub turn_radius_mm: f64,
    pub landing_length_mm: f64,
    pub active_width_mm: f64,
    pub active_length_mm: f64,
    // Mesh resolution
    pub cells_per_channel_width: u32,
    pub cells_per_rib_width: u32,
    pub cells_across_channel: u32,
    pub cells_across_gdl: u32,
    pub cells_across_cl: u32,
    pub cells_across_membrane: u32,
    pub cells_along_pass: u32,
    // Cycling
    pub cycle_profile: PemfcCycleProfile,
    pub start_voltage_v: f64,
    pub end_voltage_v: f64,
    pub sweep_rate_mv_s: f64,
    pub hold_time_s: f64,
    pub n_cycles: u32,
    // Degradation
    pub degradation_model: PemfcDegradationModel,
    pub initial_ecsa_m2_g: f64,
    pub carbon_loading_mg_cm2: f64,
    pub acceleration_factor: f64,
}

impl Default for PemfcConfig {
    fn default() -> Self {
        Self {
            model: PemfcModel::default(),
            t_cell_k: 353.15,
            p_anode_bar: 1.5,
            p_cathode_bar: 1.5,
            lambda_anode: 1.5,
            lambda_cathode: 2.0,
            stoich_anode: 1.2,
            stoich_cathode: 2.0,
            i_ref_a_m2: 1.0e4,
            alpha_anode: 0.5,
            alpha_cathode: 0.5,
            exchange_i_anode_a_m2: 1.0e3,
            exchange_i_cathode_a_m2: 1.0e2,
            membrane_thickness_um: 50.0,
            membrane_conductivity_s_m: 10.0,
            eod_coefficient: 1.0,
            water_uptake_max: 14.0,
            gdl_thickness_um: 200.0,
            gdl_porosity: 0.7,
            gdl_permeability_m2: 1.0e-12,
            cl_thickness_um: 10.0,
            cl_porosity: 0.4,
            flow_field: PemfcFlowField::default(),
            channel_width_mm: 1.0,
            rib_width_mm: 1.0,
            channel_depth_mm: 0.5,
            n_passes: 3,
            turn_radius_mm: 0.5,
            landing_length_mm: 10.0,
            active_width_mm: 50.0,
            active_length_mm: 50.0,
            cells_per_channel_width: 6,
            cells_per_rib_width: 6,
            cells_across_channel: 10,
            cells_across_gdl: 8,
            cells_across_cl: 4,
            cells_across_membrane: 6,
            cells_along_pass: 40,
            cycle_profile: PemfcCycleProfile::default(),
            start_voltage_v: 1.0,
            end_voltage_v: 0.4,
            sweep_rate_mv_s: 5.0,
            hold_time_s: 60.0,
            n_cycles: 1,
            degradation_model: PemfcDegradationModel::default(),
            initial_ecsa_m2_g: 80.0,
            carbon_loading_mg_cm2: 0.4,
            acceleration_factor: 1.0,
        }
    }
}

/// Configuration for an inlet wall in the digital wind tunnel.
/// References a blockMesh boundary face name (e.g. "inlet", "front") and is
/// toggled active/inactive for multi-inlet studies (crosswind, yaw sweeps).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InletWallConfig {
    /// Name of the blockMesh boundary patch that acts as this inlet
    pub patch_name: String,
    /// Inlet velocity magnitude (m/s)
    pub velocity_m_s: f64,
    /// Turbulence intensity fraction (e.g. 0.05 for 5%)
    pub turbulence_intensity: f64,
    /// Whether this inlet wall is active (default true for the primary inlet)
    pub active: bool,
}

impl InletWallConfig {
    pub fn primary(u_inf: f64, ti: f64) -> Self {
        Self {
            patch_name: "inlet".into(),
            velocity_m_s: u_inf,
            turbulence_intensity: ti,
            active: true,
        }
    }
}

/// Digital Wind Tunnel domain sizing configuration.
/// All dimensions are in multiples of the model reference length (chord).
/// The defaults reflect common external aerodynamic practice.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindTunnelConfig {
    /// Upstream distance from model origin to inlet wall (chord multiples)
    pub upstream: f64,
    /// Downstream distance from model origin to outlet wall (chord multiples)
    pub downstream: f64,
    /// Vertical half-height (above/below model) in chord multiples
    pub vertical: f64,
    /// Lateral half-width (each side) in chord multiples
    pub lateral: f64,
    /// Freestream velocity (m/s) — auto-detected from Mach if not set
    pub velocity_m_s: Option<f64>,
    /// Turbulence intensity fraction (e.g. 0.01 for 1%)
    pub turbulence_intensity: f64,
    /// Turbulence viscosity ratio (nut/nu)
    pub turbulence_viscosity_ratio: f64,
    /// Target y+ for inflation layer sizing
    pub target_yplus: f64,
    /// Additional inlet walls for crosswind / multi-inlet studies
    pub inlet_walls: Vec<InletWallConfig>,
}

impl Default for WindTunnelConfig {
    fn default() -> Self {
        Self {
            upstream: 20.0,
            downstream: 40.0,
            vertical: 25.0,
            lateral: 25.0,
            velocity_m_s: None,
            turbulence_intensity: 0.005,
            turbulence_viscosity_ratio: 10.0,
            target_yplus: 1.0,
            inlet_walls: Vec::new(),
        }
    }
}

/// Post-processed wind tunnel results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindTunnelResult {
    /// Blockage ratio (A_model / A_tunnel * 100)
    pub blockage_pct: f64,
    /// Corrected freestream velocity for blockage (m/s)
    pub u_corrected_m_s: f64,
    /// Uncorrected drag coefficient
    pub cd_uncorrected: f64,
    /// Blockage-corrected drag coefficient
    pub cd_corrected: f64,
    /// Uncorrected lift coefficient
    pub cl_uncorrected: f64,
    /// Blockage-corrected lift coefficient
    pub cl_corrected: f64,
    /// Tunnel cross-sectional area (m²)
    pub tunnel_area_m2: f64,
    /// Model frontal / planform area (m²)
    pub model_area_m2: f64,
    /// Upstream distance used (m)
    pub upstream_m: f64,
    /// Downstream distance used (m)
    pub downstream_m: f64,
    /// Vertical half-height (m)
    pub vertical_m: f64,
    /// Lateral half-width (m)
    pub lateral_m: f64,
}

/// Top-level configuration passed through the pipeline to customise
/// the blockMesh generation, solver setup, and post‑processing for a case.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaseConfig {
    /// Flow type classification
    pub flow_type: FlowType,
    /// Freestream velocity magnitude (m/s)
    pub velocity_m_s: f64,
    /// Turbulence model name (e.g. "kOmegaSST", "SpalartAllmaras")
    pub turbulence_model: String,
    /// Digital Wind Tunnel configuration (None for internal flows)
    pub wind_tunnel: Option<WindTunnelConfig>,
    /// Model reference length for chord-based sizing (m); auto-detected from STL
    pub reference_length_m: Option<f64>,
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
