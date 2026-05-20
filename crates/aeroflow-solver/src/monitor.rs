use aeroflow_core::SolverStats;

pub struct SolverMonitor;

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
        container_id: &str,
    ) -> Result<SolverStats, anyhow::Error> {
        // P2: tail solver log via tokio, parse residuals, detect divergence
        let _ = (log_path, container_id);
        Ok(SolverStats {
            iterations: 0,
            wall_time_s: 0.0,
            residual_p: 0.0,
            residual_u: 0.0,
            converged: false,
        })
    }

    pub fn parse_residual_line(line: &str) -> Option<(String, f64)> {
        // Parse lines like: "Solving for Ux:  GAMG:  Solving for p, Initial residual = 0.00123"
        let _ = line;
        None
    }
}
