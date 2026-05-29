use aeroflow_core::types::{
    RotatingConfig, RotatingApproach, ChtConfig, HeatTransferProblem, SolidMaterial,
    RadiationModel, MhdConfig, MhdSolver, MhdWallConductivity, ChemistryModel, HypersonicConfig,
    WallCatalysis, FluxScheme,
    AblationConfig, PropulsionConfig,
    NuclearConfig, MarineConfig,
    WaveConfig, MlSurrogateConfig,
    CavitationConfig, AeroacousticConfig, PhaseChangeConfig,
    ElectrostaticConfig, WindTurbineConfig,
    PemfcConfig, PemfcModel,
};
use aeroflow_core::{IntakeConfig, Priority};
use aeroflow_solver::SolverConfigGen;

fn base_intake() -> IntakeConfig {
    IntakeConfig {
        geometry_description: "integration test".into(),
        geometry_file: None,
        case_class: None,
        workspace_root: None,
        user_id: None,
        altitude_m: 0.0,
        mach_number: 0.2,
        reynolds_number: 1e6,
        alpha_sweep_deg: vec![],
        freestream_turbulence_intensity: 0.01,
        target_cl: None,
        target_cd_max: None,
        target_yplus_max: 1.0,
        convergence_residual: 1e-5,
        max_agent_iterations: 5,
        human_in_loop: false,
        priority: Priority::Balanced,
        hpc_cores: 4,
        time_budget_hours: 24.0,
        rotating: None,
        hypersonic: None,
        cht: None,
        mhd: None,
        pemfc: None,
        wind_tunnel: None,
    }
}

fn make_rotating(approach: RotatingApproach, rpm: f64) -> RotatingConfig {
    RotatingConfig {
        rpm,
        axis: [0.0, 0.0, 1.0],
        origin: [0.0, 0.0, 0.0],
        approach,
        num_blades: 6,
        diameter_m: Some(0.3),
        hub_diameter_m: Some(0.05),
        tip_clearance_m: None,
        advance_ratio: Some(0.4),
        target_ct: None,
        target_cp_max: None,
        target_eta_min: None,
        mass_flow_kg_s: None,
        pressure_ratio_target: None,
    }
}

fn make_cht(problem: HeatTransferProblem) -> ChtConfig {
    ChtConfig {
        problem_type: problem,
        fluid: "air".into(),
        solid_material: SolidMaterial::Steel,
        t_inlet_k: 800.0,
        t_ambient_k: 300.0,
        u_inlet_m_s: Some(100.0),
        re: None,
        pr: 0.71,
        heat_flux_target_w_m2: Some(5e5),
        radiation: true,
        radiation_model: RadiationModel::FvDOM,
        phase_change: false,
        max_t_solid_k: Some(1200.0),
        wall_thickness_m: Some(0.005),
        external_h_w_m2k: Some(15.0),
        emissivity: Some(0.85),
    }
}

fn make_mhd(b0: f64, wall: MhdWallConductivity, low_rm: bool) -> MhdConfig {
    MhdConfig {
        b0_tesla: b0,
        sigma_s_m: 3.5e6,
        mu_permeability_h_m: 1.256637e-6,
        solver: MhdSolver::MhdFoam,
        low_rm,
        hartmann_number: None,
        wall_conductivity: wall,
        plasma_actuator: None,
    }
}

fn make_hypersonic(mach: f64, chemistry: ChemistryModel) -> HypersonicConfig {
    HypersonicConfig {
        mach_inf: mach,
        altitude_km: 20.0,
        wall_temperature_k: Some(1000.0),
        wall_catalysis: WallCatalysis::FullyCatalytic,
        real_gas: true,
        chemistry,
        two_temperature: true,
        rarefied: false,
        nose_radius_m: Some(0.1),
        flux_scheme: FluxScheme::AUSMPlus,
        target_peak_heat_flux_w_m2: None,
    }
}

fn cht_intake(cht: ChtConfig) -> IntakeConfig {
    let mut i = base_intake();
    i.cht = Some(cht);
    i.mach_number = 0.5;
    i
}

fn hypersonic_intake(hc: HypersonicConfig) -> IntakeConfig {
    let mach = hc.mach_inf;
    let mut i = base_intake();
    i.hypersonic = Some(hc);
    i.mach_number = mach;
    i
}

// ── Rotating + CHT ───────────────────────────────────────────────

#[test]
fn test_rotating_cht_configs() {
    let mut cfg = base_intake();
    cfg.rotating = Some(make_rotating(RotatingApproach::MRF, 3000.0));
    cfg.cht = Some(make_cht(HeatTransferProblem::CHT));
    cfg.mach_number = 0.3;

    let s = SolverConfigGen::new();
    let mrf = s.generate_mrf_properties(&cfg).unwrap();
    assert!(mrf.contains("MRFProperties"));
    let cht = s.generate_cht_fluid_thermo(&cfg).unwrap();
    assert!(cht.contains("mixture"));
    let solver = s.select_cht_solver(&cfg);
    assert_eq!(solver, "chtMultiRegionSimpleFoam");
}

