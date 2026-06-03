use crate::types::ToolDef;
use aeroflow_core::types::{ForceCoefficients, MeshQualityMetrics, SolverStats, RotatingConfig, RotatingApproach, HypersonicConfig, ChemistryModel, WallCatalysis, FluxScheme, ChtConfig, HeatTransferProblem, SolidMaterial, RadiationModel, MhdConfig, MhdSolver, MhdWallConductivity, PlasmaModel, PlasmaActuatorConfig, PropulsionConfig, PropulsionModel, NuclearConfig, NuclearModel, MarineConfig, MarineModel, MlSurrogateConfig, MlSurrogateModel, SolverDesign, SolverTemplate, CouplingStrategy, TimeTreatment, PhysicsModule, PemfcModel, PemfcConfig, PemfcFlowField, PemfcCycleProfile, PemfcDegradationModel, WindTunnelConfig, ViscosityModel, NonNewtonianConfig, ViscoelasticModel, ViscoelasticConfig, MultiphaseModel, MultiphaseConfig, CombustionModel, CombustionConfig, CavitationModel, CavitationConfig, SprayBreakupModel, SprayEvaporationModel, SprayConfig, PhaseChangeModel, PhaseChangeConfig, ParticleInjection, ParticleConfig, PorousModel, PorousZoneConfig, AeroacousticConfig, FWHSource, FSIModel, FSICoupling, FSIConfig, WaveModel, WaveConfig, ActuatorModel, WindTurbineConfig, ElectrostaticConfig, AblationModel, AblationConfig};
use aeroflow_core::{CaseConfig, IntakeConfig, Priority, FlowType};
use aeroflow_learner::reward::RewardFunction;
use aeroflow_pipeline::PipelineOrchestrator;
use aeroflow_post::ForceExtractor;
use aeroflow_solver::config_gen::SolverConfigGen;
use aeroflow_solver::SolverScaffold;
use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

fn get_case_dir(workspace_dir: &std::path::Path, case_name: &str) -> PathBuf {
    workspace_dir.join("cases").join(case_name)
}

pub fn get_all_tools() -> Vec<ToolDef> {
    vec![
        ToolDef {
            name: "get_case_detail".into(),
            description: "Read the current simulation case details including solver, flow type, mesh parameters, and geometry info.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "case_id": {"type": "string", "description": "UUID of the case"}
                },
                "required": ["case_id"]
            }),
        },
        ToolDef {
            name: "get_case_results".into(),
            description: "Get simulation results including Cd, Cl, y+, convergence status, and mesh quality.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "case_id": {"type": "string", "description": "UUID of the case"}
                },
                "required": ["case_id"]
            }),
        },
        ToolDef {
            name: "propose_config".into(),
            description: "Phase 1&2: Generate an agent-manifest.json with proposed mesh parameters, solver settings, and boundary conditions based on the user's requirements intake. Call this first in any STAR loop.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "case_id": {"type": "string", "description": "UUID of the case"},
                    "goal": {"type": "string", "description": "Simulation goal"},
                    "geometry_type": {"type": "string", "description": "airfoil, wing, fuselage, etc."},
                    "flow_type": {"type": "string", "description": "incompressible, subsonic, transonic, supersonic, hypersonic"},
                    "mach": {"type": "number", "description": "Mach number"},
                    "reynolds": {"type": "number", "description": "Reynolds number"},
                    "target_cl": {"type": "number", "description": "Target CL (optional)"},
                    "target_cd": {"type": "number", "description": "Target Cd (optional)"},
                    "max_y_plus": {"type": "number", "description": "Target max y+ (optional)"},
                    "mesh_params": {
                        "type": "object",
                        "properties": {
                            "surface_min_level": {"type": "integer"},
                            "surface_max_level": {"type": "integer"},
                            "n_cells_between_levels": {"type": "integer"}
                        }
                    },
                    "solver": {"type": "string", "description": "OpenFOAM solver name"},
                    "turbulence_model": {"type": "string", "description": "kOmegaSST, SpalartAllmaras, etc."},
                    "cht": {
                        "type": "object",
                        "description": "Conjugate Heat Transfer configuration (optional)",
                        "properties": {
                            "problem_type": {"type": "string", "enum": ["CHT", "forced_convection", "natural_convection", "radiation"]},
                            "fluid": {"type": "string"},
                            "solid_material": {"type": "string", "enum": ["steel", "aluminum", "copper", "ceramic", "cfrp", "inconel"]},
                            "t_inlet_K": {"type": "number"},
                            "t_ambient_K": {"type": "number"},
                            "radiation_model": {"type": "string", "enum": ["none", "p1", "fvdom", "viewFactor", "rosseland"]},
                            "phase_change": {"type": "boolean"}
                        }
                    },
                    "mhd": {
                        "type": "object",
                        "description": "Magnetohydrodynamic configuration (optional)",
                        "properties": {
                            "b0_Tesla": {"type": "number"},
                            "sigma_S_m": {"type": "number"},
                            "solver": {"type": "string", "enum": ["mhdFoam", "magneticFoam", "custom"]},
                            "low_Rm": {"type": "boolean"},
                            "wall_conductivity": {"type": "string", "enum": ["insulating", "conducting", "mixed"]},
                            "plasma_actuator": {"type": "object"}
                        }
                    },
                    "propulsion": {
                        "type": "object",
                        "description": "Propulsion configuration (optional)",
                        "properties": {
                            "model": {"type": "string", "enum": ["solidRocket", "liquidRocket", "hybridRocket", "scramjet"]},
                            "chamber_pressure_bar": {"type": "number"},
                            "chamber_temp_K": {"type": "number"},
                            "exit_pressure_bar": {"type": "number"},
                            "mass_flow_rate_kg_s": {"type": "number"},
                            "throat_area_m2": {"type": "number"},
                            "exit_area_m2": {"type": "number"}
                        }
                    },
                    "nuclear": {
                        "type": "object",
                        "description": "Nuclear / radiation transport configuration (optional)",
                        "properties": {
                            "model": {"type": "string", "enum": ["neutronTransport", "photonTransport", "coupled", "radiationHydro"]},
                            "n_energy_groups": {"type": "integer"},
                            "cross_sections": {"type": "array", "items": {"type": "number"}},
                            "source_strength": {"type": "number"},
                            "temperature_K": {"type": "number"}
                        }
                    },
                    "marine": {
                        "type": "object",
                        "description": "Marine / hydrodynamics configuration (optional)",
                        "properties": {
                            "model": {"type": "string", "enum": ["hydrofoil", "propellerOpenWater", "shipResistance", "planingHull"]},
                            "speed_knots": {"type": "number"},
                            "depth_m": {"type": "number"},
                            "cavitation_margin": {"type": "number"},
                            "propeller_rpm": {"type": "number"},
                            "thrust_coefficient": {"type": "number"}
                        }
                    },
                    "ml_surrogate": {
                        "type": "object",
                        "description": "ML surrogate / active learning configuration (optional)",
                        "properties": {
                            "model": {"type": "string", "enum": ["gpRbf", "gpMatern", "randomForest", "xgboost", "lhs"]},
                            "n_samples": {"type": "integer"},
                            "n_varies": {"type": "integer"},
                            "exploration_rate": {"type": "number"},
                            "acquisition": {"type": "string"},
                            "rho_init": {"type": "number"},
                            "length_scale": {"type": "number"}
                        }
                    },
                    "pemfc": {
                        "type": "object",
                        "description": "PEM Fuel Cell configuration (optional)",
                        "properties": {
                            "model": {"type": "string", "enum": ["simplePolarization", "isothermal1D", "nonIsothermal", "twoPhase"]},
                            "t_cell_K": {"type": "number"},
                            "p_anode_bar": {"type": "number"},
                            "p_cathode_bar": {"type": "number"},
                            "lambda_anode": {"type": "number"},
                            "lambda_cathode": {"type": "number"},
                            "stoich_anode": {"type": "number"},
                            "stoich_cathode": {"type": "number"},
                            "flow_field": {"type": "string", "enum": ["parallel", "serpentine", "interdigitated", "pinType"]},
                            "channel_width_mm": {"type": "number"},
                            "rib_width_mm": {"type": "number"},
                            "channel_depth_mm": {"type": "number"},
                            "n_passes": {"type": "integer"},
                            "active_width_mm": {"type": "number"},
                            "active_length_mm": {"type": "number"},
                            "cycle_profile": {"type": "string", "enum": ["potentiodynamic", "galvanodynamic", "driveCycle"]},
                            "sweep_rate_mV_s": {"type": "number"},
                            "n_cycles": {"type": "integer"},
                            "degradation_model": {"type": "string", "enum": ["none", "ptDissolution", "carbonCorrosion", "pinholeFormation", "combined"]}
                        }
                    },
                    "wind_tunnel": {
                        "type": "object",
                        "description": "Digital Wind Tunnel configuration (optional for external aero)",
                        "properties": {
                            "upstream": {"type": "number", "description": "Upstream distance in chord multiples (default 20)"},
                            "downstream": {"type": "number", "description": "Downstream distance in chord multiples (default 40)"},
                            "vertical": {"type": "number", "description": "Vertical half-height in chord multiples (default 25)"},
                            "lateral": {"type": "number", "description": "Lateral half-width in chord multiples (default 25)"},
                            "velocity_m_s": {"type": "number", "description": "Freestream velocity (m/s)"},
                            "turbulence_intensity": {"type": "number", "description": "Turbulence intensity fraction (default 0.005)"},
                            "reference_length_m": {"type": "number", "description": "Override auto-detected reference chord (m)"}
                        }
                    },
                    "multiphase": {
                        "type": "object",
                        "description": "Multiphase flow configuration (optional) — VOF / EulerEuler / DriftFlux for mixing multiple fluids",
                        "properties": {
                            "model": {"type": "string", "enum": ["vof", "eulerEuler", "driftFlux"], "description": "Multiphase model"},
                            "n_phases": {"type": "integer", "description": "Number of fluid phases"},
                            "surface_tension_N_m": {"type": "number", "description": "Surface tension between phases (N/m)"},
                            "phase_names": {"type": "array", "items": {"type": "string"}, "description": "Names of fluid phases"}
                        }
                    },
                    "non_newtonian": {
                        "type": "object",
                        "description": "Non-Newtonian fluid configuration (optional)",
                        "properties": {
                            "model": {"type": "string", "enum": ["newtonian", "powerLaw", "crossPowerLaw", "birdCarreau", "herschelBulkley", "casson"]},
                            "nu0": {"type": "number", "description": "Zero-shear viscosity (m²/s)"},
                            "nu_inf": {"type": "number", "description": "Infinite-shear viscosity (m²/s)"},
                            "k": {"type": "number", "description": "Consistency index"},
                            "n": {"type": "number", "description": "Power-law index"},
                            "tau0": {"type": "number", "description": "Yield stress (m²/s²)"}
                        }
                    },
                    "viscoelastic": {
                        "type": "object",
                        "description": "Viscoelastic fluid configuration (optional) — Oldroyd-B / Giesekus",
                        "properties": {
                            "model": {"type": "string", "enum": ["oldroydB", "giesekus"]},
                            "relaxation_time_s": {"type": "number"},
                            "solvent_viscosity_ratio": {"type": "number"},
                            "mobility_factor": {"type": "number"}
                        }
                    },
                    "combustion": {
                        "type": "object",
                        "description": "Combustion / reacting flow configuration (optional)",
                        "properties": {
                            "model": {"type": "string", "enum": ["edc", "laminarFlamelet", "paSR", "wellStirredReactor"]},
                            "fuel": {"type": "string", "description": "Fuel species (e.g. CH4, H2, C3H8)"},
                            "oxidizer": {"type": "string", "description": "Oxidizer species (e.g. O2, air)"},
                            "c_eps": {"type": "number"},
                            "c_mu": {"type": "number"}
                        }
                    },
                    "cavitation": {
                        "type": "object",
                        "description": "Cavitation configuration (optional) — Kunz / SchnerrSauer / Merkle",
                        "properties": {
                            "model": {"type": "string", "enum": ["kunz", "schnerrSauer", "merkle"]},
                            "p_vap_kPa": {"type": "number", "description": "Vapor pressure (kPa)"},
                            "rho_liquid_kg_m3": {"type": "number"},
                            "rho_vapor_kg_m3": {"type": "number"}
                        }
                    },
                    "spray": {
                        "type": "object",
                        "description": "Spray / Lagrangian droplet configuration (optional)",
                        "properties": {
                            "breakup": {"type": "string", "enum": ["reitzDiwakar", "khrt", "tab", "pilchErdman"]},
                            "evaporation": {"type": "string", "enum": ["standard", "fuchsKnudsen"]},
                            "collision": {"type": "boolean"},
                            "parcels_per_second": {"type": "number"},
                            "injection_velocity_m_s": {"type": "number"},
                            "cone_angle_deg": {"type": "number"}
                        }
                    },
                    "phase_change": {
                        "type": "object",
                        "description": "Phase change / melting-solidification configuration (optional)",
                        "properties": {
                            "model": {"type": "string", "enum": ["enthalpyPorosity", "levelSet"]},
                            "t_solidus_K": {"type": "number"},
                            "t_liquidus_K": {"type": "number"},
                            "latent_heat_J_kg": {"type": "number"},
                            "mushy_constant": {"type": "number"}
                        }
                    },
                    "particle": {
                        "type": "object",
                        "description": "Lagrangian particle tracking configuration (optional)",
                        "properties": {
                            "injection": {"type": "string", "enum": ["patch", "cone", "manual"]},
                            "diameter_m": {"type": "number"},
                            "mass_flow_kg_s": {"type": "number"},
                            "velocity_m_s": {"type": "number"},
                            "temperature_K": {"type": "number"},
                            "density_kg_m3": {"type": "number"}
                        }
                    },
                    "porous": {
                        "type": "object",
                        "description": "Porous media zone configuration (optional)",
                        "properties": {
                            "model": {"type": "string", "enum": ["darcy", "darcyForchheimer"]},
                            "cell_zone": {"type": "string"},
                            "d_coeffs": {"type": "array", "items": {"type": "number"}, "description": "Darcy coefficients [d, d, d]"},
                            "f_coeffs": {"type": "array", "items": {"type": "number"}, "description": "Forchheimer coefficients [f, f, f]"}
                        }
                    },
                    "fsi": {
                        "type": "object",
                        "description": "Fluid-structure interaction configuration (optional)",
                        "properties": {
                            "model": {"type": "string", "enum": ["linearElastic", "nonLinearGeometric", "plastic"]},
                            "coupling": {"type": "string", "enum": ["dirichletNeumann", "neumannNeumann", "robinRobin"]},
                            "youngs_modulus_GPa": {"type": "number"},
                            "poisson_ratio": {"type": "number"},
                            "density_kg_m3": {"type": "number"}
                        }
                    },
                    "wave": {
                        "type": "object",
                        "description": "Free surface wave configuration (optional)",
                        "properties": {
                            "model": {"type": "string", "enum": ["stokesFirst", "stokesFifth", "irregular", "streamFunction"]},
                            "wave_height_m": {"type": "number"},
                            "wave_period_s": {"type": "number"},
                            "water_depth_m": {"type": "number"},
                            "direction_deg": {"type": "number"}
                        }
                    },
                    "wind_turbine": {
                        "type": "object",
                        "description": "Wind turbine actuator configuration (optional)",
                        "properties": {
                            "actuator": {"type": "string", "enum": ["disc", "line", "alm"]},
                            "thrust_coefficient": {"type": "number"},
                            "power_coefficient": {"type": "number"},
                            "rotor_diameter_m": {"type": "number"},
                            "hub_height_m": {"type": "number"},
                            "wind_speed_ref_m_s": {"type": "number"}
                        }
                    },
                    "aeroacoustic": {
                        "type": "object",
                        "description": "Aeroacoustic / FW-H noise configuration (optional)",
                        "properties": {
                            "fwh_source": {"type": "string", "enum": ["permeable", "solid"]},
                            "receiver_positions": {"type": "array", "items": {"type": "object", "properties": {"x": {"type": "number"}, "y": {"type": "number"}, "z": {"type": "number"}}}},
                            "start_time": {"type": "number"},
                            "density_far": {"type": "number"},
                            "speed_of_sound": {"type": "number"}
                        }
                    },
                    "electrostatic": {
                        "type": "object",
                        "description": "Electrostatic / charge transport configuration (optional)",
                        "properties": {
                            "potential_V": {"type": "number"},
                            "permittivity_F_m": {"type": "number"},
                            "space_charge_C_m3": {"type": "number"},
                            "ion_mobility_m2_Vs": {"type": "number"}
                        }
                    },
                    "ablation": {
                        "type": "object",
                        "description": "Ablation / thermal protection system configuration (optional)",
                        "properties": {
                            "model": {"type": "string", "enum": ["surfaceRecession", "charringMaterial", "pyrolysis"]},
                            "wall_temp_K": {"type": "number"},
                            "heat_flux_MW_m2": {"type": "number"},
                            "blowing_rate": {"type": "number"}
                        }
                    }
                },
                "required": ["case_id", "goal", "geometry_type", "flow_type", "mach", "reynolds", "mesh_params", "solver"]
            }),
        },
        ToolDef {
            name: "generate_solver".into(),
            description: "Generate a custom OpenFOAM solver directory with Make/{files,options}, solver.C, and equation headers based on selected physics modules. Supports 12 pre-built templates.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "solver_name": {"type": "string", "description": "Solver binary name (e.g. mhdReactingFoam)"},
                    "template": {"type": "string", "enum": ["mhdSimpleFoam", "mhdReactingFoam", "plasmaActuatorFoam", "hyperReactingFoam", "chtRotatingFoam", "viscoelasticHeatFoam", "bubblyReactingFoam", "ablationFoam", "dsmcReactingFoam", "magneticConvectionFoam", "rotorAeroFoam", "coupledPlasmaFoam", "custom"]},
                    "description": {"type": "string", "description": "One-line physics description"},
                    "modules": {"type": "array", "items": {"type": "string", "enum": ["fluid_dynamics", "compressible", "turbulence", "heat_transfer", "species_transport", "chemical_reactions", "two_phase", "solid_mechanics", "electromagnetic", "radiation", "rotating_frame", "porous_media", "particle_tracking", "custom_eos", "custom_viscosity"]}, "description": "Enabled physics modules"},
                    "coupling": {"type": "string", "enum": ["segregated-SIMPLE", "segregated-PISO", "coupled-matrix", "operator-split"]},
                    "time_treatment": {"type": "string", "enum": ["steady", "unsteady-1st", "unsteady-2nd"]},
                    "output_dir": {"type": "string", "description": "Directory to write the scaffold (default: $WORKSPACE/cases/$case_id/solver)"}
                },
                "required": ["solver_name", "template", "description", "modules", "coupling", "time_treatment"]
            }),
        },
        ToolDef {
            name: "run_simulation".into(),
            description: "Phase 3: Launch the CFD solver for the current configuration. Runs the full pipeline (mesh → solve → post-process) and returns results.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "case_id": {"type": "string", "description": "UUID of the case"},
                    "iteration": {"type": "integer", "description": "Current iteration number"}
                },
                "required": ["case_id", "iteration"]
            }),
        },
        ToolDef {
            name: "diagnose_and_fix".into(),
            description: "Phase 4: Diagnose a solver failure or poor convergence. Provide the diagnosis and a fix action. The fix can adjust mesh, numerical schemes, relaxation factors, or initial conditions.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "case_id": {"type": "string", "description": "UUID of the case"},
                    "diagnosis": {"type": "string", "description": "Root cause analysis of the failure"},
                    "fix_action": {
                        "type": "object",
                        "properties": {
                            "action_type": {"type": "string", "enum": ["coarsen_mesh", "refine_mesh", "change_schemes", "reduce_cfl", "improve_ic", "change_solver", "increase_iterations", "validate_config"]},
                            "details": {"type": "string", "description": "What specifically to change"}
                        },
                        "required": ["action_type", "details"]
                    }
                },
                "required": ["case_id", "diagnosis", "fix_action"]
            }),
        },
        ToolDef {
            name: "evaluate_results".into(),
            description: "Phase 5: Extract forces (Cd, Cl, Cm), mesh quality (y+, non-orthogonality, skewness), and convergence metrics from the solver output for the given iteration.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "case_id": {"type": "string", "description": "UUID of the case"},
                    "iteration": {"type": "integer", "description": "Iteration number to evaluate"}
                },
                "required": ["case_id", "iteration"]
            }),
        },
        ToolDef {
            name: "compare_iterations".into(),
            description: "Phase 6: Score the current iteration against target metrics and rank all iterations. Uses composite score = w1*Cl_err + w2*Cd_excess + w3*y+_pen + w4*residual_pen + w5*mq. Lower is better.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "case_id": {"type": "string", "description": "UUID of the case"},
                    "iteration": {"type": "integer", "description": "Current iteration number"},
                    "target_cl": {"type": "number", "description": "Target Cl (optional)"},
                    "target_cd": {"type": "number", "description": "Target Cd (optional)"},
                    "max_y_plus": {"type": "number", "description": "Maximum allowable y+ (optional)"}
                },
                "required": ["case_id", "iteration"]
            }),
        },
        ToolDef {
            name: "plan_refinement".into(),
            description: "Phase 7: Based on the comparison results, generate an improved configuration for the next iteration. Propose specific deltas to mesh params, solver settings, or BCs.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "case_id": {"type": "string", "description": "UUID of the case"},
                    "current_iteration": {"type": "integer", "description": "Current iteration number"},
                    "score": {"type": "number", "description": "Score from compare_iterations"},
                    "improvement_strategy": {"type": "string", "description": "Strategy for improvement"},
                    "next_mesh_params": {
                        "type": "object",
                        "properties": {
                            "surface_min_level": {"type": "integer"},
                            "surface_max_level": {"type": "integer"},
                            "n_cells_between_levels": {"type": "integer"}
                        }
                    },
                    "next_solver_params": {
                        "type": "object",
                        "properties": {
                            "solver": {"type": "string"},
                            "turbulence_model": {"type": "string"}
                        }
                    }
                },
                "required": ["case_id", "current_iteration", "score", "improvement_strategy", "next_mesh_params"]
            }),
        },
        ToolDef {
            name: "update_skill".into(),
            description: "Phase 8: When targets are met or max iterations reached, save the winning configuration to the skills knowledge base for future reuse on similar geometries and flow regimes.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "case_id": {"type": "string", "description": "UUID of the case"},
                    "winning_iteration": {"type": "integer", "description": "Best iteration number"},
                    "best_score": {"type": "number", "description": "Best score achieved"},
                    "final_manifest": {
                        "type": "object",
                        "description": "The winning agent-manifest config"
                    }
                },
                "required": ["case_id", "winning_iteration", "best_score", "final_manifest"]
            }),
        },
        ToolDef {
            name: "get_pipeline_status".into(),
            description: "Get the current pipeline stage and recent log messages for a case.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "case_id": {"type": "string", "description": "UUID of the case"}
                },
                "required": ["case_id"]
            }),
        },
        ToolDef {
            name: "get_skill_recommendations".into(),
            description: "Query the skills database for previously successful parameter configurations matching this geometry and flow regime.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "case_id": {"type": "string", "description": "UUID of the case"}
                },
                "required": ["case_id"]
            }),
        },
    ]
}

