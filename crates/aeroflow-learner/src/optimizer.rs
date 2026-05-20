use crate::gp::GaussianProcess;
use aeroflow_core::{IntakeConfig, ScoringWeights};
use aeroflow_skills::SkillsDb;
use crate::RewardFunction;

pub struct Optimizer {
    db: Option<SkillsDb>,
    #[allow(dead_code)]
    reward_fn: RewardFunction,
}

impl Optimizer {
    pub fn new(db: Option<SkillsDb>) -> Self {
        Self {
            db,
            reward_fn: RewardFunction::new(ScoringWeights::default()),
        }
    }

    /// Run Bayesian optimization trials for a skill.
    ///
    /// Each trial samples a different parameter configuration, runs the
    /// pipeline (simulated), computes reward, and stores results in the DB.
    pub async fn run_trials(
        &self,
        _intake: &IntakeConfig,
        skill_id: uuid::Uuid,
        n_trials: u32,
    ) -> Result<Vec<TrialResult>, anyhow::Error> {
        anyhow::ensure!(n_trials > 0, "Need at least 1 trial");

        // Fetch historical trials from DB for this skill
        let history = if let Some(ref db) = self.db {
            db.get_trials(skill_id, 100).await?
        } else {
            Vec::new()
        };

        let mut gp = GaussianProcess::new();

        // If we have history, fit the GP
        if history.len() >= 2 {
            let x: Vec<f64> = (0..history.len()).map(|i| i as f64 / history.len() as f64).collect();
            let y: Vec<f64> = history.iter().map(|t| t.reward).collect();
            if let Err(e) = gp.fit(&x, &y) {
                tracing::warn!("GP fit failed (need more data): {}", e);
            }
        }

        let mut results = Vec::new();
        let best_so_far = history.iter()
            .map(|t| t.reward)
            .fold(f64::MAX, |a, b| a.min(b));

        let mut current_best = best_so_far;

        for trial in 1..=n_trials {
            // Suggest next parameters using GP Expected Improvement
            let params = if history.len() >= 2 {
                let candidates: Vec<f64> = (0..=50).map(|i| i as f64 / 50.0).collect();
                let (best_x, _) = gp.expected_improvement(&candidates, current_best);
                serde_json::json!({
                    "gp_index": best_x,
                    "mesh_size": 0.02 + best_x * 0.08,
                    "layers": (4.0 + best_x * 12.0).round() as u32,
                    "relaxation_p": 0.3 + best_x * 0.5,
                })
            } else {
                // Random exploration for initial trials
                Self::random_params(trial, n_trials)
            };

            tracing::info!(
                "Trial {}/{}: params={:?}",
                trial, n_trials, params
            );

            // In P4, this would call the real pipeline.
            // For now, simulate with random-ish reward that improves over time.
            let reward = self.simulate_trial(&params, trial, n_trials);

            let converged = reward < 0.3;

            // Store in DB
            if let Some(ref db) = self.db {
                if let Err(e) = db.insert_trial(
                    skill_id,
                    None,
                    &params,
                    reward,
                    converged,
                ).await {
                    tracing::warn!("Failed to insert trial result: {}", e);
                }
            }

            // Update GP with new data point
            let n_total = history.len() + results.len() + 1;
            let _x_new = (n_total - 1) as f64 / n_total.max(1) as f64;
            if results.len() >= 1 {
                // Refit GP every 3 trials to include new observations
                if trial % 3 == 0 || trial == n_trials {
                    let mut all_x: Vec<f64> = (0..n_total).map(|i| i as f64 / n_total.max(1) as f64).collect();
                    // Truncate to match available y values
                    all_x.truncate(history.len() + results.len());
                    let mut all_y: Vec<f64> = history.iter().map(|t| t.reward).collect();
                    all_y.extend(results.iter().map(|r: &TrialResult| r.reward));
                    let _ = gp.fit(&all_x, &all_y);
                }
            }

            if reward < current_best {
                current_best = reward;
            }

            results.push(TrialResult {
                trial_number: trial,
                parameters: params,
                reward,
                converged,
                deprecated: false,
            });
        }

        Ok(results)
    }

