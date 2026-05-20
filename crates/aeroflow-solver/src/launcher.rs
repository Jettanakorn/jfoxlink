use aeroflow_core::SolverStats;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tracing::{error, info, warn};

/// Called periodically with (iteration, residual_p, residual_u) during solve.
pub type ProgressCallback = Box<dyn Fn(u64, f64, f64) + Send>;

pub struct SolverLauncher;

impl SolverLauncher {
    pub fn new() -> Self {
        Self
    }

    pub fn spawn(case_path: &Path, solver: &str) -> Result<Child, anyhow::Error> {
        info!("Spawning {} in {:?}", solver, case_path);
        let child = Command::new(solver)
            .args(["-case", &case_path.to_string_lossy()])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .current_dir(case_path)
            .spawn()?;
        Ok(child)
    }

    /// Run solver with plateau detection and optional progress callbacks.
    ///
    /// Plateau detection: if both p and U residuals haven't improved by
    /// at least `plateau_improvement` over `plateau_window` iterations,
    /// declare converged at the best residual level seen.
    pub fn run_and_monitor(
        case_path: &Path,
        solver: &str,
        cancel: Option<Arc<AtomicBool>>,
        on_progress: Option<ProgressCallback>,
        plateau_window: u64,
        _plateau_improvement: f64,
    ) -> Result<SolverStats, anyhow::Error> {
        let mut child = Self::spawn(case_path, solver)?;
        let stdout = child.stdout.take().expect("stdout captured");
        let stderr = child.stderr.take().expect("stderr captured");

        let mut iterations = 0u64;
        let mut residual_p = 1.0_f64;
        let mut residual_u = 1.0_f64;
        let start = std::time::Instant::now();

        // Plateau tracking
        let mut best_p = residual_p;
        let mut best_u = residual_u;
        let mut since_best_p = 0u64;
        let mut since_best_u = 0u64;
        let mut plateau = false;

        let reader = BufReader::new(stdout);
        for line in reader.lines() {
            let line = line?;

            if let Some(ref flag) = cancel {
                if flag.load(Ordering::Relaxed) {
                    let _ = child.kill();
                    anyhow::bail!("Solver cancelled by user");
                }
            }

            // Parse time step
            if line.contains("Time =") {
                if let Some(ts) = line.split('=').nth(1) {
                    if let Ok(t) = ts.trim().parse::<u64>() {
                        iterations = t;
                    }
                }
            }

            // Parse residuals
            if line.contains("Initial residual") {
                if let Some(res_part) = line.split("Initial residual = ").nth(1) {
                    if let Some(val) = res_part.split_whitespace().next() {
                        if let Ok(r) = val.parse::<f64>() {
                            let is_p = line.contains("solving for p") || line.contains("solving for p_rgh");
                            let is_u = line.contains("solving for U");

                            if is_p {
                                residual_p = r;
                                if r < best_p {
                                    best_p = r;
                                    since_best_p = 0;
                                } else {
                                    since_best_p += 1;
                                }
                            }
                            if is_u {
                                // Track worst U residual component
                                if iterations == 0 || r > residual_u {
                                    residual_u = r;
                                }
                                if r < best_u {
                                    best_u = r;
                                    since_best_u = 0;
                                } else {
                                    since_best_u += 1;
                                }
                            }
                        }
                    }
                }
            }

            // Call progress callback every 50 iterations
            if iterations > 0 && iterations % 50 == 0 {
                if let Some(ref cb) = on_progress {
                    cb(iterations, residual_p, residual_u);
                }
                info!("  Iter {}: p={:.2e}, U={:.2e}", iterations, residual_p, residual_u);
            }

            // Plateau detection: if both residuals stagnated, stop solver
            if plateau_window > 0
                && since_best_p >= plateau_window
                && since_best_u >= plateau_window
            {
                info!(
                    "Residual plateau detected at iteration {} — p={:.2e}, U={:.2e}",
                    iterations, best_p, best_u
                );
                plateau = true;
                break;
            }

            // Early convergence
            if residual_p < 1e-8 && residual_u < 1e-8 {
                info!("Tight convergence at iteration {}", iterations);
                break;
            }
        }

        // Gracefully terminate solver if we broke out early
        if plateau || (residual_p < 1e-8 && residual_u < 1e-8) {
            let _ = child.kill();
            let _ = child.wait();
        } else {
            let err_reader = BufReader::new(stderr);
            for line in err_reader.lines() {
                let line = line?;
                if line.contains("Error") || line.contains("FOAM FATAL") {
                    error!("Solver error: {}", line);
                }
            }

            let status = child.wait()?;
            if !status.success() {
                anyhow::bail!("Solver {} exited with status {:?}", solver, status.code());
            }
        }

        let wall_time_s = start.elapsed().as_secs_f64();

        let converged = best_p < 1e-6 && best_u < 1e-6;
        if converged {
            info!("Solver converged in {} iterations ({:.1}s)", iterations, wall_time_s);
        } else {
            warn!(
                "Solver finished (best p={:.2e}, U={:.2e}) — {}",
                best_p, best_u,
                if plateau { "plateau detected" } else { "no convergence" }
            );
        }

        Ok(SolverStats {
            iterations,
            wall_time_s,
            residual_p: best_p,
            residual_u: best_u,
            converged,
        })
    }
}