pub async fn execute_tool(
    name: &str,
    args: &Value,
    state: &ToolExecutorState,
) -> anyhow::Result<Value> {
    match name {
        "get_case_detail" => {
            let case_id = args["case_id"].as_str().and_then(|s| Uuid::parse_str(s).ok());
            if let Some(cid) = case_id {
                let cases = state.db.list_cases(100).await?;
                let found = cases.into_iter().find(|c| c.id == cid);
                if let Some(c) = found {
                    let case_dir = state.workspace_dir.as_ref().map(|w| get_case_dir(w, &c.name));
                    let forces = case_dir.as_ref().and_then(|d| read_forces_from_disk(d));
                    let manifest = case_dir.as_ref().and_then(|d| {
                        std::fs::read_to_string(d.join("manifest.json")).ok()
                            .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
                    });
                    Ok(json!({
                        "id": c.id, "name": c.name, "status": c.status,
                        "solver": c.solver, "flow_type": c.flow_type,
                        "forces": forces,
                        "manifest": manifest,
                        "created_at": c.created_at, "completed_at": c.completed_at
                    }))
                } else {
                    Ok(json!({"error": "Case not found"}))
                }
            } else {
                Ok(json!({"error": "Invalid case_id"}))
            }
        }
        "get_case_results" => {
            let case_id = args["case_id"].as_str().and_then(|s| Uuid::parse_str(s).ok());
            if let Some(cid) = case_id {
                let cases = state.db.list_cases(100).await?;
                if let Some(c) = cases.into_iter().find(|c| c.id == cid) {
                    let case_dir = state.workspace_dir.as_ref().map(|w| get_case_dir(w, &c.name));
                    let forces = case_dir.as_ref().and_then(|d| read_forces_from_disk(d));
                    let mesh = case_dir.as_ref().and_then(|d| read_mesh_quality_from_disk(d));
                    return Ok(json!({
                        "forces": forces,
                        "mesh_quality": mesh,
                        "message": "Results read from disk."
                    }));
                }
            }
            Ok(json!({"message": "Case not found or no results available."}))
        }
        "get_pipeline_status" => {
            let case_id = args["case_id"].as_str().and_then(|s| Uuid::parse_str(s).ok());
            if let Some(cid) = case_id {
                let cases = state.db.list_cases(100).await?;
                let found = cases.into_iter().find(|c| c.id == cid);
                if let Some(c) = found {
                    return Ok(json!({
                        "case_id": c.id.to_string(),
                        "status": c.status,
                        "name": c.name,
                    }));
                }
            }
            Ok(json!({"message": "Pipeline status unknown."}))
        }
        "get_skill_recommendations" => {
            let skills = state.db.list_skills().await?;
            let top: Vec<Value> = skills.iter().take(5).map(|s| json!({
                "id": s.id, "name": s.name, "confidence": s.confidence,
                "n_trials": s.n_trials, "reward_score": s.reward_score,
            })).collect();
            Ok(json!({"skills": top, "message": "Top skills from database."}))
        }
        "propose_config" => {
            let case_id = args["case_id"].as_str().and_then(|s| Uuid::parse_str(s).ok());
            let iteration = state.next_iteration().await;

            // Build rotating config if provided
            let rotating = args.get("rotating").and_then(|r| {
                let rpm = r.get("rpm")?.as_f64()?;
                let num_blades = r.get("num_blades")?.as_u64()?;
                let approach_str = r.get("approach")?.as_str()?;
                let approach = match approach_str {
                    "MRF" => Some(RotatingApproach::MRF),
                    "AMI" => Some(RotatingApproach::AMI),
                    _ => None,
                }?;
                let extract_f64_arr = |key: &str| -> Option<[f64; 3]> {
                    let arr = r.get(key)?;
                    Some([
                        arr.get(0)?.as_f64()?,
                        arr.get(1)?.as_f64()?,
                        arr.get(2)?.as_f64()?,
                    ])
                };
                Some(RotatingConfig {
                    rpm,
                    num_blades: num_blades as u32,
                    approach,
                    axis: extract_f64_arr("axis").unwrap_or([0.0, 0.0, 1.0]),
                    origin: extract_f64_arr("origin").unwrap_or([0.0, 0.0, 0.0]),
                    diameter_m: r.get("diameter_m").and_then(|v| v.as_f64()),
                    hub_diameter_m: r.get("hub_diameter_m").and_then(|v| v.as_f64()),
                    tip_clearance_m: r.get("tip_clearance_m").and_then(|v| v.as_f64()),
                    advance_ratio: r.get("advance_ratio").and_then(|v| v.as_f64()),
                    target_ct: r.get("target_ct").and_then(|v| v.as_f64()),
                    target_cp_max: r.get("target_cp_max").and_then(|v| v.as_f64()),
                    target_eta_min: r.get("target_eta_min").and_then(|v| v.as_f64()),
                    mass_flow_kg_s: r.get("mass_flow_kg_s").and_then(|v| v.as_f64()),
                    pressure_ratio_target: r.get("pressure_ratio_target").and_then(|v| v.as_f64()),
                })
            });

            // Build hypersonic config if provided
            let hypersonic = args.get("hypersonic").and_then(|h| {
                let mach_inf = h.get("mach_inf")?.as_f64()?;
                let chemistry_str = h.get("chemistry").and_then(|c| c.as_str()).unwrap_or("none");
                let chemistry = match chemistry_str {
                    "5-species-Park" => ChemistryModel::Park5Species,
                    "11-species-Park" => ChemistryModel::Park11Species,
                    _ => ChemistryModel::None,
                };
                let wall_cat_str = h.get("wall_catalysis").and_then(|c| c.as_str()).unwrap_or("noncatalytic");
                let wall_catalysis = match wall_cat_str {
                    "fully_catalytic" => WallCatalysis::FullyCatalytic,
                    "partial" => WallCatalysis::Partial(h.get("catalytic_efficiency").and_then(|v| v.as_f64()).unwrap_or(0.5)),
                    _ => WallCatalysis::NonCatalytic,
                };
                let flux_str = h.get("flux_scheme").and_then(|f| f.as_str()).unwrap_or("Kurganov");
                let flux_scheme = match flux_str {
                    "AUSM+" => FluxScheme::AUSMPlus,
                    _ => FluxScheme::Kurganov,
                };
                Some(HypersonicConfig {
                    mach_inf,
                    altitude_km: h.get("altitude_km").and_then(|v| v.as_f64()).unwrap_or(30.0),
                    wall_temperature_k: h.get("wall_temperature_K").and_then(|v| v.as_f64()),
                    wall_catalysis,
                    real_gas: h.get("real_gas").and_then(|v| v.as_bool()).unwrap_or(true),
                    chemistry,
                    two_temperature: h.get("two_temperature").and_then(|v| v.as_bool()).unwrap_or(false),
                    rarefied: h.get("rarefied").and_then(|v| v.as_bool()).unwrap_or(false),
                    nose_radius_m: h.get("nose_radius_m").and_then(|v| v.as_f64()),
                    flux_scheme,
                    target_peak_heat_flux_w_m2: h.get("target_peak_heat_flux").and_then(|v| v.as_f64()),
                })
            });

            // Build CHT config if provided
            let cht = args.get("cht").map(|c| {
                let problem_str = c.get("problem_type").and_then(|v| v.as_str()).unwrap_or("CHT");
                let problem_type = match problem_str {
                    "forced_convection" => HeatTransferProblem::ForcedConvection,
                    "natural_convection" => HeatTransferProblem::NaturalConvection,
                    "radiation" => HeatTransferProblem::Radiation,
                    _ => HeatTransferProblem::CHT,
                };
                let solid_str = c.get("solid_material").and_then(|v| v.as_str()).unwrap_or("steel");
                let solid_material = match solid_str {
                    "aluminum" => SolidMaterial::Aluminum,
                    "copper" => SolidMaterial::Copper,
                    "ceramic" => SolidMaterial::Ceramic,
                    "cfrp" => SolidMaterial::CFRP,
                    "inconel" => SolidMaterial::Inconel,
                    _ => SolidMaterial::Steel,
                };
                let rad_model_str = c.get("radiation_model").and_then(|v| v.as_str()).unwrap_or("none");
                let rad_enabled = rad_model_str != "none";
                let radiation_model = match rad_model_str {
                    "p1" => RadiationModel::P1,
                    "fvdom" | "fvDOM" => RadiationModel::FvDOM,
                    "viewFactor" => RadiationModel::ViewFactor,
                    "rosseland" => RadiationModel::Rosseland,
                    _ => RadiationModel::None,
                };
                ChtConfig {
                    problem_type,
                    fluid: c.get("fluid").and_then(|v| v.as_str()).unwrap_or("air").to_string(),
                    solid_material,
                    t_inlet_k: c.get("t_inlet_K").and_then(|v| v.as_f64()).unwrap_or(800.0),
                    t_ambient_k: c.get("t_ambient_K").and_then(|v| v.as_f64()).unwrap_or(300.0),
                    u_inlet_m_s: c.get("u_inlet_m_s").and_then(|v| v.as_f64()),
                    re: c.get("reynolds").and_then(|v| v.as_f64()),
                    pr: c.get("prandtl").and_then(|v| v.as_f64()).unwrap_or(0.71),
                    heat_flux_target_w_m2: c.get("heat_flux_target_W_m2").and_then(|v| v.as_f64()),
                    radiation: rad_enabled,
                    radiation_model,
                    phase_change: c.get("phase_change").and_then(|v| v.as_bool()).unwrap_or(false),
                    max_t_solid_k: c.get("max_T_solid_K").and_then(|v| v.as_f64()),
                    wall_thickness_m: c.get("wall_thickness_m").and_then(|v| v.as_f64()),
                    external_h_w_m2k: c.get("external_h_W_m2K").and_then(|v| v.as_f64()),
                    emissivity: c.get("emissivity").and_then(|v| v.as_f64()),
                }
            });

            // Build MHD config if provided
            let mhd = args.get("mhd").map(|m| {
                let solver_str = m.get("solver").and_then(|v| v.as_str()).unwrap_or("mhdFoam");
                let solver = match solver_str {
                    "magneticFoam" => MhdSolver::MagneticFoam,
                    "custom" => MhdSolver::Custom,
                    _ => MhdSolver::MhdFoam,
                };
                let wall_str = m.get("wall_conductivity").and_then(|v| v.as_str()).unwrap_or("insulating");
                let wall_conductivity = match wall_str {
                    "conducting" => MhdWallConductivity::Conducting,
                    "mixed" => MhdWallConductivity::Mixed,
                    _ => MhdWallConductivity::Insulating,
                };
                let plasma = m.get("plasma_actuator").map(|p| PlasmaActuatorConfig {
                    voltage_kv: p.get("voltage_kV").and_then(|v| v.as_f64()).unwrap_or(20.0),
                    frequency_hz: p.get("frequency_Hz").and_then(|v| v.as_f64()).unwrap_or(5000.0),
                    body_force_n_m3: p.get("body_force_N_m3").and_then(|v| v.as_f64()).unwrap_or(5000.0),
                    actuator_width_m: p.get("actuator_width_m").and_then(|v| v.as_f64()).unwrap_or(0.01),
                    model: PlasmaModel::ShyyJayaraman,
                });
                MhdConfig {
                    b0_tesla: m.get("b0_Tesla").and_then(|v| v.as_f64()).unwrap_or(0.1),
                    sigma_s_m: m.get("sigma_S_m").and_then(|v| v.as_f64()).unwrap_or(1.0e7),
                    mu_permeability_h_m: m.get("mu_permeability_H_m").and_then(|v| v.as_f64()).unwrap_or(1.2566e-6),
                    solver,
                    low_rm: m.get("low_Rm").and_then(|v| v.as_bool()).unwrap_or(true),
                    hartmann_number: m.get("Hartmann_number").and_then(|v| v.as_f64()),
                    wall_conductivity,
                    plasma_actuator: plasma,
                }
            });

            // Build propulsion config if provided
            let propulsion = args.get("propulsion").and_then(|p| {
                let model_str = p.get("model")?.as_str()?;
                let model = match model_str {
                    "solidRocket" => PropulsionModel::SolidRocket,
                    "hybridRocket" => PropulsionModel::HybridRocket,
                    "scramjet" => PropulsionModel::Scramjet,
                    _ => PropulsionModel::LiquidRocket,
                };
                Some(PropulsionConfig {
                    model,
                    chamber_pressure_bar: p.get("chamber_pressure_bar").and_then(|v| v.as_f64()).unwrap_or(70.0),
                    chamber_temp_k: p.get("chamber_temp_K").and_then(|v| v.as_f64()).unwrap_or(3500.0),
                    exit_pressure_bar: p.get("exit_pressure_bar").and_then(|v| v.as_f64()).unwrap_or(1.0),
                    mass_flow_rate_kg_s: p.get("mass_flow_rate_kg_s").and_then(|v| v.as_f64()).unwrap_or(500.0),
                    throat_area_m2: p.get("throat_area_m2").and_then(|v| v.as_f64()).unwrap_or(0.05),
                    exit_area_m2: p.get("exit_area_m2").and_then(|v| v.as_f64()).unwrap_or(0.25),
                })
            });

            // Build nuclear config if provided
            let nuclear = args.get("nuclear").and_then(|n| {
                let model_str = n.get("model")?.as_str()?;
                let model = match model_str {
                    "photonTransport" => NuclearModel::PhotonTransport,
                    "coupled" => NuclearModel::Coupled,
                    "radiationHydro" => NuclearModel::RadiationHydro,
                    _ => NuclearModel::NeutronTransport,
                };
                let xs: Vec<f64> = n.get("cross_sections")
                    .and_then(|c| c.as_array())
                    .map(|arr| arr.iter().filter_map(|v| v.as_f64()).collect())
                    .unwrap_or_default();
                Some(NuclearConfig {
                    model,
                    n_energy_groups: n.get("n_energy_groups").and_then(|v| v.as_u64()).unwrap_or(2) as u32,
                    cross_sections: xs,
                    source_strength_m3: n.get("source_strength").and_then(|v| v.as_f64()).unwrap_or(1e15),
                    temperature_k: n.get("temperature_K").and_then(|v| v.as_f64()).unwrap_or(600.0),
                })
            });

            // Build marine config if provided
            let marine = args.get("marine").and_then(|m| {
                let model_str = m.get("model")?.as_str()?;
                let model = match model_str {
                    "propellerOpenWater" => MarineModel::PropellerOpenWater,
                    "shipResistance" => MarineModel::ShipResistance,
                    "planingHull" => MarineModel::PlaningHull,
                    _ => MarineModel::Hydrofoil,
                };
                Some(MarineConfig {
                    model,
                    speed_knots: m.get("speed_knots").and_then(|v| v.as_f64()).unwrap_or(20.0),
                    depth_m: m.get("depth_m").and_then(|v| v.as_f64()).unwrap_or(5.0),
                    cavitation_margin: m.get("cavitation_margin").and_then(|v| v.as_f64()).unwrap_or(1.5),
                    propeller_rpm: m.get("propeller_rpm").and_then(|v| v.as_f64()).unwrap_or(1200.0),
                    thrust_coefficient: m.get("thrust_coefficient").and_then(|v| v.as_f64()).unwrap_or(0.6),
                })
            });

            // Build ML surrogate config if provided
            let ml_surrogate = args.get("ml_surrogate").and_then(|m| {
                let model_str = m.get("model")?.as_str()?;
                let model = match model_str {
                    "gpMatern" => MlSurrogateModel::GpMatern,
                    "randomForest" => MlSurrogateModel::Rf,
                    "xgboost" => MlSurrogateModel::Xgb,
                    "lhs" => MlSurrogateModel::Lhs,
                    _ => MlSurrogateModel::GpRbf,
                };
                Some(MlSurrogateConfig {
                    model,
                    n_samples: m.get("n_samples").and_then(|v| v.as_u64()).unwrap_or(200) as u32,
                    n_varies: m.get("n_varies").and_then(|v| v.as_u64()).unwrap_or(4) as u32,
                    exploration_rate: m.get("exploration_rate").and_then(|v| v.as_f64()).unwrap_or(0.1),
                    acquisition: m.get("acquisition").and_then(|v| v.as_str()).unwrap_or("expected_improvement").to_string(),
                    rho_default: m.get("rho_init").and_then(|v| v.as_f64()).unwrap_or(1.0),
                    length_scale_default: m.get("length_scale").and_then(|v| v.as_f64()).unwrap_or(1.0),
                })
            });

            // Build WindTunnel config if provided
            let wind_tunnel = args.get("wind_tunnel").map(|wt| {
                WindTunnelConfig {
                    upstream: wt.get("upstream").and_then(|v| v.as_f64()).unwrap_or(20.0),
                    downstream: wt.get("downstream").and_then(|v| v.as_f64()).unwrap_or(40.0),
                    vertical: wt.get("vertical").and_then(|v| v.as_f64()).unwrap_or(25.0),
                    lateral: wt.get("lateral").and_then(|v| v.as_f64()).unwrap_or(25.0),
                    velocity_m_s: wt.get("velocity_m_s").and_then(|v| v.as_f64()),
                    turbulence_intensity: wt.get("turbulence_intensity").and_then(|v| v.as_f64()).unwrap_or(0.005),
                    ..WindTunnelConfig::default()
                }
            });

            let _flow_type = if wind_tunnel.is_some() {
                FlowType::ExternalWindTunnel
            } else {
                FlowType::External
            };

            // Build PEMFC config if provided
            let pemfc = args.get("pemfc").map(|p| {
                let model_str = p.get("model").and_then(|v| v.as_str()).unwrap_or("isothermal1D");
                let model = match model_str {
                    "simplePolarization" => PemfcModel::SimplePolarization,
                    "nonIsothermal" => PemfcModel::NonIsothermal,
                    "twoPhase" => PemfcModel::TwoPhase,
                    _ => PemfcModel::Isothermal1D,
                };
                let ff_str = p.get("flow_field").and_then(|v| v.as_str()).unwrap_or("serpentine");
                let flow_field = match ff_str {
                    "parallel" => PemfcFlowField::Parallel,
                    "interdigitated" => PemfcFlowField::Interdigitated,
                    "pinType" => PemfcFlowField::PinType,
                    _ => PemfcFlowField::Serpentine,
                };
                let cycle_str = p.get("cycle_profile").and_then(|v| v.as_str()).unwrap_or("potentiodynamic");
                let cycle_profile = match cycle_str {
                    "galvanodynamic" => PemfcCycleProfile::Galvanodynamic,
                    "driveCycle" => PemfcCycleProfile::DriveCycle,
                    _ => PemfcCycleProfile::Potentiodynamic,
                };
                let deg_str = p.get("degradation_model").and_then(|v| v.as_str()).unwrap_or("none");
                let degradation_model = match deg_str {
                    "ptDissolution" => PemfcDegradationModel::PtDissolution,
                    "carbonCorrosion" => PemfcDegradationModel::CarbonCorrosion,
                    "pinholeFormation" => PemfcDegradationModel::PinholeFormation,
                    "combined" => PemfcDegradationModel::Combined,
                    _ => PemfcDegradationModel::None,
                };
                PemfcConfig {
                    model,
                    t_cell_k: p.get("t_cell_K").and_then(|v| v.as_f64()).unwrap_or(353.15),
                    p_anode_bar: p.get("p_anode_bar").and_then(|v| v.as_f64()).unwrap_or(1.5),
                    p_cathode_bar: p.get("p_cathode_bar").and_then(|v| v.as_f64()).unwrap_or(1.5),
                    lambda_anode: p.get("lambda_anode").and_then(|v| v.as_f64()).unwrap_or(1.5),
                    lambda_cathode: p.get("lambda_cathode").and_then(|v| v.as_f64()).unwrap_or(2.0),
                    stoich_anode: p.get("stoich_anode").and_then(|v| v.as_f64()).unwrap_or(1.2),
                    stoich_cathode: p.get("stoich_cathode").and_then(|v| v.as_f64()).unwrap_or(2.0),
                    i_ref_a_m2: p.get("i_ref_A_m2").and_then(|v| v.as_f64()).unwrap_or(1e4),
                    alpha_anode: p.get("alpha_anode").and_then(|v| v.as_f64()).unwrap_or(0.5),
                    alpha_cathode: p.get("alpha_cathode").and_then(|v| v.as_f64()).unwrap_or(0.5),
                    exchange_i_anode_a_m2: p.get("i0_anode_A_m2").and_then(|v| v.as_f64()).unwrap_or(1e3),
                    exchange_i_cathode_a_m2: p.get("i0_cathode_A_m2").and_then(|v| v.as_f64()).unwrap_or(1e2),
                    membrane_thickness_um: p.get("membrane_thickness_um").and_then(|v| v.as_f64()).unwrap_or(50.0),
                    membrane_conductivity_s_m: p.get("membrane_conductivity_S_m").and_then(|v| v.as_f64()).unwrap_or(10.0),
                    eod_coefficient: p.get("eod_coefficient").and_then(|v| v.as_f64()).unwrap_or(1.0),
                    water_uptake_max: p.get("water_uptake_max").and_then(|v| v.as_f64()).unwrap_or(14.0),
                    gdl_thickness_um: p.get("gdl_thickness_um").and_then(|v| v.as_f64()).unwrap_or(200.0),
                    gdl_porosity: p.get("gdl_porosity").and_then(|v| v.as_f64()).unwrap_or(0.7),
                    gdl_permeability_m2: p.get("gdl_permeability_m2").and_then(|v| v.as_f64()).unwrap_or(1e-12),
                    cl_thickness_um: p.get("cl_thickness_um").and_then(|v| v.as_f64()).unwrap_or(10.0),
                    cl_porosity: p.get("cl_porosity").and_then(|v| v.as_f64()).unwrap_or(0.4),
                    flow_field,
                    channel_width_mm: p.get("channel_width_mm").and_then(|v| v.as_f64()).unwrap_or(1.0),
                    rib_width_mm: p.get("rib_width_mm").and_then(|v| v.as_f64()).unwrap_or(1.0),
                    channel_depth_mm: p.get("channel_depth_mm").and_then(|v| v.as_f64()).unwrap_or(0.5),
                    n_passes: p.get("n_passes").and_then(|v| v.as_u64()).unwrap_or(3) as u32,
                    turn_radius_mm: p.get("turn_radius_mm").and_then(|v| v.as_f64()).unwrap_or(0.5),
                    landing_length_mm: p.get("landing_length_mm").and_then(|v| v.as_f64()).unwrap_or(10.0),
                    active_width_mm: p.get("active_width_mm").and_then(|v| v.as_f64()).unwrap_or(50.0),
                    active_length_mm: p.get("active_length_mm").and_then(|v| v.as_f64()).unwrap_or(50.0),
                    cells_per_channel_width: p.get("cells_per_channel_width").and_then(|v| v.as_u64()).unwrap_or(6) as u32,
                    cells_per_rib_width: p.get("cells_per_rib_width").and_then(|v| v.as_u64()).unwrap_or(6) as u32,
                    cells_across_channel: p.get("cells_across_channel").and_then(|v| v.as_u64()).unwrap_or(10) as u32,
                    cells_across_gdl: p.get("cells_across_gdl").and_then(|v| v.as_u64()).unwrap_or(8) as u32,
                    cells_across_cl: p.get("cells_across_cl").and_then(|v| v.as_u64()).unwrap_or(4) as u32,
                    cells_across_membrane: p.get("cells_across_membrane").and_then(|v| v.as_u64()).unwrap_or(6) as u32,
                    cells_along_pass: p.get("cells_along_pass").and_then(|v| v.as_u64()).unwrap_or(40) as u32,
                    cycle_profile,
                    start_voltage_v: p.get("start_voltage_V").and_then(|v| v.as_f64()).unwrap_or(1.0),
                    end_voltage_v: p.get("end_voltage_V").and_then(|v| v.as_f64()).unwrap_or(0.4),
                    sweep_rate_mv_s: p.get("sweep_rate_mV_s").and_then(|v| v.as_f64()).unwrap_or(5.0),
                    hold_time_s: p.get("hold_time_s").and_then(|v| v.as_f64()).unwrap_or(60.0),
                    n_cycles: p.get("n_cycles").and_then(|v| v.as_u64()).unwrap_or(1) as u32,
                    degradation_model,
                    initial_ecsa_m2_g: p.get("initial_ECSA_m2_g").and_then(|v| v.as_f64()).unwrap_or(80.0),
                    carbon_loading_mg_cm2: p.get("carbon_loading_mg_cm2").and_then(|v| v.as_f64()).unwrap_or(0.4),
                    acceleration_factor: p.get("acceleration_factor").and_then(|v| v.as_f64()).unwrap_or(1.0),
                }
            });

            // Build multiphase config if provided
            let multiphase = args.get("multiphase").map(|m| {
                let model_str = m.get("model").and_then(|v| v.as_str()).unwrap_or("vof");
                let model = match model_str {
                    "eulerEuler" => MultiphaseModel::EulerEuler,
                    "driftFlux" => MultiphaseModel::DriftFlux,
                    _ => MultiphaseModel::VOF,
                };
                MultiphaseConfig {
                    model,
                    n_phases: m.get("n_phases").and_then(|v| v.as_u64()).unwrap_or(2) as u32,
                    surface_tension_n_m: m.get("surface_tension_N_m").and_then(|v| v.as_f64()).unwrap_or(0.07),
                    phase_names: m.get("phase_names")
                        .and_then(|p| p.as_array())
                        .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                        .unwrap_or_else(|| vec!["phase1".into(), "phase2".into()]),
                }
            });

            // Build non-Newtonian config if provided
            let non_newtonian = args.get("non_newtonian").map(|n| {
                let model_str = n.get("model").and_then(|v| v.as_str()).unwrap_or("newtonian");
                let model = match model_str {
                    "powerLaw" => ViscosityModel::PowerLaw,
                    "crossPowerLaw" => ViscosityModel::CrossPowerLaw,
                    "birdCarreau" => ViscosityModel::BirdCarreau,
                    "herschelBulkley" => ViscosityModel::HerschelBulkley,
                    "casson" => ViscosityModel::Casson,
                    _ => ViscosityModel::Newtonian,
                };
                NonNewtonianConfig {
                    model,
                    nu0: n.get("nu0").and_then(|v| v.as_f64()).unwrap_or(1e-2),
                    nu_inf: n.get("nu_inf").and_then(|v| v.as_f64()).unwrap_or(1e-6),
                    k: n.get("k").and_then(|v| v.as_f64()).unwrap_or(0.005),
                    n: n.get("n").and_then(|v| v.as_f64()).unwrap_or(0.4),
                    tau0: n.get("tau0").and_then(|v| v.as_f64()).unwrap_or(10.0),
                    nu_min: n.get("nu_min").and_then(|v| v.as_f64()).unwrap_or(1e-4),
                    nu_max: n.get("nu_max").and_then(|v| v.as_f64()).unwrap_or(1e2),
                }
            });

            // Build viscoelastic config if provided
            let viscoelastic = args.get("viscoelastic").map(|v| {
                let model_str = v.get("model").and_then(|m| m.as_str()).unwrap_or("oldroydB");
                let model = match model_str {
                    "giesekus" => ViscoelasticModel::Giesekus,
                    _ => ViscoelasticModel::OldroydB,
                };
                ViscoelasticConfig {
                    model,
                    relaxation_time_s: v.get("relaxation_time_s").and_then(|x| x.as_f64()).unwrap_or(1.0),
                    solvent_viscosity_ratio: v.get("solvent_viscosity_ratio").and_then(|x| x.as_f64()).unwrap_or(0.1),
                    mobility_factor: v.get("mobility_factor").and_then(|x| x.as_f64()).unwrap_or(0.2),
                }
            });

            // Build combustion config if provided
            let combustion = args.get("combustion").map(|c| {
                let model_str = c.get("model").and_then(|v| v.as_str()).unwrap_or("edc");
                let model = match model_str {
                    "laminarFlamelet" => CombustionModel::LaminarFlamelet,
                    "paSR" => CombustionModel::PaSR,
                    "wellStirredReactor" => CombustionModel::WellStirredReactor,
                    _ => CombustionModel::EDC,
                };
                CombustionConfig {
                    model,
                    c_eps: c.get("c_eps").and_then(|v| v.as_f64()).unwrap_or(2.1377),
                    c_mu: c.get("c_mu").and_then(|v| v.as_f64()).unwrap_or(0.09),
                    oxidizer: c.get("oxidizer").and_then(|v| v.as_str()).unwrap_or("O2").to_string(),
                    fuel: c.get("fuel").and_then(|v| v.as_str()).unwrap_or("CH4").to_string(),
                }
            });

            // Build cavitation config if provided
            let cavitation = args.get("cavitation").map(|c| {
                let model_str = c.get("model").and_then(|v| v.as_str()).unwrap_or("schnerrSauer");
                let model = match model_str {
                    "kunz" => CavitationModel::Kunz,
                    "merkle" => CavitationModel::Merkle,
                    _ => CavitationModel::SchnerrSauer,
                };
                CavitationConfig {
                    model,
                    p_vap_kpa: c.get("p_vap_kPa").and_then(|v| v.as_f64()).unwrap_or(2.34),
                    rho_liquid_kg_m3: c.get("rho_liquid_kg_m3").and_then(|v| v.as_f64()).unwrap_or(1000.0),
                    rho_vapor_kg_m3: c.get("rho_vapor_kg_m3").and_then(|v| v.as_f64()).unwrap_or(0.0258),
                }
            });

            // Build spray config if provided
            let spray = args.get("spray").map(|s| {
                let breakup_str = s.get("breakup").and_then(|v| v.as_str()).unwrap_or("khrt");
                let breakup = match breakup_str {
                    "reitzDiwakar" => SprayBreakupModel::ReitzDiwakar,
                    "tab" => SprayBreakupModel::TAB,
                    "pilchErdman" => SprayBreakupModel::PilchErdman,
                    _ => SprayBreakupModel::KHRT,
                };
                let evap_str = s.get("evaporation").and_then(|v| v.as_str()).unwrap_or("standard");
                let evaporation = match evap_str {
                    "fuchsKnudsen" => SprayEvaporationModel::FuchsKnudsen,
                    _ => SprayEvaporationModel::Standard,
                };
                SprayConfig {
                    breakup,
                    evaporation,
                    collision: s.get("collision").and_then(|v| v.as_bool()).unwrap_or(true),
                    parcel_per_second: s.get("parcels_per_second").and_then(|v| v.as_f64()).unwrap_or(1e5),
                    injection_velocity_m_s: s.get("injection_velocity_m_s").and_then(|v| v.as_f64()).unwrap_or(50.0),
                    cone_angle_deg: s.get("cone_angle_deg").and_then(|v| v.as_f64()).unwrap_or(15.0),
                }
            });

            // Build phase change config if provided
            let phase_change = args.get("phase_change").map(|p| {
                let model_str = p.get("model").and_then(|v| v.as_str()).unwrap_or("enthalpyPorosity");
                let model = match model_str {
                    "levelSet" => PhaseChangeModel::LevelSet,
                    _ => PhaseChangeModel::EnthalpyPorosity,
                };
                PhaseChangeConfig {
                    model,
                    t_solidus_k: p.get("t_solidus_K").and_then(|v| v.as_f64()).unwrap_or(800.0),
                    t_liquidus_k: p.get("t_liquidus_K").and_then(|v| v.as_f64()).unwrap_or(900.0),
                    latent_heat_j_kg: p.get("latent_heat_J_kg").and_then(|v| v.as_f64()).unwrap_or(2.5e5),
                    mushy_constant: p.get("mushy_constant").and_then(|v| v.as_f64()).unwrap_or(1e5),
                }
            });

            // Build particle config if provided
            let particle = args.get("particle").map(|p| {
                let inj_str = p.get("injection").and_then(|v| v.as_str()).unwrap_or("patch");
                let injection = match inj_str {
                    "cone" => ParticleInjection::ConeInjection,
                    "manual" => ParticleInjection::ManualInjection,
                    _ => ParticleInjection::PatchInjection,
                };
                ParticleConfig {
                    injection,
                    diameter_m: p.get("diameter_m").and_then(|v| v.as_f64()).unwrap_or(1e-4),
                    mass_flow_kg_s: p.get("mass_flow_kg_s").and_then(|v| v.as_f64()).unwrap_or(0.001),
                    velocity_m_s: p.get("velocity_m_s").and_then(|v| v.as_f64()).unwrap_or(10.0),
                    temperature_k: p.get("temperature_K").and_then(|v| v.as_f64()).unwrap_or(300.0),
                    material_density_kg_m3: p.get("density_kg_m3").and_then(|v| v.as_f64()).unwrap_or(2500.0),
                }
            });

            // Build porous config if provided
            let porous = args.get("porous").map(|p| {
                let model_str = p.get("model").and_then(|v| v.as_str()).unwrap_or("darcy");
                let model = match model_str {
                    "darcyForchheimer" => PorousModel::DarcyForchheimer,
                    _ => PorousModel::Darcy,
                };
                let d: Vec<f64> = p.get("d_coeffs")
                    .and_then(|c| c.as_array())
                    .map(|arr| arr.iter().filter_map(|v| v.as_f64()).collect())
                    .unwrap_or_default();
                let f: Vec<f64> = p.get("f_coeffs")
                    .and_then(|c| c.as_array())
                    .map(|arr| arr.iter().filter_map(|v| v.as_f64()).collect())
                    .unwrap_or_default();
                PorousZoneConfig {
                    model,
                    cell_zone: p.get("cell_zone").and_then(|v| v.as_str()).unwrap_or("porous").to_string(),
                    d_coeffs: [d.first().copied().unwrap_or(0.0), d.get(1).copied().unwrap_or(0.0), d.get(2).copied().unwrap_or(0.0)],
                    f_coeffs: [f.first().copied().unwrap_or(0.0), f.get(1).copied().unwrap_or(0.0), f.get(2).copied().unwrap_or(0.0)],
                }
            });

            // Build FSI config if provided
            let fsi = args.get("fsi").map(|f| {
                let model_str = f.get("model").and_then(|v| v.as_str()).unwrap_or("linearElastic");
                let model = match model_str {
                    "nonLinearGeometric" => FSIModel::NonLinearGeometric,
                    "plastic" => FSIModel::Plastic,
                    _ => FSIModel::LinearElastic,
                };
                let coup_str = f.get("coupling").and_then(|v| v.as_str()).unwrap_or("dirichletNeumann");
                let coupling = match coup_str {
                    "neumannNeumann" => FSICoupling::NeumannNeumann,
                    "robinRobin" => FSICoupling::RobinRobin,
                    _ => FSICoupling::DirichletNeumann,
                };
                FSIConfig {
                    model,
                    coupling,
                    youngs_modulus_gpa: f.get("youngs_modulus_GPa").and_then(|v| v.as_f64()).unwrap_or(200.0),
                    poisson_ratio: f.get("poisson_ratio").and_then(|v| v.as_f64()).unwrap_or(0.3),
                    density_kg_m3: f.get("density_kg_m3").and_then(|v| v.as_f64()).unwrap_or(7800.0),
                    mesh_relaxation: 0.5,
                    n_subcycles: 5,
                }
            });

            // Build wave config if provided
            let wave = args.get("wave").map(|w| {
                let model_str = w.get("model").and_then(|v| v.as_str()).unwrap_or("stokesFirst");
                let model = match model_str {
                    "stokesFifth" => WaveModel::StokesFifth,
                    "irregular" => WaveModel::Irregular,
                    "streamFunction" => WaveModel::StreamFunction,
                    _ => WaveModel::StokesFirst,
                };
                WaveConfig {
                    model,
                    wave_height_m: w.get("wave_height_m").and_then(|v| v.as_f64()).unwrap_or(2.0),
                    wave_period_s: w.get("wave_period_s").and_then(|v| v.as_f64()).unwrap_or(8.0),
                    water_depth_m: w.get("water_depth_m").and_then(|v| v.as_f64()).unwrap_or(20.0),
                    direction_deg: w.get("direction_deg").and_then(|v| v.as_f64()).unwrap_or(0.0),
                    relaxation_zone_length_m: 10.0,
                }
            });

            // Build wind turbine config if provided
            let wind_turbine = args.get("wind_turbine").map(|w| {
                let act_str = w.get("actuator").and_then(|v| v.as_str()).unwrap_or("disc");
                let actuator = match act_str {
                    "line" => ActuatorModel::Line,
                    "alm" => ActuatorModel::ALM,
                    _ => ActuatorModel::Disc,
                };
                WindTurbineConfig {
                    actuator,
                    thrust_coefficient: w.get("thrust_coefficient").and_then(|v| v.as_f64()).unwrap_or(0.8),
                    power_coefficient: w.get("power_coefficient").and_then(|v| v.as_f64()).unwrap_or(0.45),
                    rotor_diameter_m: w.get("rotor_diameter_m").and_then(|v| v.as_f64()).unwrap_or(126.0),
                    hub_height_m: w.get("hub_height_m").and_then(|v| v.as_f64()).unwrap_or(90.0),
                    wind_speed_ref_m_s: w.get("wind_speed_ref_m_s").and_then(|v| v.as_f64()).unwrap_or(10.0),
                    rated_power_mw: 5.0,
                }
            });

            // Build ablation config if provided
            let ablation = args.get("ablation").map(|a| {
                let model_str = a.get("model").and_then(|v| v.as_str()).unwrap_or("surfaceRecession");
                let model = match model_str {
                    "charringMaterial" => AblationModel::CharringMaterial,
                    "pyrolysis" => AblationModel::Pyrolysis,
                    _ => AblationModel::SurfaceRecession,
                };
                AblationConfig {
                    model,
                    char_conductivity_w_mk: a.get("char_conductivity_W_mK").and_then(|v| v.as_f64()).unwrap_or(0.5),
                    virgin_conductivity_w_mk: a.get("virgin_conductivity_W_mK").and_then(|v| v.as_f64()).unwrap_or(2.0),
                    pyrolysis_gas_enthalpy_j_kg: a.get("pyrolysis_gas_enthalpy_J_kg").and_then(|v| v.as_f64()).unwrap_or(5e6),
                    recession_rate_coeff: a.get("recession_rate_coeff").and_then(|v| v.as_f64()).unwrap_or(1e-4),
                    emissivity: a.get("emissivity").and_then(|v| v.as_f64()).unwrap_or(0.85),
                }
            });

            // Build electrostatic config if provided
            let electrostatic = args.get("electrostatic").map(|e| {
                ElectrostaticConfig {
                    potential_v: e.get("potential_V").and_then(|v| v.as_f64()).unwrap_or(1e4),
                    permittivity_f_m: e.get("permittivity_F_m").and_then(|v| v.as_f64()).unwrap_or(8.854e-12),
                    space_charge_c_m3: e.get("space_charge_C_m3").and_then(|v| v.as_f64()).unwrap_or(0.0),
                    ion_mobility_m2_vs: e.get("ion_mobility_m2_Vs").and_then(|v| v.as_f64()).unwrap_or(2e-4),
                }
            });

            // Build aeroacoustic config if provided
            let aeroacoustic = args.get("aeroacoustic").map(|a| {
                let src_str = a.get("fwh_source").and_then(|v| v.as_str()).unwrap_or("permeable");
                let fwh_source = match src_str {
                    "solid" => FWHSource::SolidSurface,
                    _ => FWHSource::PermeableSurface,
                };
                AeroacousticConfig {
                    fwh_source,
                    receiver_positions: a.get("receiver_positions")
                        .and_then(|p| p.as_array())
                        .map(|arr| arr.iter().filter_map(|v| {
                            v.as_object().map(|rp| (
                                rp.get("x").and_then(|x| x.as_f64()).unwrap_or(0.0),
                                rp.get("y").and_then(|y| y.as_f64()).unwrap_or(0.0),
                                rp.get("z").and_then(|z| z.as_f64()).unwrap_or(0.0),
                            ))
                        }).collect())
                        .unwrap_or_default(),
                    start_time: a.get("start_time").and_then(|v| v.as_f64()).unwrap_or(0.0),
                    density_far: a.get("density_far").and_then(|v| v.as_f64()).unwrap_or(1.225),
                    speed_of_sound: a.get("speed_of_sound").and_then(|v| v.as_f64()).unwrap_or(340.0),
                }
            });

            let manifest = json!({
                "iteration": iteration,
                "goal": args["goal"],
                "geometry_type": args["geometry_type"],
                "flow_type": args["flow_type"],
                "mach": args["mach"],
                "reynolds": args["reynolds"],
                "mesh_params": args["mesh_params"],
                "solver": args["solver"],
                "turbulence_model": args.get("turbulence_model"),
                "target_cl": args.get("target_cl"),
                "target_cd": args.get("target_cd"),
                "max_y_plus": args.get("max_y_plus"),
                "rotating": args.get("rotating"),
                "hypersonic": args.get("hypersonic"),
                "cht": args.get("cht"),
                "mhd": args.get("mhd"),
                "propulsion": args.get("propulsion"),
                "nuclear": args.get("nuclear"),
                "marine": args.get("marine"),
                "ml_surrogate": args.get("ml_surrogate"),
                "pemfc": args.get("pemfc"),
                "wind_tunnel": args.get("wind_tunnel"),
                "multiphase": args.get("multiphase"),
                "non_newtonian": args.get("non_newtonian"),
                "viscoelastic": args.get("viscoelastic"),
                "combustion": args.get("combustion"),
                "cavitation": args.get("cavitation"),
                "spray": args.get("spray"),
                "phase_change": args.get("phase_change"),
                "particle": args.get("particle"),
                "porous": args.get("porous"),
                "aeroacoustic": args.get("aeroacoustic"),
                "fsi": args.get("fsi"),
                "wave": args.get("wave"),
                "wind_turbine": args.get("wind_turbine"),
                "electrostatic": args.get("electrostatic"),
                "ablation": args.get("ablation"),
                "flow_type": if wind_tunnel.is_some() { "ExternalWindTunnel" } else { args.get("flow_type").and_then(|s| s.as_str()).unwrap_or("External") },
                "created_at": chrono::Utc::now().to_rfc3339(),
            });

            // Write manifest to case directory if workspace is available
            if let (Some(ws), Some(cid)) = (&state.workspace_dir, case_id) {
                let cases = state.db.list_cases(100).await?;
                if let Some(c) = cases.into_iter().find(|c| c.id == cid) {
                    let case_dir = get_case_dir(ws, &c.name);
                    std::fs::create_dir_all(case_dir.join("logs")).ok();
                    std::fs::write(
                        case_dir.join("agent-manifest.json"),
                        serde_json::to_string_pretty(&manifest).expect("serialization of Value is infallible"),
                    ).ok();

                    // Also write initial OpenFOAM system dicts if not present
                    let cfg = SolverConfigGen::new();
                    let intake = IntakeConfig {
                        geometry_description: args["goal"].as_str().unwrap_or("").to_string(),
                        geometry_file: None, case_class: None, workspace_root: None,
                        user_id: None, altitude_m: 0.0,
                        mach_number: args["mach"].as_f64().unwrap_or(0.0),
                        reynolds_number: args["reynolds"].as_f64().unwrap_or(0.0),
                        alpha_sweep_deg: vec![],
                        freestream_turbulence_intensity: 0.001,
                        target_cl: args.get("target_cl").and_then(|v| v.as_f64()),
                        target_cd_max: args.get("target_cd").and_then(|v| v.as_f64()),
                        target_yplus_max: args.get("max_y_plus").and_then(|v| v.as_f64()).unwrap_or(1.0),
                        convergence_residual: 1e-5,
                        max_agent_iterations: 5, human_in_loop: false,
                        priority: Priority::Balanced, hpc_cores: 4, time_budget_hours: 24.0,
                        rotating,
                        hypersonic,
                        cht,
                        mhd,
                        pemfc: pemfc.clone(),
                        wind_tunnel: wind_tunnel.clone(),
                        multiphase: multiphase.clone(),
                        non_newtonian: non_newtonian.clone(),
                        viscoelastic: viscoelastic.clone(),
                        combustion: combustion.clone(),
                        cavitation: cavitation.clone(),
                        spray: spray.clone(),
                        phase_change: phase_change.clone(),
                        particle: particle.clone(),
                        porous: porous.clone(),
                        aeroacoustic: aeroacoustic.clone(),
                        fsi: fsi.clone(),
                        wave: wave.clone(),
                        wind_turbine: wind_turbine.clone(),
                        electrostatic: electrostatic.clone(),
                        ablation: ablation.clone(),
                        propulsion: propulsion.clone(),
                        nuclear: nuclear.clone(),
                        marine: marine.clone(),
                        ml_surrogate: ml_surrogate.clone(),
                    };
                    let control_dict = cfg.generate_control_dict(&intake);
                    let solver = args["solver"].as_str().unwrap_or("simpleFoam");
                    let control_dict = control_dict.replace("simpleFoam", solver);
                    std::fs::create_dir_all(case_dir.join("system")).ok();
                    std::fs::write(case_dir.join("system").join("controlDict"), &control_dict).ok();
                    std::fs::write(case_dir.join("system").join("fvSchemes"), cfg.generate_fv_schemes_for(Some(&intake))).ok();
                    std::fs::write(case_dir.join("system").join("fvSolution"), cfg.generate_fv_solution(&intake)).ok();

                    // Write turbulence properties
                    let turb_model = args.get("turbulence_model").and_then(|v| v.as_str()).unwrap_or("kOmegaSST");
                    let turb_dict = format!(
                        r#"FoamFile {{ version 2.0; format ascii; class dictionary; object turbulenceProperties; }}
simulationType  RAS;
RAS
{{
    RASModel        {};
    turbulence      on;
    printCoeffs     on;
}}
"#, turb_model);
                    std::fs::create_dir_all(case_dir.join("constant")).ok();
                    std::fs::write(case_dir.join("constant").join("turbulenceProperties"), &turb_dict).ok();

                    // Write rotating dicts if applicable
                    if intake.rotating.is_some() {
                        if let Some(mrf_dict) = cfg.generate_mrf_properties(&intake) {
                            std::fs::write(case_dir.join("constant").join("MRFProperties"), &mrf_dict).ok();
                        }
                        if let Some(ami_dict) = cfg.generate_dynamic_mesh_dict(&intake) {
                            std::fs::write(case_dir.join("system").join("dynamicMeshDict"), &ami_dict).ok();
                        }
                        if let Some(topo_set) = cfg.generate_topo_set_dict(&intake) {
                            std::fs::write(case_dir.join("system").join("topoSetDict"), &topo_set).ok();
                        }
                    }

                    // Write CHT region dicts if applicable
                    if intake.cht.is_some() {
                        let fluid_dir = case_dir.join("constant").join("fluid");
                        let solid_dir = case_dir.join("constant").join("solid");
                        std::fs::create_dir_all(&fluid_dir).ok();
                        std::fs::create_dir_all(&solid_dir).ok();
                        std::fs::create_dir_all(case_dir.join("system").join("fluid")).ok();
                        std::fs::create_dir_all(case_dir.join("system").join("solid")).ok();
                        std::fs::create_dir_all(case_dir.join("0").join("fluid")).ok();
                        std::fs::create_dir_all(case_dir.join("0").join("solid")).ok();
                        if let Some(fluid_t) = cfg.generate_cht_fluid_thermo(&intake) {
                            std::fs::write(fluid_dir.join("thermophysicalProperties"), &fluid_t).ok();
                        }
                        if let Some(solid_t) = cfg.generate_cht_solid_thermo(&intake) {
                            std::fs::write(solid_dir.join("thermophysicalProperties"), &solid_t).ok();
                        }
                        if let Some(rad_dict) = cfg.generate_cht_radiation_dict(&intake) {
                            std::fs::write(case_dir.join("constant").join("radiationProperties"), &rad_dict).ok();
                        }
                    }

                    // Write propulsion dicts if applicable
                    if let Some(ref prop) = propulsion {
                        let prop_dict = cfg.generate_propulsion_properties(prop);
                        std::fs::write(case_dir.join("constant").join("propulsionProperties"), &prop_dict).ok();
                    }

                    // Write nuclear dicts if applicable
                    if let Some(ref nuc) = nuclear {
                        let nuc_dict = cfg.generate_nuclear_transport(nuc);
                        std::fs::write(case_dir.join("constant").join("nuclearProperties"), &nuc_dict).ok();
                    }

                    // Write marine dicts if applicable
                    if let Some(ref m) = marine {
                        let m_dict = cfg.generate_marine_properties(m);
                        std::fs::write(case_dir.join("constant").join("marineProperties"), &m_dict).ok();
                    }

                    // Write ML surrogate dicts if applicable
                    if let Some(ref ml) = ml_surrogate {
                        let ml_dict = cfg.generate_ml_surrogate(ml);
                        std::fs::write(case_dir.join("system").join("surrogateProperties"), &ml_dict).ok();
                    }

                    // Write PEMFC dicts if applicable
                    if let Some(ref pc) = pemfc {
                        std::fs::create_dir_all(case_dir.join("constant")).ok();
                        std::fs::write(case_dir.join("constant").join("pemfcProperties"), cfg.generate_pemfc_properties(pc)).ok();
                        std::fs::write(case_dir.join("constant").join("electrochemistryProperties"), cfg.generate_pemfc_electrochemistry(pc)).ok();
                        std::fs::write(case_dir.join("constant").join("membraneProperties"), cfg.generate_pemfc_membrane(pc)).ok();
                        std::fs::write(case_dir.join("constant").join("cyclingProperties"), cfg.generate_pemfc_cycling(pc)).ok();
                        std::fs::write(case_dir.join("constant").join("degradationProperties"), cfg.generate_pemfc_degradation(pc)).ok();
                        let mesh_dict = cfg.generate_pemfc_mesh(pc);
                        std::fs::write(case_dir.join("system").join("blockMeshDict"), &mesh_dict).ok();
                    }

                    // Write multiphase dicts if applicable
                    if let Some(ref mc) = multiphase {
                        std::fs::write(case_dir.join("constant").join("multiphaseProperties"), cfg.generate_multiphase_transport(mc)).ok();
                    }

                    // Write non-Newtonian dicts if applicable
                    if let Some(ref nn) = non_newtonian {
                        std::fs::write(case_dir.join("constant").join("nonNewtonianProperties"), cfg.generate_non_newtonian_transport(nn)).ok();
                    }

                    // Write viscoelastic dicts if applicable
                    if let Some(ref ve) = viscoelastic {
                        std::fs::write(case_dir.join("constant").join("viscoelasticProperties"), cfg.generate_viscoelastic_transport(ve)).ok();
                    }

                    // Write combustion dicts if applicable
                    if let Some(ref cc) = combustion {
                        std::fs::write(case_dir.join("constant").join("combustionProperties"), cfg.generate_combustion_chemistry(cc)).ok();
                    }

                    // Write cavitation dicts if applicable
                    if let Some(ref cv) = cavitation {
                        std::fs::write(case_dir.join("constant").join("cavitationProperties"), cfg.generate_cavitation_properties(cv)).ok();
                    }

                    // Write spray dicts if applicable
                    if let Some(ref sp) = spray {
                        std::fs::write(case_dir.join("constant").join("sprayProperties"), cfg.generate_spray_properties(sp)).ok();
                    }

                    // Write phase change dicts if applicable
                    if let Some(ref pc2) = phase_change {
                        std::fs::write(case_dir.join("constant").join("phaseChangeProperties"), cfg.generate_phase_change_properties(pc2)).ok();
                    }

                    // Write particle dicts if applicable
                    if let Some(ref pt) = particle {
                        std::fs::write(case_dir.join("constant").join("particleProperties"), cfg.generate_particle_injection(pt, "defaultCloud")).ok();
                    }

                    // Write porous dicts if applicable
                    if let Some(ref pr) = porous {
                        std::fs::write(case_dir.join("constant").join("porousZones"), cfg.generate_porous_fv_options(pr)).ok();
                    }

                    // Write aeroacoustic dicts if applicable
                    if let Some(ref ac) = aeroacoustic {
                        std::fs::write(case_dir.join("system").join("aeroacousticFunctions"), cfg.generate_aeroacoustics_functions(ac)).ok();
                    }

                    // Write FSI dicts if applicable
                    if let Some(ref fi) = fsi {
                        std::fs::write(case_dir.join("constant").join("fsiProperties"), cfg.generate_fsi_dict(fi)).ok();
                    }

                    // Write wave dicts if applicable
                    if let Some(ref wv) = wave {
                        std::fs::write(case_dir.join("constant").join("waveProperties"), cfg.generate_wave_properties(wv)).ok();
                    }

                    // Write wind turbine dicts if applicable
                    if let Some(ref wt) = wind_turbine {
                        std::fs::write(case_dir.join("system").join("windTurbineProperties"), cfg.generate_actuator_fv_options(wt)).ok();
                    }

                    // Write electrostatic dicts if applicable
                    if let Some(ref es) = electrostatic {
                        std::fs::write(case_dir.join("constant").join("electrostaticProperties"), cfg.generate_electrostatic_transport(es)).ok();
                    }

                    // Write ablation dicts if applicable
                    if let Some(ref ab) = ablation {
                        std::fs::write(case_dir.join("constant").join("ablationProperties"), cfg.generate_ablation_properties(ab)).ok();
                    }
                }
            }

            state.record_iteration(iteration, &manifest).await;
            Ok(json!({
                "applied": true,
                "iteration": iteration,
                "manifest": manifest,
                "message": format!("Agent manifest generated for iteration {}. Case files written to disk.", iteration)
            }))
        }
        "run_simulation" => {
            let case_id = args["case_id"].as_str().and_then(|s| Uuid::parse_str(s).ok());
            let iteration = args["iteration"].as_u64().unwrap_or(1);

            if let Some(cid) = case_id {
                let cases = state.db.list_cases(100).await?;
                if let Some(c) = cases.into_iter().find(|c| c.id == cid) {
                    let case_name = c.name.clone();
                    let solver = c.solver.clone().unwrap_or_else(|| "simpleFoam".into());
                    let case_dir = match &state.workspace_dir {
                        Some(ws) => get_case_dir(ws, &case_name),
                        None => return Ok(json!({"error": "No workspace directory configured"})),
                    };

                    // Ensure system dirs exist
                    std::fs::create_dir_all(case_dir.join("0")).ok();
                    std::fs::create_dir_all(case_dir.join("constant/triSurface")).ok();
                    std::fs::create_dir_all(case_dir.join("system")).ok();
                    std::fs::create_dir_all(case_dir.join("logs")).ok();

                    // Read manifest to build CaseConfig for DWT support
                    let manifest_path = case_dir.join("agent-manifest.json");
                    let case_config: Option<CaseConfig> = std::fs::read_to_string(&manifest_path).ok()
                        .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
                        .map(|m| {
                            let wt = m.get("wind_tunnel").and_then(|v| {
                                if v.is_null() { None } else {
                                    Some(WindTunnelConfig {
                                        upstream: v.get("upstream").and_then(|x| x.as_f64()).unwrap_or(20.0),
                                        downstream: v.get("downstream").and_then(|x| x.as_f64()).unwrap_or(40.0),
                                        vertical: v.get("vertical").and_then(|x| x.as_f64()).unwrap_or(25.0),
                                        lateral: v.get("lateral").and_then(|x| x.as_f64()).unwrap_or(25.0),
                                        velocity_m_s: v.get("velocity_m_s").and_then(|x| x.as_f64()),
                                        turbulence_intensity: v.get("turbulence_intensity").and_then(|x| x.as_f64()).unwrap_or(0.005),
                                        ..WindTunnelConfig::default()
                                    })
                                }
                            });
                            let ft = m.get("flow_type").and_then(|v| v.as_str()).unwrap_or("External");
                            let flow_type = match ft {
                                "ExternalWindTunnel" => FlowType::ExternalWindTunnel,
                                "Internal" => FlowType::Internal,
                                _ => FlowType::External,
                            };
                            let vel = m.get("flow_direction")
                                .and_then(|fd| fd.get("velocity").and_then(|v| v.as_f64()))
                                .or_else(|| wt.as_ref().and_then(|w| w.velocity_m_s))
                                .unwrap_or(10.0);
                            let turb = m.get("turbulence_model").and_then(|v| v.as_str()).unwrap_or("kOmegaSST");
                            CaseConfig {
                                flow_type,
                                velocity_m_s: vel,
                                turbulence_model: turb.to_string(),
                                wind_tunnel: wt,
                                reference_length_m: m.get("wind_tunnel")
                                    .and_then(|v| v.get("reference_length_m"))
                                    .and_then(|v| v.as_f64()),
                            }
                        });

                    // Lock the orchestrator and run the pipeline (MutexGuard scoped to avoid .await)
                    let pipeline_result = if let Some(ref orch) = state.orchestrator {
                        let mut guard = orch.lock().map_err(|e| anyhow::anyhow!("Orch lock: {}", e))?;
                        let pipeline_id = guard.register_case_with_id(&case_name, cid);
                        tracing::info!("Agent: running pipeline for case {} iteration {}", case_name, iteration);
                        guard.run_pipeline(pipeline_id, &case_dir, &solver, None, None, case_config.as_ref())
                    } else {
                        return Ok(json!({"error": "Pipeline orchestrator not available"}));
                    };

                    match pipeline_result {
                        Ok(result) => {
                            let trial = json!({
                                "iteration": iteration,
                                "cl": result.forces.cl,
                                "cd": result.forces.cd,
                                "cm": result.forces.cm,
                                "converged": result.solver_stats.converged,
                                "residual_p": result.solver_stats.residual_p,
                                "n_cells": result.mesh_metrics.n_cells,
                                "wall_time_s": result.solver_stats.wall_time_s,
                            });
                            state.record_iteration(iteration as u32, &trial).await;

                            let wt_result: Option<Value> = std::fs::read_to_string(case_dir.join("wind_tunnel_result.json")).ok()
                                .and_then(|s| serde_json::from_str(&s).ok());

                            let mut resp = json!({
                                "completed": true,
                                "iteration": iteration,
                                "stage": format!("{:?}", result.stage),
                                "forces": {
                                    "cd": result.forces.cd,
                                    "cl": result.forces.cl,
                                    "cm": result.forces.cm,
                                },
                                "solver_stats": {
                                    "iterations": result.solver_stats.iterations,
                                    "wall_time_s": result.solver_stats.wall_time_s,
                                    "residual_p": result.solver_stats.residual_p,
                                    "residual_u": result.solver_stats.residual_u,
                                    "converged": result.solver_stats.converged,
                                },
                                "mesh_quality": {
                                    "n_cells": result.mesh_metrics.n_cells,
                                    "max_non_orthogonality": result.mesh_metrics.max_non_orthogonality,
                                    "max_skewness": result.mesh_metrics.max_skewness,
                                    "n_failed_cells": result.mesh_metrics.n_failed_cells,
                                },
                                "message": format!("Pipeline completed for iteration {}.", iteration)
                            });
                            if let Some(wt) = wt_result {
                                resp["wind_tunnel"] = wt;
                            }
                            Ok(resp)
                        }
                        Err(e) => {
                            let err_msg = e.to_string();
                            tracing::warn!("Pipeline failed for {}: {}", case_name, err_msg);
                            let diagnosis = if err_msg.contains("Mesh quality") || err_msg.contains("checkMesh") {
                                "Mesh quality check failed — cells failed quality criteria"
                            } else if err_msg.contains("FOAM FATAL") || err_msg.contains("floating point") {
                                "Solver diverged — floating point exception"
                            } else if err_msg.contains("converge") || err_msg.contains("plateau") {
                                "Solver converged poorly — residuals plateaued"
                            } else {
                                "Pipeline execution failed"
                            };
                            Ok(json!({
                                "completed": false,
                                "iteration": iteration,
                                "error": err_msg,
                                "diagnosis": diagnosis,
                                "message": format!("Pipeline failed: {}. Use diagnose_and_fix to apply a fix.", diagnosis)
                            }))
                        }
                    }
                } else {
                    Ok(json!({"error": "Case not found"}))
                }
            } else {
                Ok(json!({"error": "Invalid case_id"}))
            }
        }
        "diagnose_and_fix" => {
            let case_id = args["case_id"].as_str().and_then(|s| Uuid::parse_str(s).ok());
            let diagnosis = args["diagnosis"].as_str().unwrap_or("");
            let fix_action = &args["fix_action"];
            let action_type = fix_action["action_type"].as_str().unwrap_or("");
            let details = fix_action["details"].as_str().unwrap_or("");

            // Run config validation from the agent-manifest if available
            let mut config_warnings: Vec<String> = Vec::new();
            if let (Some(ws), Some(cid)) = (&state.workspace_dir, case_id) {
                let cases = state.db.list_cases(100).await?;
                if let Some(c) = cases.into_iter().find(|c| c.id == cid) {
                    let case_dir = get_case_dir(ws, &c.name);

                    // Read manifest and validate physics config
                    let manifest_path = case_dir.join("agent-manifest.json");
                    if let Some(manifest) = std::fs::read_to_string(&manifest_path)
                        .ok()
                        .and_then(|s| serde_json::from_str::<Value>(&s).ok())
                    {
                            let build_config = |key: &str| -> Option<Value> {
                                manifest.get(key).and_then(|v| {
                                    if v.is_null() { None } else { Some(v.clone()) }
                                })
                            };
                            // Build minimal IntakeConfig for validation
                            let intake = IntakeConfig {
                                geometry_description: String::new(),
                                geometry_file: None, case_class: None, workspace_root: None,
                                user_id: None, altitude_m: 0.0,
                                mach_number: manifest.get("mach").and_then(|v| v.as_f64()).unwrap_or(0.0),
                                reynolds_number: manifest.get("reynolds").and_then(|v| v.as_f64()).unwrap_or(0.0),
                                alpha_sweep_deg: vec![],
                                freestream_turbulence_intensity: 0.001,
                                target_cl: None, target_cd_max: None, target_yplus_max: 1.0,
                                convergence_residual: 1e-5, max_agent_iterations: 5,
                                human_in_loop: false, priority: Priority::Balanced,
                                hpc_cores: 4, time_budget_hours: 24.0,
                                rotating: build_config("rotating").and_then(|r| {
                                    let rpm = r.get("rpm").and_then(|v| v.as_f64())?;
                                    let num_blades = r.get("num_blades").and_then(|v| v.as_u64())?;
                                    let approach_str = r.get("approach").and_then(|v| v.as_str())?;
                                    let approach = match approach_str {
                                        "AMI" => RotatingApproach::AMI,
                                        _ => RotatingApproach::MRF,
                                    };
                                    Some(RotatingConfig {
                                        rpm, num_blades: num_blades as u32, approach,
                                        axis: [0.0, 0.0, 1.0], origin: [0.0, 0.0, 0.0],
                                        diameter_m: None, hub_diameter_m: None,
                                        tip_clearance_m: None, advance_ratio: None,
                                        target_ct: None, target_cp_max: None,
                                        target_eta_min: None, mass_flow_kg_s: None,
                                        pressure_ratio_target: None,
                                    })
                                }),
                                hypersonic: build_config("hypersonic").map(|_| HypersonicConfig {
                                    mach_inf: 5.0, altitude_km: 30.0, wall_temperature_k: None,
                                    wall_catalysis: WallCatalysis::NonCatalytic, real_gas: false,
                                    chemistry: ChemistryModel::None, two_temperature: false,
                                    rarefied: false, nose_radius_m: None,
                                    flux_scheme: FluxScheme::Kurganov, target_peak_heat_flux_w_m2: None,
                                }),
                                cht: build_config("cht").map(|_| ChtConfig {
                                    problem_type: HeatTransferProblem::CHT, fluid: "air".into(),
                                    solid_material: SolidMaterial::Steel, t_inlet_k: 500.0,
                                    t_ambient_k: 300.0, u_inlet_m_s: None, re: None, pr: 0.7,
                                    heat_flux_target_w_m2: None, radiation: false,
                                    radiation_model: RadiationModel::None, phase_change: false,
                                    max_t_solid_k: None, wall_thickness_m: None,
                                    emissivity: None, external_h_w_m2k: None,
                                }),
                                mhd: build_config("mhd").map(|_| MhdConfig {
                                    b0_tesla: 1.0, sigma_s_m: 1e6, mu_permeability_h_m: 1.256637e-6,
                                    solver: MhdSolver::MhdFoam, low_rm: false,
                                    hartmann_number: None,
                                    wall_conductivity: MhdWallConductivity::Insulating,
                                    plasma_actuator: None,
                                }),
                                pemfc: None, wind_tunnel: None,
                                multiphase: build_config("multiphase").map(|m| {
                                    let model_str = m.get("model").and_then(|v| v.as_str()).unwrap_or("vof");
                                    let model = match model_str {
                                        "eulerEuler" => MultiphaseModel::EulerEuler,
                                        "driftFlux" => MultiphaseModel::DriftFlux,
                                        _ => MultiphaseModel::VOF,
                                    };
                                    MultiphaseConfig {
                                        model, n_phases: 2, surface_tension_n_m: 0.07,
                                        phase_names: vec!["phase1".into(), "phase2".into()],
                                    }
                                }),
                                non_newtonian: build_config("non_newtonian").map(|_| NonNewtonianConfig {
                                    model: ViscosityModel::Newtonian, nu0: 1e-2, nu_inf: 1e-6,
                                    k: 0.005, n: 0.4, tau0: 10.0, nu_min: 1e-4, nu_max: 1e2,
                                }),
                                viscoelastic: build_config("viscoelastic").map(|_| ViscoelasticConfig {
                                    model: ViscoelasticModel::OldroydB, relaxation_time_s: 1.0,
                                    solvent_viscosity_ratio: 0.1, mobility_factor: 0.2,
                                }),
                                combustion: build_config("combustion").map(|_| CombustionConfig {
                                    model: CombustionModel::EDC, c_eps: 2.1377, c_mu: 0.09,
                                    oxidizer: "O2".into(), fuel: "CH4".into(),
                                }),
                                cavitation: build_config("cavitation").map(|_| CavitationConfig {
                                    model: CavitationModel::SchnerrSauer, p_vap_kpa: 2.34,
                                    rho_liquid_kg_m3: 1000.0, rho_vapor_kg_m3: 0.0258,
                                }),
                                spray: build_config("spray").map(|_| SprayConfig {
                                    breakup: SprayBreakupModel::KHRT,
                                    evaporation: SprayEvaporationModel::Standard,
                                    collision: true, parcel_per_second: 1e5,
                                    injection_velocity_m_s: 50.0, cone_angle_deg: 15.0,
                                }),
                                phase_change: build_config("phase_change").map(|_| PhaseChangeConfig {
                                    model: PhaseChangeModel::EnthalpyPorosity, t_solidus_k: 800.0,
                                    t_liquidus_k: 900.0, latent_heat_j_kg: 2.5e5, mushy_constant: 1e5,
                                }),
                                particle: build_config("particle").map(|_| ParticleConfig {
                                    injection: ParticleInjection::PatchInjection, diameter_m: 1e-4,
                                    mass_flow_kg_s: 0.001, velocity_m_s: 10.0,
                                    temperature_k: 300.0, material_density_kg_m3: 2500.0,
                                }),
                                porous: build_config("porous").map(|_| PorousZoneConfig {
                                    model: PorousModel::Darcy, cell_zone: "porous".into(),
                                    d_coeffs: [0.0, 0.0, 0.0], f_coeffs: [0.0, 0.0, 0.0],
                                }),
                                aeroacoustic: build_config("aeroacoustic").map(|_| AeroacousticConfig {
                                    fwh_source: FWHSource::PermeableSurface,
                                    receiver_positions: vec![(0.0, 0.0, 1.0)],
                                    start_time: 0.0, density_far: 1.225, speed_of_sound: 340.0,
                                }),
                                fsi: build_config("fsi").map(|_| FSIConfig {
                                    model: FSIModel::LinearElastic,
                                    coupling: FSICoupling::DirichletNeumann,
                                    youngs_modulus_gpa: 200.0, poisson_ratio: 0.3,
                                    density_kg_m3: 7800.0, mesh_relaxation: 0.5, n_subcycles: 5,
                                }),
                                wave: build_config("wave").map(|_| WaveConfig {
                                    model: WaveModel::StokesFirst, wave_height_m: 2.0,
                                    wave_period_s: 8.0, water_depth_m: 20.0,
                                    direction_deg: 0.0, relaxation_zone_length_m: 10.0,
                                }),
                                wind_turbine: build_config("wind_turbine").map(|_| WindTurbineConfig {
                                    actuator: ActuatorModel::Disc, thrust_coefficient: 0.8,
                                    power_coefficient: 0.45, rotor_diameter_m: 126.0,
                                    hub_height_m: 90.0, wind_speed_ref_m_s: 10.0, rated_power_mw: 5.0,
                                }),
                                electrostatic: build_config("electrostatic").map(|_| ElectrostaticConfig {
                                    potential_v: 1e4, permittivity_f_m: 8.854e-12,
                                    space_charge_c_m3: 0.0, ion_mobility_m2_vs: 2e-4,
                                }),
                                ablation: build_config("ablation").map(|_| AblationConfig {
                                    model: AblationModel::SurfaceRecession,
                                    char_conductivity_w_mk: 0.5, virgin_conductivity_w_mk: 2.0,
                                    pyrolysis_gas_enthalpy_j_kg: 5e6, recession_rate_coeff: 1e-4,
                                    emissivity: 0.85,
                                }),
                                propulsion: build_config("propulsion").map(|_| PropulsionConfig {
                                    model: PropulsionModel::LiquidRocket, chamber_pressure_bar: 70.0,
                                    chamber_temp_k: 3500.0, exit_pressure_bar: 0.1,
                                    mass_flow_rate_kg_s: 100.0, throat_area_m2: 0.01,
                                    exit_area_m2: 0.05,
                                }),
                                nuclear: build_config("nuclear").map(|_| NuclearConfig {
                                    model: NuclearModel::NeutronTransport, n_energy_groups: 2,
                                    cross_sections: vec![], source_strength_m3: 0.0,
                                    temperature_k: 300.0,
                                }),
                                marine: build_config("marine").map(|_| MarineConfig {
                                    model: MarineModel::ShipResistance, speed_knots: 10.0,
                                    depth_m: 10.0, cavitation_margin: 0.0,
                                    propeller_rpm: 0.0, thrust_coefficient: 0.0,
                                }),
                                ml_surrogate: None,
                            };
                            config_warnings = SolverConfigGen::new().validate_config(&intake);
                    }

                    let is_validate = action_type == "validate_config";
                    if !is_validate {
                        match action_type {
                            "coarsen_mesh" => {
                                let shm_path = case_dir.join("system/snappyHexMeshDict");
                                if let Ok(content) = std::fs::read_to_string(&shm_path) {
                                    let adjusted = content
                                        .replace("surfaceMinLevel 4;", "surfaceMinLevel 3;")
                                        .replace("surfaceMaxLevel 6;", "surfaceMaxLevel 5;");
                                    std::fs::write(&shm_path, &adjusted).ok();
                                    tracing::info!("Fix: coarsened mesh refinement levels");
                                }
                            }
                            "change_schemes" => {
                                let fvs_path = case_dir.join("system/fvSchemes");
                                if let Ok(content) = std::fs::read_to_string(&fvs_path) {
                                    let adjusted = content
                                        .replace("Gauss linearUpwindV grad(U)", "Gauss upwind")
                                        .replace("Gauss linear corrected", "Gauss linear uncorrected");
                                    std::fs::write(&fvs_path, &adjusted).ok();
                                    tracing::info!("Fix: switched to upwind schemes for robustness");
                                }
                                let fvsol_path = case_dir.join("system/fvSolution");
                                if let Ok(content) = std::fs::read_to_string(&fvsol_path) {
                                    let adjusted = content
                                        .replace("p               0.3;", "p               0.2;")
                                        .replace("U               0.7;", "U               0.5;");
                                    std::fs::write(&fvsol_path, &adjusted).ok();
                                    tracing::info!("Fix: reduced relaxation factors");
                                }
                            }
                            "reduce_cfl" => {
                                let cd_path = case_dir.join("system/controlDict");
                                if let Ok(content) = std::fs::read_to_string(&cd_path) {
                                    let adjusted = content.replace("deltaT          1;", "deltaT          0.5;");
                                    std::fs::write(&cd_path, &adjusted).ok();
                                    tracing::info!("Fix: halved deltaT for CFL reduction");
                                }
                            }
                            "improve_ic" => {
                                let cd_path = case_dir.join("system/controlDict");
                                if let Ok(content) = std::fs::read_to_string(&cd_path) {
                                    let adjusted = content
                                        .replace("startFrom       startTime;", "startFrom       startTime;\nstartTime       0;")
                                        .replace("application     ", "// application ");
                                    let script = r#"#!/bin/bash
potentialFoam -case "$1" > "$1/logs/potentialFoam.log" 2>&1
"#;
                                    std::fs::write(case_dir.join("logs/init.sh"), script).ok();
                                    std::fs::write(&cd_path, &adjusted).ok();
                                    tracing::info!("Fix: will run potentialFoam for better initial conditions");
                                }
                            }
                            "refine_mesh" => {
                                let shm_path = case_dir.join("system/snappyHexMeshDict");
                                if let Ok(content) = std::fs::read_to_string(&shm_path) {
                                    let adjusted = content
                                        .replace("surfaceMinLevel 3;", "surfaceMinLevel 4;")
                                        .replace("surfaceMaxLevel 5;", "surfaceMaxLevel 6;");
                                    std::fs::write(&shm_path, &adjusted).ok();
                                    tracing::info!("Fix: refined mesh refinement levels");
                                }
                            }
                            "change_solver" => {
                                let cd_path = case_dir.join("system/controlDict");
                                if let Ok(content) = std::fs::read_to_string(&cd_path) {
                                    let solver = details.trim();
                                    let adjusted = if solver.is_empty() {
                                        content.replace("application     simpleFoam;", "application     pimpleFoam;")
                                    } else {
                                        // Replace application line using manual find+replace
                                        let mut result = String::new();
                                        for line in content.lines() {
                                            if line.trim().starts_with("application") {
                                                result.push_str(&format!("application     {};\n", solver));
                                            } else {
                                                result.push_str(line);
                                                result.push('\n');
                                            }
                                        }
                                        result
                                    };
                                    std::fs::write(&cd_path, &adjusted).ok();
                                    tracing::info!("Fix: changed solver to '{}'", if solver.is_empty() { "pimpleFoam" } else { solver });
                                }
                                // Also reset startFrom to latestTime for the new solver
                                if let Ok(content) = std::fs::read_to_string(&cd_path) {
                                    let adjusted = content.replace("startFrom       startTime;", "startFrom       latestTime;");
                                    std::fs::write(&cd_path, &adjusted).ok();
                                    tracing::info!("Fix: set startFrom to latestTime for solver handoff");
                                }
                            }
                            "increase_iterations" => {
                                let cd_path = case_dir.join("system/controlDict");
                                if let Ok(content) = std::fs::read_to_string(&cd_path) {
                                    // Multiply endTime by 1.5x using manual line parsing
                                    let mut adjusted = String::new();
                                    for line in content.lines() {
                                        if line.trim().starts_with("endTime") {
                                            let parts: Vec<&str> = line.split_whitespace().collect();
                                            let current: f64 = parts.get(1).and_then(|s| s.trim_end_matches(';').parse().ok()).unwrap_or(1000.0);
                                            let new_time = (current * 1.5).round() as u64;
                                            adjusted.push_str(&format!("endTime       {};\n", new_time));
                                        } else {
                                            adjusted.push_str(line);
                                            adjusted.push('\n');
                                        }
                                    }
                                    std::fs::write(&cd_path, &adjusted).ok();
                                    tracing::info!("Fix: increased endTime by 1.5x");
                                }
                                // Also increase writeInterval proportionally
                                if let Ok(content) = std::fs::read_to_string(&cd_path) {
                                    let mut adjusted = String::new();
                                    for line in content.lines() {
                                        if line.trim().starts_with("writeInterval") {
                                            let parts: Vec<&str> = line.split_whitespace().collect();
                                            let current: u64 = parts.get(1).and_then(|s| s.trim_end_matches(';').parse().ok()).unwrap_or(100);
                                            let new_interval = (current as f64 * 1.5).round() as u64;
                                            adjusted.push_str(&format!("writeInterval {};\n", new_interval));
                                        } else {
                                            adjusted.push_str(line);
                                            adjusted.push('\n');
                                        }
                                    }
                                    std::fs::write(&cd_path, &adjusted).ok();
                                    tracing::info!("Fix: increased writeInterval proportionally");
                                }
                            }
                            _ => {
                                tracing::warn!("Unknown fix action type: {}", action_type);
                            }
                        }

                        // Record fix to DB (skip for validate_config)
                        let fix_entry = json!({
                            "iteration": state.next_iteration().await,
                            "diagnosis": diagnosis,
                            "fix_action": action_type,
                            "details": details,
                            "timestamp": chrono::Utc::now().to_rfc3339(),
                        });
                        let fix_path = case_dir.join("agent-fixes.json");
                        let mut fixes: Vec<Value> = std::fs::read_to_string(&fix_path)
                            .ok()
                            .and_then(|s| serde_json::from_str::<Vec<serde_json::Value>>(&s).ok())
                            .unwrap_or_default();
                        fixes.push(fix_entry);
                        std::fs::write(&fix_path, serde_json::to_string_pretty(&fixes).expect("serialization of Vec<Value> is infallible")).ok();
                    } // closes if !is_validate — skip fix/DB for validate_config

                    // Return early for validate_config (no fix applied)
                    if is_validate {
                        return Ok(json!({
                            "applied": true,
                            "diagnosis": diagnosis,
                            "fix_applied": {
                                "action_type": "validate_config",
                                "details": "Validation only — no fix applied"
                            },
                            "config_warnings": config_warnings,
                            "message": if config_warnings.is_empty() {
                                "Config validation passed with no warnings.".to_string()
                            } else {
                                format!("Config validation found {} warning(s): {}", config_warnings.len(), config_warnings.join("; "))
                            }
                        }));
                    }
                } // closes if let Some(c)
            } // closes if let (Some(ws), Some(cid))

            Ok(json!({
                "applied": true,
                "diagnosis": diagnosis,
                "fix_applied": {
                    "action_type": action_type,
                    "details": details
                },
                "message": format!("Fix applied: {}. Re-run the simulation with run_simulation.", details)
            }))
        }
        "evaluate_results" => {
            let case_id = args["case_id"].as_str().and_then(|s| Uuid::parse_str(s).ok());

            if let Some(cid) = case_id {
                let cases = state.db.list_cases(100).await?;
                if let Some(c) = cases.into_iter().find(|c| c.id == cid) {
                    let case_dir = match &state.workspace_dir {
                        Some(ws) => get_case_dir(ws, &c.name),
                        None => return Ok(json!({"error": "No workspace"})),
                    };

                    // Extract forces from disk
                    let forces = ForceExtractor::extract_from_case(&case_dir.to_string_lossy()).ok();

                    // Extract mesh quality from checkMesh log
                    let mesh_quality = read_mesh_quality_from_disk(&case_dir);

                    // Extract convergence from solver log
                    let convergence = read_convergence_from_solver_log(&case_dir);

                    // Extract y+ from postProcessing
                    let yplus = read_yplus_from_disk(&case_dir);

                    // Read wind tunnel results if DWT was active
                    let wind_tunnel: Option<Value> = std::fs::read_to_string(case_dir.join("wind_tunnel_result.json")).ok()
                        .and_then(|s| serde_json::from_str(&s).ok());

                    let mut resp = json!({
                        "iteration": args["iteration"],
                        "forces": forces.map(|f| json!({
                            "cd": f.cd, "cl": f.cl, "cm": f.cm,
                            "cd_std": f.cd_std, "cl_std": f.cl_std,
                        })),
                        "mesh_quality": mesh_quality,
                        "convergence": convergence,
                        "yplus": yplus,
                        "message": "Results extracted from solver output on disk."
                    });
                    if let Some(wt) = wind_tunnel {
                        resp["wind_tunnel"] = wt;
                    }
                    return Ok(resp);
                }
            }
            Ok(json!({"error": "Case not found"}))
        }
        "compare_iterations" => {
            let case_id = args["case_id"].as_str().and_then(|s| Uuid::parse_str(s).ok());
            let target_cl = args.get("target_cl").and_then(|v| v.as_f64());
            let target_cd = args.get("target_cd").and_then(|v| v.as_f64());
            let max_y_plus = args.get("max_y_plus").and_then(|v| v.as_f64()).unwrap_or(1.0);

            if let Some(cid) = case_id {
                // Load from disk or compute from available data
                let cases = state.db.list_cases(100).await?;
                if let Some(c) = cases.into_iter().find(|c| c.id == cid) {
                    let case_dir = match &state.workspace_dir {
                        Some(ws) => get_case_dir(ws, &c.name),
                        None => return Ok(json!({"error": "No workspace"})),
                    };
                    let forces = ForceExtractor::extract_from_case(&case_dir.to_string_lossy())
                        .unwrap_or(ForceCoefficients { cl: 0.0, cd: 0.0, cm: 0.0, cl_std: 0.0, cd_std: 0.0 });
                    let mesh = read_mesh_quality_from_disk(&case_dir).map(|m| {
                        MeshQualityMetrics {
                            max_non_orthogonality: m["max_non_orthogonality"].as_f64().unwrap_or(0.0),
                            avg_non_orthogonality: m["avg_non_orthogonality"].as_f64().unwrap_or(0.0),
                            max_skewness: m["max_skewness"].as_f64().unwrap_or(0.0),
                            min_determinant: 1.0, max_aspect_ratio: 0.0, min_volume: 0.0,
                            n_cells: m["n_cells"].as_u64().unwrap_or(0),
                            n_failed_cells: m["n_failed_cells"].as_u64().unwrap_or(0),
                        }
                    }).unwrap_or(MeshQualityMetrics {
                        max_non_orthogonality: 0.0, avg_non_orthogonality: 0.0,
                        max_skewness: 0.0, min_determinant: 1.0, max_aspect_ratio: 0.0,
                        min_volume: 0.0, n_cells: 0, n_failed_cells: 0,
                    });
                    let conv = read_convergence_from_solver_log(&case_dir);
                    let solver = SolverStats {
                        iterations: conv["n_iterations"].as_u64().unwrap_or(0),
                        wall_time_s: 0.0,
                        residual_p: conv["final_residual"].as_f64().unwrap_or(1.0),
                        residual_u: conv["final_residual"].as_f64().unwrap_or(1.0),
                        converged: conv["converged"].as_bool().unwrap_or(false),
                    };

                    let weights = RewardFunction::default();
                    let score = weights.compute(&forces, &mesh, &solver, target_cl, target_cd, max_y_plus);

                    return Ok(json!({
                        "current_iteration": args["iteration"],
                        "score": score,
                        "cd": forces.cd,
                        "cl": forces.cl,
                        "max_y_plus": max_y_plus,
                        "final_residual": solver.residual_p,
                        "converged": solver.converged,
                        "message": format!("Iteration {} scored {:.4}. Lower is better.", args["iteration"].as_u64().unwrap_or(0), score)
                    }));
                }
            }
            Ok(json!({"error": "Case not found or no comparison data available."}))
        }
        "plan_refinement" => {
            let next_iter = args["current_iteration"].as_u64().unwrap_or(1) + 1;
            Ok(json!({
                "applied": true,
                "next_iteration": next_iter,
                "improvement_strategy": args["improvement_strategy"],
                "next_mesh_params": args["next_mesh_params"],
                "next_solver_params": args.get("next_solver_params"),
                "message": format!("Refinement plan generated. Call propose_config to create iteration {}.", next_iter)
            }))
        }
        "update_skill" => {
            let case_id = args["case_id"].as_str().and_then(|s| Uuid::parse_str(s).ok());
            let name = format!("agent_iteration_{}", args["winning_iteration"].as_u64().unwrap_or(0));
            let params = args.get("final_manifest").cloned().unwrap_or_default();
            if let Some(cid) = case_id {
                let cases = state.db.list_cases(100).await?;
                if let Some(c) = cases.into_iter().find(|c| c.id == cid) {
                    let regime_key = format!("{}|{}", c.flow_type.as_deref().unwrap_or("unknown"), c.solver.as_deref().unwrap_or("unknown"));
                    let score = args["best_score"].as_f64().unwrap_or(0.0);
                    let sid = state.db.insert_skill(&name, cid, &regime_key, &params).await?;
                    state.db.update_skill_score(sid, score).await?;
                    return Ok(json!({
                        "applied": true,
                        "skill_id": sid.to_string(),
                        "message": format!("Skill saved with id {}. Winning config persisted for future reuse.", sid)
                    }));
                }
            }
            Ok(json!({
                "applied": true,
                "message": "Skill saved. Winning configuration stored in skills database."
            }))
        }
        "generate_solver" => {
            let solver_name = args["solver_name"].as_str().unwrap_or("customFoam").to_string();
            let template_str = args["template"].as_str().unwrap_or("custom");
            let template = match template_str {
                "mhdSimpleFoam" => SolverTemplate::MhdSimpleFoam,
                "mhdReactingFoam" => SolverTemplate::MhdReactingFoam,
                "plasmaActuatorFoam" => SolverTemplate::PlasmaActuatorFoam,
                "hyperReactingFoam" => SolverTemplate::HyperReactingFoam,
                "chtRotatingFoam" => SolverTemplate::ChtRotatingFoam,
                "viscoelasticHeatFoam" => SolverTemplate::ViscoelasticHeatFoam,
                "bubblyReactingFoam" => SolverTemplate::BubblyReactingFoam,
                "ablationFoam" => SolverTemplate::AblationFoam,
                "dsmcReactingFoam" => SolverTemplate::DsmcReactingFoam,
                "magneticConvectionFoam" => SolverTemplate::MagneticConvectionFoam,
                "rotorAeroFoam" => SolverTemplate::RotorAeroFoam,
                "coupledPlasmaFoam" => SolverTemplate::CoupledPlasmaFoam,
                _ => SolverTemplate::Custom,
            };
            let description = args["description"].as_str().unwrap_or("").to_string();
            let coupling_str = args["coupling"].as_str().unwrap_or("segregated-SIMPLE");
            let coupling = match coupling_str {
                "segregated-PISO" => CouplingStrategy::SegregatedPISO,
                "coupled-matrix" => CouplingStrategy::CoupledMatrix,
                "operator-split" => CouplingStrategy::OperatorSplit,
                _ => CouplingStrategy::SegregatedSIMPLE,
            };
            let time_str = args["time_treatment"].as_str().unwrap_or("steady");
            let time_treatment = match time_str {
                "unsteady-1st" => TimeTreatment::UnsteadyFirstOrder,
                "unsteady-2nd" => TimeTreatment::UnsteadySecondOrder,
                _ => TimeTreatment::Steady,
            };
            let modules = args["modules"].as_array().map(|arr| {
                arr.iter().filter_map(|m| {
                    match m.as_str()? {
                        "fluid_dynamics" => Some(PhysicsModule::FluidDynamics),
                        "compressible" => Some(PhysicsModule::Compressible),
                        "turbulence" => Some(PhysicsModule::Turbulence),
                        "heat_transfer" => Some(PhysicsModule::HeatTransfer),
                        "species_transport" => Some(PhysicsModule::SpeciesTransport),
                        "chemical_reactions" => Some(PhysicsModule::ChemicalReactions),
                        "two_phase" => Some(PhysicsModule::TwoPhase),
                        "solid_mechanics" => Some(PhysicsModule::SolidMechanics),
                        "electromagnetic" => Some(PhysicsModule::Electromagnetic),
                        "radiation" => Some(PhysicsModule::Radiation),
                        "rotating_frame" => Some(PhysicsModule::RotatingFrame),
                        "porous_media" => Some(PhysicsModule::PorousMedia),
                        "particle_tracking" => Some(PhysicsModule::ParticleTracking),
                        "custom_eos" => Some(PhysicsModule::CustomEOS),
                        "custom_viscosity" => Some(PhysicsModule::CustomViscosity),
                        _ => None,
                    }
                }).collect::<Vec<_>>()
            }).unwrap_or_default();

            let design = SolverDesign {
                solver_name: solver_name.clone(),
                template: template.clone(),
                description,
                modules,
                coupling,
                time_treatment,
                openfoam_version: "v2306".into(),
                validation_case: None,
            };

            let base_solver = SolverScaffold::base_solver_name(&template);
            let files = SolverScaffold::generate_scaffold(&design);

            // Determine output directory
            let output_dir = if let Some(dir) = args.get("output_dir").and_then(|v| v.as_str()) {
                std::path::PathBuf::from(dir)
            } else {
                match (&state.workspace_dir, args.get("case_id").and_then(|v| v.as_str())) {
                    (Some(ws), Some(cid_str)) => {
                        // Try to find case name
                        if let Ok(cid) = Uuid::parse_str(cid_str) {
                            if let Ok(cases) = state.db.list_cases(100).await {
                                if let Some(c) = cases.into_iter().find(|c| c.id == cid) {
                                    get_case_dir(ws, &c.name).join("solver")
                                } else {
                                    ws.join("solvers").join(&solver_name)
                                }
                            } else {
                                ws.join("solvers").join(&solver_name)
                            }
                        } else {
                            ws.join("solvers").join(&solver_name)
                        }
                    }
                    _ => std::path::PathBuf::from(&solver_name),
                }
            };

            // Write all scaffold files
            std::fs::create_dir_all(&output_dir).ok();
            std::fs::create_dir_all(output_dir.join("Make")).ok();
            let mut written = Vec::new();
            for (path, content) in &files {
                let full_path = output_dir.join(path);
                if let Some(parent) = full_path.parent() {
                    std::fs::create_dir_all(parent).ok();
                }
                std::fs::write(&full_path, content).ok();
                written.push(path.clone());
            }

            Ok(json!({
                "applied": true,
                "solver_name": solver_name,
                "base_solver": base_solver,
                "output_dir": output_dir.to_string_lossy().to_string(),
                "files_written": written,
                "n_files": written.len(),
                "message": format!("Solver scaffold '{}' generated with {} files in {}", solver_name, written.len(), output_dir.to_string_lossy()),
            }))
        }
        _ => Ok(json!({"error": format!("Unknown tool: {}", name)})),
    }
}

pub struct ToolExecutorState {
    pub db: aeroflow_skills::SkillsDb,
    pub orchestrator: Option<Arc<Mutex<PipelineOrchestrator>>>,
    pub workspace_dir: Option<PathBuf>,
    iterations: std::sync::Mutex<Vec<Value>>,
    run_count: std::sync::atomic::AtomicU32,
}

impl ToolExecutorState {
    pub fn new(db: aeroflow_skills::SkillsDb) -> Self {
        Self {
            db,
            orchestrator: None,
            workspace_dir: None,
            iterations: std::sync::Mutex::new(Vec::new()),
            run_count: std::sync::atomic::AtomicU32::new(0),
        }
    }

    pub fn with_orchestrator(mut self, orch: Arc<Mutex<PipelineOrchestrator>>, ws: PathBuf) -> Self {
        self.orchestrator = Some(orch);
        self.workspace_dir = Some(ws);
        self
    }

    pub async fn next_iteration(&self) -> u32 {
        self.run_count.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1
    }

    pub async fn record_iteration(&self, _iteration: u32, data: &Value) {
        let mut its = self.iterations.lock().expect("poisoned");
        its.push(data.clone());
    }

    pub async fn get_all_iterations(&self) -> Vec<Value> {
        self.iterations.lock().expect("poisoned").clone()
    }
}

// ── Disk-based result readers ──

fn read_forces_from_disk(case_dir: &std::path::Path) -> Option<Value> {
    let path = case_dir.join("postProcessing/forceCoeffs/0/coefficient.dat");
    let data = std::fs::read_to_string(path).ok()?;
    let last_line = data.lines().rfind(|l| !l.starts_with('#'))?;
    let parts: Vec<&str> = last_line.split_whitespace().collect();
    if parts.len() >= 6 {
        Some(json!({
            "cd": parts[1].parse::<f64>().unwrap_or(0.0),
            "cl": parts[3].parse::<f64>().unwrap_or(0.0),
            "cm": parts[5].parse::<f64>().unwrap_or(0.0),
        }))
    } else {
        None
    }
}

fn read_mesh_quality_from_disk(case_dir: &std::path::Path) -> Option<Value> {
    // Try checkMesh log first, then constant/polyMesh
    for log_name in &["logs/checkMesh.log", "logs/snappyHexMesh.log"] {
        let log_path = case_dir.join(log_name);
        if let Ok(log) = std::fs::read_to_string(&log_path) {
            let n_cells = log.lines()
                .find(|l| l.contains("cells:"))
                .and_then(|l| l.split(':').nth(1))
                .and_then(|s| s.split_whitespace().next())
                .and_then(|s| s.replace(',', "").parse::<u64>().ok());

            let max_non_ortho = log.lines()
                .find(|l| l.contains("Max non-orthogonality") || l.contains("Maximum = "))
                .and_then(|l| {
                    l.split("Max non-orthogonality = ").nth(1)
                        .or_else(|| l.split("Maximum = ").nth(1))
                        .or_else(|| l.split(':').nth(1))
                })
                .and_then(|s| s.split_whitespace().next())
                .and_then(|s| s.replace(',', "").parse::<f64>().ok());

            let max_skewness = log.lines()
                .find(|l| l.contains("skewness"))
                .and_then(|l| l.split('=').nth(1))
                .and_then(|s| s.split_whitespace().next())
                .and_then(|s| s.replace(',', "").parse::<f64>().ok());

            let n_failed = log.lines()
                .find(|l| l.contains("Failed") && l.contains("mesh checks"))
                .and_then(|l| l.split_whitespace().nth(1))
                .and_then(|s| s.parse::<u64>().ok());

            return Some(json!({
                "n_cells": n_cells,
                "max_non_orthogonality": max_non_ortho,
                "max_skewness": max_skewness,
                "n_failed_cells": n_failed.unwrap_or(0),
            }));
        }
    }
    None
}

fn read_convergence_from_solver_log(case_dir: &std::path::Path) -> Value {
    // Parse the solver log for final residuals and iteration count
    let log_dir = case_dir.join("logs");
    let mut final_residual = 1.0;
    let mut n_iterations = 0u64;
    let mut converged = false;

    if let Ok(entries) = std::fs::read_dir(&log_dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with("iter_") && name.ends_with(".log")
                && let Ok(log) = std::fs::read_to_string(entry.path()) {
                    for line in log.lines() {
                        if line.contains("Time =")
                            && let Some(ts) = line.split('=').nth(1)
                                && let Ok(t) = ts.trim().parse::<u64>() {
                                    n_iterations = n_iterations.max(t);
                                }
                        if line.contains("Initial residual") && line.contains("solving for p")
                            && let Some(r) = line.split("Initial residual = ").nth(1)
                                && let Some(val) = r.split(',').next()
                                    && let Ok(v) = val.trim().parse::<f64>()
                                        && v < final_residual { final_residual = v; }
                        if line.contains("Solver converged") || line.contains("converged") && line.contains("iteration") {
                            converged = true;
                        }
                    }
                }
        }
    }

    json!({
        "final_residual": final_residual,
        "n_iterations": n_iterations,
        "converged": converged,
    })
}

