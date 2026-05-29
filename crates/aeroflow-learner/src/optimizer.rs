use crate::gp::GaussianProcess;
use aeroflow_core::{IntakeConfig, TrialOutput};
use aeroflow_skills::SkillsDb;

pub type TrialRunner = Box<dyn Fn(MeshParamsTrial) -> Result<TrialOutput, anyhow::Error> + Send + Sync>;

pub struct Optimizer {
    db: Option<SkillsDb>,
}

impl Optimizer {
    pub fn new(db: Option<SkillsDb>) -> Self {
        Self { db }
    }

    /// Run Bayesian optimization trials for a skill.
    ///
    /// Each trial samples a different parameter configuration, runs the
    /// actual CFD pipeline via `runner`, computes reward, and stores results.
    pub async fn run_trials(
        &self,
        _intake: &IntakeConfig,
        skill_id: uuid::Uuid,
        n_trials: u32,
        runner: &TrialRunner,
    ) -> Result<Vec<TrialResult>, anyhow::Error> {
        anyhow::ensure!(n_trials > 0, "Need at least 1 trial");

        let history = if let Some(ref db) = self.db {
            db.get_trials(skill_id, 100).await?
        } else {
            Vec::new()
        };

        let mut gp = GaussianProcess::new();

        if history.len() >= 2 {
            let x: Vec<f64> = (0..history.len()).map(|i| i as f64 / history.len() as f64).collect();
            let y: Vec<f64> = history.iter().map(|t| t.reward).collect();
            if let Err(e) = gp.fit(&x, &y) {
                tracing::warn!("GP fit failed: {}", e);
            }
        }

        let mut results = Vec::new();
        let best_so_far = history.iter()
            .map(|t| t.reward)
            .fold(f64::MAX, |a, b| a.min(b));

        let mut current_best = best_so_far;

        for trial in 1..=n_trials {
            let (params, mp) = if history.len() + results.len() >= 2 {
                let candidates: Vec<f64> = (0..=50).map(|i| i as f64 / 50.0).collect();
                let (best_x, _) = gp.expected_improvement(&candidates, current_best);
                Self::params_from_gp(best_x, trial, n_trials)
            } else {
                Self::random_params(trial, n_trials)
            };

            tracing::info!(
                "Trial {}/{}: surface=({},{}), region=({},{}), nCellsBetween={}",
                trial, n_trials,
                mp.surface_min_level, mp.surface_max_level,
                mp.region_min_level, mp.region_max_level,
                mp.n_cells_between_levels,
            );

            let output = match runner(mp) {
                Ok(o) => o,
                Err(e) => {
                    tracing::warn!("Trial {}/{} failed: {}", trial, n_trials, e);
                    let r = TrialResult {
                        trial_number: trial,
                        parameters: params.clone(),
                        reward: 1.0,
                        converged: false,
                        deprecated: false,
                        runtime_s: Some(0.0),
                        n_cells: Some(0),
                        cd: None,
                        cl: None,
                    };
                    if let Some(ref db) = self.db {
                        let _ = db.insert_trial(skill_id, None, &params, 1.0, false).await;
                    }
                    results.push(r);
                    continue;
                }
            };

            let converged = output.converged;
            let cd = output.cd;
            let cl = output.cl;

            // Reward: minimize Cd, penalize runtime & mesh failures
            let cd_score = (cd / 0.01).min(2.0);
            let mesh_penalty = if output.n_failed_cells > 0 { 0.2 } else { 0.0 };
            let runtime_penalty = (output.runtime_s / 7200.0).min(0.5);
            let convergence_penalty = if converged { 0.0 } else { 0.3 };
            let reward = (cd_score * 0.5 + mesh_penalty + runtime_penalty * 0.2 + convergence_penalty * 0.3)
                .clamp(0.01, 2.0);

            if let Some(ref db) = self.db {
                let _ = db.insert_trial(skill_id, None, &params, reward, converged).await;
            }

            let n_total = history.len() + results.len() + 1;
            if !results.is_empty() && (trial % 3 == 0 || trial == n_trials) {
                let mut all_x: Vec<f64> = (0..n_total).map(|i| i as f64 / n_total.max(1) as f64).collect();
                all_x.truncate(history.len() + results.len());
                let mut all_y: Vec<f64> = history.iter().map(|t| t.reward).collect();
                all_y.extend(results.iter().map(|r: &TrialResult| r.reward));
                let _ = gp.fit(&all_x, &all_y);
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
                runtime_s: Some(output.runtime_s),
                n_cells: Some(output.n_cells),
                cd: Some(cd),
                cl: Some(cl),
            });
        }