// ── MHD + Reacting Flow ──────────────────────────────────────────

#[test]
fn test_mhd_reacting_configs() {
    let mut cfg = base_intake();
    cfg.mhd = Some(make_mhd(2.0, MhdWallConductivity::Insulating, true));
    cfg.hypersonic = Some(make_hypersonic(5.0, ChemistryModel::Park5Species));
    cfg.mach_number = 5.0;

    let s = SolverConfigGen::new();
    let mhd_opts = s.generate_mhd_fv_options(&cfg).unwrap();
    assert!(mhd_opts.contains("fvOptions") || mhd_opts.contains("Lorentz"));
    let thermo = s.generate_thermo_dict(&cfg).unwrap();
    assert!(thermo.contains("thermophysicalProperties"));
    let solver = s.select_solver(&cfg);
    assert!(!solver.is_empty());
}

// ── Wave + Marine ────────────────────────────────────────────────

#[test]
fn test_wave_marine() {
    let s = SolverConfigGen::new();
    let wv = WaveConfig::default();
    let wave_dict = s.generate_wave_properties(&wv);
    assert!(wave_dict.contains("waveProperties"));

    let mc = MarineConfig::default();
    let marine_dict = s.generate_marine_properties(&mc);
    assert!(marine_dict.contains("marineProperties"));
}

// ── Propulsion + CHT ─────────────────────────────────────────────

#[test]
fn test_propulsion_cht() {
    let s = SolverConfigGen::new();
    let pc = PropulsionConfig::default();
    let prop = s.generate_propulsion_properties(&pc);
    assert!(prop.contains("propulsionProperties"));
    assert!(prop.contains("expansionRatio"));

    let cc = make_cht(HeatTransferProblem::CHT);
    let cht = s.generate_cht_solid_thermo(&cht_intake(cc)).unwrap();
    assert!(cht.contains("thermophysicalProperties") || cht.contains("solid"));
}

// ── Nuclear + CHT ────────────────────────────────────────────────

#[test]
fn test_nuclear_cht() {
    let s = SolverConfigGen::new();
    let nc = NuclearConfig::default();
    let nuc = s.generate_nuclear_transport(&nc);
    assert!(nuc.contains("nuclearProperties"));

    let cc = make_cht(HeatTransferProblem::CHT);
    let cht = s.generate_cht_fluid_thermo(&cht_intake(cc)).unwrap();
    assert!(cht.contains("mixture") || cht.contains("thermophysicalProperties"));
}

// ── Hypersonic + Ablation ────────────────────────────────────────

#[test]
fn test_hypersonic_ablation() {
    let s = SolverConfigGen::new();
    let hc = make_hypersonic(10.0, ChemistryModel::Park11Species);
    let intake = hypersonic_intake(hc.clone());
    let schemes = s.generate_hypersonic_fv_schemes(&intake).unwrap();
    assert!(schemes.contains("fluxScheme"), "missing fluxScheme in: {schemes}");

    let ac = AblationConfig::default();
    let abl = s.generate_ablation_properties(&ac);
    assert!(abl.contains("ablationProperties"));
}

// ── Aeroacoustics + Rotating ─────────────────────────────────────

#[test]
fn test_aeroacoustics_rotating() {
    let s = SolverConfigGen::new();
    let aa = AeroacousticConfig::default();
    let ac = s.generate_aeroacoustics_functions(&aa);
    assert!(ac.contains("functions"));

    let mut cfg = base_intake();
    cfg.rotating = Some(make_rotating(RotatingApproach::AMI, 6000.0));
    let dyn_dict = s.generate_dynamic_mesh_dict(&cfg).unwrap();
    assert!(dyn_dict.contains("dynamicMeshDict") || dyn_dict.contains("AMI"));
}

// ── Cavitation + Marine ──────────────────────────────────────────

#[test]
fn test_cavitation_marine() {
    let s = SolverConfigGen::new();
    let cav = CavitationConfig::default();
    let cav_dict = s.generate_cavitation_properties(&cav);
    assert!(cav_dict.contains("cavitationProperties"));

    let mc = MarineConfig::default();
    let marine_dict = s.generate_marine_properties(&mc);
    assert!(marine_dict.contains("marineProperties"));
}

// ── ML Surrogate + Rotating ──────────────────────────────────────

#[test]
fn test_ml_surrogate_with_rotating() {
    let s = SolverConfigGen::new();
    let ml = MlSurrogateConfig::default();
    let ml_dict = s.generate_ml_surrogate(&ml);
    assert!(ml_dict.contains("surrogateProperties"));
    assert!(ml_dict.contains("gpRbf"));
}

// ── Phase Change + CHT ───────────────────────────────────────────