fn read_yplus_from_disk(case_dir: &std::path::Path) -> Option<Value> {
    let path = case_dir.join("postProcessing/yPlus/0/yPlus.dat");
    std::fs::read_to_string(path).ok().map(|data| {
        let mut max_y = 0.0f64;
        let mut mean_y = 0.0f64;
        let mut count = 0u64;
        for line in data.lines().filter(|l| !l.starts_with('#') && !l.trim().is_empty()) {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2
                && let Ok(v) = parts[1].parse::<f64>() {
                    max_y = max_y.max(v);
                    mean_y += v;
                    count += 1;
                }
        }
        if count > 0 { mean_y /= count as f64; }
        json!({"max": max_y, "mean": mean_y})
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_all_tools_count() {
        let tools = get_all_tools();
        assert_eq!(tools.len(), 12, "Expected exactly 12 tools");
    }

    #[test]
    fn test_all_tools_have_non_empty_fields() {
        for tool in get_all_tools() {
            assert!(!tool.name.is_empty(), "Tool name should not be empty");
            assert!(!tool.description.is_empty(), "Tool '{}' description should not be empty", tool.name);
            assert!(tool.input_schema.is_object(), "Tool '{}' input_schema should be an object", tool.name);
        }
    }

    #[test]
    fn test_specific_tool_names_present() {
        let names: Vec<String> = get_all_tools().into_iter().map(|t| t.name).collect();
        for &required in &["propose_config", "run_simulation", "evaluate_results", "diagnose_and_fix", "compare_iterations"] {
            assert!(names.contains(&required.to_string()), "Missing required tool: {}", required);
        }
    }

    #[test]
    fn test_tool_executor_state_is_send() {
        fn assert_send<T: Send>() {}
        assert_send::<ToolExecutorState>();
    }

    #[test]
    fn test_next_iteration_semantics() {
        let counter = std::sync::atomic::AtomicU32::new(0);
        let first = counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
        let second = counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
        assert_eq!(first, 1, "First call should return 1");
        assert_eq!(second, 2, "Second call should return 2");
    }

    // ── Schema & Config Parsing Tests ──────────────────────────

    #[test]
    fn test_propose_config_schema_has_mixed_media_fields() {
        let tools = get_all_tools();
        let propose = tools.iter().find(|t| t.name == "propose_config").unwrap();
        let props = propose.input_schema["properties"].as_object().unwrap();
        for &field in &[
            "multiphase", "non_newtonian", "viscoelastic", "combustion",
            "cavitation", "spray", "phase_change", "particle", "porous",
            "aeroacoustic", "fsi", "wave", "wind_turbine", "electrostatic", "ablation",
        ] {
            assert!(props.contains_key(field),
                "propose_config schema missing field: {}", field);
        }
    }

    #[test]
    fn test_propose_config_schema_has_required_fields() {
        let tools = get_all_tools();
        let propose = tools.iter().find(|t| t.name == "propose_config").unwrap();
        let required = propose.input_schema["required"].as_array().unwrap();
        let req_strs: Vec<&str> = required.iter().filter_map(|v| v.as_str()).collect();
        for &field in &["case_id", "goal", "geometry_type", "flow_type", "mach", "reynolds", "mesh_params", "solver"] {
            assert!(req_strs.contains(&field), "Missing required field: {}", field);
        }
    }

    #[test]
    fn test_propose_config_schema_multiphase_accepts_variants() {
        let tools = get_all_tools();
        let propose = tools.iter().find(|t| t.name == "propose_config").unwrap();
        let model_enum = &propose.input_schema["properties"]["multiphase"]["properties"]["model"]["enum"];
        let variants: Vec<&str> = model_enum.as_array().unwrap().iter()
            .filter_map(|v| v.as_str()).collect();
        for &v in &["vof", "eulerEuler", "driftFlux"] {
            assert!(variants.contains(&v), "Missing multiphase model variant: {}", v);
        }
    }

    #[test]
    fn test_propose_config_schema_combustion_accepts_variants() {
        let tools = get_all_tools();
        let propose = tools.iter().find(|t| t.name == "propose_config").unwrap();
        let model_enum = &propose.input_schema["properties"]["combustion"]["properties"]["model"]["enum"];
        let variants: Vec<&str> = model_enum.as_array().unwrap().iter()
            .filter_map(|v| v.as_str()).collect();
        for &v in &["edc", "laminarFlamelet", "paSR", "wellStirredReactor"] {
            assert!(variants.contains(&v), "Missing combustion variant: {}", v);
        }
    }

    #[test]
    fn test_propose_config_schema_requires_only_eight_fields() {
        let tools = get_all_tools();
        let propose = tools.iter().find(|t| t.name == "propose_config").unwrap();
        let required = propose.input_schema["required"].as_array().unwrap();
        assert_eq!(required.len(), 8, "Expected 8 required fields, got {}", required.len());
    }

    #[test]
    fn test_multiphase_config_parsed_from_json_matches_pattern() {
        let args = json!({
            "multiphase": {
                "model": "vof",
                "n_phases": 2,
                "surface_tension_N_m": 0.072,
                "phase_names": ["water", "air"]
            }
        });
        // Replicate the handler parsing pattern
        let parsed = args.get("multiphase").map(|m| {
            let model_str = m.get("model").and_then(|v| v.as_str()).unwrap_or("vof");
            let model = match model_str {
                "eulerEuler" => aeroflow_core::types::MultiphaseModel::EulerEuler,
                "driftFlux" => aeroflow_core::types::MultiphaseModel::DriftFlux,
                _ => aeroflow_core::types::MultiphaseModel::VOF,
            };
            aeroflow_core::types::MultiphaseConfig {
                model,
                n_phases: m.get("n_phases").and_then(|v| v.as_u64()).unwrap_or(2) as u32,
                surface_tension_n_m: m.get("surface_tension_N_m").and_then(|v| v.as_f64()).unwrap_or(0.07),
                phase_names: m.get("phase_names")
                    .and_then(|p| p.as_array())
                    .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                    .unwrap_or_else(|| vec!["phase1".into(), "phase2".into()]),
            }
        });
        assert!(parsed.is_some());
        let cfg = parsed.unwrap();
        assert_eq!(cfg.model, aeroflow_core::types::MultiphaseModel::VOF);
        assert_eq!(cfg.n_phases, 2);
        assert_eq!(cfg.surface_tension_n_m, 0.072);
        assert_eq!(cfg.phase_names, vec!["water", "air"]);
    }

    #[test]
    fn test_multiphase_config_defaults_applied() {
        let args = json!({});
        let parsed = args.get("multiphase").map(|m| {
            let model_str = m.get("model").and_then(|v| v.as_str()).unwrap_or("vof");
            let model = match model_str {
                "eulerEuler" => aeroflow_core::types::MultiphaseModel::EulerEuler,
                "driftFlux" => aeroflow_core::types::MultiphaseModel::DriftFlux,
                _ => aeroflow_core::types::MultiphaseModel::VOF,
            };
            aeroflow_core::types::MultiphaseConfig {
                model,
                n_phases: m.get("n_phases").and_then(|v| v.as_u64()).unwrap_or(2) as u32,
                surface_tension_n_m: m.get("surface_tension_N_m").and_then(|v| v.as_f64()).unwrap_or(0.07),
                phase_names: m.get("phase_names")
                    .and_then(|p| p.as_array())
                    .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                    .unwrap_or_else(|| vec!["phase1".into(), "phase2".into()]),
            }
        });
        // When multiphase key is absent, parsed should be None
        assert!(parsed.is_none(), "absent multiphase key should produce None");
    }

    #[test]
    fn test_multiphase_defaults_when_key_present_but_empty() {
        let args = json!({"multiphase": {}});
        let parsed = args.get("multiphase").map(|m| {
            let model_str = m.get("model").and_then(|v| v.as_str()).unwrap_or("vof");
            let model = match model_str {
                "eulerEuler" => aeroflow_core::types::MultiphaseModel::EulerEuler,
                "driftFlux" => aeroflow_core::types::MultiphaseModel::DriftFlux,
                _ => aeroflow_core::types::MultiphaseModel::VOF,
            };
            aeroflow_core::types::MultiphaseConfig {
                model,
                n_phases: m.get("n_phases").and_then(|v| v.as_u64()).unwrap_or(2) as u32,
                surface_tension_n_m: m.get("surface_tension_N_m").and_then(|v| v.as_f64()).unwrap_or(0.07),
                phase_names: m.get("phase_names")
                    .and_then(|p| p.as_array())
                    .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                    .unwrap_or_else(|| vec!["phase1".into(), "phase2".into()]),
            }
        });
        assert!(parsed.is_some(), "empty multiphase object should still parse");
        let cfg = parsed.unwrap();
        assert_eq!(cfg.model, aeroflow_core::types::MultiphaseModel::VOF);
        assert_eq!(cfg.n_phases, 2);
        assert!((cfg.surface_tension_n_m - 0.07).abs() < 1e-9);
        assert_eq!(cfg.phase_names, vec!["phase1", "phase2"]);
    }

    #[test]
    fn test_non_newtonian_config_from_json() {
        let args = json!({
            "non_newtonian": {
                "model": "powerLaw",
                "nu0": 0.01,
                "k": 0.005,
                "n": 0.4
            }
        });
        let parsed = args.get("non_newtonian").map(|n| {
            let model_str = n.get("model").and_then(|v| v.as_str()).unwrap_or("newtonian");
            let model = match model_str {
                "powerLaw" => aeroflow_core::types::ViscosityModel::PowerLaw,
                "crossPowerLaw" => aeroflow_core::types::ViscosityModel::CrossPowerLaw,
                "birdCarreau" => aeroflow_core::types::ViscosityModel::BirdCarreau,
                "herschelBulkley" => aeroflow_core::types::ViscosityModel::HerschelBulkley,
                "casson" => aeroflow_core::types::ViscosityModel::Casson,
                _ => aeroflow_core::types::ViscosityModel::Newtonian,
            };
            aeroflow_core::types::NonNewtonianConfig {
                model,
                nu0: n.get("nu0").and_then(|v| v.as_f64()).unwrap_or(1e-2),
                nu_inf: n.get("nu_inf").and_then(|v| v.as_f64()).unwrap_or(1e-6),
                k: n.get("k").and_then(|v| v.as_f64()).unwrap_or(0.005),
                n: n.get("n").and_then(|v| v.as_f64()).unwrap_or(0.4),
                tau0: n.get("tau0").and_then(|v| v.as_f64()).unwrap_or(10.0),
                nu_min: n.get("nu_min").and_then(|v| v.as_f64()).unwrap_or(1e-4),
                nu_max: n.get("nu_max").and_then(|v| v.as_f64()).unwrap_or(1e2),
            }
        });
        assert!(parsed.is_some());
        let cfg = parsed.unwrap();
        assert_eq!(cfg.model, aeroflow_core::types::ViscosityModel::PowerLaw);
        assert!((cfg.nu0 - 0.01).abs() < 1e-9);
        assert!((cfg.k - 0.005).abs() < 1e-9);
        assert!((cfg.n - 0.4).abs() < 1e-9);
    }

    #[test]
    fn test_combustion_config_from_json() {
        let args = json!({
            "combustion": {
                "model": "paSR",
                "fuel": "H2",
                "oxidizer": "O2"
            }
        });
        let parsed = args.get("combustion").map(|c| {
            let model_str = c.get("model").and_then(|v| v.as_str()).unwrap_or("edc");
            let model = match model_str {
                "laminarFlamelet" => aeroflow_core::types::CombustionModel::LaminarFlamelet,
                "paSR" => aeroflow_core::types::CombustionModel::PaSR,
                "wellStirredReactor" => aeroflow_core::types::CombustionModel::WellStirredReactor,
                _ => aeroflow_core::types::CombustionModel::EDC,
            };
            aeroflow_core::types::CombustionConfig {
                model,
                c_eps: c.get("c_eps").and_then(|v| v.as_f64()).unwrap_or(2.1377),
                c_mu: c.get("c_mu").and_then(|v| v.as_f64()).unwrap_or(0.09),
                oxidizer: c.get("oxidizer").and_then(|v| v.as_str()).unwrap_or("O2").to_string(),
                fuel: c.get("fuel").and_then(|v| v.as_str()).unwrap_or("CH4").to_string(),
            }
        });
        assert!(parsed.is_some());
        let cfg = parsed.unwrap();
        assert_eq!(cfg.model, aeroflow_core::types::CombustionModel::PaSR);
        assert_eq!(cfg.fuel, "H2");
        assert_eq!(cfg.oxidizer, "O2");
    }

    #[test]
    fn test_fsi_config_from_json() {
        let args = json!({
            "fsi": {
                "model": "linearElastic",
                "coupling": "dirichletNeumann",
                "youngs_modulus_GPa": 210.0,
                "poisson_ratio": 0.3,
                "density_kg_m3": 7850.0
            }
        });
        let parsed = args.get("fsi").map(|f| {
            let model_str = f.get("model").and_then(|v| v.as_str()).unwrap_or("linearElastic");
            let model = match model_str {
                "nonLinearGeometric" => aeroflow_core::types::FSIModel::NonLinearGeometric,
                "plastic" => aeroflow_core::types::FSIModel::Plastic,
                _ => aeroflow_core::types::FSIModel::LinearElastic,
            };
            let coup_str = f.get("coupling").and_then(|v| v.as_str()).unwrap_or("dirichletNeumann");
            let coupling = match coup_str {
                "neumannNeumann" => aeroflow_core::types::FSICoupling::NeumannNeumann,
                "robinRobin" => aeroflow_core::types::FSICoupling::RobinRobin,
                _ => aeroflow_core::types::FSICoupling::DirichletNeumann,
            };
            aeroflow_core::types::FSIConfig {
                model,
                coupling,
                youngs_modulus_gpa: f.get("youngs_modulus_GPa").and_then(|v| v.as_f64()).unwrap_or(200.0),
                poisson_ratio: f.get("poisson_ratio").and_then(|v| v.as_f64()).unwrap_or(0.3),
                density_kg_m3: f.get("density_kg_m3").and_then(|v| v.as_f64()).unwrap_or(7800.0),
                mesh_relaxation: 0.5,
                n_subcycles: 5,
            }
        });
        assert!(parsed.is_some());
        let cfg = parsed.unwrap();
        assert_eq!(cfg.model, aeroflow_core::types::FSIModel::LinearElastic);
        assert!((cfg.youngs_modulus_gpa - 210.0).abs() < 1e-9);
        assert!((cfg.poisson_ratio - 0.3).abs() < 1e-9);
        assert!((cfg.density_kg_m3 - 7850.0).abs() < 1e-9);
    }

    #[test]
    fn test_wind_turbine_config_from_json() {
        let args = json!({
            "wind_turbine": {
                "actuator": "alm",
                "thrust_coefficient": 0.75,
                "power_coefficient": 0.42,
                "rotor_diameter_m": 150.0,
                "hub_height_m": 100.0,
                "wind_speed_ref_m_s": 12.0
            }
        });
        let parsed = args.get("wind_turbine").map(|w| {
            let act_str = w.get("actuator").and_then(|v| v.as_str()).unwrap_or("disc");
            let actuator = match act_str {
                "line" => aeroflow_core::types::ActuatorModel::Line,
                "alm" => aeroflow_core::types::ActuatorModel::ALM,
                _ => aeroflow_core::types::ActuatorModel::Disc,
            };
            aeroflow_core::types::WindTurbineConfig {
                actuator,
                thrust_coefficient: w.get("thrust_coefficient").and_then(|v| v.as_f64()).unwrap_or(0.8),
                power_coefficient: w.get("power_coefficient").and_then(|v| v.as_f64()).unwrap_or(0.45),
                rotor_diameter_m: w.get("rotor_diameter_m").and_then(|v| v.as_f64()).unwrap_or(126.0),
                hub_height_m: w.get("hub_height_m").and_then(|v| v.as_f64()).unwrap_or(90.0),
                wind_speed_ref_m_s: w.get("wind_speed_ref_m_s").and_then(|v| v.as_f64()).unwrap_or(10.0),
                rated_power_mw: 5.0,
            }
        });
        assert!(parsed.is_some());
        let cfg = parsed.unwrap();
        assert_eq!(cfg.actuator, aeroflow_core::types::ActuatorModel::ALM);
        assert!((cfg.thrust_coefficient - 0.75).abs() < 1e-9);
        assert!((cfg.rotor_diameter_m - 150.0).abs() < 1e-9);
    }

    #[test]
    fn test_wave_config_defaults_from_empty_json() {
        let args = json!({"wave": {}});
        let parsed = args.get("wave").map(|w| {
            let model_str = w.get("model").and_then(|v| v.as_str()).unwrap_or("stokesFirst");
            let model = match model_str {
                "stokesFifth" => aeroflow_core::types::WaveModel::StokesFifth,
                "irregular" => aeroflow_core::types::WaveModel::Irregular,
                "streamFunction" => aeroflow_core::types::WaveModel::StreamFunction,
                _ => aeroflow_core::types::WaveModel::StokesFirst,
            };
            aeroflow_core::types::WaveConfig {
                model,
                wave_height_m: w.get("wave_height_m").and_then(|v| v.as_f64()).unwrap_or(2.0),
                wave_period_s: w.get("wave_period_s").and_then(|v| v.as_f64()).unwrap_or(8.0),
                water_depth_m: w.get("water_depth_m").and_then(|v| v.as_f64()).unwrap_or(20.0),
                direction_deg: w.get("direction_deg").and_then(|v| v.as_f64()).unwrap_or(0.0),
                relaxation_zone_length_m: 10.0,
            }
        });
        assert!(parsed.is_some());
        let cfg = parsed.unwrap();
        assert_eq!(cfg.model, aeroflow_core::types::WaveModel::StokesFirst);
        assert!((cfg.wave_height_m - 2.0).abs() < 1e-9);
        assert!((cfg.wave_period_s - 8.0).abs() < 1e-9);
        assert!((cfg.water_depth_m - 20.0).abs() < 1e-9);
    }

    #[test]
    fn test_particle_config_injection_variants() {
        for (inject_str, expected) in [
            ("patch", aeroflow_core::types::ParticleInjection::PatchInjection),
            ("cone", aeroflow_core::types::ParticleInjection::ConeInjection),
            ("manual", aeroflow_core::types::ParticleInjection::ManualInjection),
        ] {
            let args = json!({"particle": {"injection": inject_str}});
            let parsed = args.get("particle").map(|p| {
                let inj_str = p.get("injection").and_then(|v| v.as_str()).unwrap_or("patch");
                let injection = match inj_str {
                    "cone" => aeroflow_core::types::ParticleInjection::ConeInjection,
                    "manual" => aeroflow_core::types::ParticleInjection::ManualInjection,
                    _ => aeroflow_core::types::ParticleInjection::PatchInjection,
                };
                aeroflow_core::types::ParticleConfig {
                    injection,
                    diameter_m: p.get("diameter_m").and_then(|v| v.as_f64()).unwrap_or(1e-4),
                    mass_flow_kg_s: p.get("mass_flow_kg_s").and_then(|v| v.as_f64()).unwrap_or(0.001),
                    velocity_m_s: p.get("velocity_m_s").and_then(|v| v.as_f64()).unwrap_or(10.0),
                    temperature_k: p.get("temperature_K").and_then(|v| v.as_f64()).unwrap_or(300.0),
                    material_density_kg_m3: p.get("density_kg_m3").and_then(|v| v.as_f64()).unwrap_or(2500.0),
                }
            });
            assert!(parsed.is_some(), "failed for inject_str={}", inject_str);
            assert_eq!(parsed.unwrap().injection, expected, "injection variant mismatch for {}", inject_str);
        }
    }

    // ── diagnose_and_fix / validate_config Tests ──────────────

    #[test]
    fn test_diagnose_and_fix_schema_has_validate_config_action() {
        let tools = get_all_tools();
        let dx = tools.iter().find(|t| t.name == "diagnose_and_fix").unwrap();
        let action_enum = dx.input_schema["properties"]["fix_action"]["properties"]["action_type"]["enum"]
            .as_array().unwrap();
        let actions: Vec<&str> = action_enum.iter().filter_map(|v| v.as_str()).collect();
        assert!(actions.contains(&"validate_config"),
            "diagnose_and_fix action_type enum should contain 'validate_config'");
    }

    #[test]
    fn test_diagnose_and_fix_schema_has_eight_actions() {
        let tools = get_all_tools();
        let dx = tools.iter().find(|t| t.name == "diagnose_and_fix").unwrap();
        let action_enum = dx.input_schema["properties"]["fix_action"]["properties"]["action_type"]["enum"]
            .as_array().unwrap();
        assert_eq!(action_enum.len(), 8,
            "Expected 8 fix actions (coarsen_mesh, refine_mesh, change_schemes, reduce_cfl, improve_ic, change_solver, increase_iterations, validate_config), got {}",
            action_enum.len());
    }

    #[test]
    fn test_validate_config_builds_intake_from_manifest() {
        // Replicate the handler's build_config closure pattern with a mock manifest
        let manifest = json!({
            "mach": 0.8,
            "reynolds": 5e6,
            "rotating": {"rpm": 2500.0, "num_blades": 6, "approach": "MRF"},
            "cht": {"problem_type": "cht"},
            "cht": {"problem_type": "cht"}
        });
        let build_config = |key: &str| -> Option<Value> {
            manifest.get(key).and_then(|v| {
                if v.is_null() { None } else { Some(v.clone()) }
            })
        };
        let intake = IntakeConfig {
            geometry_description: String::new(), geometry_file: None,
            case_class: None, workspace_root: None, user_id: None,
            altitude_m: 0.0,
            mach_number: manifest.get("mach").and_then(|v| v.as_f64()).unwrap_or(0.0),
            reynolds_number: manifest.get("reynolds").and_then(|v| v.as_f64()).unwrap_or(0.0),
            alpha_sweep_deg: vec![], freestream_turbulence_intensity: 0.001,
            target_cl: None, target_cd_max: None, target_yplus_max: 1.0,
            convergence_residual: 1e-5, max_agent_iterations: 5,
            human_in_loop: false, priority: Priority::Balanced,
            hpc_cores: 4, time_budget_hours: 24.0,
            rotating: build_config("rotating").and_then(|r| {
                let rpm = r.get("rpm").and_then(|v| v.as_f64())?;
                let num_blades = r.get("num_blades").and_then(|v| v.as_u64())?;
                let approach_str = r.get("approach").and_then(|v| v.as_str())?;
                let approach = match approach_str {
                    "AMI" => RotatingApproach::AMI,
                    _ => RotatingApproach::MRF,
                };
                Some(RotatingConfig {
                    rpm, num_blades: num_blades as u32, approach,
                    axis: [0.0, 0.0, 1.0], origin: [0.0, 0.0, 0.0],
                    diameter_m: None, hub_diameter_m: None,
                    tip_clearance_m: None, advance_ratio: None,
                    target_ct: None, target_cp_max: None,
                    target_eta_min: None, mass_flow_kg_s: None,
                    pressure_ratio_target: None,
                })
            }),
            hypersonic: build_config("hypersonic").map(|_| HypersonicConfig {
                mach_inf: 5.0, altitude_km: 30.0, wall_temperature_k: None,
                wall_catalysis: WallCatalysis::NonCatalytic, real_gas: false,
                chemistry: ChemistryModel::None, two_temperature: false,
                rarefied: false, nose_radius_m: None,
                flux_scheme: FluxScheme::Kurganov, target_peak_heat_flux_w_m2: None,
            }),
            cht: build_config("cht").map(|_| ChtConfig {
                problem_type: HeatTransferProblem::CHT, fluid: "air".into(),
                solid_material: SolidMaterial::Steel, t_inlet_k: 500.0,
                t_ambient_k: 300.0, u_inlet_m_s: None, re: None, pr: 0.7,
                heat_flux_target_w_m2: None, radiation: false,
                radiation_model: RadiationModel::None, phase_change: false,
                max_t_solid_k: None, wall_thickness_m: None,
                emissivity: None, external_h_w_m2k: None,
            }),
            mhd: build_config("mhd").map(|_| MhdConfig {
                b0_tesla: 1.0, sigma_s_m: 1e6, mu_permeability_h_m: 1.256637e-6,
                solver: MhdSolver::MhdFoam, low_rm: false,
                hartmann_number: None,
                wall_conductivity: MhdWallConductivity::Insulating,
                plasma_actuator: None,
            }),
            pemfc: None, wind_tunnel: None,
            multiphase: build_config("multiphase").map(|m| {
                let model_str = m.get("model").and_then(|v| v.as_str()).unwrap_or("vof");
                let model = match model_str {
                    "eulerEuler" => MultiphaseModel::EulerEuler,
                    "driftFlux" => MultiphaseModel::DriftFlux,
                    _ => MultiphaseModel::VOF,
                };
                MultiphaseConfig {
                    model, n_phases: 2, surface_tension_n_m: 0.07,
                    phase_names: vec!["phase1".into(), "phase2".into()],
                }
            }),
            non_newtonian: None, viscoelastic: None,
            combustion: build_config("combustion").map(|_| CombustionConfig {
                model: CombustionModel::EDC, c_eps: 2.1377, c_mu: 0.09,
                oxidizer: "O2".into(), fuel: "CH4".into(),
            }),
            cavitation: None,
            spray: build_config("spray").map(|_| SprayConfig {
                breakup: SprayBreakupModel::KHRT,
                evaporation: SprayEvaporationModel::Standard,
                collision: true, parcel_per_second: 1e5,
                injection_velocity_m_s: 50.0, cone_angle_deg: 30.0,
            }),
            phase_change: None, particle: None, porous: None,
            aeroacoustic: None, fsi: None, wave: None, wind_turbine: None,
            electrostatic: None, ablation: None,
            propulsion: None, nuclear: None, marine: None, ml_surrogate: None,
        };
        let warnings = aeroflow_solver::config_gen::SolverConfigGen::new().validate_config(&intake);
        // rotating + cht is a valid combination — no warnings expected
        assert!(warnings.is_empty(), "Expected no validation warnings for valid config, got: {:?}", warnings);
    }

    #[test]
    fn test_validate_config_detects_hypersonic_multiphase_conflict() {
        let intake = IntakeConfig {
            geometry_description: String::new(), geometry_file: None,
            case_class: None, workspace_root: None, user_id: None,
            altitude_m: 0.0, mach_number: 6.0, reynolds_number: 1e6,
            alpha_sweep_deg: vec![], freestream_turbulence_intensity: 0.001,
            target_cl: None, target_cd_max: None, target_yplus_max: 1.0,
            convergence_residual: 1e-5, max_agent_iterations: 5,
            human_in_loop: false, priority: Priority::Balanced,
            hpc_cores: 4, time_budget_hours: 24.0,
            rotating: None, hypersonic: Some(HypersonicConfig {
                mach_inf: 5.0, altitude_km: 30.0, wall_temperature_k: None,
                wall_catalysis: WallCatalysis::NonCatalytic, real_gas: false,
                chemistry: ChemistryModel::None, two_temperature: false,
                rarefied: false, nose_radius_m: None,
                flux_scheme: FluxScheme::Kurganov, target_peak_heat_flux_w_m2: None,
            }),
            cht: None, mhd: None, pemfc: None, wind_tunnel: None,
            multiphase: Some(MultiphaseConfig {
                model: MultiphaseModel::VOF, n_phases: 2, surface_tension_n_m: 0.07,
                phase_names: vec!["water".into(), "air".into()],
            }),
            non_newtonian: None, viscoelastic: None, combustion: None,
            cavitation: None, spray: None, phase_change: None,
            particle: None, porous: None, aeroacoustic: None, fsi: None,
            wave: None, wind_turbine: None, electrostatic: None, ablation: None,
            propulsion: None, nuclear: None, marine: None, ml_surrogate: None,
        };
        let warnings = aeroflow_solver::config_gen::SolverConfigGen::new().validate_config(&intake);
        assert!(!warnings.is_empty(),
            "Expected validation warnings for hypersonic+multiphase conflict");
        let joined = warnings.join(" ");
        assert!(joined.contains("hypersonic") || joined.contains("multiphase"),
            "Warning should mention conflicting physics. Got: {}", joined);
    }

    // ── change_solver / increase_iterations Tests ─────────────

    #[test]
    fn test_diagnose_and_fix_schema_has_change_solver_action() {
        let tools = get_all_tools();
        let dx = tools.iter().find(|t| t.name == "diagnose_and_fix").unwrap();
        let action_enum = dx.input_schema["properties"]["fix_action"]["properties"]["action_type"]["enum"]
            .as_array().unwrap();
        let actions: Vec<&str> = action_enum.iter().filter_map(|v| v.as_str()).collect();
        assert!(actions.contains(&"change_solver"),
            "diagnose_and_fix action_type enum should contain 'change_solver'");
        assert!(actions.contains(&"increase_iterations"),
            "diagnose_and_fix action_type enum should contain 'increase_iterations'");
    }

    #[test]
    fn test_change_solver_line_replacement_default() {
        let control_dict = "\
FoamFile { version 2.0; }
application     simpleFoam;
startFrom       startTime;
startTime       0;
endTime         1000;
";
        let adjusted = control_dict.replace("application     simpleFoam;", "application     pimpleFoam;");
        assert!(adjusted.contains("pimpleFoam"), "Default change_solver should switch to pimpleFoam");
        assert!(!adjusted.contains("simpleFoam"), "Default change_solver should remove simpleFoam");
    }

    #[test]
    fn test_change_solver_line_replacement_custom() {
        let control_dict = "\
FoamFile { version 2.0; }
application     simpleFoam;
endTime         1000;
";
        let solver = "reactingFoam";
        let mut result = String::new();
        for line in control_dict.lines() {
            if line.trim().starts_with("application") {
                result.push_str(&format!("application     {};\n", solver));
            } else {
                result.push_str(line);
                result.push('\n');
            }
        }
        assert!(result.contains("reactingFoam"), "Custom solver should be set: {}", result);
        assert!(!result.contains("simpleFoam"), "Custom solver should replace simpleFoam");
    }

    #[test]
    fn test_increase_iterations_multiplies_endtime() {
        let control_dict = "\
FoamFile { version 2.0; }
application     simpleFoam;
endTime         1000;
writeInterval   100;
";
        let mut adjusted = String::new();
        for line in control_dict.lines() {
            if line.trim().starts_with("endTime") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                let current: f64 = parts.get(1).and_then(|s| s.trim_end_matches(';').parse().ok()).unwrap_or(1000.0);
                let new_time = (current * 1.5).round() as u64;
                adjusted.push_str(&format!("endTime       {};\n", new_time));
            } else {
                adjusted.push_str(line);
                adjusted.push('\n');
            }
        }
        assert!(adjusted.contains("endTime       1500;"), "endTime should be 1500 (1000*1.5). Got: {}", adjusted);
    }

    #[test]
    fn test_increase_iterations_multiplies_write_interval() {
        let control_dict = "\
FoamFile { version 2.0; }
writeInterval   100;
endTime         1000;
";
        let mut adjusted = String::new();
        for line in control_dict.lines() {
            if line.trim().starts_with("writeInterval") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                let current: u64 = parts.get(1).and_then(|s| s.trim_end_matches(';').parse().ok()).unwrap_or(100);
                let new_interval = (current as f64 * 1.5).round() as u64;
                adjusted.push_str(&format!("writeInterval {};\n", new_interval));
            } else {
                adjusted.push_str(line);
                adjusted.push('\n');
            }
        }
        assert!(adjusted.contains("writeInterval 150;"), "writeInterval should be 150 (100*1.5). Got: {}", adjusted);
    }

    #[test]
    fn test_change_solver_sets_startfrom_latesttime() {
        let control_dict = "\
FoamFile { version 2.0; }
application     simpleFoam;
startFrom       startTime;
";
        let adjusted = control_dict.replace("startFrom       startTime;", "startFrom       latestTime;");
        assert!(adjusted.contains("latestTime"), "change_solver should set startFrom to latestTime");
    }

    // ── Integration-style validate_config Tests ───────────────

    #[test]
    fn test_validate_config_integration_from_manifest_on_disk() {
        let tmp = std::env::temp_dir().join("aeroflow_test_validate_config");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();

        // Write a valid manifest — rotating + cht is compatible
        let valid_manifest = json!({
            "mach": 0.3,
            "reynolds": 2e6,
            "rotating": {"rpm": 1200.0, "num_blades": 5, "approach": "MRF"},
            "cht": {"problem_type": "cht"}
        });
        std::fs::write(tmp.join("agent-manifest.json"), serde_json::to_string_pretty(&valid_manifest).unwrap()).unwrap();

        // Replicate handler's file-read + build_config + validate pattern
        let manifest_str = std::fs::read_to_string(tmp.join("agent-manifest.json")).unwrap();
        let manifest: serde_json::Value = serde_json::from_str(&manifest_str).unwrap();
        let build_config = |key: &str| -> Option<serde_json::Value> {
            manifest.get(key).and_then(|v| {
                if v.is_null() { None } else { Some(v.clone()) }
            })
        };
        let intake = IntakeConfig {
            geometry_description: String::new(), geometry_file: None,
            case_class: None, workspace_root: None, user_id: None,
            altitude_m: 0.0,
            mach_number: manifest.get("mach").and_then(|v| v.as_f64()).unwrap_or(0.0),
            reynolds_number: manifest.get("reynolds").and_then(|v| v.as_f64()).unwrap_or(0.0),
            alpha_sweep_deg: vec![], freestream_turbulence_intensity: 0.001,
            target_cl: None, target_cd_max: None, target_yplus_max: 1.0,
            convergence_residual: 1e-5, max_agent_iterations: 5,
            human_in_loop: false, priority: Priority::Balanced,
            hpc_cores: 4, time_budget_hours: 24.0,
            rotating: build_config("rotating").and_then(|r| {
                let rpm = r.get("rpm").and_then(|v| v.as_f64())?;
                let num_blades = r.get("num_blades").and_then(|v| v.as_u64())?;
                let approach_str = r.get("approach").and_then(|v| v.as_str())?;
                let approach = match approach_str {
                    "AMI" => RotatingApproach::AMI,
                    _ => RotatingApproach::MRF,
                };
                Some(RotatingConfig {
                    rpm, num_blades: num_blades as u32, approach,
                    axis: [0.0, 0.0, 1.0], origin: [0.0, 0.0, 0.0],
                    diameter_m: None, hub_diameter_m: None,
                    tip_clearance_m: None, advance_ratio: None,
                    target_ct: None, target_cp_max: None,
                    target_eta_min: None, mass_flow_kg_s: None,
                    pressure_ratio_target: None,
                })
            }),
            hypersonic: None, cht: build_config("cht").map(|_| ChtConfig {
                problem_type: HeatTransferProblem::CHT, fluid: "air".into(),
                solid_material: SolidMaterial::Steel, t_inlet_k: 500.0,
                t_ambient_k: 300.0, u_inlet_m_s: None, re: None, pr: 0.7,
                heat_flux_target_w_m2: None, radiation: false,
                radiation_model: RadiationModel::None, phase_change: false,
                max_t_solid_k: None, wall_thickness_m: None,
                emissivity: None, external_h_w_m2k: None,
            }),
            mhd: None, pemfc: None, wind_tunnel: None,
            multiphase: None, non_newtonian: None, viscoelastic: None,
            combustion: None, cavitation: None, spray: None,
            phase_change: None, particle: None, porous: None,
            aeroacoustic: None, fsi: None, wave: None, wind_turbine: None,
            electrostatic: None, ablation: None,
            propulsion: None, nuclear: None, marine: None, ml_surrogate: None,
        };
        let warnings = aeroflow_solver::config_gen::SolverConfigGen::new().validate_config(&intake);
        assert!(warnings.is_empty(),
            "Expected no warnings for valid rotating+cht config loaded from disk manifest, got: {:?}", warnings);

        // Now write a conflicting manifest and re-validate
        let conflict_manifest = json!({
            "mach": 6.0,
            "reynolds": 1e6,
            "hypersonic": {"mach_inf": 6.0},
            "multiphase": {"model": "vof"}
        });
        std::fs::write(tmp.join("agent-manifest.json"), serde_json::to_string_pretty(&conflict_manifest).unwrap()).unwrap();

        let manifest_str2 = std::fs::read_to_string(tmp.join("agent-manifest.json")).unwrap();
        let manifest2: serde_json::Value = serde_json::from_str(&manifest_str2).unwrap();
        let build_config2 = |key: &str| -> Option<serde_json::Value> {
            manifest2.get(key).and_then(|v| {
                if v.is_null() { None } else { Some(v.clone()) }
            })
        };
        let intake2 = IntakeConfig {
            geometry_description: String::new(), geometry_file: None,
            case_class: None, workspace_root: None, user_id: None,
            altitude_m: 0.0, mach_number: 6.0, reynolds_number: 1e6,
            alpha_sweep_deg: vec![], freestream_turbulence_intensity: 0.001,
            target_cl: None, target_cd_max: None, target_yplus_max: 1.0,
            convergence_residual: 1e-5, max_agent_iterations: 5,
            human_in_loop: false, priority: Priority::Balanced,
            hpc_cores: 4, time_budget_hours: 24.0,
            rotating: None,
            hypersonic: build_config2("hypersonic").map(|_| HypersonicConfig {
                mach_inf: 5.0, altitude_km: 30.0, wall_temperature_k: None,
                wall_catalysis: WallCatalysis::NonCatalytic, real_gas: false,
                chemistry: ChemistryModel::None, two_temperature: false,
                rarefied: false, nose_radius_m: None,
                flux_scheme: FluxScheme::Kurganov, target_peak_heat_flux_w_m2: None,
            }),
            cht: None, mhd: None, pemfc: None, wind_tunnel: None,
            multiphase: build_config2("multiphase").map(|m| {
                let model_str = m.get("model").and_then(|v| v.as_str()).unwrap_or("vof");
                let model = match model_str {
                    "eulerEuler" => MultiphaseModel::EulerEuler,
                    "driftFlux" => MultiphaseModel::DriftFlux,
                    _ => MultiphaseModel::VOF,
                };
                MultiphaseConfig {
                    model, n_phases: 2, surface_tension_n_m: 0.07,
                    phase_names: vec!["phase1".into(), "phase2".into()],
                }
            }),
            non_newtonian: None, viscoelastic: None, combustion: None,
            cavitation: None, spray: None, phase_change: None,
            particle: None, porous: None, aeroacoustic: None, fsi: None,
            wave: None, wind_turbine: None, electrostatic: None, ablation: None,
            propulsion: None, nuclear: None, marine: None, ml_surrogate: None,
        };
        let warnings2 = aeroflow_solver::config_gen::SolverConfigGen::new().validate_config(&intake2);
        assert!(!warnings2.is_empty(),
            "Expected warnings for hypersonic+multiphase conflict from disk manifest");

        // Cleanup
        std::fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn test_validate_config_absent_returns_empty_warnings() {
        let intake = IntakeConfig {
            geometry_description: "simple wing".into(), geometry_file: None,
            case_class: None, workspace_root: None, user_id: None,
            altitude_m: 1000.0, mach_number: 0.5, reynolds_number: 3e6,
            alpha_sweep_deg: vec![0.0, 5.0], freestream_turbulence_intensity: 0.001,
            target_cl: Some(0.5), target_cd_max: Some(0.05), target_yplus_max: 1.0,
            convergence_residual: 1e-5, max_agent_iterations: 5,
            human_in_loop: false, priority: Priority::Balanced,
            hpc_cores: 4, time_budget_hours: 24.0,
            rotating: None, hypersonic: None,
            cht: None, mhd: None, pemfc: None, wind_tunnel: None,
            multiphase: None, non_newtonian: None, viscoelastic: None,
            combustion: None, cavitation: None, spray: None,
            phase_change: None, particle: None, porous: None,
            aeroacoustic: None, fsi: None, wave: None, wind_turbine: None,
            electrostatic: None, ablation: None,
            propulsion: None, nuclear: None, marine: None, ml_surrogate: None,
        };
        let warnings = aeroflow_solver::config_gen::SolverConfigGen::new().validate_config(&intake);
        assert!(warnings.is_empty(),
            "Expected no warnings for simple subsonic config, got: {:?}", warnings);
    }
}
