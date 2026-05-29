use aeroflow_core::types::{ChemistryModel, FluxScheme, HeatTransferProblem, RadiationModel, SolidMaterial, MhdSolver, MhdWallConductivity, ViscosityModel, NonNewtonianConfig, PorousModel, PorousZoneConfig, ParticleInjection, ParticleConfig, ViscoelasticModel, ViscoelasticConfig, MultiphaseModel, MultiphaseConfig, JanafSpecies, CombustionModel, CombustionConfig, CavitationModel, CavitationConfig, SprayBreakupModel, SprayEvaporationModel, SprayConfig, TurbulenceModel, FSIModel, FSICoupling, FSIConfig, AeroacousticConfig, FWHSource, WaveModel, WaveConfig, PhaseChangeModel, PhaseChangeConfig, ActuatorModel, WindTurbineConfig, ElectrostaticConfig, AblationModel, AblationConfig, PropulsionModel, PropulsionConfig, NuclearModel, NuclearConfig, MarineModel, MarineConfig, MlSurrogateModel, MlSurrogateConfig, PemfcModel, PemfcConfig, PemfcFlowField, PemfcCycleProfile, PemfcDegradationModel};
use aeroflow_core::{IntakeConfig, OpenFOAMFormat, Priority, RotatingApproach};

pub struct SolverConfigGen {
    write_format: OpenFOAMFormat,
}

impl Default for SolverConfigGen {
    fn default() -> Self {
        Self::new()
    }
}

impl SolverConfigGen {
    pub fn new() -> Self {
        Self {
            write_format: OpenFOAMFormat::Binary,
        }
    }

    pub fn with_format(format: OpenFOAMFormat) -> Self {
        Self { write_format: format }
    }

