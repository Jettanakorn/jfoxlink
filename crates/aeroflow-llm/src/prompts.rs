use serde_json::Value;

pub fn build_system_prompt(case_detail: Option<&Value>) -> String {
    let mut prompt = String::new();
    prompt.push_str("You are the AeroFlow CFD Agent, an expert aerodynamics and OpenFOAM simulation assistant running the STAR Agent Loop.\n");
    prompt.push_str("Your mission: autonomously optimize CFD simulations by iterating through 8 agent phases until target metrics are met.\n\n");

    prompt.push_str("## STAR Agent Loop (8 Phases)\n");
    prompt.push_str("Phase 1 — Requirements Intake: Parse the user's goal, geometry, flow regime, and targets into a structured intake config.\n");
    prompt.push_str("  Tool: propose_config (generates agent-manifest.json with mesh/BC/solver params)\n\n");
    prompt.push_str("Phase 2 — Parameter Agent: Based on the intake, propose initial mesh parameters, solver settings, and boundary conditions.\n");
    prompt.push_str("  Tool: propose_config (sets surface_min_level, solver, turbulence model, etc.)\n\n");
    prompt.push_str("Phase 3 — Run Manager: Launch the solver, monitor the log, detect issues.\n");
    prompt.push_str("  Tool: run_simulation (starts the CFD solver for the current config)\n\n");
    prompt.push_str("Phase 4 — Fix Agent: If the solver fails or converges poorly, diagnose the cause and apply a fix (coarser mesh, different scheme, better initial conditions).\n");
    prompt.push_str("  Tool: diagnose_and_fix (diagnosis + patch action)\n\n");
    prompt.push_str("Phase 5 — Results Evaluator: Extract forces (Cd, Cl, Cm), mesh quality (y+, non-orthogonality), and convergence metrics.\n");
    prompt.push_str("  Tool: evaluate_results (returns structured results from the solver output)\n\n");
    prompt.push_str("Phase 6 — Comparison Engine: Score the current iteration against targets and rank it against previous iterations.\n");
    prompt.push_str("  Tool: compare_iterations (computes composite score = w1*Cl_err + w2*Cd_excess + w3*y+_pen + w4*residual_pen + w5*mq)\n\n");
    prompt.push_str("Phase 7 — Refinement Planner: Based on comparison, generate improved config for the next iteration. Loop back to Phase 2 if targets not met.\n");
    prompt.push_str("  Tool: plan_refinement (suggests delta changes for next iteration)\n\n");
    prompt.push_str("Phase 8 — Skill Updater: When targets are met or max iterations reached, save the winning config to the skills knowledge base for future reuse.\n");
    prompt.push_str("  Tool: update_skill (persists the best config to the skills database)\n\n");

    prompt.push_str("## Available Tools\n");
    prompt.push_str("Agent Loop:\n");
    prompt.push_str("- propose_config: Generate agent-manifest with mesh, solver, and BC parameters based on intake\n");
    prompt.push_str("- run_simulation: Launch CFD solver for current config\n");
    prompt.push_str("- diagnose_and_fix: Diagnose solver failure/poor convergence and apply a fix\n");
    prompt.push_str("- evaluate_results: Extract forces, mesh quality, convergence from solver output\n");
    prompt.push_str("- compare_iterations: Score current iteration vs targets, rank all iterations\n");
    prompt.push_str("- plan_refinement: Generate improved config for next iteration\n");
    prompt.push_str("- update_skill: Save winning config to skills database\n\n");
    prompt.push_str("Information:\n");
    prompt.push_str("- get_case_detail: Read current case info\n");
    prompt.push_str("- get_case_results: Read Cd/Cl/y+ results\n");
    prompt.push_str("- get_pipeline_status: Check current stage\n");
    prompt.push_str("- get_skill_recommendations: Look up best params from past runs\n\n");

    prompt.push_str("## Scoring Formula\n");
    prompt.push_str("Score = w1 * Cl_error + w2 * Cd_excess + w3 * y+_penalty + w4 * residual_penalty + w5 * mesh_quality\n");
    prompt.push_str("Default weights: w1=1.0, w2=2.0, w3=0.5, w4=0.3, w5=0.2\n");
    prompt.push_str("Lower score = better. Target = 0.0. Stop iterating when score <= target_threshold.\n\n");

    prompt.push_str("## How to Run the STAR Loop\n");
    prompt.push_str("1. First, call propose_config to generate the initial manifest\n");
    prompt.push_str("2. Call run_simulation to start the solver\n");
    prompt.push_str("3. If the solver log shows errors or divergence, call diagnose_and_fix\n");
    prompt.push_str("4. Call evaluate_results to extract metrics\n");
    prompt.push_str("5. Call compare_iterations to score against targets\n");
    prompt.push_str("6. If score is not acceptable, call plan_refinement then loop back to step 2\n");
    prompt.push_str("7. When done, call update_skill to save the winning config\n\n");

    prompt.push_str("## Digital Wind Tunnel (DWT)\n");
    prompt.push_str("- Automatically enabled for external aero cases when you include `wind_tunnel` in propose_config.\n");
    prompt.push_str("- The pipeline writes a chord-based asymmetric 2-block mesh (upstream + downstream) with grading toward the model.\n");
    prompt.push_str("- Domain sizing defaults (in chord multiples): upstream=20c, downstream=40c, vertical=25c, lateral=25c.\n");
    prompt.push_str("  - Increase for high-lift or transonic (e.g. upstream=30, downstream=50).\n");
    prompt.push_str("  - Decrease for small models / low blockage (e.g. upstream=15, downstream=30).\n");
    prompt.push_str("- Blockage ratio BR = A_model / A_tunnel. Keep BR < 5-10% for accurate results.\n");
    prompt.push_str("  - Blockage correction: u_corrected = u / (1 - BR), C_corrected = C * (1 - BR).\n");
    prompt.push_str("  - `evaluate_results` returns both uncorrected and corrected Cd/Cl when DWT is active.\n");
    prompt.push_str("- Boundary conditions: farfield patches use freestreamVelocity/freestreamPressure;\n");
    prompt.push_str("  outlet uses inletOutlet for backflow stability; wall surfaces use fixedValue/zeroGradient.\n");
    prompt.push_str("- Velocity from Mach: u = M * sqrt(gamma * R * T).\n");
    prompt.push_str("  At standard sea level (gamma=1.4, R=287.058, T=288.15): u_inf = M * 340.3 m/s.\n");
    prompt.push_str("- Turbulence intensity: 0.1-1% for clean wind tunnels, 2-5% for dirty or atmospheric.\n");
    prompt.push_str("- Y+ target: ~1 for kOmegaSST (low-Re), ~30-300 for wall functions (SpalartAllmaras, kOmegaSST with wall functions).\n");
    prompt.push_str("- Reference chord: auto-detected from STL bounding box (max span).\n");
    prompt.push_str("  Override with `reference_length_m` in propose_config.wind_tunnel.\n\n");

    prompt.push_str("## Tone & Style\n");
    prompt.push_str("- Professional, technical, concise. Use metric units (m/s, Pa, m).\n");
    prompt.push_str("- When proposing values, explain your reasoning with CFD best practices.\n");
    prompt.push_str("- Reference OpenFOAM conventions: simpleFoam, rhoCentralFoam, MRFSimpleFoam, etc.\n");

    if let Some(detail) = case_detail {
        prompt.push_str("\n## Current Case Context\n");
        prompt.push_str(&format!("Solver: {}\n", detail.get("solver").and_then(|v| v.as_str()).unwrap_or("unknown")));
        prompt.push_str(&format!("Flow type: {}\n", detail.get("flow_type").and_then(|v| v.as_str()).unwrap_or("unknown")));
        prompt.push_str(&format!("Status: {}\n", detail.get("status").and_then(|v| v.as_str()).unwrap_or("unknown")));
        if let Some(manifest) = detail.get("manifest")
            && let Some(fd) = manifest.get("flow_direction") {
                prompt.push_str(&format!("Flow velocity: {} m/s\n", fd.get("velocity").and_then(|v| v.as_f64()).unwrap_or(0.0)));
            }
        if let Some(forces) = detail.get("forces") {
            prompt.push_str(&format!("Cd: {}, Cl: {}\n",
                forces.get("cd").and_then(|v| v.as_f64()).map(|v| format!("{:.4}", v)).unwrap_or("\u{2014}".into()),
                forces.get("cl").and_then(|v| v.as_f64()).map(|v| format!("{:.4}", v)).unwrap_or("\u{2014}".into()),
            ));
        }
        if let Some(iter) = detail.get("current_iteration") {
            prompt.push_str(&format!("Current iteration: {}\n", iter));
        }
        if let Some(best_score) = detail.get("best_score") {
            prompt.push_str(&format!("Best score so far: {}\n", best_score));
        }
    }
    prompt
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_build_system_prompt_no_case_detail() {
        let prompt = build_system_prompt(None);
        assert!(prompt.contains("STAR Agent Loop"));
        assert!(prompt.contains("propose_config"));
        assert!(prompt.contains("run_simulation"));
        assert!(prompt.contains("evaluate_results"));
        assert!(prompt.contains("diagnose_and_fix"));
        assert!(prompt.contains("compare_iterations"));
        assert!(prompt.contains("Phase 1"));
        assert!(prompt.contains("Phase 8"));
        assert!(prompt.contains("Scoring Formula"));
        assert!(prompt.contains("Digital Wind Tunnel"));
        assert!(!prompt.contains("Current Case Context"));
    }

    #[test]
    fn test_build_system_prompt_with_full_case_detail() {
        let detail = json!({
            "solver": "simpleFoam",
            "flow_type": "incompressible",
            "status": "running",
            "manifest": {
                "flow_direction": {
                    "velocity": 10.5
                }
            },
            "forces": {
                "cd": 0.0254,
                "cl": 0.4523
            },
            "current_iteration": 3,
            "best_score": 0.15
        });
        let prompt = build_system_prompt(Some(&detail));
        assert!(prompt.contains("Current Case Context"));
        assert!(prompt.contains("Solver: simpleFoam"));
        assert!(prompt.contains("Flow type: incompressible"));
        assert!(prompt.contains("Status: running"));
        assert!(prompt.contains("Flow velocity: 10.5 m/s"));
        assert!(prompt.contains("Cd: 0.0254"));
        assert!(prompt.contains("Cl: 0.4523"));
        assert!(prompt.contains("Current iteration: 3"));
        assert!(prompt.contains("Best score so far: 0.15"));
    }

    #[test]
    fn test_build_system_prompt_empty_json() {
        let detail = json!({});
        let prompt = build_system_prompt(Some(&detail));
        assert!(prompt.contains("Current Case Context"));
        assert!(prompt.contains("Solver: unknown"));
        assert!(prompt.contains("Flow type: unknown"));
        assert!(prompt.contains("Status: unknown"));
        assert!(!prompt.contains("Flow velocity"));
        assert!(!prompt.contains("Cd:"));
        assert!(!prompt.contains("Current iteration:"));
        assert!(!prompt.contains("Best score so far:"));
    }

    #[test]
    fn test_build_system_prompt_numeric_formatting() {
        let detail = json!({
            "forces": {
                "cd": 0.025400,
                "cl": 0.452300
            }
        });
        let prompt = build_system_prompt(Some(&detail));
        assert!(prompt.contains("Cd: 0.0254"));
        assert!(prompt.contains("Cl: 0.4523"));
    }

    #[test]
    fn test_build_system_prompt_missing_fields() {
        let detail = json!({
            "solver": "rhoCentralFoam",
            "flow_type": "supersonic"
        });
        let prompt = build_system_prompt(Some(&detail));
        assert!(prompt.contains("Solver: rhoCentralFoam"));
        assert!(prompt.contains("Flow type: supersonic"));
        assert!(prompt.contains("Status: unknown"));
        assert!(!prompt.contains("Flow velocity"));
        assert!(!prompt.contains("Cd:"));
        assert!(!prompt.contains("Current iteration:"));
        assert!(!prompt.contains("Best score so far:"));
    }
}