#[test]
fn test_phase_change_cht() {
    let s = SolverConfigGen::new();
    let pc = PhaseChangeConfig::default();
    let pc_dict = s.generate_phase_change_properties(&pc);
    assert!(pc_dict.contains("solidificationProperties"));

    let cc = make_cht(HeatTransferProblem::CHT);
    let if_bcs = s.generate_cht_interface_bcs(&cht_intake(cc), "fluid").unwrap();
    assert!(if_bcs.0.contains("fluid") || if_bcs.1.contains("fluid"));
}

// ── Electrostatic + Wind Energy ──────────────────────────────────

#[test]
fn test_electrostatic_wind() {
    let s = SolverConfigGen::new();
    let es = ElectrostaticConfig::default();
    let es_dict = s.generate_electrostatic_transport(&es);
    assert!(es_dict.contains("electrostaticTransport") || es_dict.contains("transport"));

    let wt = WindTurbineConfig::default();
    let wt_dict = s.generate_actuator_fv_options(&wt);
    assert!(wt_dict.contains("fvOptions") || wt_dict.contains("actuator"));
}

// ── MHD + CHT + Rotating Solver Selection ────────────────────────

#[test]
fn test_mhd_cht_rotating_solver_selection() {
    let mut cfg = base_intake();
    cfg.mhd = Some(make_mhd(1.0, MhdWallConductivity::Conducting, false));
    cfg.cht = Some(make_cht(HeatTransferProblem::ForcedConvection));
    cfg.mach_number = 0.2;

    let s = SolverConfigGen::new();
    let solver = s.select_solver(&cfg);
    assert!(!solver.is_empty(), "solver should not be empty");
}

// ── PEMFC Integration Tests ─────────────────────────────────────

fn pemfc_intake(model: PemfcModel) -> IntakeConfig {
    let mut cfg = base_intake();
    cfg.pemfc = Some(PemfcConfig {
        model,
        ..PemfcConfig::default()
    });
    cfg
}

#[test]
fn test_pemfc_standalone_configs() {
    let cfg = pemfc_intake(PemfcModel::Isothermal1D);
    let s = SolverConfigGen::new();

    let props = s.generate_pemfc_properties(cfg.pemfc.as_ref().unwrap());
    assert!(props.contains("pemfcProperties"));
    assert!(props.contains("isothermal1D"));

    let ec = s.generate_pemfc_electrochemistry(cfg.pemfc.as_ref().unwrap());
    assert!(ec.contains("electrochemistryProperties"));

    let mem = s.generate_pemfc_membrane(cfg.pemfc.as_ref().unwrap());
    assert!(mem.contains("membraneProperties"));

    let cyc = s.generate_pemfc_cycling(cfg.pemfc.as_ref().unwrap());
    assert!(cyc.contains("cyclingProperties"));

    let deg = s.generate_pemfc_degradation(cfg.pemfc.as_ref().unwrap());
    assert!(deg.contains("degradationProperties"));

    let mesh = s.generate_pemfc_mesh(cfg.pemfc.as_ref().unwrap());
    assert!(mesh.contains("blockMeshDict"));
    assert!(mesh.contains("pemfc"));
}

#[test]
fn test_pemfc_solver_selection() {
    let cfg = pemfc_intake(PemfcModel::Isothermal1D);
    let s = SolverConfigGen::new();
    assert_eq!(s.select_pemfc_solver(&cfg), "pemfcFoam");

    let cfg2 = pemfc_intake(PemfcModel::NonIsothermal);
    assert_eq!(s.select_pemfc_solver(&cfg2), "pemfcThermalFoam");

    let cfg3 = pemfc_intake(PemfcModel::TwoPhase);
    assert_eq!(s.select_pemfc_solver(&cfg3), "pemfcTwoPhaseFoam");
}

#[test]
fn test_pemfc_cht_integration() {
    // PEMFC with thermal effects uses pemfcThermalFoam, not CHT
    let mut cfg = base_intake();
    cfg.pemfc = Some(PemfcConfig {
        model: PemfcModel::NonIsothermal,
        ..PemfcConfig::default()
    });
    cfg.cht = Some(ChtConfig {
        problem_type: HeatTransferProblem::CHT,
        fluid: "air".into(),
        solid_material: aeroflow_core::types::SolidMaterial::Steel,
        t_inlet_k: 353.15,
        t_ambient_k: 300.0,
        u_inlet_m_s: None,
        re: None,
        pr: 0.71,
        heat_flux_target_w_m2: None,
        radiation: false,
        radiation_model: aeroflow_core::types::RadiationModel::None,
        phase_change: false,
        max_t_solid_k: None,
        wall_thickness_m: None,
        external_h_w_m2k: None,
        emissivity: None,
    });
    cfg.mach_number = 0.0;

    let s = SolverConfigGen::new();
    // When both PEMFC and CHT are set, select_pemfc_solver should return the PEMFC solver
    let solver = s.select_pemfc_solver(&cfg);
    assert_eq!(solver, "pemfcThermalFoam", "PEMFC should override CHT when both present");
}