    /// Select solver based on flow regime, rotating machinery, and physics modules
    pub fn select_solver(&self, intake: &IntakeConfig) -> &'static str {
        // Hypersonic takes highest priority
        if let Some(ref hyp) = intake.hypersonic {
            if hyp.rarefied { return "dsmcFoam"; }
            if hyp.chemistry != ChemistryModel::None || hyp.two_temperature { return "hy2Foam"; }
            if hyp.mach_inf > 8.0 { return "hy2Foam"; }
            if hyp.real_gas { return "rhoCentralFoam"; }
            return "rhoCentralFoam";
        }
        // Combustion / reacting flow
        if intake.combustion.is_some() {
            return "reactingFoam";
        }
        // Spray / Lagrangian particles
        if intake.spray.is_some() {
            return "sprayFoam";
        }
        // Cavitation — use interPhaseChangeFoam
        if intake.cavitation.is_some() {
            return "interPhaseChangeFoam";
        }
        // Multiphase flow
        if let Some(ref mp) = intake.multiphase {
            match mp.model {
                MultiphaseModel::VOF if intake.mach_number > 0.3 => return "compressibleInterFoam",
                MultiphaseModel::VOF => return "interFoam",
                MultiphaseModel::EulerEuler => return "multiphaseEulerFoam",
                MultiphaseModel::DriftFlux => return "driftFluxFoam",
            }
        }
        // Phase change (melting / solidification)
        if intake.phase_change.is_some() {
            return "solidificationMeltingFoam";
        }
        // Non-Newtonian / viscoelastic
        if intake.non_newtonian.is_some() || intake.viscoelastic.is_some() {
            return "nonNewtonianIcoFoam";
        }
        // Wind turbine actuator
        if intake.wind_turbine.is_some() {
            return "pimpleFoam";
        }
        // Free surface waves
        if intake.wave.is_some() {
            return "interFoam";
        }
        // Rotating machinery
        if let Some(ref rot) = intake.rotating {
            let ma = intake.mach_number;
            return match rot.approach {
                RotatingApproach::MRF => {
                    if ma > 0.3 { "rhoSimpleFoam" } else { "simpleFoam" }
                }
                RotatingApproach::AMI => {
                    if ma > 0.3 { "rhoPimpleFoam" } else { "pimpleFoam" }
                }
            };
        }
        // Default Mach-based selection
        match () {
            _ if intake.mach_number < 0.3 => "simpleFoam",
            _ if intake.mach_number < 0.8 => "rhoSimpleFoam",
            _ if intake.mach_number < 1.2 => "rhoCentralFoam",
            _ => "rhoCentralFoam",
        }
    }

    /// Select turbulence model (from SKILL.md Step 3)
    pub fn select_turbulence_model(&self, intake: &IntakeConfig) -> &'static str {
        match (intake.reynolds_number, intake.mach_number, &intake.priority) {
            (re, _, Priority::Speed) if re > 1e6 => "SpalartAllmaras",
            (re, _, Priority::Accuracy) if re > 1e6 => "kOmegaSST",
            (_, ma, _) if ma > 0.5 => "kOmegaSST",
            _ => "kOmegaSST",
        }
    }

    /// Compute relaxation factors from priority (from SKILL.md Rule P5)
    pub fn relaxation_factors(&self, intake: &IntakeConfig) -> (f64, f64) {
        match intake.priority {
            Priority::Speed => (0.8, 0.4),
            Priority::Balanced => (0.7, 0.3),
            Priority::Accuracy => (0.5, 0.2),
        }
    }

    /// Generate controlDict content
    pub fn generate_control_dict(&self, intake: &IntakeConfig) -> String {
        let solver = self.select_solver(intake);
        let format = self.write_format.label();
        let two_pi = 2.0 * std::f64::consts::PI;

        let (end_time_str, delta_t_str, write_interval_str) = if let Some(ref rot) = intake.rotating {
            match rot.approach {
                RotatingApproach::MRF => {
                    let iters = match intake.priority {
                        Priority::Speed => 1500,
                        Priority::Balanced => 3000,
                        Priority::Accuracy => 5000,
                    };
                    (iters.to_string(), "1".to_string(), "500".to_string())
                }
                RotatingApproach::AMI => {
                    let omega_rad = rot.rpm * two_pi / 60.0;
                    let blade_pitch = two_pi / rot.num_blades as f64;
                    let dt = (blade_pitch / omega_rad) / 20.0;
                    let steps_per_rev = (two_pi / (omega_rad * dt)).ceil() as u64;
                    (format!("{}", steps_per_rev * 5), format!("{:.6e}", dt), format!("{}", (steps_per_rev / 20).max(1)))
                }
            }
        } else {
            let iters = match intake.priority {
                Priority::Speed => 1500,
                Priority::Balanced => 3000,
                Priority::Accuracy => 5000,
            };
            (iters.to_string(), "1".to_string(), "500".to_string())
        };

        let functions = self.generate_forces_functions(intake);
        format!(
            r#"FoamFile {{ version 2.0; format {}; class dictionary; object controlDict; }}
application     {};
startFrom       startTime;
startTime       0;
stopAt          endTime;
endTime         {};
deltaT          {};
writeControl    adjustableRunTime;
writeInterval   {};
purgeWrite      3;
writePrecision  8;
writeCompression on;
timeFormat      general;
timePrecision   6;
runTimeModifiable true;
functions
{{
{}
}}
"#, format, solver, end_time_str, delta_t_str, write_interval_str, functions)
    }

    /// Generate forceCoeffs function object(s) for the case, including propeller variants
    fn generate_forces_functions(&self, intake: &IntakeConfig) -> String {
        let base = r#"    forceCoeffs
    {{
        type            forceCoeffs;
        libs            (forces);
        patches         (blade);
        rho             rhoInf;
        rhoInf          1.225;
        CofR            (0 0 0);
        liftDir         (0 0 1);
        dragDir         (1 0 0);
        pitchAxis       (0 1 0);
        magUInf         9.15;
        lRef            0.3;
        Aref            0.3;
        writeControl    timeStep;
        writeInterval   10;
    }}"#;
        if let Some(ref rot) = intake.rotating {
            let patches = format!("(blade{})", if rot.num_blades > 1 {
                (1..=rot.num_blades).map(|i| format!(" blade{}", i)).collect::<String>()
            } else { "".into() });
            let d = rot.diameter_m.unwrap_or(0.3);
            let area = std::f64::consts::PI * d * d / 4.0;
            format!(
                r#"    propellerForces
    {{
        type            forceCoeffs;
        libs            (forces);
        patches         {};
        rho             rhoInf;
        rhoInf          1.225;
        CofR            ({} {} {});
        liftDir         ({} {} {});
        dragDir         ({} {} {});
        pitchAxis       (0 1 0);
        magUInf         {};
        lRef            {};
        Aref            {};
        writeControl    timeStep;
        writeInterval   10;
    }}"#,
                patches,
                rot.origin[0], rot.origin[1], rot.origin[2],
                rot.axis[0], rot.axis[1], rot.axis[2],
                if rot.axis[0].abs() > 0.5 { "1" } else { "0" },
                if rot.axis[1].abs() > 0.5 { "1" } else { "0" },
                if rot.axis[2].abs() > 0.5 { "1" } else { "0" },
                intake.mach_number * 340.3,
                d, area
            )
        } else {
            base.to_string()
        }
    }

    /// Generate MRFProperties dictionary for MRF approach
    pub fn generate_mrf_properties(&self, intake: &IntakeConfig) -> Option<String> {
        let rot = intake.rotating.as_ref()?;
        if rot.approach != RotatingApproach::MRF { return None; }
        let omega = rot.rpm * std::f64::consts::TAU / 60.0;
        Some(format!(
            r#"FoamFile {{ version 2.0; format ascii; class dictionary; object MRFProperties; }}

MRF1
{{
    cellZone        rotatingZone;
    active          yes;
    nonRotatingPatches ();
    origin          ({} {} {});
    axis            ({} {} {});
    omega           [0 0 -1 0 1 0 0] {};
}}
"#,
            rot.origin[0], rot.origin[1], rot.origin[2],
            rot.axis[0], rot.axis[1], rot.axis[2],
            omega
        ))
    }

    /// Generate dynamicMeshDict for AMI (sliding mesh) approach
    pub fn generate_dynamic_mesh_dict(&self, intake: &IntakeConfig) -> Option<String> {
        let rot = intake.rotating.as_ref()?;
        if rot.approach != RotatingApproach::AMI { return None; }
        let omega = rot.rpm * std::f64::consts::TAU / 60.0;
        Some(format!(
            r#"FoamFile {{ version 2.0; format ascii; class dictionary; object dynamicMeshDict; }}

dynamicFvMesh   dynamicMotionSolverFvMesh;

motionSolverLibs ("libfvMotionSolvers.so");

solver          solidBody;
solidBodyMotionFunction  rotatingMotion;
rotatingMotionCoeffs
{{
    origin      ({} {} {});
    axis        ({} {} {});
    omega       {};
}}
"#,
            rot.origin[0], rot.origin[1], rot.origin[2],
            rot.axis[0], rot.axis[1], rot.axis[2],
            omega
        ))
    }

    /// Generate topoSetDict for creating the rotating cell zone
    pub fn generate_topo_set_dict(&self, intake: &IntakeConfig) -> Option<String> {
        let rot = intake.rotating.as_ref()?;
        let radius = rot.diameter_m.unwrap_or(1.0) / 2.0;

        Some(format!(
            r#"FoamFile {{ version 2.0; format ascii; class dictionary; object topoSetDict; }}

actions
(
    {{
        name    rotatingZone;
        type    cellSet;
        action  new;
        source  cylinderToCell;
        sourceInfo
        {{
            p1      ({} {} {});
            p2      ({} {} {});
            radius  {};
        }}
    }}
    {{
        name    rotatingZone;
        type    cellZoneSet;
        action  new;
        source  setToCellZone;
        sourceInfo
        {{
            set     rotatingZone;
        }}
    }}
);
"#,
            rot.origin[0], rot.origin[1], rot.origin[2],
            rot.origin[0], rot.origin[1], rot.origin[2] + 0.01,
            radius
        ))
    }

    /// Generate fvSchemes content with second-order schemes suitable for the flow regime
    pub fn generate_fv_schemes(&self) -> String {
        self.generate_fv_schemes_for(None)
    }

    pub fn generate_fv_schemes_for(&self, intake: Option<&IntakeConfig>) -> String {
        let is_ami = intake.and_then(|i| i.rotating.as_ref())
            .is_some_and(|r| r.approach == RotatingApproach::AMI);
        let ddt = if is_ami { "Euler" } else { "steadyState" };
        format!(
            r#"FoamFile {{ version 2.0; format ascii; class dictionary; object fvSchemes; }}

ddtSchemes
{{
    default         {};
}}

gradSchemes
{{
    default         Gauss linear;
}}

divSchemes
{{
    default         none;
    div(phi,U)      Gauss linearUpwindV grad(U);
    div(phi,k)      Gauss upwind;
    div(phi,omega)  Gauss upwind;
    div(phi,epsilon) Gauss upwind;
    div(phi,R)      Gauss upwind;
    div((nuEff*dev2(T(grad(U))))) Gauss linear;
}}

laplacianSchemes
{{
    default         Gauss linear corrected;
}}

interpolationSchemes
{{
    default         linear;
}}

snGradSchemes
{{
    default         corrected;
}}

wallDist
{{
    method          meshWave;
}}
"#, ddt
        )
    }

    /// Generate fvSolution content based on intake config priority
    pub fn generate_fv_solution(&self, intake: &IntakeConfig) -> String {
        let (urf_p, urf_u) = self.relaxation_factors(intake);
        let n_non_orth = match intake.priority {
            Priority::Speed => 0,
            Priority::Balanced => 2,
            Priority::Accuracy => 3,
        };
        let residual_target = match intake.priority {
            Priority::Speed => "1e-4",
            Priority::Balanced => "1e-5",
            Priority::Accuracy => "1e-6",
        };

        format!(
            r#"FoamFile {{ version 2.0; format ascii; class dictionary; object fvSolution; }}

solvers
{{
    p
    {{
        solver          GAMG;
        tolerance       {};
        relTol          0.01;
        smoother        GaussSeidel;
    }}
    U
    {{
        solver          smoothSolver;
        smoother        symGaussSeidel;
        tolerance       {};
        relTol          0;
        nSweeps         3;
    }}
    k
    {{
        $U;
        tolerance       {};
        relTol          0;
        nSweeps         3;
    }}
    omega
    {{
        $U;
        tolerance       {};
        relTol          0;
        nSweeps         3;
    }}
}}

{}

relaxationFactors
{{
    fields
    {{
        p               {};
    }}
    equations
    {{
        U               {};
        k               {};
        omega           {};
    }}
}}
"#,
            residual_target, residual_target, residual_target, residual_target,
            if intake.rotating.as_ref().is_some_and(|r| r.approach == RotatingApproach::AMI) {
                format!(
                    r#"PIMPLE
{{
    nNonOrthogonalCorrectors {};
    nOuterCorrectors 3;
    nCorrectors      2;
    residualControl
    {{
        U              {};
        p              {};
        k              {};
        omega          {};
    }}
    pRefPoint       (0 0 0);
    pRefValue       0;
}}"#,
                    n_non_orth, residual_target, residual_target, residual_target, residual_target
                )
            } else {
                format!(
                    r#"SIMPLE
{{
    nNonOrthogonalCorrectors {};
    residualControl
    {{
        U              {};
        p              {};
        k              {};
        omega          {};
    }}
    pRefPoint       (0 0 0);
    pRefValue       0;
}}"#,
                    n_non_orth, residual_target, residual_target, residual_target, residual_target
                )
            },
            urf_p, urf_u, urf_u, urf_u
        )
    }

    // ── Hypersonic Methods ───────────────────────────────────────

    /// Select flux scheme for hypersonic shock capturing
    pub fn select_hypersonic_flux(&self, intake: &IntakeConfig) -> &'static str {
        intake.hypersonic.as_ref().map_or("Kurganov", |h| match h.flux_scheme {
            FluxScheme::AUSMPlus => "AUSM+",
            FluxScheme::Kurganov => "Kurganov",
        })
    }

    /// Generate thermophysicalProperties for real gas / JANAF thermodynamics
    pub fn generate_thermo_dict(&self, intake: &IntakeConfig) -> Option<String> {
        let hyp = intake.hypersonic.as_ref()?;
        if !hyp.real_gas && hyp.chemistry == ChemistryModel::None { return None; }
        let thermo_type = if hyp.two_temperature {
            "hePsiThermo"
        } else if hyp.chemistry != ChemistryModel::None {
            "multiComponentMixture"
        } else {
            "hePsiThermo"
        };
        let mixture = if hyp.chemistry != ChemistryModel::None {
            "multiComponentMixture"
        } else {
            "pureMixture"
        };
        Some(format!(
            r#"FoamFile {{ version 2.0; format ascii; class dictionary; object thermophysicalProperties; }}

thermoType
{{
    type            {};
    mixture         {};
    transport       sutherland;
    thermo          janaf;
    equationOfState perfectGas;
    specie          specie;
    energy          sensibleEnthalpy;
}}

mixture
{{
    specie
    {{
        molWeight    28.966;
    }}
    thermodynamics
    {{
        Tlow    200;
        Thigh  6000;
        Tcommon 1000;
        highCpCoeffs  (3.697578 6.135197e-4 -1.26e-7 1.745e-11 -6.56e-16 -1233.93 2.05);
        lowCpCoeffs   (3.298677 1.408240e-3 -3.96e-6 6.113e-9 -2.032e-12 -1020.90 3.95);
    }}
    transport
    {{
        As      1.458e-6;
        Ts      110.4;
    }}
}}
"#, thermo_type, mixture
        ))
    }

    /// Generate chemistryProperties for chemical nonequilibrium (5-species Park model)
    pub fn generate_chemistry_properties(&self, intake: &IntakeConfig) -> Option<String> {
        let hyp = intake.hypersonic.as_ref()?;
        if hyp.chemistry == ChemistryModel::None { return None; }
        let n_species = match hyp.chemistry {
            ChemistryModel::Park5Species => 5,
            ChemistryModel::Park11Species => 11,
            _ => 0,
        };
        if n_species == 0 { return None; }
        let ode_solver = if hyp.two_temperature { "SIBS" } else { "EulerImplicit" };
        Some(format!(
            r#"FoamFile {{ version 2.0; format ascii; class dictionary; object chemistryProperties; }}

chemistryType
{{
    solver            ode;
    method            {};
}}

chemistrySolver
{{
    tolerance         1e-9;
}}

{}species
(
    N2
    O2
    NO
    N
    O{}
);
"#, ode_solver,
            if n_species == 11 { "\n    N+\n    O+\n    NO+\n    e-\n    Ar\n" } else { "" },
            if n_species == 11 { "\n    N+\n    O+\n    NO+\n    e-\n    Ar" } else { "" }
        ))
    }

    /// Generate radiationProperties for fvDOM (radiative heat transfer at Ma > 15)
    pub fn generate_radiation_dict(&self, intake: &IntakeConfig) -> Option<String> {
        let hyp = intake.hypersonic.as_ref()?;
        if hyp.mach_inf < 15.0 { return None; }
        Some(r#"FoamFile { version 2.0; format ascii; class dictionary; object radiationProperties; }

radiation
{
    radiationModel  fvDOM;
    absorptionEmissionModel constantAbsorptionEmission;
    constantAbsorptionEmissionCoeffs
    {
        absorptivity    absorptivity [0 -1 0 0 0 0 0] 0.5;
        emissivity      emissivity   [0 0 0 0 0 0 0]  0.5;
    }
    solverFreq      1;
}
"#.to_string())
    }

    /// Generate hypersonic-specific fvSchemes (TVD limiters, flux scheme)
    pub fn generate_hypersonic_fv_schemes(&self, intake: &IntakeConfig) -> Option<String> {
        let _ = intake.hypersonic.as_ref()?;
        let flux = self.select_hypersonic_flux(intake);
        Some(format!(
            r#"fluxScheme      {};

divSchemes
{{
    default         none;
    div(phi,U)      Gauss limitedLinearV 1;
    div(phi,e)      Gauss limitedLinear 1;
    div(phi,K)      Gauss limitedLinear 1;
    div(phiv,p)     Gauss limitedLinear 1;
    div(phi,Ekp)    Gauss limitedLinear 1;
    div(phi,k)      Gauss upwind;
    div(phi,omega)  Gauss upwind;
    div((nuEff*dev2(T(grad(U))))) Gauss linear;
}}
"#, flux
        ))
    }

    /// Generate wallHeatFlux function object for aerothermodynamic monitoring
    pub fn generate_heat_flux_monitor(&self, intake: &IntakeConfig) -> Option<String> {
        let _ = intake.hypersonic.as_ref()?;
        Some(r#"    heatFlux
    {
        type            wallHeatFlux;
        libs            ("libfieldFunctionObjects.so");
        patches         (wall);
        writeControl    timeStep;
        writeInterval   1;
    }
"#.to_string())
    }

    // ── CHT (Conjugate Heat Transfer) Methods ───────────────────

    /// Select solver for CHT / thermal problems
    pub fn select_cht_solver(&self, intake: &IntakeConfig) -> &'static str {
        if let Some(ref cht) = intake.cht {
            match cht.problem_type {
                HeatTransferProblem::ForcedConvection if intake.mach_number < 0.3 => "buoyantBoussinesqSimpleFoam",
                HeatTransferProblem::NaturalConvection => "buoyantSimpleFoam",
                HeatTransferProblem::CHT if cht.phase_change => "icoReactingMultiphaseInterFoam",
                HeatTransferProblem::CHT => "chtMultiRegionSimpleFoam",
                HeatTransferProblem::Radiation => "buoyantSimpleFoam",
                _ => "buoyantSimpleFoam",
            }
        } else {
            self.select_solver(intake)
        }
    }

    /// Generate thermophysicalProperties for the fluid region of a CHT case
    pub fn generate_cht_fluid_thermo(&self, intake: &IntakeConfig) -> Option<String> {
        let cht = intake.cht.as_ref()?;
        let pr = cht.pr;
        let is_boussinesq = matches!(cht.problem_type, HeatTransferProblem::ForcedConvection)
            && intake.mach_number < 0.3;
        let (thermo_type, mixture, transport, thermo, eos, energy) = if is_boussinesq {
            ("heRhoThermo", "pureMixture", "const", "eConst", "rhoConst", "sensibleInternalEnergy")
        } else {
            ("hePsiThermo", "pureMixture", "sutherland", "hConst", "perfectGas", "sensibleEnthalpy")
        };
        Some(format!(
            r#"FoamFile {{ version 2.0; format {}; class dictionary; object thermophysicalProperties; }}

thermoType
{{
    type            {};
    mixture         {};
    transport       {};
    thermo          {};
    equationOfState {};
    energy          {};
}}

mixture
{{
    specie
    {{
        molWeight   28.97;
    }}
    equationOfState
    {{
        rho         1.225;
    }}
    thermodynamics
    {{
        Cp         1005.0;
        Hf          0.0;
    }}
    transport
    {{
        mu          1.8e-5;
        Pr           {};
    }}
}}
"#, self.write_format.label(), thermo_type, mixture, transport, thermo, eos, energy, pr)
        )
    }

    /// Generate thermophysicalProperties for the solid region of a CHT case
    pub fn generate_cht_solid_thermo(&self, intake: &IntakeConfig) -> Option<String> {
        let cht = intake.cht.as_ref()?;
        let (rho, cp, kappa) = cht.solid_material.thermal_properties();
        let mol_weight = match cht.solid_material {
            SolidMaterial::Steel => 55.85,
            SolidMaterial::Aluminum => 26.98,
            SolidMaterial::Copper => 63.55,
            SolidMaterial::Ceramic => 40.08,
            SolidMaterial::CFRP => 12.01,
            SolidMaterial::Inconel => 58.69,
            SolidMaterial::Custom(_, _, _) => 55.0,
        };
        Some(format!(
            r#"FoamFile {{ version 2.0; format {}; class dictionary; object thermophysicalProperties; }}

thermoType
{{
    type            heSolidThermo;
    mixture         pureMixture;
    transport       constIso;
    thermo          eConst;
    equationOfState rhoConst;
    specie          specie;
    energy          sensibleInternalEnergy;
}}

mixture
{{
    specie
    {{
        molWeight   {};
    }}
    equationOfState
    {{
        rho         {};
    }}
    thermodynamics
    {{
        Cv          {};
        Hf          0.0;
    }}
    transport
    {{
        kappa       {};
    }}
}}
"#, self.write_format.label(), mol_weight, rho, cp, kappa)
        )
    }

    /// Generate radiationProperties for a CHT case (fluid region)
    pub fn generate_cht_radiation_dict(&self, intake: &IntakeConfig) -> Option<String> {
        let cht = intake.cht.as_ref()?;
        if !cht.radiation { return None; }
        let rad_model = match cht.radiation_model {
            RadiationModel::None => return None,
            RadiationModel::P1 => "P1",
            RadiationModel::FvDOM => "fvDOM",
            RadiationModel::ViewFactor => "viewFactor",
            RadiationModel::Rosseland => "Rosseland",
        };
        let coeffs = match cht.radiation_model {
            RadiationModel::FvDOM => {
                r#"fvDOMCoeffs
{
    nPhi    3;
    nTheta  3;
    maxIter 4;
    tolerance 1e-3;
}"#
            }
            _ => "",
        };
        Some(format!(
            r#"FoamFile {{ version 2.0; format {}; class dictionary; object radiationProperties; }}

radiationModel  {};

{}
absorptionEmissionModel constantAbsorptionEmission;
scatterModel    none;
sootModel       none;
"#, self.write_format.label(), rad_model, coeffs)
        )
    }

    /// Generate coupled temperature boundary condition for a fluid-solid interface
    pub fn generate_cht_interface_bcs(&self, intake: &IntakeConfig, interface_patch: &str) -> Option<(String, String)> {
        let _ = intake.cht.as_ref()?;
        let fluid_bc = format!(
            r#"{}_fluid
{{
    type        compressible::turbulentTemperatureCoupledBaffleMixed;
    Tnbr        T;
    kappaMethod fluidThermo;
    value       $internalField;
}}
"#, interface_patch);
        let solid_bc = format!(
            r#"{}_solid
{{
    type        compressible::turbulentTemperatureCoupledBaffleMixed;
    Tnbr        T;
    kappaMethod solidThermo;
    value       $internalField;
}}
"#, interface_patch);
        Some((fluid_bc, solid_bc))
    }

    /// Generate T boundary condition for an external CHT wall (convective + radiation)
    pub fn generate_cht_external_wall_bc(&self, intake: &IntakeConfig) -> Option<String> {
        let cht = intake.cht.as_ref()?;
        let h = cht.external_h_w_m2k.unwrap_or(15.0);
        let t_amb = cht.t_ambient_k;
        let eps = cht.emissivity.unwrap_or(0.85);
        Some(format!(
            r#"outer_wall
{{
    type            externalWallHeatFluxTemperature;
    mode            coefficient;
    Ta              uniform {};
    h               uniform {};
    emissivity      {};
    thicknessLayers ( 0.002 );
    kappaLayers     ( 16.0  );
    value           uniform {};
}}
"#, t_amb, h, eps, t_amb)
        )
    }

    // ── MHD / Electromagnetic Methods ──────────────────────────

    /// Select solver for MHD cases
    pub fn select_mhd_solver(&self, intake: &IntakeConfig) -> &'static str {
        if let Some(ref mhd) = intake.mhd {
            match mhd.solver {
                MhdSolver::MhdFoam => "mhdFoam",
                MhdSolver::MagneticFoam => "magneticFoam",
                MhdSolver::Custom => "mhdFoam",
            }
        } else {
            self.select_solver(intake)
        }
    }

    /// Compute Hartmann number Ha = B * L * sqrt(sigma / mu)
    pub fn hartmann_number(b0: f64, l_ref: f64, sigma: f64, mu_visc: f64) -> f64 {
        b0 * l_ref * (sigma / mu_visc).sqrt()
    }

    /// Compute magnetic Reynolds number Rm = mu0 * sigma * U * L
    pub fn magnetic_reynolds_number(u: f64, l_ref: f64, sigma: f64) -> f64 {
        const MU0: f64 = 1.25663706212e-6;
        MU0 * sigma * u * l_ref
    }

    /// Generate transportProperties for MHD with sigma and mu (permeability)
    pub fn generate_mhd_transport_props(&self, intake: &IntakeConfig, nu: f64) -> Option<String> {
        let mhd = intake.mhd.as_ref()?;
        Some(format!(
            r#"FoamFile {{ version 2.0; format {}; class dictionary; object transportProperties; }}

transportModel  Newtonian;
nu              nu [0 2 -1 0 0 0 0] {:.6e};

sigma           sigma [-1 -3 3 0 0 2 0] {:.6e};
mu              mu [1 1 -2 0 0 -2 0] {:.6e};
"#, self.write_format.label(), nu, mhd.sigma_s_m, mhd.mu_permeability_h_m)
        )
    }

    /// Generate Lorentz force fvOptions source for low-Rm MHD
    pub fn generate_mhd_fv_options(&self, intake: &IntakeConfig) -> Option<String> {
        let mhd = intake.mhd.as_ref()?;
        if !mhd.low_rm { return None; }
        let _b = mhd.b0_tesla;
        let _sigma = mhd.sigma_s_m;
        Some(format!(
            r#"FoamFile {{ version 2.0; format {}; class dictionary; object fvOptions; }}

lorentzForce
{{
    type            vectorExplicitSource;
    volumeMode      absolute;
    selectionMode   all;

    // J×B = σ[(U·B)B - B²U] — linearised damping term
    // Applied as explicit source: Su = -sigma*B²*U + sigma*(U·B)*B
    fields          (U);
}}
"#, self.write_format.label())
        )
    }

    /// Generate B (magnetic field) initial condition based on wall conductivity type
    pub fn generate_mhd_b_field(&self, intake: &IntakeConfig) -> Option<String> {
        let mhd = intake.mhd.as_ref()?;
        let b0 = mhd.b0_tesla;
        let (wall_type, inlet_type, outlet_type) = match mhd.wall_conductivity {
            MhdWallConductivity::Conducting => ("fixedValue", "fixedValue", "zeroGradient"),
            MhdWallConductivity::Insulating => ("zeroGradient", "fixedValue", "zeroGradient"),
            MhdWallConductivity::Mixed => ("fixedValue", "fixedValue", "zeroGradient"),
        };
        Some(format!(
            r#"FoamFile {{ version 2.0; format {}; class dictionary; class volVectorField; object B; }}

dimensions       [1 0 -2 0 0 -1 0];
internalField    uniform (0 {} 0);

boundaryField
{{
    inlet
    {{
        type            {};
        value           uniform (0 {} 0);
    }}
    outlet
    {{
        type            {};
    }}
    walls
    {{
        type            {};
        value           uniform (0 {} 0);
    }}
}}
"#, self.write_format.label(), b0, inlet_type, b0, outlet_type, wall_type, b0)
        )
    }

    /// Generate DBD plasma actuator fvOptions source term
    pub fn generate_plasma_actuator_fv_options(&self, intake: &IntakeConfig) -> Option<String> {
        let mhd = intake.mhd.as_ref()?;
        let plasma = mhd.plasma_actuator.as_ref()?;
        let f_body = plasma.body_force_n_m3;
        Some(format!(
            r#"FoamFile {{ version 2.0; format {}; class dictionary; object fvOptions; }}

bodyForceModel
{{
    type            vectorSemiImplicitSource;
    volumeMode      absolute;
    selectionMode   cellZone;
    cellZone        actuatorZone;
    injectionRateSuSp
    {{
        U           (( {:.6e} 0 0 ) 0);
    }}
}}
"#, self.write_format.label(), f_body)
        )
    }

    /// Generate porous media Darcy-Forchheimer fvOptions dictionary
    pub fn generate_porous_fv_options(&self, porous: &PorousZoneConfig) -> String {
        let d = format!(
            "d    ({:.8e} {:.8e} {:.8e});",
            porous.d_coeffs[0], porous.d_coeffs[1], porous.d_coeffs[2]
        );
        let f = if porous.model == PorousModel::DarcyForchheimer {
            format!(
                "f    ({:.8e} {:.8e} {:.8e});",
                porous.f_coeffs[0], porous.f_coeffs[1], porous.f_coeffs[2]
            )
        } else {
            "f    (0 0 0);".into()
        };
        format!(
            r#"/*--------------------------------*- C++ -*----------------------------------*\
  =========                 |
  \\      /  F ield         | OpenFOAM: The Open Source CFD Toolbox
   \\    /   O peration     |
    \\  /    A nd           | www.openfoam.com
     \\/     M anipulation  |
\*---------------------------------------------------------------------------*/
FoamFile {{
    version     2.0;
    format      ascii;
    class       dictionary;
    location    "constant";
    object      fvOptions;
}}
// * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * //

porousSource1
{{
    type            explicitPorositySource;
    active          yes;
    explicitPorositySourceCoeffs
    {{
        selectionMode   cellZone;
        cellZone        {};
        type            DarcyForchheimer;
        DarcyForchheimerCoeffs
        {{
            {}
            {}
            coordinateSystem
            {{
                type        cartesian;
                origin      (0 0 0);
                coordinateRotation
                {{
                    type        axesRotation;
                    e1          (1 0 0);
                    e2          (0 1 0);
                }}
            }}
        }}
    }}
}}
"#,
            porous.cell_zone, d, f
        )
    }

    /// Generate non-Newtonian transportProperties dictionary
    pub fn generate_non_newtonian_transport(&self, nn: &NonNewtonianConfig) -> String {
        let model_str = match nn.model {
            ViscosityModel::Newtonian => "Newtonian",
            ViscosityModel::PowerLaw => "powerLaw",
            ViscosityModel::CrossPowerLaw => "CrossPowerLaw",
            ViscosityModel::BirdCarreau => "BirdCarreau",
            ViscosityModel::HerschelBulkley => "HerschelBulkley",
            ViscosityModel::Casson => "Casson",
        };

        let model_block = match nn.model {
            ViscosityModel::Newtonian => String::new(),
            ViscosityModel::PowerLaw => format!(
                "powerLawCoeffs\n{{\n    k       {};\n    n       {};\n    nuMin    {};\n    nuMax   {};\n}}",
                nn.k, nn.n, nn.nu_min, nn.nu_max
            ),
            ViscosityModel::CrossPowerLaw => format!(
                "CrossPowerLawCoeffs\n{{\n    nu0     {};\n    nuInf   {};\n    m       {};\n    n       {};\n}}",
                nn.nu0, nn.nu_inf, nn.k, nn.n
            ),
            ViscosityModel::BirdCarreau => format!(
                "BirdCarreauCoeffs\n{{\n    nu0     {};\n    nuInf   {};\n    k       {};\n    n       {};\n}}",
                nn.nu0, nn.nu_inf, nn.k, nn.n
            ),
            ViscosityModel::HerschelBulkley => format!(
                "HerschelBulkleyCoeffs\n{{\n    k       {};\n    n       {};\n    tau0    {};\n    nu0     {};\n}}",
                nn.k, nn.n, nn.tau0, nn.nu0
            ),
            ViscosityModel::Casson => format!(
                "CassonCoeffs\n{{\n    nu0     {};\n    tau0    {};\n}}",
                nn.nu0, nn.tau0
            ),
        };

        format!(
            r#"/*--------------------------------*- C++ -*----------------------------------*\
  =========                 |
  \\      /  F ield         | OpenFOAM: The Open Source CFD Toolbox
   \\    /   O peration     |
    \\  /    A nd           | www.openfoam.com
     \\/     M anipulation  |
\*---------------------------------------------------------------------------*/
FoamFile {{
    version     2.0;
    format      ascii;
    class       dictionary;
    location    "constant";
    object      transportProperties;
}}
// * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * //

transportModel  {};
nu              {};{}
"#,
            model_str,
            nn.nu0,
            if model_block.is_empty() {
                String::new()
            } else {
                format!("\n{}", model_block)
            }
        )
    }

    /// Generate particle injection DPM dictionary
    pub fn generate_particle_injection(&self, pc: &ParticleConfig, cloud_name: &str) -> String {
        let inj_type = match pc.injection {
            ParticleInjection::PatchInjection => "patchInjection",
            ParticleInjection::ConeInjection => "coneInjection",
            ParticleInjection::ManualInjection => "manualInjection",
        };
        format!(
            r#"/*--------------------------------*- C++ -*----------------------------------*\
  =========                 |
  \\      /  F ield         | OpenFOAM: The Open Source CFD Toolbox
   \\    /   O peration     |
    \\  /    A nd           | www.openfoam.com
     \\/     M anipulation  |
\*---------------------------------------------------------------------------*/
FoamFile {{
    version     2.0;
    format      ascii;
    class       dictionary;
    location    "constant";
    object      {}Properties;
}}
// * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * //

{0}Coeffs
{{
    particleDiameter    {:e};
    massFlowRate        {:e};
    U0                  {:e};
    T0                  {};
    rhoParticle         {};
}}

injectionModels
{{
    model1
    {{
        type            {};
        InjectionModel  {}
        {{
            type            {}
        }}
        positions        (0 0 0);
        diameters       constant {:e};
    }}
}}
"#,
            cloud_name,
            pc.diameter_m,
            pc.mass_flow_kg_s,
            pc.velocity_m_s,
            pc.temperature_k,
            pc.material_density_kg_m3,
            inj_type,
            inj_type,
            inj_type,
            pc.diameter_m,
        )
    }

    /// Generate viscoelastic transportProperties dictionary
    pub fn generate_viscoelastic_transport(&self, ve: &ViscoelasticConfig) -> String {
        let model_str = match ve.model {
            ViscoelasticModel::OldroydB => "Oldroyd-B",
            ViscoelasticModel::Giesekus => "Giesekus",
        };
        format!(
            r#"/*--------------------------------*- C++ -*----------------------------------*\
  =========                 |
  \\      /  F ield         | OpenFOAM: The Open Source CFD Toolbox
   \\    /   O peration     |
    \\  /    A nd           | www.openfoam.com
     \\/     M anipulation  |
\*---------------------------------------------------------------------------*/
FoamFile {{
    version     2.0;
    format      ascii;
    class       dictionary;
    location    "constant";
    object      transportProperties;
}}
// * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * //

transportModel  viscoelastic;
viscoelasticCoeffs
{{
    model               {};
    relaxationTime      {};
    solventViscosityRatio {};
    mobilityFactor      {};
}}
"#,
            model_str,
            ve.relaxation_time_s,
            ve.solvent_viscosity_ratio,
            ve.mobility_factor,
        )
    }

    /// Generate multiphase transportProperties dictionary
    pub fn generate_multiphase_transport(&self, mc: &MultiphaseConfig) -> String {
        let model_str = match mc.model {
            MultiphaseModel::VOF => "VOF",
            MultiphaseModel::EulerEuler => "Eulerian",
            MultiphaseModel::DriftFlux => "driftFlux",
        };

        let mut phase_block = String::new();
        for i in 0..mc.n_phases as usize {
            let name = if i < mc.phase_names.len() {
                &mc.phase_names[i]
            } else {
                &format!("phase{}", i + 1)
            };
            phase_block.push_str(&format!(
                "{}\n{{\n    type    fluid;\n    nu      {};\n    rho     {};\n}}\n\n",
                name,
                1e-6 + (i as f64) * 1e-6,
                1000.0 + (i as f64) * 100.0,
            ));
        }

        format!(
            r#"/*--------------------------------*- C++ -*----------------------------------*\
  =========                 |
  \\      /  F ield         | OpenFOAM: The Open Source CFD Toolbox
   \\    /   O peration     |
    \\  /    A nd           | www.openfoam.com
     \\/     M anipulation  |
\*---------------------------------------------------------------------------*/
FoamFile {{
    version     2.0;
    format      ascii;
    class       dictionary;
    location    "constant";
    object      transportProperties;
}}
// * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * //

transportModel  {};
sigma           {};
phases
{{
{}
}}
"#,
            model_str, mc.surface_tension_n_m, phase_block
        )
    }

    // ── JANAF Thermochemistry ─────────────────────────────────────
    /// Generate JANAF thermochemistry dictionary for a species
    pub fn generate_janaf_thermo(&self, species: &JanafSpecies) -> String {
        let (name, coeffs_low, coeffs_high) = match species {
            JanafSpecies::N2 => ("N2", janaf_n2_low(), janaf_n2_high()),
            JanafSpecies::O2 => ("O2", janaf_o2_low(), janaf_o2_high()),
            JanafSpecies::NO => ("NO", janaf_no_low(), janaf_no_high()),
            JanafSpecies::N => ("N", janaf_n_low(), janaf_n_high()),
            JanafSpecies::O => ("O", janaf_o_low(), janaf_o_high()),
            JanafSpecies::Ar => ("Ar", janaf_ar_low(), janaf_ar_high()),
            JanafSpecies::CO2 => ("CO2", janaf_co2_low(), janaf_co2_high()),
            JanafSpecies::H2O => ("H2O", janaf_h2o_low(), janaf_h2o_high()),
            JanafSpecies::Custom(n, l, h) => (n.as_str(), *l, *h),
        };
        format!(
            r#"    {}
    {{
        type        janaf;
        CpCoeffs    <8> ({:.10e} {:.10e} {:.10e} {:.10e} {:.10e} {:.10e} {:.10e} {:.10e});
        HiCoeffs    <8> ({:.10e} {:.10e} {:.10e} {:.10e} {:.10e} {:.10e} {:.10e} {:.10e});
    }}
"#,
            name,
            coeffs_low[0], coeffs_low[1], coeffs_low[2], coeffs_low[3],
            coeffs_low[4], coeffs_low[5], coeffs_low[6], 0.0,
            coeffs_high[0], coeffs_high[1], coeffs_high[2], coeffs_high[3],
            coeffs_high[4], coeffs_high[5], coeffs_high[6], 0.0,
        )
    }

    /// Generate chemistryProperties dictionary for combustion
    pub fn generate_combustion_chemistry(&self, cc: &CombustionConfig) -> String {
        let model_str = match cc.model {
            CombustionModel::EDC => "EDC",
            CombustionModel::LaminarFlamelet => "laminarFlamelet",
            CombustionModel::PaSR => "PaSR",
            CombustionModel::WellStirredReactor => "wellStirredReactor",
        };
        format!(
            r#"/*--------------------------------*- C++ -*----------------------------------*\
  =========                 |
  \\      /  F ield         | OpenFOAM: The Open Source CFD Toolbox
   \\    /   O peration     |
    \\  /    A nd           | www.openfoam.com
     \\/     M anipulation  |
\*---------------------------------------------------------------------------*/
FoamFile {{
    version     2.0;
    format      ascii;
    class       dictionary;
    location    "constant";
    object      chemistryProperties;
}}
// * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * //

chemistryType
{{
    solver      {};{}
}}

chemistry
{{
    chemistrySolver    {}
    initialChemicalTimeStep  1e-7;
}}
"#,
            model_str,
            if cc.model == CombustionModel::EDC {
                String::new()
            } else {
                format!("\n    method      {}", model_str)
            },
            model_str,
        )
    }

    /// Generate cavitationProperties dictionary
    pub fn generate_cavitation_properties(&self, cav: &CavitationConfig) -> String {
        let model_str = match cav.model {
            CavitationModel::Kunz => "Kunz",
            CavitationModel::SchnerrSauer => "SchnerrSauer",
            CavitationModel::Merkle => "Merkle",
        };
        format!(
            r#"/*--------------------------------*- C++ -*----------------------------------*\
  =========                 |
  \\      /  F ield         | OpenFOAM: The Open Source CFD Toolbox
   \\    /   O peration     |
    \\  /    A nd           | www.openfoam.com
     \\/     M anipulation  |
\*---------------------------------------------------------------------------*/
FoamFile {{
    version     2.0;
    format      ascii;
    class       dictionary;
    location    "constant";
    object      cavitationProperties;
}}
// * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * //

model       {};
pSat        {};
rhoLiquid   {};
rhoVapor    {};
"#,
            model_str, cav.p_vap_kpa * 1000.0, cav.rho_liquid_kg_m3, cav.rho_vapor_kg_m3,
        )
    }

    /// Generate sprayProperties dictionary for Lagrangian spray
    pub fn generate_spray_properties(&self, sp: &SprayConfig) -> String {
        let breakup_str = match sp.breakup {
            SprayBreakupModel::ReitzDiwakar => "ReitzDiwakar",
            SprayBreakupModel::KHRT => "KHRT",
            SprayBreakupModel::TAB => "TAB",
            SprayBreakupModel::PilchErdman => "PilchErdman",
        };
        let evap_str = match sp.evaporation {
            SprayEvaporationModel::Standard => "standardEvaporation",
            SprayEvaporationModel::FuchsKnudsen => "FuchsKnudsen",
        };
        format!(
            r#"/*--------------------------------*- C++ -*----------------------------------*\
  =========                 |
  \\      /  F ield         | OpenFOAM: The Open Source CFD Toolbox
   \\    /   O peration     |
    \\  /    A nd           | www.openfoam.com
     \\/     M anipulation  |
\*---------------------------------------------------------------------------*/
FoamFile {{
    version     2.0;
    format      ascii;
    class       dictionary;
    location    "constant";
    object      sprayProperties;
}}
// * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * //

breakupModel        {};
evaporationModel    {};
collision           {};
parcelPerSecond     {:e};
injectionVelocity   {};
coneAngle           {};
"#,
            breakup_str, evap_str,
            if sp.collision { "yes" } else { "no" },
            sp.parcel_per_second,
            sp.injection_velocity_m_s,
            sp.cone_angle_deg,
        )
    }

    /// Generate turbulenceProperties dictionary with extended model selection
    pub fn generate_turbulence_properties(&self, tm: &TurbulenceModel) -> String {
        let engine = match tm {
            TurbulenceModel::KEpsilon => "kEpsilon",
            TurbulenceModel::RealizableKEpsilon => "realizableKE",
            TurbulenceModel::KOmegaSST => "kOmegaSST",
            TurbulenceModel::KOmegaSSTSAS => "kOmegaSSTSAS",
            TurbulenceModel::V2F => "v2f",
            TurbulenceModel::LES => "Smagorinsky",
            TurbulenceModel::LESigma => "sigma",
            TurbulenceModel::KEqnLES => "kEqn",
            TurbulenceModel::PANS => "PANS",
            TurbulenceModel::SpalartAllmarasDES => "SpalartAllmarasDES",
            TurbulenceModel::SpalartAllmarasIDDES => "SpalartAllmarasIDDES",
        };
        format!(
            r#"/*--------------------------------*- C++ -*----------------------------------*\
  =========                 |
  \\      /  F ield         | OpenFOAM: The Open Source CFD Toolbox
   \\    /   O peration     |
    \\  /    A nd           | www.openfoam.com
     \\/     M anipulation  |
\*---------------------------------------------------------------------------*/
FoamFile {{
    version     2.0;
    format      ascii;
    class       dictionary;
    location    "constant";
    object      turbulenceProperties;
}}
// * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * //

simulationType  {};

{}Coeffs
{{
    // P3: add model-specific coefficients
}}
"#,
            if matches!(tm, TurbulenceModel::LES | TurbulenceModel::LESigma | TurbulenceModel::KEqnLES) {
                "LES"
            } else {
                "RAS"
            },
            engine,
        )
    }

    /// Generate FSI coupling dictionary for solid mechanics
    pub fn generate_fsi_dict(&self, fsi: &FSIConfig) -> String {
        let model_str = match fsi.model {
            FSIModel::LinearElastic => "linearElastic",
            FSIModel::NonLinearGeometric => "nonLinearGeometric",
            FSIModel::Plastic => "plastic",
        };
        let coupl_str = match fsi.coupling {
            FSICoupling::DirichletNeumann => "Dirichlet-Neumann",
            FSICoupling::NeumannNeumann => "Neumann-Neumann",
            FSICoupling::RobinRobin => "Robin-Robin",
        };
        format!(
            r#"/*--------------------------------*- C++ -*----------------------------------*\
  =========                 |
  \\      /  F ield         | OpenFOAM: The Open Source CFD Toolbox
   \\    /   O peration     |
    \\  /    A nd           | www.openfoam.com
     \\/     M anipulation  |
\*---------------------------------------------------------------------------*/
FoamFile {{
    version     2.0;
    format      ascii;
    class       dictionary;
    location    "constant";
    object      mechanicalProperties;
}}
// * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * //

model               {};
coupling            {};
E                   {:e};
nu                  {};
rho                 {};
meshRelaxation      {};
nSubCycles          {};
"#,
            model_str, coupl_str,
            fsi.youngs_modulus_gpa * 1e9,
            fsi.poisson_ratio,
            fsi.density_kg_m3,
            fsi.mesh_relaxation,
            fsi.n_subcycles,
        )
    }
}

// ── JANAF NASA 7-coefficient polynomial data ──────────────────────

fn janaf_n2_low() -> [f64; 7] {
    [3.298677000e+00, 1.408240700e-03, -3.963222000e-06, 5.641515000e-09, -2.444855400e-12, -1.020899900e+03, 3.950372000e+00]
}

fn janaf_n2_high() -> [f64; 7] {
    [2.926640000e+00, 1.487976800e-03, -5.684760000e-07, 1.009703800e-10, -6.753351000e-15, -9.227977000e+02, 5.980528000e+00]
}

fn janaf_o2_low() -> [f64; 7] {
    [3.782456360e+00, -2.996734160e-03, 9.847302010e-06, -9.681295090e-09, 3.243728370e-12, -1.063943560e+03, 3.657675730e+00]
}

fn janaf_o2_high() -> [f64; 7] {
    [3.282537840e+00, 1.483087540e-03, -7.579666690e-07, 2.094705550e-10, -2.167177940e-14, -1.088457720e+03, 5.453231290e+00]
}

fn janaf_no_low() -> [f64; 7] {
    [4.218596960e+00, -4.639877680e-03, 2.132343720e-05, -2.193117870e-08, 7.525348680e-12, 9.843263200e+01, 2.280226750e+00]
}

fn janaf_no_high() -> [f64; 7] {
    [3.260431340e+00, 1.011422820e-03, -4.458257600e-07, 1.192501920e-10, -1.327340610e-14, 9.905475100e+01, 6.372809410e+00]
}

fn janaf_n_low() -> [f64; 7] {
    [2.484207680e+00, 8.538748560e-04, -3.650718740e-06, 5.148160170e-09, -2.270669030e-12, 5.610855370e+04, 4.867157210e+00]
}

fn janaf_n_high() -> [f64; 7] {
    [2.415942430e+00, 1.748974540e-04, -1.192276560e-07, 3.651908020e-11, -3.989317620e-15, 5.613452740e+04, 4.728587400e+00]
}

fn janaf_o_low() -> [f64; 7] {
    [3.168267100e+00, -3.279318840e-03, 6.643063960e-06, -6.128066240e-09, 2.112659710e-12, 2.912225920e+04, 2.051933460e+00]
}

fn janaf_o_high() -> [f64; 7] {
    [2.543636970e+00, -2.731624860e-04, -4.190295200e-09, 5.954789300e-12, -1.602293670e-15, 2.923886700e+04, 4.922294570e+00]
}

fn janaf_ar_low() -> [f64; 7] {
    [2.500000000e+00, 0.0, 0.0, 0.0, 0.0, -7.453750000e+02, 4.366000000e+00]
}

fn janaf_ar_high() -> [f64; 7] {
    [2.500000000e+00, 0.0, 0.0, 0.0, 0.0, -7.453750000e+02, 4.366000000e+00]
}

fn janaf_co2_low() -> [f64; 7] {
    [2.356773520e+00, 8.984596770e-03, -7.123562690e-06, 2.459190220e-09, -1.436995480e-13, -4.837196970e+04, 9.901052220e+00]
}

fn janaf_co2_high() -> [f64; 7] {
    [4.636511110e+00, 2.741456910e-03, -1.104336550e-06, 2.030030230e-10, -1.381568940e-14, -4.896096460e+04, -1.935312060e+00]
}

fn janaf_h2o_low() -> [f64; 7] {
    [4.198640560e+00, -2.036434100e-03, 6.520402110e-06, -5.487970620e-09, 1.771978170e-12, -3.029372670e+04, -8.490322080e-01]
}

fn janaf_h2o_high() -> [f64; 7] {
    [3.033992490e+00, 2.176918040e-03, -1.640725180e-07, -9.704198700e-11, 1.682009920e-14, -3.000429710e+04, 4.966770100e+00]
}

// ── Impl continuation for aeroacoustics, waves, phase change, wind, electrostatics, ablation ──

impl SolverConfigGen {
    /// Generate FW-H aeroacoustics controlDict function entry
    pub fn generate_aeroacoustics_functions(&self, aa: &AeroacousticConfig) -> String {
        let src = match aa.fwh_source {
            FWHSource::PermeableSurface => "permeable",
            FWHSource::SolidSurface => "solid",
        };
        let mut recv = String::new();
        for (i, (x, y, z)) in aa.receiver_positions.iter().enumerate() {
            recv.push_str(&format!("        receiver{}  ({}, {}, {});\n", i + 1, x, y, z));
        }
        format!(
            r#"functions
{{
    FWH
    {{
        type            FfowcsWilliamsHawkings;
        libs            ("libFWH.so");
        source          {};
        receivers
        {{
{}
        }}
        rhoInf          {};
        cInf            {};
        writeInterval   1;
        startTime       {};
    }}
}}
"#,
            src, recv, aa.density_far, aa.speed_of_sound, aa.start_time
        )
    }

    /// Generate waveProperties for wave generation and absorption
    pub fn generate_wave_properties(&self, wc: &WaveConfig) -> String {
        let model_str = match wc.model {
            WaveModel::StokesFirst => "StokesI",
            WaveModel::StokesFifth => "StokesV",
            WaveModel::Irregular => "irregular",
            WaveModel::StreamFunction => "streamFunction",
        };
        format!(
            r#"/*--------------------------------*- C++ -*----------------------------------*\
  =========                 |
  \\      /  F ield         | OpenFOAM: The Open Source CFD Toolbox
   \\    /   O peration     |
    \\  /    A nd           | www.openfoam.com
     \\/     M anipulation  |
\*---------------------------------------------------------------------------*/
FoamFile {{
    version     2.0;
    format      ascii;
    class       dictionary;
    location    "constant";
    object      waveProperties;
}}
// * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * //

model           {};
height          {};
period          {};
depth           {};
direction       {};
relaxationZone  {};
"#,
            model_str, wc.wave_height_m, wc.wave_period_s,
            wc.water_depth_m, wc.direction_deg, wc.relaxation_zone_length_m,
        )
    }

    /// Generate solidificationProperties for phase change
    pub fn generate_phase_change_properties(&self, pc: &PhaseChangeConfig) -> String {
        let model_str = match pc.model {
            PhaseChangeModel::EnthalpyPorosity => "enthalpyPorosity",
            PhaseChangeModel::LevelSet => "levelSet",
        };
        format!(
            r#"/*--------------------------------*- C++ -*----------------------------------*\
  =========                 |
  \\      /  F ield         | OpenFOAM: The Open Source CFD Toolbox
   \\    /   O peration     |
    \\  /    A nd           | www.openfoam.com
     \\/     M anipulation  |
\*---------------------------------------------------------------------------*/
FoamFile {{
    version     2.0;
    format      ascii;
    class       dictionary;
    location    "constant";
    object      solidificationProperties;
}}
// * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * //

model           {};
TSolidus        {};
TLiquidus       {};
L               {};
A mushy         {};
"#,
            model_str, pc.t_solidus_k, pc.t_liquidus_k,
            pc.latent_heat_j_kg, pc.mushy_constant,
        )
    }

    /// Generate actuator disc/line fvOptions for wind turbines
    pub fn generate_actuator_fv_options(&self, wt: &WindTurbineConfig) -> String {
        let act_type = match wt.actuator {
            ActuatorModel::Disc => "actuationDiskSource",
            ActuatorModel::Line => "actuatorLineSource",
            ActuatorModel::ALM => "actuatorLineSource",
        };
        format!(
            r#"/*--------------------------------*- C++ -*----------------------------------*\
  =========                 |
  \\      /  F ield         | OpenFOAM: The Open Source CFD Toolbox
   \\    /   O peration     |
    \\  /    A nd           | www.openfoam.com
     \\/     M anipulation  |
\*---------------------------------------------------------------------------*/
FoamFile {{
    version     2.0;
    format      ascii;
    class       dictionary;
    location    "constant";
    object      fvOptions;
}}
// * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * //

windTurbine
{{
    type                    {};
    active                  yes;
    {}
    fieldNames              (U);
    Cp                      {};
    Ct                      {};
    rotorDiameter           {};
    hubHeight               {};
    ratedPower              {};
    windSpeedRef            {};
    nBlades                 3;
}}
"#,
            act_type,
            if wt.actuator == ActuatorModel::Disc {
                String::new()
            } else {
                "nCellsPerBlade      50;".to_string()
            },
            wt.power_coefficient,
            wt.thrust_coefficient,
            wt.rotor_diameter_m,
            wt.hub_height_m,
            wt.rated_power_mw * 1e6,
            wt.wind_speed_ref_m_s,
        )
    }

    /// Generate electrostatic transportProperties
    pub fn generate_electrostatic_transport(&self, ec: &ElectrostaticConfig) -> String {
        format!(
            r#"/*--------------------------------*- C++ -*----------------------------------*\
  =========                 |
  \\      /  F ield         | OpenFOAM: The Open Source CFD Toolbox
   \\    /   O peration     |
    \\  /    A nd           | www.openfoam.com
     \\/     M anipulation  |
\*---------------------------------------------------------------------------*/
FoamFile {{
    version     2.0;
    format      ascii;
    class       dictionary;
    location    "constant";
    object      transportProperties;
}}
// * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * //

transportModel  electrostatic;
potential       {};
epsilon         {:e};
spaceCharge     {:e};
ionMobility     {:e};
"#,
            ec.potential_v, ec.permittivity_f_m, ec.space_charge_c_m3, ec.ion_mobility_m2_vs,
        )
    }

    /// Generate ablation boundary condition dictionary
    pub fn generate_ablation_properties(&self, ab: &AblationConfig) -> String {
        let model_str = match ab.model {
            AblationModel::SurfaceRecession => "surfaceRecession",
            AblationModel::CharringMaterial => "charringMaterial",
            AblationModel::Pyrolysis => "pyrolysis",
        };
        format!(
            r#"/*--------------------------------*- C++ -*----------------------------------*\
  =========                 |
  \\      /  F ield         | OpenFOAM: The Open Source CFD Toolbox
   \\    /   O peration     |
    \\  /    A nd           | www.openfoam.com
     \\/     M anipulation  |
\*---------------------------------------------------------------------------*/
FoamFile {{
    version     2.0;
    format      ascii;
    class       dictionary;
    location    "constant";
    object      ablationProperties;
}}
// * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * //

model               {};
kChar               {};
kVirgin             {};
hPyrolysis          {};
recessionCoeff      {};
emissivity          {};
"#,
            model_str, ab.char_conductivity_w_mk, ab.virgin_conductivity_w_mk,
            ab.pyrolysis_gas_enthalpy_j_kg, ab.recession_rate_coeff, ab.emissivity,
        )
    }

    /// Generate nozzle geometry and propulsion properties
    pub fn generate_propulsion_properties(&self, pc: &PropulsionConfig) -> String {
        let model_str = match pc.model {
            PropulsionModel::SolidRocket => "solidRocket",
            PropulsionModel::LiquidRocket => "liquidRocket",
            PropulsionModel::HybridRocket => "hybridRocket",
            PropulsionModel::Scramjet => "scramjet",
        };
        let expansion_ratio = if pc.throat_area_m2 > 0.0 {
            pc.exit_area_m2 / pc.throat_area_m2
        } else {
            1.0
        };
        format!(
            r#"/*--------------------------------*- C++ -*----------------------------------*\
  =========                 |
  \\      /  F ield         | OpenFOAM: The Open Source CFD Toolbox
   \\    /   O peration     |
    \\  /    A nd           | www.openfoam.com
     \\/     M anipulation  |
\*---------------------------------------------------------------------------*/
FoamFile {{
    version     2.0;
    format      ascii;
    class       dictionary;
    location    "constant";
    object      propulsionProperties;
}}
// * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * //

propulsionType      {};
p0                  {};
T0                  {};
pExit               {};
mdot                {};
At                  {};
Ae                  {};
expansionRatio      {};
"#,
            model_str,
            pc.chamber_pressure_bar * 1e5,
            pc.chamber_temp_k,
            pc.exit_pressure_bar * 1e5,
            pc.mass_flow_rate_kg_s,
            pc.throat_area_m2,
            pc.exit_area_m2,
            expansion_ratio,
        )
    }

    /// Generate nuclear / radiation transport cross-section dictionary
    pub fn generate_nuclear_transport(&self, nc: &NuclearConfig) -> String {
        let model_str = match nc.model {
            NuclearModel::NeutronTransport => "neutronTransport",
            NuclearModel::PhotonTransport => "photonTransport",
            NuclearModel::Coupled => "coupled",
            NuclearModel::RadiationHydro => "radiationHydro",
        };
        let mut xs_block = String::new();
        for (i, xs) in nc.cross_sections.iter().enumerate() {
            xs_block.push_str(&format!("        group{}    {};\n", i + 1, xs));
        }
        format!(
            r#"/*--------------------------------*- C++ -*----------------------------------*\
  =========                 |
  \\      /  F ield         | OpenFOAM: The Open Source CFD Toolbox
   \\    /   O peration     |
    \\  /    A nd           | www.openfoam.com
     \\/     M anipulation  |
\*---------------------------------------------------------------------------*/
FoamFile {{
    version     2.0;
    format      ascii;
    class       dictionary;
    location    "constant";
    object      nuclearProperties;
}}
// * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * //

transportModel      {};
nEnergyGroups        {};
crossSections
{{
{}
}}
sourceStrength       {:e};
T                    {};
"#,
            model_str, nc.n_energy_groups, xs_block, nc.source_strength_m3, nc.temperature_k,
        )
    }

    /// Generate marine / hydrodynamics properties
    pub fn generate_marine_properties(&self, mc: &MarineConfig) -> String {
        let model_str = match mc.model {
            MarineModel::Hydrofoil => "hydrofoil",
            MarineModel::PropellerOpenWater => "propellerOpenWater",
            MarineModel::ShipResistance => "shipResistance",
            MarineModel::PlaningHull => "planingHull",
        };
        format!(
            r#"/*--------------------------------*- C++ -*----------------------------------*\
  =========                 |
  \\      /  F ield         | OpenFOAM: The Open Source CFD Toolbox
   \\    /   O peration     |
    \\  /    A nd           | www.openfoam.com
     \\/     M anipulation  |
\*---------------------------------------------------------------------------*/
FoamFile {{
    version     2.0;
    format      ascii;
    class       dictionary;
    location    "constant";
    object      marineProperties;
}}
// * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * //

model               {};
speed               {};
depth               {};
cavitationMargin    {};
propellerRPM        {};
thrustCoeff         {};
"#,
            model_str,
            mc.speed_knots * 0.514444,
            mc.depth_m,
            mc.cavitation_margin,
            mc.propeller_rpm / 60.0,
            mc.thrust_coefficient,
        )
    }

    /// Generate ML surrogate / active learning parameter dictionary
    pub fn generate_ml_surrogate(&self, mc: &MlSurrogateConfig) -> String {
        let model_str = match mc.model {
            MlSurrogateModel::GpRbf => "gpRbf",
            MlSurrogateModel::GpMatern => "gpMatern",
            MlSurrogateModel::Rf => "randomForest",
            MlSurrogateModel::Xgb => "xgboost",
            MlSurrogateModel::Lhs => "lhs",
        };
        format!(
            r#"/*--------------------------------*- C++ -*----------------------------------*\
  =========                 |
  \\      /  F ield         | OpenFOAM: The Open Source CFD Toolbox
   \\    /   O peration     |
    \\  /    A nd           | www.openfoam.com
     \\/     M anipulation  |
\*---------------------------------------------------------------------------*/
FoamFile {{
    version     2.0;
    format      ascii;
    class       dictionary;
    location    "system";
    object      surrogateProperties;
}}
// * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * //

surrogateModel      {};
nSamples            {};
nVaries             {};
explorationRate     {:e};
acquisition         {};
rhoInit             {};
lengthScale         {};
"#,
            model_str,
            mc.n_samples,
            mc.n_varies,
            mc.exploration_rate,
            mc.acquisition,
            mc.rho_default,
            mc.length_scale_default,
        )
    }

    // ── PEMFC Config Generation ──────────────────────────────────

    /// Select solver based on PEMFC model tier
    pub fn select_pemfc_solver(&self, intake: &IntakeConfig) -> &'static str {
        if let Some(ref pc) = intake.pemfc {
            match pc.model {
                PemfcModel::SimplePolarization => "pemfcFoam",
                PemfcModel::Isothermal1D => "pemfcFoam",
                PemfcModel::NonIsothermal => "pemfcThermalFoam",
                PemfcModel::TwoPhase => "pemfcTwoPhaseFoam",
            }
        } else {
            self.select_solver(intake)
        }
    }

    /// Generate pemfcProperties dictionary
    pub fn generate_pemfc_properties(&self, pc: &PemfcConfig) -> String {
        let model_str = match pc.model {
            PemfcModel::SimplePolarization => "simplePolarization",
            PemfcModel::Isothermal1D => "isothermal1D",
            PemfcModel::NonIsothermal => "nonIsothermal",
            PemfcModel::TwoPhase => "twoPhase",
        };
        let flow_str = match pc.flow_field {
            PemfcFlowField::Parallel => "parallel",
            PemfcFlowField::Serpentine => "serpentine",
            PemfcFlowField::Interdigitated => "interdigitated",
            PemfcFlowField::PinType => "pinType",
        };
        format!(
            r#"/*--------------------------------*- C++ -*----------------------------------*\
  =========                 |
  \\      /  F ield         | OpenFOAM: The Open Source CFD Toolbox
   \\    /   O peration     |
    \\  /    A nd           | www.openfoam.com
     \\/     M anipulation  |
\*---------------------------------------------------------------------------*/
FoamFile {{
    version     2.0;
    format      ascii;
    class       dictionary;
    location    "constant";
    object      pemfcProperties;
}}
// * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * //

model               {};
flowField           {};
tCell               {:e};
pAnode              {:e};
pCathode            {:e};
lambdaAnode         {};
lambdaCathode       {};
stoichAnode         {};
stoichCathode       {};
gdlPorosity         {};
gdlPermeability     {:e};
clPorosity          {};
"#,
            model_str, flow_str,
            pc.t_cell_k, pc.p_anode_bar, pc.p_cathode_bar,
            pc.lambda_anode, pc.lambda_cathode,
            pc.stoich_anode, pc.stoich_cathode,
            pc.gdl_porosity, pc.gdl_permeability_m2, pc.cl_porosity,
        )
    }

    /// Generate electrochemistryProperties dictionary
    pub fn generate_pemfc_electrochemistry(&self, pc: &PemfcConfig) -> String {
        format!(
            r#"/*--------------------------------*- C++ -*----------------------------------*\
  =========                 |
  \\      /  F ield         | OpenFOAM: The Open Source CFD Toolbox
   \\    /   O peration     |
    \\  /    A nd           | www.openfoam.com
     \\/     M anipulation  |
\*---------------------------------------------------------------------------*/
FoamFile {{
    version     2.0;
    format      ascii;
    class       dictionary;
    location    "constant";
    object      electrochemistryProperties;
}}
// * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * //

iRef                {:e};
alphaA              {};
alphaC              {};
i0Anode             {:e};
i0Cathode           {:e};
nernstRef           {:e};
"#,
            pc.i_ref_a_m2,
            pc.alpha_anode, pc.alpha_cathode,
            pc.exchange_i_anode_a_m2, pc.exchange_i_cathode_a_m2,
            1.23_f64,
        )
    }

    /// Generate membraneProperties dictionary
    pub fn generate_pemfc_membrane(&self, pc: &PemfcConfig) -> String {
        format!(
            r#"/*--------------------------------*- C++ -*----------------------------------*\
  =========                 |
  \\      /  F ield         | OpenFOAM: The Open Source CFD Toolbox
   \\    /   O peration     |
    \\  /    A nd           | www.openfoam.com
     \\/     M anipulation  |
\*---------------------------------------------------------------------------*/
FoamFile {{
    version     2.0;
    format      ascii;
    class       dictionary;
    location    "constant";
    object      membraneProperties;
}}
// * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * //

membraneThickness   {:e};
membraneConductivity {:e};
eodCoefficient      {};
waterUptakeMax      {};
"#,
            pc.membrane_thickness_um * 1.0e-6,
            pc.membrane_conductivity_s_m,
            pc.eod_coefficient,
            pc.water_uptake_max,
        )
    }

    /// Generate cyclingProperties dictionary
    pub fn generate_pemfc_cycling(&self, pc: &PemfcConfig) -> String {
        let cycle_str = match pc.cycle_profile {
            PemfcCycleProfile::Potentiodynamic => "potentiodynamic",
            PemfcCycleProfile::Galvanodynamic => "galvanodynamic",
            PemfcCycleProfile::DriveCycle => "driveCycle",
        };
        format!(
            r#"/*--------------------------------*- C++ -*----------------------------------*\
  =========                 |
  \\      /  F ield         | OpenFOAM: The Open Source CFD Toolbox
   \\    /   O peration     |
    \\  /    A nd           | www.openfoam.com
     \\/     M anipulation  |
\*---------------------------------------------------------------------------*/
FoamFile {{
    version     2.0;
    format      ascii;
    class       dictionary;
    location    "constant";
    object      cyclingProperties;
}}
// * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * //

cycleProfile        {};
startVoltage        {};
endVoltage          {};
sweepRate           {:e};
holdTime            {};
nCycles             {};
"#,
            cycle_str,
            pc.start_voltage_v, pc.end_voltage_v,
            pc.sweep_rate_mv_s * 1.0e-3,
            pc.hold_time_s, pc.n_cycles,
        )
    }

    /// Generate degradationProperties dictionary
    pub fn generate_pemfc_degradation(&self, pc: &PemfcConfig) -> String {
        let deg_str = match pc.degradation_model {
            PemfcDegradationModel::None => "none",
            PemfcDegradationModel::PtDissolution => "ptDissolution",
            PemfcDegradationModel::CarbonCorrosion => "carbonCorrosion",
            PemfcDegradationModel::PinholeFormation => "pinholeFormation",
            PemfcDegradationModel::Combined => "combined",
        };
        format!(
            r#"/*--------------------------------*- C++ -*----------------------------------*\
  =========                 |
  \\      /  F ield         | OpenFOAM: The Open Source CFD Toolbox
   \\    /   O peration     |
    \\  /    A nd           | www.openfoam.com
     \\/     M anipulation  |
\*---------------------------------------------------------------------------*/
FoamFile {{
    version     2.0;
    format      ascii;
    class       dictionary;
    location    "constant";
    object      degradationProperties;
}}
// * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * //

degradationModel    {};
initialECSA         {:e};
carbonLoading       {};
accelerationFactor  {};
"#,
            deg_str,
            pc.initial_ecsa_m2_g,
            pc.carbon_loading_mg_cm2,
            pc.acceleration_factor,
        )
    }

    /// Generate blockMeshDict for PEMFC geometry
    pub fn generate_pemfc_mesh(&self, pc: &PemfcConfig) -> String {
        let cw = pc.channel_width_mm * 1.0e-3;
        let rw = pc.rib_width_mm * 1.0e-3;
        let cd = pc.channel_depth_mm * 1.0e-3;
        let gdl = pc.gdl_thickness_um * 1.0e-6;
        let cl = pc.cl_thickness_um * 1.0e-6;
        let mem = pc.membrane_thickness_um * 1.0e-6;
        let aw = pc.active_width_mm * 1.0e-3;
        let al = pc.active_length_mm * 1.0e-3;

        let n_chan = (aw / (cw + rw)).floor() as u32;
        let n_x = pc.cells_along_pass;
        let n_cw = pc.cells_per_channel_width;
        let n_rw = pc.cells_per_rib_width;
        let n_cd = pc.cells_across_channel;
        let n_gdl = pc.cells_across_gdl;
        let n_cl = pc.cells_across_cl;
        let n_mem = pc.cells_across_membrane;
        let n_layer = 2 * (n_cd + n_gdl + n_cl) + n_mem;

        let total_y = 2.0 * cd + 2.0 * gdl + 2.0 * cl + mem;
        let total_x = al;
        let total_z = aw;

        let nx = n_x;
        let ny = n_layer;
        let nz = n_chan * (n_cw + n_rw);

        format!(
            r#"/*--------------------------------*- C++ -*----------------------------------*\
  =========                 |
  \\      /  F ield         | OpenFOAM: The Open Source CFD Toolbox
   \\    /   O peration     |
    \\  /    A nd           | www.openfoam.com
     \\/     M anipulation  |
\*---------------------------------------------------------------------------*/
FoamFile {{
    version     2.0;
    format      ascii;
    class       dictionary;
    location    "system";
    object      blockMeshDict;
}}
// * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * //

// PEMFC block-structured mesh
// Active area: {al} x {aw} m
// Total thickness: {total_y:.6e} m
// Flow-field type: {ff_type}

convertToMeters 1.0;

vertices
(
    (0 0 0)
    ({total_x:.6e} 0 0)
    ({total_x:.6e} {total_y:.6e} 0)
    (0 {total_y:.6e} 0)
    (0 0 {total_z:.6e})
    ({total_x:.6e} 0 {total_z:.6e})
    ({total_x:.6e} {total_y:.6e} {total_z:.6e})
    (0 {total_y:.6e} {total_z:.6e})
);

blocks
(
    hex (0 1 2 3 4 5 6 7) pemfc ({nx} {ny} {nz}) simpleGrading (1 1 1)
);

edges
(
);

boundary
(
    anodeFlowPlate
    {{
        type wall;
        faces
        (
            (0 1 5 4)
        );
    }}
    cathodeFlowPlate
    {{
        type wall;
        faces
        (
            (3 2 6 7)
        );
    }}
    inlet
    {{
        type patch;
        faces
        (
            (0 3 7 4)
        );
    }}
    outlet
    {{
        type patch;
        faces
        (
            (1 2 6 5)
        );
    }}
    sides
    {{
        type symmetryPlane;
        faces
        (
            (4 5 6 7)
            (0 1 2 3)
        );
    }}
);

mergePatchPairs
(
);
"#,
            ff_type = match pc.flow_field {
                PemfcFlowField::Parallel => "parallel",
                PemfcFlowField::Serpentine => "serpentine",
                PemfcFlowField::Interdigitated => "interdigitated",
                PemfcFlowField::PinType => "pinType",
            },
            al = al,
            aw = aw,
            total_y = total_y,
            total_x = total_x,
            total_z = total_z,
            nx = nx,
            ny = ny,
            nz = nz,
        )
    }

    /// Validate combined physics module configuration.
    /// Returns a list of warning/error messages. Empty means all clear.
    pub fn validate_config(&self, intake: &IntakeConfig) -> Vec<String> {
        let mut warnings: Vec<String> = Vec::new();

        let has_hypersonic = intake.hypersonic.is_some();
        let has_multiphase = intake.multiphase.is_some();
        let has_combustion = intake.combustion.is_some();
        let has_cavitation = intake.cavitation.is_some();
        let has_spray = intake.spray.is_some();
        let has_phase_change = intake.phase_change.is_some();
        let has_non_newtonian = intake.non_newtonian.is_some();
        let has_wave = intake.wave.is_some();
        let has_wind_turbine = intake.wind_turbine.is_some();
        let has_fsi = intake.fsi.is_some();
        let has_ablation = intake.ablation.is_some();
        let is_ami = intake.rotating.as_ref().is_some_and(|r| matches!(r.approach, RotatingApproach::AMI));

        // Hypersonic + others
        if has_hypersonic && has_multiphase {
            warnings.push("Hypersonic flow with multiphase is not physically meaningful".into());
        }
        if has_hypersonic && has_wave {
            warnings.push("Hypersonic flow with free surface waves is not physically meaningful".into());
        }
        if has_hypersonic && has_non_newtonian {
            warnings.push("Hypersonic flow with non-Newtonian rheology is unusual — verify physics".into());
        }

        // Combustion conflicts
        if has_combustion && has_cavitation {
            warnings.push("Combustion and cavitation are mutually exclusive mass-transfer models".into());
        }
        if has_combustion && has_spray {
            warnings.push("Combustion with spray Lagrangian phase requires reactingFoam with sprayFoam features".into());
        }

        // VOF + phase change
        if intake.multiphase.as_ref().is_some_and(|mp| matches!(mp.model, MultiphaseModel::VOF)) && has_phase_change {
            warnings.push("VOF multiphase with phase change (enthalpy porosity) requires compressibleInterFoam with solidificationMeltingFoam — verify solver capability".into());
        }

        // FSI conflicts
        if has_fsi && is_ami {
            warnings.push("FSI with AMI sliding mesh is not supported — use FSI with static mesh or MRF".into());
        }

        // Wave + wind turbine
        if has_wave && has_wind_turbine {
            warnings.push("Wave generation with wind turbine actuator is experimental — verify wave damping zone placement".into());
        }

        // Spray + multiphase
        if has_spray && has_multiphase {
            warnings.push("Spray injection into multiphase flow requires reactingParcelFoam or sprayFoam with multiphase support".into());
        }

        // Cavitation + phase change
        if has_cavitation && has_phase_change {
            warnings.push("Cavitation and phase-change models are unlikely to be compatible".into());
        }

        // Ablation + combustion
        if has_ablation && has_combustion {
            warnings.push("Ablation with volumetric combustion requires careful pyrolysis gas modeling".into());
        }

        // Multiphase + Non-Newtonian
        if has_multiphase && has_non_newtonian {
            warnings.push("Non-Newtonian rheology in multiphase flow requires viscoelasticTransportModel support".into());
        }

        warnings
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aeroflow_core::types::{RotatingConfig, HypersonicConfig, WallCatalysis, ChtConfig, MhdConfig, MhdSolver, MhdWallConductivity, PlasmaModel, PlasmaActuatorConfig, PemfcConfig, PemfcModel};

    fn base_intake() -> IntakeConfig {
        IntakeConfig {
            geometry_description: "test".into(),
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
            multiphase: None,
            non_newtonian: None,
            viscoelastic: None,
            combustion: None,
            cavitation: None,
            spray: None,
            phase_change: None,
            particle: None,
            porous: None,
            aeroacoustic: None,
            fsi: None,
            wave: None,
            wind_turbine: None,
            electrostatic: None,
            ablation: None,
            propulsion: None,
            nuclear: None,
            marine: None,
            ml_surrogate: None,
        }
    }

    fn rotating_intake(approach: RotatingApproach, rpm: f64, num_blades: u32, mach: f64) -> IntakeConfig {
        let mut i = base_intake();
        i.mach_number = mach;
        i.rotating = Some(RotatingConfig {
            rpm,
            axis: [0.0, 0.0, 1.0],
            origin: [0.0, 0.0, 0.0],
            approach,
            num_blades,
            diameter_m: Some(0.5),
            hub_diameter_m: Some(0.05),
            tip_clearance_m: None,
            advance_ratio: Some(0.5),
            target_ct: None,
            target_cp_max: None,
            target_eta_min: None,
            mass_flow_kg_s: None,
            pressure_ratio_target: None,
        });
        i
    }

    // ── select_solver ──────────────────────────────────────────

    #[test]
    fn test_select_solver_subsonic_no_rotating() {
        let cfg = SolverConfigGen::new();
        assert_eq!(cfg.select_solver(&base_intake()), "simpleFoam");
    }

    #[test]
    fn test_select_solver_transonic_no_rotating() {
        let mut i = base_intake();
        i.mach_number = 0.6;
        assert_eq!(SolverConfigGen::new().select_solver(&i), "rhoSimpleFoam");
    }

    #[test]
    fn test_select_solver_supersonic_no_rotating() {
        let mut i = base_intake();
        i.mach_number = 1.5;
        assert_eq!(SolverConfigGen::new().select_solver(&i), "rhoCentralFoam");
    }

    #[test]
    fn test_select_solver_mrf_subsonic() {
        let i = rotating_intake(RotatingApproach::MRF, 2500.0, 3, 0.15);
        assert_eq!(SolverConfigGen::new().select_solver(&i), "simpleFoam");
    }

    #[test]
    fn test_select_solver_mrf_compressible() {
        let i = rotating_intake(RotatingApproach::MRF, 5000.0, 5, 0.5);
        assert_eq!(SolverConfigGen::new().select_solver(&i), "rhoSimpleFoam");
    }

    #[test]
    fn test_select_solver_ami_subsonic() {
        let i = rotating_intake(RotatingApproach::AMI, 2500.0, 3, 0.15);
        assert_eq!(SolverConfigGen::new().select_solver(&i), "pimpleFoam");
    }

    #[test]
    fn test_select_solver_ami_compressible() {
        let i = rotating_intake(RotatingApproach::AMI, 5000.0, 5, 0.5);
        assert_eq!(SolverConfigGen::new().select_solver(&i), "rhoPimpleFoam");
    }

    // ── select_turbulence_model ────────────────────────────────

    #[test]
    fn test_turbulence_low_re_speed() {
        let mut i = base_intake();
        i.reynolds_number = 1e5;
        i.priority = Priority::Speed;
        assert_eq!(SolverConfigGen::new().select_turbulence_model(&i), "kOmegaSST");
    }

    #[test]
    fn test_turbulence_high_re_speed() {
        let mut i = base_intake();
        i.reynolds_number = 5e6;
        i.priority = Priority::Speed;
        assert_eq!(SolverConfigGen::new().select_turbulence_model(&i), "SpalartAllmaras");
    }

    #[test]
    fn test_turbulence_high_re_accuracy() {
        let mut i = base_intake();
        i.reynolds_number = 5e6;
        i.priority = Priority::Accuracy;
        assert_eq!(SolverConfigGen::new().select_turbulence_model(&i), "kOmegaSST");
    }

    #[test]
    fn test_turbulence_high_mach() {
        let mut i = base_intake();
        i.mach_number = 0.7;
        assert_eq!(SolverConfigGen::new().select_turbulence_model(&i), "kOmegaSST");
    }

    // ── relaxation_factors ─────────────────────────────────────

    #[test]
    fn test_relaxation_speed() {
        let mut i = base_intake();
        i.priority = Priority::Speed;
        let (p, u) = SolverConfigGen::new().relaxation_factors(&i);
        assert_eq!((p, u), (0.8, 0.4));
    }

    #[test]
    fn test_relaxation_balanced() {
        let mut i = base_intake();
        i.priority = Priority::Balanced;
        let (p, u) = SolverConfigGen::new().relaxation_factors(&i);
        assert_eq!((p, u), (0.7, 0.3));
    }

    #[test]
    fn test_relaxation_accuracy() {
        let mut i = base_intake();
        i.priority = Priority::Accuracy;
        let (p, u) = SolverConfigGen::new().relaxation_factors(&i);
        assert_eq!((p, u), (0.5, 0.2));
    }

    // ── generate_control_dict ──────────────────────────────────

    #[test]
    fn test_control_dict_non_rotating() {
        let s = SolverConfigGen::new().generate_control_dict(&base_intake());
        assert!(s.contains("simpleFoam"));
        assert!(s.contains("endTime         3000"));
        assert!(s.contains("deltaT          1"));
        assert!(s.contains("writeInterval   500"));
    }

    #[test]
    fn test_control_dict_mrf() {
        let i = rotating_intake(RotatingApproach::MRF, 2500.0, 3, 0.15);
        let s = SolverConfigGen::new().generate_control_dict(&i);
        assert!(s.contains("simpleFoam"));
        assert!(s.contains("deltaT          1"));
        assert!(s.contains("propellerForces"));
    }

    #[test]
    fn test_control_dict_ami() {
        let i = rotating_intake(RotatingApproach::AMI, 2500.0, 3, 0.15);
        let s = SolverConfigGen::new().generate_control_dict(&i);
        assert!(s.contains("pimpleFoam"));
        assert!(s.contains("propellerForces"));
        // AMI should have a small deltaT (scientific notation)
        assert!(s.contains("e-"));
    }

    #[test]
    fn test_control_dict_solver_override() {
        let i = rotating_intake(RotatingApproach::MRF, 1000.0, 2, 0.1);
        let s = SolverConfigGen::new().generate_control_dict(&i);
        assert!(s.contains("simpleFoam"));
    }

    // ── generate_mrf_properties ────────────────────────────────

    #[test]
    fn test_mrf_properties_returned_for_mrf_approach() {
        let i = rotating_intake(RotatingApproach::MRF, 1500.0, 3, 0.2);
        let d = SolverConfigGen::new().generate_mrf_properties(&i);
        assert!(d.is_some());
        let s = d.unwrap();
        assert!(s.contains("MRFProperties"));
        assert!(s.contains("rotatingZone"));
        assert!(s.contains("omega"));
    }

    #[test]
    fn test_mrf_properties_none_for_ami() {
        let i = rotating_intake(RotatingApproach::AMI, 1500.0, 3, 0.2);
        assert!(SolverConfigGen::new().generate_mrf_properties(&i).is_none());
    }

    #[test]
    fn test_mrf_properties_none_for_no_rotating() {
        assert!(SolverConfigGen::new().generate_mrf_properties(&base_intake()).is_none());
    }

    // ── generate_dynamic_mesh_dict ─────────────────────────────

    #[test]
    fn test_dynamic_mesh_dict_returned_for_ami() {
        let i = rotating_intake(RotatingApproach::AMI, 1500.0, 3, 0.2);
        let d = SolverConfigGen::new().generate_dynamic_mesh_dict(&i);
        assert!(d.is_some());
        let s = d.unwrap();
        assert!(s.contains("dynamicMeshDict"));
        assert!(s.contains("dynamicMotionSolverFvMesh"));
        assert!(s.contains("rotatingMotion"));
    }

    #[test]
    fn test_dynamic_mesh_dict_none_for_mrf() {
        let i = rotating_intake(RotatingApproach::MRF, 1500.0, 3, 0.2);
        assert!(SolverConfigGen::new().generate_dynamic_mesh_dict(&i).is_none());
    }

    #[test]
    fn test_dynamic_mesh_dict_none_for_no_rotating() {
        assert!(SolverConfigGen::new().generate_dynamic_mesh_dict(&base_intake()).is_none());
    }

    // ── generate_topo_set_dict ─────────────────────────────────

    #[test]
    fn test_topo_set_dict_generated() {
        let i = rotating_intake(RotatingApproach::MRF, 1500.0, 3, 0.2);
        let d = SolverConfigGen::new().generate_topo_set_dict(&i);
        assert!(d.is_some());
        let s = d.unwrap();
        assert!(s.contains("topoSetDict"));
        assert!(s.contains("cylinderToCell"));
        assert!(s.contains("radius"));
    }

    #[test]
    fn test_topo_set_dict_no_rotating() {
        assert!(SolverConfigGen::new().generate_topo_set_dict(&base_intake()).is_none());
    }

    #[test]
    fn test_topo_set_dict_uses_diameter() {
        let i = rotating_intake(RotatingApproach::AMI, 2000.0, 4, 0.3);
        let d = SolverConfigGen::new().generate_topo_set_dict(&i).unwrap();
        assert!(d.contains("radius  "));
    }

    // ── generate_fv_schemes_for ────────────────────────────────

    #[test]
    fn test_fv_schemes_default_steady() {
        let s = SolverConfigGen::new().generate_fv_schemes(); // delegates to _for(None)
        assert!(s.contains("steadyState"));
    }

    #[test]
    fn test_fv_schemes_ami_euler() {
        let i = rotating_intake(RotatingApproach::AMI, 2500.0, 3, 0.2);
        let s = SolverConfigGen::new().generate_fv_schemes_for(Some(&i));
        assert!(s.contains("Euler"));
        assert!(!s.contains("steadyState"));
    }

    #[test]
    fn test_fv_schemes_mrf_steady() {
        let i = rotating_intake(RotatingApproach::MRF, 2500.0, 3, 0.2);
        let s = SolverConfigGen::new().generate_fv_schemes_for(Some(&i));
        assert!(s.contains("steadyState"));
    }

    // ── generate_fv_solution ───────────────────────────────────

    #[test]
    fn test_fv_solution_contains_simple() {
        let s = SolverConfigGen::new().generate_fv_solution(&base_intake());
        assert!(s.contains("SIMPLE"));
        assert!(!s.contains("PIMPLE"));
    }

    #[test]
    fn test_fv_solution_contains_pimple_for_ami() {
        let i = rotating_intake(RotatingApproach::AMI, 2500.0, 3, 0.2);
        let s = SolverConfigGen::new().generate_fv_solution(&i);
        assert!(s.contains("PIMPLE"));
        assert!(!s.contains("SIMPLE"));
    }

    #[test]
    fn test_fv_solution_residuals_by_priority() {
        let speed = {
            let mut i = base_intake();
            i.priority = Priority::Speed;
            SolverConfigGen::new().generate_fv_solution(&i)
        };
        assert!(speed.contains("1e-4"));

        let acc = {
            let mut i = base_intake();
            i.priority = Priority::Accuracy;
            SolverConfigGen::new().generate_fv_solution(&i)
        };
        assert!(acc.contains("1e-6"));
    }

    // ── generate_forces_functions (indirect) ───────────────────

    #[test]
    fn test_control_dict_contains_force_coeffs_non_rotating() {
        let s = SolverConfigGen::new().generate_control_dict(&base_intake());
        assert!(s.contains("forceCoeffs"));
        assert!(s.contains("patches         (blade)"));
    }

    #[test]
    fn test_control_dict_propeller_forces_rotating() {
        let i = rotating_intake(RotatingApproach::MRF, 2500.0, 3, 0.2);
        let s = SolverConfigGen::new().generate_control_dict(&i);
        assert!(s.contains("propellerForces"));
        assert!(s.contains("patches         (blade blade1 blade2 blade3)"));
    }

    #[test]
    fn test_control_dict_with_format() {
        let cfg = SolverConfigGen::with_format(OpenFOAMFormat::Ascii);
        let s = cfg.generate_control_dict(&base_intake());
        assert!(s.contains("format ascii;"));
    }

    // ── Hypersonic helper ─────────────────────────────────────────

    fn hypersonic_intake(mach: f64, chemistry: ChemistryModel, two_temp: bool) -> IntakeConfig {
        let mut i = base_intake();
        i.mach_number = mach;
        i.hypersonic = Some(HypersonicConfig {
            mach_inf: mach,
            altitude_km: 30.0,
            wall_temperature_k: Some(1500.0),
            wall_catalysis: WallCatalysis::NonCatalytic,
            real_gas: true,
            chemistry,
            two_temperature: two_temp,
            rarefied: false,
            nose_radius_m: Some(0.05),
            flux_scheme: FluxScheme::Kurganov,
            target_peak_heat_flux_w_m2: None,
        });
        i
    }

    // ── select_solver (hypersonic) ───────────────────────────────

    #[test]
    fn test_select_solver_hypersonic_rho_central() {
        let i = hypersonic_intake(7.0, ChemistryModel::None, false);
        assert_eq!(SolverConfigGen::new().select_solver(&i), "rhoCentralFoam");
    }

    #[test]
    fn test_select_solver_hypersonic_hy2foam_chemistry() {
        let i = hypersonic_intake(10.0, ChemistryModel::Park5Species, false);
        assert_eq!(SolverConfigGen::new().select_solver(&i), "hy2Foam");
    }

    #[test]
    fn test_select_solver_hypersonic_hy2foam_two_temp() {
        let i = hypersonic_intake(10.0, ChemistryModel::Park5Species, true);
        assert_eq!(SolverConfigGen::new().select_solver(&i), "hy2Foam");
    }

    #[test]
    fn test_select_solver_hypersonic_dsmc() {
        let mut i = hypersonic_intake(10.0, ChemistryModel::None, false);
        i.hypersonic.as_mut().unwrap().rarefied = true;
        assert_eq!(SolverConfigGen::new().select_solver(&i), "dsmcFoam");
    }

    #[test]
    fn test_select_solver_hypersonic_high_mach_triggers_hy2foam() {
        let i = hypersonic_intake(9.0, ChemistryModel::None, false);
        assert_eq!(SolverConfigGen::new().select_solver(&i), "hy2Foam");
    }

    // ── select_solver (mixed-media) ────────────────────────────

    #[test]
    fn test_select_solver_multiphase_vof_subsonic() {
        let mut i = base_intake();
        i.multiphase = Some(MultiphaseConfig {
            model: MultiphaseModel::VOF,
            n_phases: 2,
            surface_tension_n_m: 0.07,
            phase_names: vec!["water".into(), "air".into()],
        });
        assert_eq!(SolverConfigGen::new().select_solver(&i), "interFoam");
    }

    #[test]
    fn test_select_solver_multiphase_vof_transonic() {
        let mut i = base_intake();
        i.mach_number = 0.5;
        i.multiphase = Some(MultiphaseConfig {
            model: MultiphaseModel::VOF,
            n_phases: 2,
            surface_tension_n_m: 0.07,
            phase_names: vec!["water".into(), "air".into()],
        });
        assert_eq!(SolverConfigGen::new().select_solver(&i), "compressibleInterFoam");
    }

    #[test]
    fn test_select_solver_multiphase_euler_euler() {
        let mut i = base_intake();
        i.multiphase = Some(MultiphaseConfig {
            model: MultiphaseModel::EulerEuler,
            n_phases: 2,
            surface_tension_n_m: 0.07,
            phase_names: vec!["oil".into(), "water".into()],
        });
        assert_eq!(SolverConfigGen::new().select_solver(&i), "multiphaseEulerFoam");
    }

    #[test]
    fn test_select_solver_multiphase_drift_flux() {
        let mut i = base_intake();
        i.multiphase = Some(MultiphaseConfig {
            model: MultiphaseModel::DriftFlux,
            n_phases: 2,
            surface_tension_n_m: 0.07,
            phase_names: vec!["gas".into(), "liquid".into()],
        });
        assert_eq!(SolverConfigGen::new().select_solver(&i), "driftFluxFoam");
    }

    #[test]
    fn test_select_solver_combustion() {
        let mut i = base_intake();
        i.combustion = Some(CombustionConfig {
            model: CombustionModel::EDC,
            c_eps: 2.1377,
            c_mu: 0.09,
            oxidizer: "O2".into(),
            fuel: "CH4".into(),
        });
        assert_eq!(SolverConfigGen::new().select_solver(&i), "reactingFoam");
    }

    #[test]
    fn test_select_solver_spray() {
        let mut i = base_intake();
        i.spray = Some(SprayConfig {
            breakup: SprayBreakupModel::KHRT,
            evaporation: SprayEvaporationModel::Standard,
            collision: true,
            parcel_per_second: 1e5,
            injection_velocity_m_s: 50.0,
            cone_angle_deg: 15.0,
        });
        assert_eq!(SolverConfigGen::new().select_solver(&i), "sprayFoam");
    }

    #[test]
    fn test_select_solver_cavitation() {
        let mut i = base_intake();
        i.cavitation = Some(CavitationConfig {
            model: CavitationModel::SchnerrSauer,
            p_vap_kpa: 2.34,
            rho_liquid_kg_m3: 1000.0,
            rho_vapor_kg_m3: 0.0258,
        });
        assert_eq!(SolverConfigGen::new().select_solver(&i), "interPhaseChangeFoam");
    }

    #[test]
    fn test_select_solver_phase_change() {
        let mut i = base_intake();
        i.phase_change = Some(PhaseChangeConfig {
            model: PhaseChangeModel::EnthalpyPorosity,
            t_solidus_k: 800.0,
            t_liquidus_k: 900.0,
            latent_heat_j_kg: 2.5e5,
            mushy_constant: 1e5,
        });
        assert_eq!(SolverConfigGen::new().select_solver(&i), "solidificationMeltingFoam");
    }

    #[test]
    fn test_select_solver_non_newtonian() {
        let mut i = base_intake();
        i.non_newtonian = Some(NonNewtonianConfig {
            model: ViscosityModel::PowerLaw,
            nu0: 1e-2,
            nu_inf: 1e-6,
            k: 0.005,
            n: 0.4,
            tau0: 10.0,
            nu_min: 1e-4,
            nu_max: 1e2,
        });
        assert_eq!(SolverConfigGen::new().select_solver(&i), "nonNewtonianIcoFoam");
    }

    #[test]
    fn test_select_solver_viscoelastic() {
        let mut i = base_intake();
        i.viscoelastic = Some(ViscoelasticConfig {
            model: ViscoelasticModel::OldroydB,
            relaxation_time_s: 1.0,
            solvent_viscosity_ratio: 0.1,
            mobility_factor: 0.2,
        });
        assert_eq!(SolverConfigGen::new().select_solver(&i), "nonNewtonianIcoFoam");
    }

    #[test]
    fn test_select_solver_wind_turbine() {
        let mut i = base_intake();
        i.wind_turbine = Some(WindTurbineConfig {
            actuator: ActuatorModel::Disc,
            thrust_coefficient: 0.8,
            power_coefficient: 0.45,
            rotor_diameter_m: 126.0,
            hub_height_m: 90.0,
            wind_speed_ref_m_s: 10.0,
            rated_power_mw: 5.0,
        });
        assert_eq!(SolverConfigGen::new().select_solver(&i), "pimpleFoam");
    }

    #[test]
    fn test_select_solver_wave() {
        let mut i = base_intake();
        i.wave = Some(WaveConfig {
            model: WaveModel::StokesFirst,
            wave_height_m: 2.0,
            wave_period_s: 8.0,
            water_depth_m: 20.0,
            direction_deg: 0.0,
            relaxation_zone_length_m: 10.0,
        });
        assert_eq!(SolverConfigGen::new().select_solver(&i), "interFoam");
    }

    #[test]
    fn test_select_solver_multiphase_plus_rotating_multiphase_wins() {
        let mut i = base_intake();
        i.multiphase = Some(MultiphaseConfig {
            model: MultiphaseModel::VOF,
            n_phases: 2,
            surface_tension_n_m: 0.07,
            phase_names: vec!["water".into(), "air".into()],
        });
        i.rotating = Some(RotatingConfig {
            rpm: 2500.0, axis: [0.0, 0.0, 1.0], origin: [0.0, 0.0, 0.0],
            approach: RotatingApproach::MRF, num_blades: 3,
            diameter_m: Some(0.5), hub_diameter_m: Some(0.05),
            tip_clearance_m: None, advance_ratio: Some(0.5),
            target_ct: None, target_cp_max: None, target_eta_min: None,
            mass_flow_kg_s: None, pressure_ratio_target: None,
        });
        // Multiphase has higher priority than rotating
        assert_eq!(SolverConfigGen::new().select_solver(&i), "interFoam");
    }

    #[test]
    fn test_select_solver_hypersonic_over_multiphase() {
        let mut i = hypersonic_intake(7.0, ChemistryModel::None, false);
        i.multiphase = Some(MultiphaseConfig {
            model: MultiphaseModel::VOF,
            n_phases: 2,
            surface_tension_n_m: 0.07,
            phase_names: vec!["water".into(), "air".into()],
        });
        // Hypersonic has highest priority
        assert_eq!(SolverConfigGen::new().select_solver(&i), "rhoCentralFoam");
    }

    #[test]
    fn test_select_solver_combustion_over_spray() {
        let mut i = base_intake();
        i.combustion = Some(CombustionConfig {
            model: CombustionModel::EDC,
            c_eps: 2.1377, c_mu: 0.09,
            oxidizer: "O2".into(), fuel: "CH4".into(),
        });
        i.spray = Some(SprayConfig {
            breakup: SprayBreakupModel::KHRT, evaporation: SprayEvaporationModel::Standard,
            collision: true, parcel_per_second: 1e5,
            injection_velocity_m_s: 50.0, cone_angle_deg: 15.0,
        });
        // Combustion has higher priority than spray
        assert_eq!(SolverConfigGen::new().select_solver(&i), "reactingFoam");
    }

    #[test]
    fn test_select_solver_spray_over_multiphase() {
        let mut i = base_intake();
        i.spray = Some(SprayConfig {
            breakup: SprayBreakupModel::KHRT, evaporation: SprayEvaporationModel::Standard,
            collision: true, parcel_per_second: 1e5,
            injection_velocity_m_s: 50.0, cone_angle_deg: 15.0,
        });
        i.multiphase = Some(MultiphaseConfig {
            model: MultiphaseModel::VOF, n_phases: 2,
            surface_tension_n_m: 0.07,
            phase_names: vec!["water".into(), "air".into()],
        });
        // Spray has higher priority than multiphase
        assert_eq!(SolverConfigGen::new().select_solver(&i), "sprayFoam");
    }

    #[test]
    fn test_select_solver_wave_over_rotating() {
        let mut i = base_intake();
        i.wave = Some(WaveConfig {
            model: WaveModel::StokesFirst, wave_height_m: 2.0,
            wave_period_s: 8.0, water_depth_m: 20.0,
            direction_deg: 0.0, relaxation_zone_length_m: 10.0,
        });
        i.rotating = Some(RotatingConfig {
            rpm: 2500.0, axis: [0.0, 0.0, 1.0], origin: [0.0, 0.0, 0.0],
            approach: RotatingApproach::MRF, num_blades: 3,
            diameter_m: Some(0.5), hub_diameter_m: Some(0.05),
            tip_clearance_m: None, advance_ratio: Some(0.5),
            target_ct: None, target_cp_max: None, target_eta_min: None,
            mass_flow_kg_s: None, pressure_ratio_target: None,
        });
        // Wave has higher priority than rotating
        assert_eq!(SolverConfigGen::new().select_solver(&i), "interFoam");
    }

    #[test]
    fn test_select_solver_multiphase_config_roundtrip() {
        let orig = MultiphaseConfig {
            model: MultiphaseModel::EulerEuler,
            n_phases: 3,
            surface_tension_n_m: 0.05,
            phase_names: vec!["oil".into(), "water".into(), "gas".into()],
        };
        let json = serde_json::to_string(&orig).unwrap();
        let deser: MultiphaseConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(orig.model, deser.model);
        assert_eq!(orig.n_phases, deser.n_phases);
        assert_eq!(orig.phase_names, deser.phase_names);
    }

    #[test]
    fn test_select_solver_combustion_config_roundtrip() {
        let orig = CombustionConfig {
            model: CombustionModel::PaSR,
            c_eps: 1.5,
            c_mu: 0.08,
            oxidizer: "air".into(),
            fuel: "H2".into(),
        };
        let json = serde_json::to_string(&orig).unwrap();
        let deser: CombustionConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(orig.model, deser.model);
        assert_eq!(orig.fuel, deser.fuel);
    }

    #[test]
    fn test_select_solver_spray_config_roundtrip() {
        let orig = SprayConfig {
            breakup: SprayBreakupModel::TAB,
            evaporation: SprayEvaporationModel::FuchsKnudsen,
            collision: false,
            parcel_per_second: 5e4,
            injection_velocity_m_s: 30.0,
            cone_angle_deg: 20.0,
        };
        let json = serde_json::to_string(&orig).unwrap();
        let deser: SprayConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(orig.breakup, deser.breakup);
        assert_eq!(orig.evaporation, deser.evaporation);
    }

    // ── validate_config Tests ───────────────────────────────────

    #[test]
    fn test_validate_clean_no_warnings() {
        let i = base_intake();
        let warnings = SolverConfigGen::new().validate_config(&i);
        assert!(warnings.is_empty(), "expected no warnings, got: {:?}", warnings);
    }

    #[test]
    fn test_validate_hypersonic_multiphase_conflict() {
        let mut i = base_intake();
        i.hypersonic = Some(HypersonicConfig {
            mach_inf: 5.0, altitude_km: 30.0, wall_temperature_k: None,
            wall_catalysis: WallCatalysis::NonCatalytic, real_gas: false,
            chemistry: ChemistryModel::None, two_temperature: false,
            rarefied: false, nose_radius_m: None,
            flux_scheme: FluxScheme::Kurganov, target_peak_heat_flux_w_m2: None,
        });
        i.multiphase = Some(MultiphaseConfig {
            model: MultiphaseModel::VOF, n_phases: 2,
            surface_tension_n_m: 0.07,
            phase_names: vec!["water".into(), "air".into()],
        });
        let warnings = SolverConfigGen::new().validate_config(&i);
        assert!(warnings.iter().any(|w| w.contains("Hypersonic") && w.contains("multiphase")),
            "expected hypersonic+multiphase warning, got: {:?}", warnings);
    }

    #[test]
    fn test_validate_hypersonic_wave_conflict() {
        let mut i = base_intake();
        i.hypersonic = Some(HypersonicConfig {
            mach_inf: 5.0, altitude_km: 30.0, wall_temperature_k: None,
            wall_catalysis: WallCatalysis::NonCatalytic, real_gas: false,
            chemistry: ChemistryModel::None, two_temperature: false,
            rarefied: false, nose_radius_m: None,
            flux_scheme: FluxScheme::Kurganov, target_peak_heat_flux_w_m2: None,
        });
        i.wave = Some(WaveConfig {
            model: WaveModel::StokesFirst, wave_height_m: 2.0, wave_period_s: 8.0,
            water_depth_m: 20.0, direction_deg: 0.0, relaxation_zone_length_m: 10.0,
        });
        let warnings = SolverConfigGen::new().validate_config(&i);
        assert!(warnings.iter().any(|w| w.contains("Hypersonic") && w.contains("waves")),
            "expected hypersonic+wave warning, got: {:?}", warnings);
    }

    #[test]
    fn test_validate_combustion_cavitation_conflict() {
        let mut i = base_intake();
        i.combustion = Some(CombustionConfig {
            model: CombustionModel::EDC, c_eps: 2.1377, c_mu: 0.09,
            oxidizer: "O2".into(), fuel: "CH4".into(),
        });
        i.cavitation = Some(CavitationConfig {
            model: CavitationModel::SchnerrSauer, p_vap_kpa: 2.34,
            rho_liquid_kg_m3: 1000.0, rho_vapor_kg_m3: 0.0258,
        });
        let warnings = SolverConfigGen::new().validate_config(&i);
        assert!(warnings.iter().any(|w| w.contains("Combustion") && w.contains("cavitation")),
            "expected combustion+cavitation warning, got: {:?}", warnings);
    }

    #[test]
    fn test_validate_fsi_ami_conflict() {
        let mut i = base_intake();
        i.fsi = Some(FSIConfig {
            model: FSIModel::LinearElastic, coupling: FSICoupling::DirichletNeumann,
            youngs_modulus_gpa: 200.0, poisson_ratio: 0.3, density_kg_m3: 7800.0,
            mesh_relaxation: 0.5, n_subcycles: 5,
        });
        i.rotating = Some(RotatingConfig {
            rpm: 2500.0, axis: [0.0, 0.0, 1.0], origin: [0.0, 0.0, 0.0],
            approach: RotatingApproach::AMI, num_blades: 3,
            diameter_m: Some(0.5), hub_diameter_m: Some(0.05),
            tip_clearance_m: None, advance_ratio: Some(0.5),
            target_ct: None, target_cp_max: None, target_eta_min: None,
            mass_flow_kg_s: None, pressure_ratio_target: None,
        });
        let warnings = SolverConfigGen::new().validate_config(&i);
        assert!(warnings.iter().any(|w| w.contains("FSI") && w.contains("AMI")),
            "expected FSI+AMI warning, got: {:?}", warnings);
    }

    #[test]
    fn test_validate_fsi_mrf_no_conflict() {
        // MRF should NOT trigger the FSI+AMI warning
        let mut i = base_intake();
        i.fsi = Some(FSIConfig {
            model: FSIModel::LinearElastic, coupling: FSICoupling::DirichletNeumann,
            youngs_modulus_gpa: 200.0, poisson_ratio: 0.3, density_kg_m3: 7800.0,
            mesh_relaxation: 0.5, n_subcycles: 5,
        });
        i.rotating = Some(RotatingConfig {
            rpm: 2500.0, axis: [0.0, 0.0, 1.0], origin: [0.0, 0.0, 0.0],
            approach: RotatingApproach::MRF, num_blades: 3,
            diameter_m: Some(0.5), hub_diameter_m: Some(0.05),
            tip_clearance_m: None, advance_ratio: Some(0.5),
            target_ct: None, target_cp_max: None, target_eta_min: None,
            mass_flow_kg_s: None, pressure_ratio_target: None,
        });
        let warnings = SolverConfigGen::new().validate_config(&i);
        assert!(!warnings.iter().any(|w| w.contains("FSI")),
            "FSI+MRF should not produce FSI warning, got: {:?}", warnings);
    }

    #[test]
    fn test_validate_wave_wind_turbine_warning() {
        let mut i = base_intake();
        i.wave = Some(WaveConfig {
            model: WaveModel::StokesFirst, wave_height_m: 2.0, wave_period_s: 8.0,
            water_depth_m: 20.0, direction_deg: 0.0, relaxation_zone_length_m: 10.0,
        });
        i.wind_turbine = Some(WindTurbineConfig {
            actuator: ActuatorModel::Disc, thrust_coefficient: 0.8,
            power_coefficient: 0.45, rotor_diameter_m: 126.0,
            hub_height_m: 90.0, wind_speed_ref_m_s: 10.0, rated_power_mw: 5.0,
        });
        let warnings = SolverConfigGen::new().validate_config(&i);
        assert!(warnings.iter().any(|w| w.contains("wave") && w.contains("wind turbine")),
            "expected wave+wind_turbine warning, got: {:?}", warnings);
    }

    // ── select_hypersonic_flux ───────────────────────────────────

    #[test]
    fn test_hypersonic_flux_kurganov_default() {
        let i = hypersonic_intake(7.0, ChemistryModel::None, false);
        assert_eq!(SolverConfigGen::new().select_hypersonic_flux(&i), "Kurganov");
    }

    #[test]
    fn test_hypersonic_flux_ausm_plus() {
        let mut i = hypersonic_intake(7.0, ChemistryModel::None, false);
        i.hypersonic.as_mut().unwrap().flux_scheme = FluxScheme::AUSMPlus;
        assert_eq!(SolverConfigGen::new().select_hypersonic_flux(&i), "AUSM+");
    }

    #[test]
    fn test_hypersonic_flux_non_hypersonic_returns_kurganov() {
        assert_eq!(SolverConfigGen::new().select_hypersonic_flux(&base_intake()), "Kurganov");
    }

    // ── generate_thermo_dict ────────────────────────────────────

    #[test]
    fn test_thermo_dict_none_for_non_hypersonic() {
        assert!(SolverConfigGen::new().generate_thermo_dict(&base_intake()).is_none());
    }

    #[test]
    fn test_thermo_dict_returned_for_real_gas() {
        let i = hypersonic_intake(7.0, ChemistryModel::None, false);
        let d = SolverConfigGen::new().generate_thermo_dict(&i);
        assert!(d.is_some());
        let s = d.unwrap();
        assert!(s.contains("thermophysicalProperties"));
        assert!(s.contains("janaf"));
        assert!(s.contains("sutherland"));
    }

    #[test]
    fn test_thermo_dict_multi_component_for_chemistry() {
        let i = hypersonic_intake(7.0, ChemistryModel::Park5Species, false);
        let d = SolverConfigGen::new().generate_thermo_dict(&i).unwrap();
        assert!(d.contains("multiComponentMixture"));
    }

    #[test]
    fn test_thermo_dict_two_temperature_type() {
        let i = hypersonic_intake(10.0, ChemistryModel::Park5Species, true);
        let d = SolverConfigGen::new().generate_thermo_dict(&i).unwrap();
        assert!(d.contains("hePsiThermo"));
    }

    // ── generate_chemistry_properties ───────────────────────────

    #[test]
    fn test_chemistry_none_for_non_hypersonic() {
        assert!(SolverConfigGen::new().generate_chemistry_properties(&base_intake()).is_none());
    }

    #[test]
    fn test_chemistry_none_when_chemistry_is_none() {
        let i = hypersonic_intake(7.0, ChemistryModel::None, false);
        assert!(SolverConfigGen::new().generate_chemistry_properties(&i).is_none());
    }

    #[test]
    fn test_chemistry_5_species_includes_o2_n2_no() {
        let i = hypersonic_intake(10.0, ChemistryModel::Park5Species, false);
        let d = SolverConfigGen::new().generate_chemistry_properties(&i).unwrap();
        assert!(d.contains("O2"));
        assert!(d.contains("N2"));
        assert!(d.contains("NO"));
        assert!(!d.contains("N+"));
    }

    #[test]
    fn test_chemistry_11_species_includes_ions() {
        let i = hypersonic_intake(10.0, ChemistryModel::Park11Species, false);
        let d = SolverConfigGen::new().generate_chemistry_properties(&i).unwrap();
        assert!(d.contains("N+"));
        assert!(d.contains("e-"));
    }

    #[test]
    fn test_chemistry_two_temperature_uses_sibs() {
        let i = hypersonic_intake(10.0, ChemistryModel::Park5Species, true);
        let d = SolverConfigGen::new().generate_chemistry_properties(&i).unwrap();
        assert!(d.contains("SIBS"));
    }

    // ── generate_radiation_dict ─────────────────────────────────

    #[test]
    fn test_radiation_none_for_sub_hypersonic() {
        let i = hypersonic_intake(10.0, ChemistryModel::None, false);
        assert!(SolverConfigGen::new().generate_radiation_dict(&i).is_none());
    }

    #[test]
    fn test_radiation_returned_for_extreme_hypersonic() {
        let i = hypersonic_intake(18.0, ChemistryModel::None, false);
        let d = SolverConfigGen::new().generate_radiation_dict(&i);
        assert!(d.is_some());
        let s = d.unwrap();
        assert!(s.contains("fvDOM"));
    }

    #[test]
    fn test_radiation_none_for_non_hypersonic() {
        assert!(SolverConfigGen::new().generate_radiation_dict(&base_intake()).is_none());
    }

    // ── generate_hypersonic_fv_schemes ──────────────────────────

    #[test]
    fn test_hypersonic_fv_schemes_none_for_non_hypersonic() {
        assert!(SolverConfigGen::new().generate_hypersonic_fv_schemes(&base_intake()).is_none());
    }

    #[test]
    fn test_hypersonic_fv_schemes_includes_flux_scheme() {
        let i = hypersonic_intake(7.0, ChemistryModel::None, false);
        let d = SolverConfigGen::new().generate_hypersonic_fv_schemes(&i).unwrap();
        assert!(d.contains("fluxScheme      Kurganov"));
    }

    #[test]
    fn test_hypersonic_fv_schemes_limited_linear() {
        let i = hypersonic_intake(7.0, ChemistryModel::None, false);
        let d = SolverConfigGen::new().generate_hypersonic_fv_schemes(&i).unwrap();
        assert!(d.contains("limitedLinearV 1"));
    }

    // ── generate_heat_flux_monitor ──────────────────────────────

    #[test]
    fn test_heat_flux_monitor_none_for_non_hypersonic() {
        assert!(SolverConfigGen::new().generate_heat_flux_monitor(&base_intake()).is_none());
    }

    #[test]
    fn test_heat_flux_monitor_returned_for_hypersonic() {
        let i = hypersonic_intake(7.0, ChemistryModel::None, false);
        let d = SolverConfigGen::new().generate_heat_flux_monitor(&i).unwrap();
        assert!(d.contains("wallHeatFlux"));
        assert!(d.contains("libfieldFunctionObjects"));
    }

    // ── CHT Config Generation Tests ──────────────────────────────

    fn cht_intake(problem_type: HeatTransferProblem, ma: f64, radiation: bool, phase_change: bool) -> IntakeConfig {
        let mut i = base_intake();
        i.mach_number = ma;
        i.cht = Some(ChtConfig {
            problem_type,
            fluid: "air".into(),
            solid_material: SolidMaterial::Steel,
            t_inlet_k: 800.0,
            t_ambient_k: 300.0,
            u_inlet_m_s: Some(100.0),
            re: None,
            pr: 0.71,
            heat_flux_target_w_m2: Some(5e5),
            radiation,
            radiation_model: if radiation { RadiationModel::FvDOM } else { RadiationModel::None },
            phase_change,
            max_t_solid_k: Some(1200.0),
            wall_thickness_m: Some(0.005),
            external_h_w_m2k: Some(15.0),
            emissivity: Some(0.85),
        });
        i
    }

    #[test]
    fn test_select_cht_solver_forced_convection() {
        let i = cht_intake(HeatTransferProblem::ForcedConvection, 0.2, false, false);
        assert_eq!(SolverConfigGen::new().select_cht_solver(&i), "buoyantBoussinesqSimpleFoam");
    }

    #[test]
    fn test_select_cht_solver_natural_convection() {
        let i = cht_intake(HeatTransferProblem::NaturalConvection, 0.0, false, false);
        assert_eq!(SolverConfigGen::new().select_cht_solver(&i), "buoyantSimpleFoam");
    }

    #[test]
    fn test_select_cht_solver_cht() {
        let i = cht_intake(HeatTransferProblem::CHT, 0.5, false, false);
        assert_eq!(SolverConfigGen::new().select_cht_solver(&i), "chtMultiRegionSimpleFoam");
    }

    #[test]
    fn test_select_cht_solver_phase_change() {
        let i = cht_intake(HeatTransferProblem::CHT, 0.5, false, true);
        assert_eq!(SolverConfigGen::new().select_cht_solver(&i), "icoReactingMultiphaseInterFoam");
    }

    #[test]
    fn test_select_cht_solver_radiation() {
        let i = cht_intake(HeatTransferProblem::Radiation, 0.1, true, false);
        assert_eq!(SolverConfigGen::new().select_cht_solver(&i), "buoyantSimpleFoam");
    }

    #[test]
    fn test_select_cht_solver_no_cht_falls_through() {
        assert_eq!(SolverConfigGen::new().select_cht_solver(&base_intake()), "simpleFoam");
    }

    #[test]
    fn test_generate_cht_fluid_thermo_boussinesq() {
        let i = cht_intake(HeatTransferProblem::ForcedConvection, 0.2, false, false);
        let d = SolverConfigGen::new().generate_cht_fluid_thermo(&i).unwrap();
        assert!(d.contains("eConst"));
        assert!(d.contains("rhoConst"));
        assert!(d.contains("sensibleInternalEnergy"));
    }

    #[test]
    fn test_generate_cht_fluid_thermo_compressible() {
        let i = cht_intake(HeatTransferProblem::CHT, 0.5, false, false);
        let d = SolverConfigGen::new().generate_cht_fluid_thermo(&i).unwrap();
        assert!(d.contains("hConst"));
        assert!(d.contains("perfectGas"));
        assert!(d.contains("sensibleEnthalpy"));
        assert!(d.contains("Pr           0.71"));
    }

    #[test]
    fn test_generate_cht_fluid_thermo_none_for_non_cht() {
        assert!(SolverConfigGen::new().generate_cht_fluid_thermo(&base_intake()).is_none());
    }

    #[test]
    fn test_generate_cht_solid_thermo_steel() {
        let i = cht_intake(HeatTransferProblem::CHT, 0.5, false, false);
        let d = SolverConfigGen::new().generate_cht_solid_thermo(&i).unwrap();
        assert!(d.contains("heSolidThermo"));
        assert!(d.contains("molWeight   55.85"));
        assert!(d.contains("rho         7850"));
        assert!(d.contains("Cv          502"));
        assert!(d.contains("kappa       45"));
    }

    #[test]
    fn test_generate_cht_solid_thermo_none_for_non_cht() {
        assert!(SolverConfigGen::new().generate_cht_solid_thermo(&base_intake()).is_none());
    }

    #[test]
    fn test_generate_cht_radiation_dict_none_when_disabled() {
        let i = cht_intake(HeatTransferProblem::CHT, 0.5, false, false);
        assert!(SolverConfigGen::new().generate_cht_radiation_dict(&i).is_none());
    }

    #[test]
    fn test_generate_cht_radiation_dict_fvdom() {
        let i = cht_intake(HeatTransferProblem::CHT, 0.5, true, false);
        let d = SolverConfigGen::new().generate_cht_radiation_dict(&i).unwrap();
        assert!(d.contains("fvDOM"));
        assert!(d.contains("nPhi    3"));
    }

    #[test]
    fn test_generate_cht_radiation_dict_none_for_non_cht() {
        assert!(SolverConfigGen::new().generate_cht_radiation_dict(&base_intake()).is_none());
    }

    #[test]
    fn test_generate_cht_interface_bcs() {
        let i = cht_intake(HeatTransferProblem::CHT, 0.5, false, false);
        let (fluid, solid) = SolverConfigGen::new().generate_cht_interface_bcs(&i, "interface").unwrap();
        assert!(fluid.contains("interface_fluid"));
        assert!(solid.contains("interface_solid"));
        assert!(fluid.contains("turbulentTemperatureCoupledBaffleMixed"));
        assert!(solid.contains("turbulentTemperatureCoupledBaffleMixed"));
        assert!(fluid.contains("kappaMethod fluidThermo"));
        assert!(solid.contains("kappaMethod solidThermo"));
    }

    #[test]
    fn test_generate_cht_interface_bcs_none_for_non_cht() {
        assert!(SolverConfigGen::new().generate_cht_interface_bcs(&base_intake(), "interface").is_none());
    }

    #[test]
    fn test_generate_cht_external_wall_bc() {
        let i = cht_intake(HeatTransferProblem::CHT, 0.5, false, false);
        let d = SolverConfigGen::new().generate_cht_external_wall_bc(&i).unwrap();
        assert!(d.contains("externalWallHeatFluxTemperature"));
        assert!(d.contains("Ta              uniform 300"));
        assert!(d.contains("h               uniform 15"));
        assert!(d.contains("emissivity      0.85"));
    }

    #[test]
    fn test_generate_cht_external_wall_bc_none_for_non_cht() {
        assert!(SolverConfigGen::new().generate_cht_external_wall_bc(&base_intake()).is_none());
    }

    // ── MHD Config Generation Tests ────────────────────────────────

    fn mhd_intake(solver: MhdSolver, b0: f64, low_rm: bool, wall_cond: MhdWallConductivity, plasma: bool) -> IntakeConfig {
        let mut i = base_intake();
        i.mhd = Some(MhdConfig {
            b0_tesla: b0,
            sigma_s_m: 1.0e7,
            mu_permeability_h_m: 1.2566e-6,
            solver,
            low_rm,
            hartmann_number: None,
            wall_conductivity: wall_cond,
            plasma_actuator: if plasma {
                Some(PlasmaActuatorConfig {
                    voltage_kv: 20.0,
                    frequency_hz: 5000.0,
                    body_force_n_m3: 5000.0,
                    actuator_width_m: 0.01,
                    model: PlasmaModel::ShyyJayaraman,
                })
            } else { None },
        });
        i
    }

    #[test]
    fn test_select_mhd_solver_mhdfoam() {
        let i = mhd_intake(MhdSolver::MhdFoam, 0.1, true, MhdWallConductivity::Insulating, false);
        assert_eq!(SolverConfigGen::new().select_mhd_solver(&i), "mhdFoam");
    }

    #[test]
    fn test_select_mhd_solver_magnetic_foam() {
        let i = mhd_intake(MhdSolver::MagneticFoam, 0.1, false, MhdWallConductivity::Conducting, false);
        assert_eq!(SolverConfigGen::new().select_mhd_solver(&i), "magneticFoam");
    }

    #[test]
    fn test_select_mhd_solver_no_mhd_falls_through() {
        assert_eq!(SolverConfigGen::new().select_mhd_solver(&base_intake()), "simpleFoam");
    }

    #[test]
    fn test_hartmann_number_typical() {
        let ha = SolverConfigGen::hartmann_number(0.1, 0.01, 1.0e7, 0.000234);
        // Ha = 0.1 * 0.01 * sqrt(1e7 / 0.000234) = 0.001 * sqrt(4.274e10) ≈ 0.001 * 206758 ≈ 207
        assert!(ha > 100.0 && ha < 500.0);
    }

    #[test]
    fn test_hartmann_number_zero_b() {
        let ha = SolverConfigGen::hartmann_number(0.0, 0.01, 1.0e7, 0.000234);
        assert!((ha - 0.0).abs() < 1e-10);
    }

    #[test]
    fn test_magnetic_reynolds_number_typical() {
        let rm = SolverConfigGen::magnetic_reynolds_number(1.0, 0.01, 1.0e7);
        // Rm = 4π×10⁻⁷ * 1e7 * 1.0 * 0.01 = 1.2566e-6 * 1e5 ≈ 0.126
        assert!(rm > 0.01 && rm < 1.0);
    }

    #[test]
    fn test_generate_mhd_transport_props() {
        let i = mhd_intake(MhdSolver::MhdFoam, 0.1, true, MhdWallConductivity::Insulating, false);
        let d = SolverConfigGen::new().generate_mhd_transport_props(&i, 2.54e-7).unwrap();
        assert!(d.contains("sigma"));
        assert!(d.contains("mu"));
        assert!(d.contains("transportModel  Newtonian"));
        assert!(d.contains("e-7")); // nu in scientific notation
    }

    #[test]
    fn test_generate_mhd_transport_props_none_for_non_mhd() {
        assert!(SolverConfigGen::new().generate_mhd_transport_props(&base_intake(), 1e-6).is_none());
    }

    #[test]
    fn test_generate_mhd_fv_options_for_low_rm() {
        let i = mhd_intake(MhdSolver::MhdFoam, 0.1, true, MhdWallConductivity::Insulating, false);
        let d = SolverConfigGen::new().generate_mhd_fv_options(&i).unwrap();
        assert!(d.contains("lorentzForce"));
        assert!(d.contains("vectorExplicitSource"));
    }

    #[test]
    fn test_generate_mhd_fv_options_none_for_non_low_rm() {
        let i = mhd_intake(MhdSolver::MhdFoam, 0.1, false, MhdWallConductivity::Insulating, false);
        assert!(SolverConfigGen::new().generate_mhd_fv_options(&i).is_none());
    }

    #[test]
    fn test_generate_mhd_fv_options_none_for_non_mhd() {
        assert!(SolverConfigGen::new().generate_mhd_fv_options(&base_intake()).is_none());
    }

    #[test]
    fn test_generate_mhd_b_field_insulating() {
        let i = mhd_intake(MhdSolver::MhdFoam, 0.5, true, MhdWallConductivity::Insulating, false);
        let d = SolverConfigGen::new().generate_mhd_b_field(&i).unwrap();
        assert!(d.contains("volVectorField"));
        assert!(d.contains("uniform (0 0.5 0)"));
        assert!(d.contains("walls\n    {\n        type            zeroGradient;"));
    }

    #[test]
    fn test_generate_mhd_b_field_conducting() {
        let i = mhd_intake(MhdSolver::MhdFoam, 1.0, true, MhdWallConductivity::Conducting, false);
        let d = SolverConfigGen::new().generate_mhd_b_field(&i).unwrap();
        assert!(d.contains("walls\n    {\n        type            fixedValue;"));
        assert!(d.contains("uniform (0 1 0)"));
    }

    #[test]
    fn test_generate_mhd_b_field_none_for_non_mhd() {
        assert!(SolverConfigGen::new().generate_mhd_b_field(&base_intake()).is_none());
    }

    #[test]
    fn test_generate_plasma_actuator_fv_options() {
        let i = mhd_intake(MhdSolver::MhdFoam, 0.1, true, MhdWallConductivity::Insulating, true);
        let d = SolverConfigGen::new().generate_plasma_actuator_fv_options(&i).unwrap();
        assert!(d.contains("bodyForceModel"));
        assert!(d.contains("vectorSemiImplicitSource"));
        assert!(d.contains("cellZone        actuatorZone"));
        // Body force in scientific format {:.6e}
        assert!(d.contains("e3"));
    }

    #[test]
    fn test_generate_plasma_actuator_none_without_plasma() {
        let i = mhd_intake(MhdSolver::MhdFoam, 0.1, true, MhdWallConductivity::Insulating, false);
        assert!(SolverConfigGen::new().generate_plasma_actuator_fv_options(&i).is_none());
    }

    #[test]
    fn test_generate_plasma_actuator_none_for_non_mhd() {
        assert!(SolverConfigGen::new().generate_plasma_actuator_fv_options(&base_intake()).is_none());
    }

    // ── Non-Newtonian Config Generation Tests ─────────────────────

    fn non_newtonian_config(model: ViscosityModel) -> NonNewtonianConfig {
        NonNewtonianConfig {
            model,
            nu0: 1e-2,
            nu_inf: 1e-6,
            k: 0.005,
            n: 0.4,
            tau0: 10.0,
            nu_min: 1e-4,
            nu_max: 1e2,
        }
    }

    #[test]
    fn test_non_newtonian_transport_newtonian() {
        let d = SolverConfigGen::new().generate_non_newtonian_transport(&non_newtonian_config(ViscosityModel::Newtonian));
        assert!(d.contains("transportModel  Newtonian"));
    }

    #[test]
    fn test_non_newtonian_transport_power_law() {
        let d = SolverConfigGen::new().generate_non_newtonian_transport(&non_newtonian_config(ViscosityModel::PowerLaw));
        assert!(d.contains("transportModel  powerLaw"));
        assert!(d.contains("k       0.005"));
        assert!(d.contains("n       0.4"));
        assert!(d.contains("nuMin"));
        assert!(d.contains("nuMax"));
    }

    #[test]
    fn test_non_newtonian_transport_cross() {
        let d = SolverConfigGen::new().generate_non_newtonian_transport(&non_newtonian_config(ViscosityModel::CrossPowerLaw));
        assert!(d.contains("CrossPowerLawCoeffs"));
        assert!(d.contains("nu0     0.01"));
    }

    #[test]
    fn test_non_newtonian_transport_bird_carreau() {
        let d = SolverConfigGen::new().generate_non_newtonian_transport(&non_newtonian_config(ViscosityModel::BirdCarreau));
        assert!(d.contains("BirdCarreauCoeffs"));
    }

    #[test]
    fn test_non_newtonian_transport_herschel_bulkley() {
        let d = SolverConfigGen::new().generate_non_newtonian_transport(&non_newtonian_config(ViscosityModel::HerschelBulkley));
        assert!(d.contains("HerschelBulkleyCoeffs"));
        assert!(d.contains("tau0    10"));
    }

    #[test]
    fn test_non_newtonian_transport_casson() {
        let d = SolverConfigGen::new().generate_non_newtonian_transport(&non_newtonian_config(ViscosityModel::Casson));
        assert!(d.contains("CassonCoeffs"));
    }

    // ── Porous Media Config Generation Tests ───────────────────────

    fn porous_config(model: PorousModel) -> PorousZoneConfig {
        PorousZoneConfig {
            model,
            cell_zone: "porousZone".into(),
            d_coeffs: [1e6, 1e6, 1e6],
            f_coeffs: [10.0, 10.0, 10.0],
        }
    }

    #[test]
    fn test_porous_fv_options_darcy() {
        let d = SolverConfigGen::new().generate_porous_fv_options(&porous_config(PorousModel::Darcy));
        assert!(d.contains("explicitPorositySource"));
        assert!(d.contains("cellZone        porousZone"));
        assert!(d.contains("d    (1.00000000e6 1.00000000e6 1.00000000e6)"));
        assert!(d.contains("f    (0 0 0)"));
    }

    #[test]
    fn test_porous_fv_options_darcy_forchheimer() {
        let d = SolverConfigGen::new().generate_porous_fv_options(&porous_config(PorousModel::DarcyForchheimer));
        assert!(d.contains("type            DarcyForchheimer"));
        assert!(d.contains("d    (1.00000000e6"));
        assert!(d.contains("f    (1.00000000e1"));
    }

    // ── Lagrangian Particle Config Generation Tests ────────────────

    fn particle_config(injection: ParticleInjection) -> ParticleConfig {
        ParticleConfig {
            injection,
            diameter_m: 1e-4,
            mass_flow_kg_s: 0.01,
            velocity_m_s: 10.0,
            temperature_k: 300.0,
            material_density_kg_m3: 2500.0,
        }
    }

    #[test]
    fn test_particle_injection_patch() {
        let d = SolverConfigGen::new().generate_particle_injection(&particle_config(ParticleInjection::PatchInjection), "cloud1");
        assert!(d.contains("cloud1Properties"));
        assert!(d.contains("patchInjection"));
        assert!(d.contains("massFlowRate        1e-2"));
    }

    #[test]
    fn test_particle_injection_cone() {
        let d = SolverConfigGen::new().generate_particle_injection(&particle_config(ParticleInjection::ConeInjection), "cloud1");
        assert!(d.contains("coneInjection"));
    }

    #[test]
    fn test_particle_injection_manual() {
        let d = SolverConfigGen::new().generate_particle_injection(&particle_config(ParticleInjection::ManualInjection), "cloud1");
        assert!(d.contains("manualInjection"));
    }

    // ── Viscoelastic Config Generation Tests ───────────────────────

    fn viscoelastic_config(model: ViscoelasticModel) -> ViscoelasticConfig {
        ViscoelasticConfig {
            model,
            relaxation_time_s: 0.1,
            solvent_viscosity_ratio: 0.1,
            mobility_factor: 0.2,
        }
    }

    #[test]
    fn test_viscoelastic_transport_oldroyd_b() {
        let d = SolverConfigGen::new().generate_viscoelastic_transport(&viscoelastic_config(ViscoelasticModel::OldroydB));
        assert!(d.contains("Oldroyd-B"));
        assert!(d.contains("relaxationTime      0.1"));
        assert!(d.contains("solventViscosityRatio 0.1"));
    }

    #[test]
    fn test_viscoelastic_transport_giesekus() {
        let d = SolverConfigGen::new().generate_viscoelastic_transport(&viscoelastic_config(ViscoelasticModel::Giesekus));
        assert!(d.contains("Giesekus"));
        assert!(d.contains("mobilityFactor      0.2"));
    }

    // ── Multiphase Config Generation Tests ─────────────────────────

    fn multiphase_config(model: MultiphaseModel) -> MultiphaseConfig {
        MultiphaseConfig {
            model,
            n_phases: 2,
            surface_tension_n_m: 0.07,
            phase_names: vec!["water".into(), "air".into()],
        }
    }

    #[test]
    fn test_multiphase_transport_vof() {
        let d = SolverConfigGen::new().generate_multiphase_transport(&multiphase_config(MultiphaseModel::VOF));
        assert!(d.contains("transportModel  VOF"));
        assert!(d.contains("sigma           0.07"));
        assert!(d.contains("water"));
        assert!(d.contains("air"));
    }

    #[test]
    fn test_multiphase_transport_eulerian() {
        let d = SolverConfigGen::new().generate_multiphase_transport(&multiphase_config(MultiphaseModel::EulerEuler));
        assert!(d.contains("transportModel  Eulerian"));
    }

    #[test]
    fn test_multiphase_transport_drift_flux() {
        let d = SolverConfigGen::new().generate_multiphase_transport(&multiphase_config(MultiphaseModel::DriftFlux));
        assert!(d.contains("transportModel  driftFlux"));
    }

    // ── JANAF Thermochemistry Tests ───────────────────────────────

    #[test]
    fn test_janaf_thermo_n2() {
        let d = SolverConfigGen::new().generate_janaf_thermo(&JanafSpecies::N2);
        assert!(d.contains("N2"));
        assert!(d.contains("janaf"));
        assert!(d.contains("3.2986770000e0"));
    }

    #[test]
    fn test_janaf_thermo_o2() {
        let d = SolverConfigGen::new().generate_janaf_thermo(&JanafSpecies::O2);
        assert!(d.contains("O2"));
        assert!(d.contains("3.7824563600e0"));
    }

    #[test]
    fn test_janaf_thermo_no() {
        let d = SolverConfigGen::new().generate_janaf_thermo(&JanafSpecies::NO);
        assert!(d.contains("NO"));
    }

    #[test]
    fn test_janaf_thermo_n() {
        let d = SolverConfigGen::new().generate_janaf_thermo(&JanafSpecies::N);
        assert!(d.contains("N"));
    }

    #[test]
    fn test_janaf_thermo_o() {
        let d = SolverConfigGen::new().generate_janaf_thermo(&JanafSpecies::O);
        assert!(d.contains("O"));
    }

    #[test]
    fn test_janaf_thermo_ar() {
        let d = SolverConfigGen::new().generate_janaf_thermo(&JanafSpecies::Ar);
        assert!(d.contains("Ar"));
    }

    #[test]
    fn test_janaf_thermo_co2() {
        let d = SolverConfigGen::new().generate_janaf_thermo(&JanafSpecies::CO2);
        assert!(d.contains("CO2"));
    }

    #[test]
    fn test_janaf_thermo_h2o() {
        let d = SolverConfigGen::new().generate_janaf_thermo(&JanafSpecies::H2O);
        assert!(d.contains("H2O"));
    }

    #[test]
    fn test_janaf_thermo_custom() {
        let d = SolverConfigGen::new().generate_janaf_thermo(&JanafSpecies::Custom("He".into(), [2.5; 7], [2.5; 7]));
        assert!(d.contains("He"));
    }

    // ── Combustion Chemistry Tests ────────────────────────────────

    #[test]
    fn test_combustion_chemistry_edc() {
        let d = SolverConfigGen::new().generate_combustion_chemistry(&CombustionConfig::default());
        assert!(d.contains("EDC"));
        assert!(d.contains("chemistryProperties"));
    }

    #[test]
    fn test_combustion_chemistry_flamelet() {
        let mut cc = CombustionConfig::default();
        cc.model = CombustionModel::LaminarFlamelet;
        let d = SolverConfigGen::new().generate_combustion_chemistry(&cc);
        assert!(d.contains("laminarFlamelet"));
    }

    #[test]
    fn test_combustion_chemistry_pasr() {
        let mut cc = CombustionConfig::default();
        cc.model = CombustionModel::PaSR;
        let d = SolverConfigGen::new().generate_combustion_chemistry(&cc);
        assert!(d.contains("PaSR"));
    }

    #[test]
    fn test_combustion_chemistry_wsr() {
        let mut cc = CombustionConfig::default();
        cc.model = CombustionModel::WellStirredReactor;
        let d = SolverConfigGen::new().generate_combustion_chemistry(&cc);
        assert!(d.contains("wellStirredReactor"));
    }

    // ── Cavitation Properties Tests ───────────────────────────────

    #[test]
    fn test_cavitation_properties_schnerr_sauer() {
        let d = SolverConfigGen::new().generate_cavitation_properties(&CavitationConfig::default());
        assert!(d.contains("SchnerrSauer"));
        assert!(d.contains("pSat        2340"));
        assert!(d.contains("rhoLiquid   1000"));
    }

    #[test]
    fn test_cavitation_properties_kunz() {
        let mut c = CavitationConfig::default();
        c.model = CavitationModel::Kunz;
        let d = SolverConfigGen::new().generate_cavitation_properties(&c);
        assert!(d.contains("Kunz"));
    }

    #[test]
    fn test_cavitation_properties_merkle() {
        let mut c = CavitationConfig::default();
        c.model = CavitationModel::Merkle;
        let d = SolverConfigGen::new().generate_cavitation_properties(&c);
        assert!(d.contains("Merkle"));
    }

    // ── Spray Properties Tests ────────────────────────────────────

    #[test]
    fn test_spray_properties_default() {
        let d = SolverConfigGen::new().generate_spray_properties(&SprayConfig::default());
        assert!(d.contains("sprayProperties"));
        assert!(d.contains("KHRT"));
        assert!(d.contains("standardEvaporation"));
        assert!(d.contains("collision           yes"));
    }

    #[test]
    fn test_spray_properties_reitz_diwakar() {
        let mut s = SprayConfig::default();
        s.breakup = SprayBreakupModel::ReitzDiwakar;
        let d = SolverConfigGen::new().generate_spray_properties(&s);
        assert!(d.contains("ReitzDiwakar"));
    }

    #[test]
    fn test_spray_properties_tab() {
        let mut s = SprayConfig::default();
        s.breakup = SprayBreakupModel::TAB;
        let d = SolverConfigGen::new().generate_spray_properties(&s);
        assert!(d.contains("TAB"));
    }

    #[test]
    fn test_spray_properties_pilch_erdman() {
        let mut s = SprayConfig::default();
        s.breakup = SprayBreakupModel::PilchErdman;
        let d = SolverConfigGen::new().generate_spray_properties(&s);
        assert!(d.contains("PilchErdman"));
    }

    #[test]
    fn test_spray_properties_no_collision() {
        let mut s = SprayConfig::default();
        s.collision = false;
        let d = SolverConfigGen::new().generate_spray_properties(&s);
        assert!(d.contains("collision           no"));
    }

    // ── Turbulence Properties Tests ───────────────────────────────

    #[test]
    fn test_turbulence_properties_k_epsilon() {
        let d = SolverConfigGen::new().generate_turbulence_properties(&TurbulenceModel::KEpsilon);
        assert!(d.contains("turbulenceProperties"));
        assert!(d.contains("RAS"));
        assert!(d.contains("kEpsilon"));
    }

    #[test]
    fn test_turbulence_properties_realizable_ke() {
        let d = SolverConfigGen::new().generate_turbulence_properties(&TurbulenceModel::RealizableKEpsilon);
        assert!(d.contains("realizableKE"));
    }

    #[test]
    fn test_turbulence_properties_sst() {
        let d = SolverConfigGen::new().generate_turbulence_properties(&TurbulenceModel::KOmegaSST);
        assert!(d.contains("kOmegaSST"));
    }

    #[test]
    fn test_turbulence_properties_sst_sas() {
        let d = SolverConfigGen::new().generate_turbulence_properties(&TurbulenceModel::KOmegaSSTSAS);
        assert!(d.contains("kOmegaSSTSAS"));
    }

    #[test]
    fn test_turbulence_properties_v2f() {
        let d = SolverConfigGen::new().generate_turbulence_properties(&TurbulenceModel::V2F);
        assert!(d.contains("v2f"));
    }

    #[test]
    fn test_turbulence_properties_les() {
        let d = SolverConfigGen::new().generate_turbulence_properties(&TurbulenceModel::LES);
        assert!(d.contains("LES"));
        assert!(d.contains("Smagorinsky"));
    }

    #[test]
    fn test_turbulence_properties_les_sigma() {
        let d = SolverConfigGen::new().generate_turbulence_properties(&TurbulenceModel::LESigma);
        assert!(d.contains("LES"));
        assert!(d.contains("sigma"));
    }

    #[test]
    fn test_turbulence_properties_keqn_les() {
        let d = SolverConfigGen::new().generate_turbulence_properties(&TurbulenceModel::KEqnLES);
        assert!(d.contains("kEqn"));
    }

    #[test]
    fn test_turbulence_properties_pans() {
        let d = SolverConfigGen::new().generate_turbulence_properties(&TurbulenceModel::PANS);
        assert!(d.contains("PANS"));
    }

    #[test]
    fn test_turbulence_properties_des() {
        let d = SolverConfigGen::new().generate_turbulence_properties(&TurbulenceModel::SpalartAllmarasDES);
        assert!(d.contains("SpalartAllmarasDES"));
    }

    #[test]
    fn test_turbulence_properties_iddes() {
        let d = SolverConfigGen::new().generate_turbulence_properties(&TurbulenceModel::SpalartAllmarasIDDES);
        assert!(d.contains("SpalartAllmarasIDDES"));
    }

    // ── FSI Config Tests ──────────────────────────────────────────

    #[test]
    fn test_fsi_dict_default() {
        let d = SolverConfigGen::new().generate_fsi_dict(&FSIConfig::default());
        assert!(d.contains("mechanicalProperties"));
        assert!(d.contains("linearElastic"));
        assert!(d.contains("Dirichlet-Neumann"));
        assert!(d.contains("E                   2e11"));
    }

    #[test]
    fn test_fsi_dict_non_linear() {
        let mut f = FSIConfig::default();
        f.model = FSIModel::NonLinearGeometric;
        let d = SolverConfigGen::new().generate_fsi_dict(&f);
        assert!(d.contains("nonLinearGeometric"));
    }

    #[test]
    fn test_fsi_dict_plastic() {
        let mut f = FSIConfig::default();
        f.model = FSIModel::Plastic;
        let d = SolverConfigGen::new().generate_fsi_dict(&f);
        assert!(d.contains("plastic"));
    }

    #[test]
    fn test_fsi_dict_robin_robin() {
        let mut f = FSIConfig::default();
        f.coupling = FSICoupling::RobinRobin;
        let d = SolverConfigGen::new().generate_fsi_dict(&f);
        assert!(d.contains("Robin-Robin"));
    }

    // ── Aeroacoustics Tests ───────────────────────────────────────

    #[test]
    fn test_aeroacoustics_solid_surface() {
        let d = SolverConfigGen::new().generate_aeroacoustics_functions(&AeroacousticConfig::default());
        assert!(d.contains("FfowcsWilliamsHawkings"));
        assert!(d.contains("solid"));
        assert!(d.contains("libFWH.so"));
    }

    #[test]
    fn test_aeroacoustics_permeable_surface() {
        let mut aa = AeroacousticConfig::default();
        aa.fwh_source = FWHSource::PermeableSurface;
        let d = SolverConfigGen::new().generate_aeroacoustics_functions(&aa);
        assert!(d.contains("permeable"));
    }

    #[test]
    fn test_aeroacoustics_receivers() {
        let aa = AeroacousticConfig {
            fwh_source: FWHSource::SolidSurface,
            receiver_positions: vec![(1.0, 0.0, 0.0), (0.0, 1.0, 0.0)],
            start_time: 0.0,
            density_far: 1.225,
            speed_of_sound: 340.0,
        };
        let d = SolverConfigGen::new().generate_aeroacoustics_functions(&aa);
        assert!(d.contains("receiver1"));
        assert!(d.contains("receiver2"));
    }

    // ── Wave Properties Tests ─────────────────────────────────────

    #[test]
    fn test_wave_properties_stokes1() {
        let d = SolverConfigGen::new().generate_wave_properties(&WaveConfig::default());
        assert!(d.contains("StokesI"));
        assert!(d.contains("height          2"));
        assert!(d.contains("period          8"));
    }

    #[test]
    fn test_wave_properties_stokes5() {
        let mut w = WaveConfig::default();
        w.model = WaveModel::StokesFifth;
        let d = SolverConfigGen::new().generate_wave_properties(&w);
        assert!(d.contains("StokesV"));
    }

    #[test]
    fn test_wave_properties_irregular() {
        let mut w = WaveConfig::default();
        w.model = WaveModel::Irregular;
        let d = SolverConfigGen::new().generate_wave_properties(&w);
        assert!(d.contains("irregular"));
    }

    #[test]
    fn test_wave_properties_stream_function() {
        let mut w = WaveConfig::default();
        w.model = WaveModel::StreamFunction;
        let d = SolverConfigGen::new().generate_wave_properties(&w);
        assert!(d.contains("streamFunction"));
    }

    // ── Phase Change Properties Tests ─────────────────────────────

    #[test]
    fn test_phase_change_enthalpy_porosity() {
        let d = SolverConfigGen::new().generate_phase_change_properties(&PhaseChangeConfig::default());
        assert!(d.contains("solidificationProperties"));
        assert!(d.contains("enthalpyPorosity"));
        assert!(d.contains("800"));
        assert!(d.contains("900"));
    }

    #[test]
    fn test_phase_change_level_set() {
        let mut p = PhaseChangeConfig::default();
        p.model = PhaseChangeModel::LevelSet;
        let d = SolverConfigGen::new().generate_phase_change_properties(&p);
        assert!(d.contains("levelSet"));
    }

    // ── Wind Turbine Actuator Tests ───────────────────────────────

    #[test]
    fn test_actuator_disc() {
        let d = SolverConfigGen::new().generate_actuator_fv_options(&WindTurbineConfig::default());
        assert!(d.contains("actuationDiskSource"));
        assert!(d.contains("0.8"));
        assert!(d.contains("0.45"));
        assert!(d.contains("rotorDiameter"));
    }

    #[test]
    fn test_actuator_line() {
        let mut w = WindTurbineConfig::default();
        w.actuator = ActuatorModel::Line;
        let d = SolverConfigGen::new().generate_actuator_fv_options(&w);
        assert!(d.contains("actuatorLineSource"));
        assert!(d.contains("nCellsPerBlade"));
    }

    #[test]
    fn test_actuator_alm() {
        let mut w = WindTurbineConfig::default();
        w.actuator = ActuatorModel::ALM;
        let d = SolverConfigGen::new().generate_actuator_fv_options(&w);
        assert!(d.contains("actuatorLineSource"));
    }

    // ── Electrostatic Transport Tests ─────────────────────────────

    #[test]
    fn test_electrostatic_transport() {
        let d = SolverConfigGen::new().generate_electrostatic_transport(&ElectrostaticConfig::default());
        assert!(d.contains("electrostatic"));
        assert!(d.contains("potential       10000"));
        assert!(d.contains("epsilon"));
    }

    // ── Ablation Properties Tests ─────────────────────────────────

    #[test]
    fn test_ablation_surface_recession() {
        let d = SolverConfigGen::new().generate_ablation_properties(&AblationConfig::default());
        assert!(d.contains("ablationProperties"));
        assert!(d.contains("surfaceRecession"));
        assert!(d.contains("0.5"));
    }

    #[test]
    fn test_ablation_charring() {
        let mut a = AblationConfig::default();
        a.model = AblationModel::CharringMaterial;
        let d = SolverConfigGen::new().generate_ablation_properties(&a);
        assert!(d.contains("charringMaterial"));
    }

    #[test]
    fn test_ablation_pyrolysis() {
        let mut a = AblationConfig::default();
        a.model = AblationModel::Pyrolysis;
        let d = SolverConfigGen::new().generate_ablation_properties(&a);
        assert!(d.contains("pyrolysis"));
    }

    // ── Propulsion Config Tests ───────────────────────────────────

    #[test]
    fn test_propulsion_default() {
        let d = SolverConfigGen::new().generate_propulsion_properties(&PropulsionConfig::default());
        assert!(d.contains("propulsionProperties"));
        assert!(d.contains("liquidRocket"));
        assert!(d.contains("p0                  7000000"));
        assert!(d.contains("expansionRatio      5"));
    }

    #[test]
    fn test_propulsion_solid() {
        let mut p = PropulsionConfig::default();
        p.model = PropulsionModel::SolidRocket;
        let d = SolverConfigGen::new().generate_propulsion_properties(&p);
        assert!(d.contains("solidRocket"));
    }

    #[test]
    fn test_propulsion_scramjet() {
        let mut p = PropulsionConfig::default();
        p.model = PropulsionModel::Scramjet;
        let d = SolverConfigGen::new().generate_propulsion_properties(&p);
        assert!(d.contains("scramjet"));
    }

    // ── Nuclear Transport Config Tests ────────────────────────────

    #[test]
    fn test_nuclear_default() {
        let d = SolverConfigGen::new().generate_nuclear_transport(&NuclearConfig::default());
        assert!(d.contains("nuclearProperties"));
        assert!(d.contains("neutronTransport"));
        assert!(d.contains("nEnergyGroups        2"));
    }

    #[test]
    fn test_nuclear_photon() {
        let mut n = NuclearConfig::default();
        n.model = NuclearModel::PhotonTransport;
        let d = SolverConfigGen::new().generate_nuclear_transport(&n);
        assert!(d.contains("photonTransport"));
    }

    #[test]
    fn test_nuclear_radiation_hydro() {
        let mut n = NuclearConfig::default();
        n.model = NuclearModel::RadiationHydro;
        let d = SolverConfigGen::new().generate_nuclear_transport(&n);
        assert!(d.contains("radiationHydro"));
    }

    // ── Marine Properties Config Tests ────────────────────────────

    #[test]
    fn test_marine_default() {
        let d = SolverConfigGen::new().generate_marine_properties(&MarineConfig::default());
        assert!(d.contains("marineProperties"));
        assert!(d.contains("hydrofoil"));
        assert!(d.contains("speed"));
    }

    #[test]
    fn test_marine_propeller() {
        let mut m = MarineConfig::default();
        m.model = MarineModel::PropellerOpenWater;
        let d = SolverConfigGen::new().generate_marine_properties(&m);
        assert!(d.contains("propellerOpenWater"));
    }

    #[test]
    fn test_marine_ship() {
        let mut m = MarineConfig::default();
        m.model = MarineModel::ShipResistance;
        let d = SolverConfigGen::new().generate_marine_properties(&m);
        assert!(d.contains("shipResistance"));
    }

    #[test]
    fn test_marine_planing() {
        let mut m = MarineConfig::default();
        m.model = MarineModel::PlaningHull;
        let d = SolverConfigGen::new().generate_marine_properties(&m);
        assert!(d.contains("planingHull"));
    }

    // ── ML Surrogate Config Tests ───────────────────────────────

    #[test]
    fn test_ml_surrogate_default() {
        let d = SolverConfigGen::new().generate_ml_surrogate(&MlSurrogateConfig::default());
        assert!(d.contains("surrogateProperties"));
        assert!(d.contains("gpRbf"));
    }

    #[test]
    fn test_ml_surrogate_gp_matern() {
        let mut m = MlSurrogateConfig::default();
        m.model = MlSurrogateModel::GpMatern;
        let d = SolverConfigGen::new().generate_ml_surrogate(&m);
        assert!(d.contains("gpMatern"));
    }

    #[test]
    fn test_ml_surrogate_xgb() {
        let mut m = MlSurrogateConfig::default();
        m.model = MlSurrogateModel::Xgb;
        let d = SolverConfigGen::new().generate_ml_surrogate(&m);
        assert!(d.contains("xgboost"));
    }

    #[test]
    fn test_ml_surrogate_lhs() {
        let mut m = MlSurrogateConfig::default();
        m.model = MlSurrogateModel::Lhs;
        let d = SolverConfigGen::new().generate_ml_surrogate(&m);
        assert!(d.contains("lhs"));
    }

    // ── PEMFC Config Generation Tests ───────────────────────────

    #[test]
    fn test_pemfc_properties_default() {
        let pc = PemfcConfig::default();
        let d = SolverConfigGen::new().generate_pemfc_properties(&pc);
        assert!(d.contains("pemfcProperties"));
        assert!(d.contains("isothermal1D"));
        assert!(d.contains("serpentine"));
    }

    #[test]
    fn test_pemfc_electrochemistry() {
        let pc = PemfcConfig::default();
        let d = SolverConfigGen::new().generate_pemfc_electrochemistry(&pc);
        assert!(d.contains("electrochemistryProperties"));
        assert!(d.contains("1e4")); // i_ref
    }

    #[test]
    fn test_pemfc_membrane() {
        let pc = PemfcConfig::default();
        let d = SolverConfigGen::new().generate_pemfc_membrane(&pc);
        assert!(d.contains("membraneProperties"));
        assert!(d.contains("membraneThickness")); // 50µm in m
    }

    #[test]
    fn test_pemfc_cycling() {
        let pc = PemfcConfig::default();
        let d = SolverConfigGen::new().generate_pemfc_cycling(&pc);
        assert!(d.contains("cyclingProperties"));
        assert!(d.contains("potentiodynamic"));
    }

    #[test]
    fn test_pemfc_degradation() {
        let pc = PemfcConfig::default();
        let d = SolverConfigGen::new().generate_pemfc_degradation(&pc);
        assert!(d.contains("degradationProperties"));
        assert!(d.contains("none"));
    }

    #[test]
    fn test_pemfc_mesh() {
        let pc = PemfcConfig::default();
        let d = SolverConfigGen::new().generate_pemfc_mesh(&pc);
        assert!(d.contains("blockMeshDict"));
        assert!(d.contains("pemfc"));
        assert!(d.contains("vertices"));
        assert!(d.contains("blocks"));
    }

    #[test]
    fn test_pemfc_select_solver_pemfc() {
        let mut intake = base_intake();
        intake.pemfc = Some(PemfcConfig::default());
        let s = SolverConfigGen::new().select_pemfc_solver(&intake);
        assert_eq!(s, "pemfcFoam");
    }

    #[test]
    fn test_pemfc_select_solver_non_isothermal() {
        let mut pc = PemfcConfig::default();
        pc.model = PemfcModel::NonIsothermal;
        let mut intake = base_intake();
        intake.pemfc = Some(pc);
        let s = SolverConfigGen::new().select_pemfc_solver(&intake);
        assert_eq!(s, "pemfcThermalFoam");
    }

    #[test]
    fn test_pemfc_select_solver_two_phase() {
        let mut pc = PemfcConfig::default();
        pc.model = PemfcModel::TwoPhase;
        let mut intake = base_intake();
        intake.pemfc = Some(pc);
        let s = SolverConfigGen::new().select_pemfc_solver(&intake);
        assert_eq!(s, "pemfcTwoPhaseFoam");
    }

    #[test]
    fn test_pemfc_select_solver_fallback() {
        let intake = base_intake(); // no pemfc
        let s = SolverConfigGen::new().select_pemfc_solver(&intake);
        assert_eq!(s, "simpleFoam");
    }

    #[test]
    fn test_pemfc_properties_all_models() {
        let models = [
            (PemfcModel::SimplePolarization, "simplePolarization"),
            (PemfcModel::Isothermal1D, "isothermal1D"),
            (PemfcModel::NonIsothermal, "nonIsothermal"),
            (PemfcModel::TwoPhase, "twoPhase"),
        ];
        for (model, expected) in &models {
            let mut pc = PemfcConfig::default();
            pc.model = model.clone();
            let d = SolverConfigGen::new().generate_pemfc_properties(&pc);
            assert!(d.contains(expected), "Missing {expected} in pemfcProperties");
        }
    }
}
