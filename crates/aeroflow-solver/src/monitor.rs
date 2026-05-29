use aeroflow_core::SolverStats;
use std::path::Path;

pub struct SolverMonitor;

impl Default for SolverMonitor {
    fn default() -> Self {
        Self::new()
    }
}

impl SolverMonitor {
    pub const DIVERGENCE_THRESHOLD: f64 = 1e3;
    pub const PLATEAU_WINDOW: u32 = 300;
    pub const FORCE_OSCILLATION_WINDOW: u32 = 100;

    pub fn new() -> Self {
        Self
    }

    pub async fn monitor_log(
        &self,
        log_path: &str,
        _container_id: &str,
    ) -> Result<SolverStats, anyhow::Error> {
        let path = Path::new(log_path);
        if !path.exists() {
            anyhow::bail!("Solver log not found: {}", log_path);
        }

        let content = std::fs::read_to_string(path)?;
        let mut iterations = 0u64;
        let mut best_p = 1.0_f64;
        let mut best_u = 1.0_f64;
        let mut converged = false;

        for line in content.lines() {
            // Parse time step
            if line.contains("Time =")
                && let Some(ts) = line.split('=').nth(1)
                    && let Ok(t) = ts.trim().parse::<u64>() {
                        iterations = t;
                    }

            // Parse residuals
            if line.contains("Initial residual")
                && let Some(res_part) = line.split("Initial residual = ").nth(1)
                    && let Some(val) = res_part.split(',').next()
                        && let Ok(r) = val.trim().parse::<f64>() {
                            let lower = line.to_lowercase();
                            let is_p = lower.contains("solving for p") || lower.contains("solving for p_rgh");
                            let is_u = lower.contains("solving for u");

                            if is_p
                                && r < best_p { best_p = r; }
                            if is_u
                                && r < best_u { best_u = r; }

                            if r > Self::DIVERGENCE_THRESHOLD {
                                // divergent — P3: log or act on this
                            }
                        }

            if line.contains("Solver converged") || (line.contains("converged") && line.contains("iteration")) {
                converged = true;
            }
        }

        let wall_time_s = 0.0;

        Ok(SolverStats {
            iterations,
            wall_time_s,
            residual_p: best_p,
            residual_u: best_u,
            converged: converged || (best_p < 2e-6 && best_u < 2e-6),
        })
    }

    pub fn parse_residual_line(line: &str) -> Option<(String, f64)> {
        if !line.contains("Initial residual") {
            return None;
        }

        // Extract field name from lines like:
        // "Solving for Ux:  GAMG:  Solving for p, Initial residual = 0.00123"
        let field = if line.contains("Solving for") {
            let after_solving = line.split("Solving for ").nth(1)?;
            let field_name = after_solving.split([':', ',']).next()?;
            field_name.trim().to_string()
        } else {
            return None;
        };

        let res_part = line.split("Initial residual = ").nth(1)?;
        let val = res_part.split(',').next()?.trim();
        let residual: f64 = val.parse().ok()?;

        Some((field, residual))
    }
}