    /// Generate random parameters for initial exploration.
    fn random_params(trial: u32, total: u32) -> serde_json::Value {
        let t = trial as f64 / total.max(1) as f64;
        serde_json::json!({
            "mesh_size": 0.02 + rand::random::<f64>() * 0.08,
            "layers": (4.0 + rand::random::<f64>() * 12.0).round() as u32,
            "relaxation_p": 0.3 + rand::random::<f64>() * 0.5,
            "schedule": if t < 0.3 { "explore" } else { "exploit" },
        })
    }

    /// Simulate a trial reward without running the actual CFD pipeline.
    /// In production this would call the pipeline orchestrator.
    fn simulate_trial(&self, params: &serde_json::Value, trial: u32, total: u32) -> f64 {
        let t = trial as f64 / total.max(1) as f64;

        // Baseline reward: improves as we learn
        let base = 0.5 - t * 0.3;

        // Noise decreases over time (exploration bonus)
        let noise = rand::random::<f64>() * (0.2 - t * 0.1).max(0.05);

        // Parameter quality score
        let layers = params.get("layers").and_then(|v| v.as_u64()).unwrap_or(8) as f64;
        let mesh_size = params.get("mesh_size").and_then(|v| v.as_f64()).unwrap_or(0.05);
        let relax = params.get("relaxation_p").and_then(|v| v.as_f64()).unwrap_or(0.5);

        // Optimal: layers=10-12, mesh_size=0.02-0.04, relax=0.5-0.7
        let layer_quality = 1.0 - ((layers - 10.0) / 8.0).abs().min(1.0);
        let mesh_quality = 1.0 - ((mesh_size - 0.03) / 0.07).abs().min(1.0);
        let relax_quality = 1.0 - ((relax - 0.6) / 0.4).abs().min(1.0);
        let config_score = (layer_quality + mesh_quality + relax_quality) / 3.0;

        // Clamp to [0, 1]
        (base + noise * (1.0 - config_score) + (1.0 - config_score) * 0.2)
            .clamp(0.01, 1.0)
    }

    /// Suggest next parameters using GP acquisition function.
    pub fn suggest_next_params(&self, history: &[TrialResult]) -> serde_json::Value {
        if history.is_empty() {
            return Self::random_params(1, 10);
        }

        let mut gp = GaussianProcess::new();
        if history.len() >= 3 {
            let n = history.len();
            let x: Vec<f64> = (0..n).map(|i| i as f64 / n as f64).collect();
            let y: Vec<f64> = history.iter().map(|t| t.reward).collect();
            let _ = gp.fit(&x, &y);

            let best_y = y.iter().cloned().fold(f64::MAX, f64::min);
            let candidates: Vec<f64> = (0..=50).map(|i| i as f64 / 50.0).collect();
            let (best_x, _) = gp.expected_improvement(&candidates, best_y);

            serde_json::json!({
                "gp_index": best_x,
                "mesh_size": 0.02 + best_x * 0.08,
                "layers": (4.0 + best_x * 12.0).round() as u32,
                "relaxation_p": 0.3 + best_x * 0.5,
            })
        } else {
            Self::random_params(1, 10)
        }
    }

    /// Prune the bottom 20% of trials by reward.
    pub fn prune_bottom_20pct(history: &mut Vec<TrialResult>) {
        let n_prune = (history.len() as f64 * 0.2) as usize;
        history.sort_by(|a, b| a.reward.partial_cmp(&b.reward).unwrap());
        for trial in history.iter_mut().take(n_prune) {
            trial.deprecated = true;
        }
    }
}

#[derive(Debug, Clone)]
pub struct TrialResult {
    pub trial_number: u32,
    pub parameters: serde_json::Value,
    pub reward: f64,
    pub converged: bool,
    pub deprecated: bool,
}

impl TrialResult {
    pub fn new(trial: u32, parameters: serde_json::Value, reward: f64, converged: bool) -> Self {
        Self {
            trial_number: trial,
            parameters,
            reward,
            converged,
            deprecated: false,
        }
    }
}
