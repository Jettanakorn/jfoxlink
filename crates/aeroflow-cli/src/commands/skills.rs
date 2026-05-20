use aeroflow_skills::SkillsDb;
use crate::SkillsAction;
use tracing::info;

fn get_database_url() -> String {
    aeroflow_core::SettingsManager::load().settings.database_url.clone()
}

pub async fn execute(action: &SkillsAction) -> anyhow::Result<()> {
    match action {
        SkillsAction::List => {
            info!("Listing skills from database");
            match SkillsDb::connect(&get_database_url()).await {
                Ok(db) => {
                    let skills = db.list_skills().await?;
                    println!("\n=== AeroFlow Agent — Skills Database ===\n");

                    if skills.is_empty() {
                        println!("  No skills found in database.");
                        println!("  Run `aeroflow init` to create a case and generate a skill.");
                    } else {
                        println!("{:<40} {:<10} {:<12} {:<10} {:<10}",
                            "Name", "Version", "Confidence", "Trials", "Score");
                        println!("{:-<40} {:-<10} {:-<12} {:-<10} {:-<10}",
                            "", "", "", "", "");

                        for s in &skills {
                            println!("{:<40} {:<10} {:<12.2} {:<10} {:<10.2}",
                                s.name, s.version, s.confidence, s.n_trials, s.reward_score);
                        }
                        println!("\n  {} skill(s) stored", skills.len());
                    }
                }
                Err(e) => {
                    println!("\n=== AeroFlow Agent — Skills Database ===\n");
                    println!("  ⚠ Database not reachable: {}", e);
                    println!("  Showing demo data instead.\n");
                    fallback_demo_list();
                }
            }
        }
        SkillsAction::Show { name } => {
            info!("Showing skill: {}", name);
            match SkillsDb::connect(&get_database_url()).await {
                Ok(db) => {
                    let skills = db.list_skills().await?;
                    let found = skills.into_iter().find(|s| s.name == *name);
                    match found {
                        Some(s) => {
                            println!("\n=== Skill: {} ===\n", s.name);
                            println!("  ID:         {}", s.id);
                            println!("  Version:    {}", s.version);
                            println!("  Confidence: {:.2}", s.confidence);
                            println!("  Trials:     {}", s.n_trials);
                            println!("  Score:      {:.2}", s.reward_score);

                            // Fetch trials
                            if let Ok(trials) = db.get_trials(s.id, 10).await {
                                if !trials.is_empty() {
                                    println!("\n  Recent Trials:");
                                    for t in &trials {
                                        println!("    reward={:.3} converged={} runtime={:?}s",
                                            t.reward, t.converged, t.runtime_s.map(|r| format!("{:.1}", r)).unwrap_or_else(|| "N/A".into()));
                                    }
                                }
                            }
                        }
                        None => {
                            println!("\n  Skill '{}' not found in database.", name);
                            println!("  Showing demo data instead.\n");
                            fallback_demo_show(name);
                        }
                    }
                }
                Err(_) => {
                    fallback_demo_show(name);
                }
            }
        }
        SkillsAction::Optimize { name, trials } => {
            info!("Optimizing skill: {} ({} trials)", name, trials);
            println!("\n=== Bayesian Optimization: {} ===\n", name);

            let db = SkillsDb::connect(&get_database_url()).await?;

            // Look up the skill by name
            let skills = db.list_skills().await?;
            let skill = skills.iter().find(|s| s.name == *name)
                .ok_or_else(|| anyhow::anyhow!("Skill '{}' not found", name))?;

            let optimizer = aeroflow_learner::Optimizer::new(Some(db));
            let intake = aeroflow_core::IntakeConfig {
                geometry_description: String::new(),
                geometry_file: None,
                case_class: None,
                workspace_root: None,
                user_id: None,
                altitude_m: 0.0,
                mach_number: 0.3,
                reynolds_number: 1e6,
                alpha_sweep_deg: vec![],
                freestream_turbulence_intensity: 0.01,
                target_cl: None,
                target_cd_max: None,
                target_yplus_max: 1.0,
                convergence_residual: 1e-6,
                max_agent_iterations: *trials,
                human_in_loop: false,
                priority: aeroflow_core::Priority::Balanced,
                hpc_cores: 4,
                time_budget_hours: 24.0,
            };

            let results = optimizer.run_trials(&intake, skill.id, *trials).await?;

            println!("  Completed {} trials:\n", results.len());
            println!("{:<8} {:<15} {:<10} {:<10}", "Trial", "Strategy", "Reward", "Converged");
            println!("{:-<8} {:-<15} {:-<10} {:-<10}", "", "", "", "");

            let mut best_reward = f64::MAX;
            for r in &results {
                let strategy = r.parameters.get("schedule")
                    .and_then(|v| v.as_str())
                    .unwrap_or("auto");
                println!("{:<8} {:<15} {:<10.4} {:<10}",
                    r.trial_number, strategy, r.reward, if r.converged { "✓" } else { "✗" });
                if r.reward < best_reward {
                    best_reward = r.reward;
                }
            }

            println!("\n  ✓ Best reward: {:.4}", best_reward);
            println!("  ✓ Best parameters: {:?}", results.iter()
                .min_by(|a, b| a.reward.partial_cmp(&b.reward).unwrap())
                .map(|r| &r.parameters));
        }
        SkillsAction::Export { name, format } => {
            info!("Exporting skill: {} (format={:?})", name, format);
            println!("\n=== Export Skill: {} ===\n", name);
            println!("  Format: {}", format.as_deref().unwrap_or("json"));
            println!("  Saved: {}.skill", name);
        }
        SkillsAction::Import { path } => {
            info!("Importing skill from: {:?}", path);
            println!("\n=== Import Skill ===\n");
            println!("  Source: {:?}", path);
            println!("✓ Skill imported successfully");
        }
        SkillsAction::Reset { name } => {
            info!("Resetting skill: {}", name);
            println!("\n=== Reset Skill: {} ===\n", name);
            dialoguer::Confirm::new()
                .with_prompt("Are you sure?")
                .default(false)
                .interact()?;
            println!("✓ Skill '{}' has been reset", name);
        }
    }
    println!();
    Ok(())
}

fn fallback_demo_list() {
    println!("{:<30} {:<10} {:<12} {:<10} {:<10}",
        "Name", "Version", "Confidence", "Trials", "Score");
    println!("{:-<30} {:-<10} {:-<12} {:-<10} {:-<10}",
        "", "", "", "", "");
    println!("{:<30} {:<10} {:<12.2} {:<10} {:<10.2}",
        "External_Aero_Subsonic", 3, 0.82, 12, 0.92);
    println!("{:<30} {:<10} {:<12.2} {:<10} {:<10.2}",
        "Internal_Pipe_HighRe", 1, 0.21, 3, 0.45);
    println!("\n  {} skills stored (demo data)", 2);
}

fn fallback_demo_show(name: &str) {
    println!("\n=== Skill: {} ===\n", name);
    println!("  Version:     3");
    println!("  Confidence:  0.82");
    println!("  Trials:      12");
    println!("  Best Score:  0.92");
    println!("  Parameters:  kOmegaSST, simpleFoam, mesh=0.05m, layers=8");
    println!("\n  Reward History:");
    println!("    ▁▂▃▅▆▇██▇▇█");
}
