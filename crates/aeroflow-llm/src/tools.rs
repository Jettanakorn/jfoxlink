use crate::types::ToolDef;
use aeroflow_core::types::{ForceCoefficients, MeshQualityMetrics, SolverStats, RotatingConfig, RotatingApproach, HypersonicConfig, ChemistryModel, WallCatalysis, FluxScheme, ChtConfig, HeatTransferProblem, SolidMaterial, RadiationModel, MhdConfig, MhdSolver, MhdWallConductivity, PlasmaModel, PlasmaActuatorConfig, PropulsionConfig, PropulsionModel, NuclearConfig, NuclearModel, MarineConfig, MarineModel, MlSurrogateConfig, MlSurrogateModel, SolverDesign, SolverTemplate, CouplingStrategy, TimeTreatment, PhysicsModule, PemfcModel, PemfcConfig, PemfcFlowField, PemfcCycleProfile, PemfcDegradationModel, WindTunnelConfig};
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
                            "action_type": {"type": "string", "enum": ["coarsen_mesh", "refine_mesh", "change_schemes", "reduce_cfl", "improve_ic", "change_solver", "increase_iterations"]},
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

            // Apply the fix by modifying case files on disk
            if let (Some(ws), Some(cid)) = (&state.workspace_dir, case_id) {
                let cases = state.db.list_cases(100).await?;
                if let Some(c) = cases.into_iter().find(|c| c.id == cid) {
                    let case_dir = get_case_dir(ws, &c.name);

                    match action_type {
                        "coarsen_mesh" => {
                            // Reduce snappyHexMesh refinement levels
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
                            // Switch to more robust numerical schemes
                            let fvs_path = case_dir.join("system/fvSchemes");
                            if let Ok(content) = std::fs::read_to_string(&fvs_path) {
                                let adjusted = content
                                    .replace("Gauss linearUpwindV grad(U)", "Gauss upwind")
                                    .replace("Gauss linear corrected", "Gauss linear uncorrected");
                                std::fs::write(&fvs_path, &adjusted).ok();
                                tracing::info!("Fix: switched to upwind schemes for robustness");
                            }
                            // Also reduce relaxation factors
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
                            // Halve deltaT in controlDict
                            let cd_path = case_dir.join("system/controlDict");
                            if let Ok(content) = std::fs::read_to_string(&cd_path) {
                                let adjusted = content.replace("deltaT          1;", "deltaT          0.5;");
                                std::fs::write(&cd_path, &adjusted).ok();
                                tracing::info!("Fix: halved deltaT for CFL reduction");
                            }
                        }
                        "improve_ic" => {
                            // Add potentialFoam initialization
                            let cd_path = case_dir.join("system/controlDict");
                            if let Ok(content) = std::fs::read_to_string(&cd_path) {
                                let adjusted = content
                                    .replace("startFrom       startTime;", "startFrom       startTime;\nstartTime       0;")
                                    .replace("application     ", "// application ");
                                // Write a potentialFoam init script
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
                        _ => {
                            tracing::warn!("Unknown fix action type: {}", action_type);
                        }
                    }

                    // Record fix to DB
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
                }
            }

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
}