        Ok(results)
    }

    fn params_from_gp(best_x: f64, trial: u32, total: u32) -> (serde_json::Value, MeshParamsTrial) {
        let t = trial as f64 / total.max(1) as f64;
        let surface_min = 1 + (best_x * 3.0).round() as u32;
        let surface_max = (surface_min + 1).min(5);
        let region_val = if best_x < 0.5 { 0 } else { 1 };
        let n_cells = if best_x < 0.3 { 5 } else if best_x < 0.7 { 4 } else { 3 };
        let mp = MeshParamsTrial {
            surface_min_level: surface_min.clamp(1, 4),
            surface_max_level: surface_max.clamp(2, 5),
            region_min_level: region_val,
            region_max_level: region_val,
            n_cells_between_levels: n_cells,
        };
        let params = serde_json::json!({
            "gp_index": best_x,
            "surface_min_level": mp.surface_min_level,
            "surface_max_level": mp.surface_max_level,
            "region_level": region_val,
            "n_cells_between_levels": n_cells,
            "schedule": if t < 0.3 { "explore" } else { "exploit" },
        });
        (params, mp)
    }

    fn random_params(trial: u32, total: u32) -> (serde_json::Value, MeshParamsTrial) {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        let t = trial as f64 / total.max(1) as f64;
        let surface_min = rng.gen_range(1..=4);
        let surface_max = (surface_min + rng.gen_range(1..=2)).min(5);
        let region_val: u32 = if rng.gen_bool(0.5) { 0 } else { 1 };
        let n_cells = rng.gen_range(3..=5);
        let mp = MeshParamsTrial {
            surface_min_level: surface_min,
            surface_max_level: surface_max,
            region_min_level: region_val,
            region_max_level: region_val,
            n_cells_between_levels: n_cells,
        };
        let params = serde_json::json!({
            "surface_min_level": mp.surface_min_level,
            "surface_max_level": mp.surface_max_level,
            "region_level": region_val,
            "n_cells_between_levels": n_cells,
            "schedule": if t < 0.3 { "explore" } else { "exploit" },
        });
        (params, mp)
    }

    pub fn suggest_next_params(&self, history: &[TrialResult]) -> serde_json::Value {
        if history.is_empty() {
            let (params, _) = Self::random_params(1, 10);
            return params;
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
            let (params, _) = Self::params_from_gp(best_x, 1, 10);
            params
        } else {
            let (params, _) = Self::random_params(1, 10);
            params
        }
    }

    pub fn prune_bottom_20pct(history: &mut [TrialResult]) {
        let n_prune = (history.len() as f64 * 0.2) as usize;
        history.sort_by(|a, b| a.reward.total_cmp(&b.reward));
        for trial in history.iter_mut().take(n_prune) {
            trial.deprecated = true;
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct MeshParamsTrial {
    pub surface_min_level: u32,
    pub surface_max_level: u32,
    pub region_min_level: u32,
    pub region_max_level: u32,
    pub n_cells_between_levels: u32,
}

#[derive(Debug, Clone)]
pub struct TrialResult {
    pub trial_number: u32,
    pub parameters: serde_json::Value,
    pub reward: f64,
    pub converged: bool,
    pub deprecated: bool,
    pub runtime_s: Option<f64>,
    pub n_cells: Option<u64>,
    pub cd: Option<f64>,
    pub cl: Option<f64>,
}

impl TrialResult {
    pub fn new(trial: u32, parameters: serde_json::Value, reward: f64, converged: bool) -> Self {
        Self {
            trial_number: trial,
            parameters,
            reward,
            converged,
            deprecated: false,
            runtime_s: None,
            n_cells: None,
            cd: None,
            cl: None,
        }
    }
}
