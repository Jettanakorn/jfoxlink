use std::f64::consts::TAU;

/// Gaussian Process regression with RBF (squared-exponential) kernel.
///
/// Supports 1-D input (scalar parameter index) for simplicity in P4.
/// Multi-dimensional support planned for P5 with `argmin`.
pub struct GaussianProcess {
    /// Training inputs (flattened to 1D index per point)
    x_train: Vec<f64>,
    /// Training targets
    y_train: Vec<f64>,
    /// Kernel hyperparameters
    length_scale: f64,
    signal_variance: f64,
    noise_variance: f64,
    /// Pre-computed Cholesky factor L of K + σ²I
    l_factor: Option<Vec<Vec<f64>>>,
}

impl GaussianProcess {
    pub fn new() -> Self {
        Self {
            x_train: Vec::new(),
            y_train: Vec::new(),
            length_scale: 0.2,
            signal_variance: 1.0,
            noise_variance: 1e-4,
            l_factor: None,
        }
    }

    pub fn with_hyperparameters(
        length_scale: f64,
        signal_variance: f64,
        noise_variance: f64,
    ) -> Self {
        Self {
            x_train: Vec::new(),
            y_train: Vec::new(),
            length_scale,
            signal_variance,
            noise_variance,
            l_factor: None,
        }
    }

    /// RBF (squared-exponential) kernel.
    fn kernel(&self, a: f64, b: f64) -> f64 {
        let diff = a - b;
        let scaled = diff / self.length_scale;
        self.signal_variance * (-0.5 * scaled * scaled).exp()
    }

    /// Fit the GP: compute Cholesky decomposition of K(X,X) + σ²I.
    pub fn fit(&mut self, x: &[f64], y: &[f64]) -> Result<(), anyhow::Error> {
        anyhow::ensure!(x.len() == y.len(), "x and y must have same length");
        anyhow::ensure!(x.len() >= 2, "Need at least 2 training points");

        self.x_train = x.to_vec();
        self.y_train = y.to_vec();
        let n = x.len();

        // Build kernel matrix K
        let mut k = vec![vec![0.0_f64; n]; n];
        for i in 0..n {
            for j in 0..n {
                k[i][j] = self.kernel(x[i], x[j]);
            }
            // Add noise variance to diagonal
            k[i][i] += self.noise_variance;
        }

        // Cholesky decomposition: K = L L^T
        let l = cholesky(&k)?;
        self.l_factor = Some(l);
        Ok(())
    }

    /// Predict mean and standard deviation at query points.
    pub fn predict(&self, x_test: &[f64]) -> Vec<(f64, f64)> {
        let n = self.x_train.len();
        let l = match &self.l_factor {
            Some(l) => l,
            None => return x_test.iter().map(|_| (0.0, self.signal_variance.sqrt())).collect(),
        };

        x_test.iter().map(|&xq| {
            // k* = K(X, xq)
            let k_star: Vec<f64> = self.x_train.iter().map(|&xi| self.kernel(xi, xq)).collect();

            // Solve L * alpha = k* for alpha (forward substitution)
            let mut alpha = k_star.clone();
            for i in 0..n {
                for j in 0..i {
                    alpha[i] -= l[i][j] * alpha[j];
                }
                alpha[i] /= l[i][i];
            }

            // Solve L^T * v = alpha for v (back substitution)
            let mut v = alpha.clone();
            for i in (0..n).rev() {
                for j in (i + 1..n).rev() {
                    v[i] -= l[j][i] * v[j];
                }
                v[i] /= l[i][i];
            }

            // Mean: k*^T * v
            let mean: f64 = k_star.iter().zip(v.iter()).map(|(k_, v_)| k_ * v_).sum();

            // Variance: k(xq, xq) - v^T * k*
            let k_xx = self.kernel(xq, xq) + self.noise_variance;
            let var: f64 = k_star.iter().zip(v.iter()).map(|(k_, v_)| k_ * v_).sum();
            let variance = (k_xx - var).max(1e-12);

            (mean, variance.sqrt())
        }).collect()
    }

    /// Expected Improvement acquisition function.
    ///
    /// `x_candidates`: points at which to evaluate EI.
    /// Returns the point with highest EI and its value.
    pub fn expected_improvement(&self, x_candidates: &[f64], best_so_far: f64) -> (f64, f64) {
        let predictions = self.predict(x_candidates);
        let mut best_ei = f64::NEG_INFINITY;
        let mut best_x = x_candidates[0];

        for (i, &(mean, std)) in predictions.iter().enumerate() {
            let x = x_candidates[i];
            let improvement = best_so_far - mean;
            if std < 1e-12 {
                if improvement > best_ei {
                    best_ei = improvement;
                    best_x = x;
                }
                continue;
            }
            let z = improvement / std;
            let ei = improvement * gaussian_cdf(z) + std * gaussian_pdf(z);
            if ei > best_ei {
                best_ei = ei;
                best_x = x;
            }
        }

        (best_x, best_ei)
    }
}

// ── Linear algebra helpers ──

/// Cholesky decomposition of a symmetric positive-definite matrix A.
/// Returns lower triangular L such that A = L L^T.
fn cholesky(a: &[Vec<f64>]) -> Result<Vec<Vec<f64>>, anyhow::Error> {
    let n = a.len();
    let mut l = vec![vec![0.0_f64; n]; n];

    for i in 0..n {
        for j in 0..=i {
            let mut sum = 0.0;
            for k in 0..j {
                sum += l[i][k] * l[j][k];
            }
            if i == j {
                let diag = a[i][i] - sum;
                anyhow::ensure!(diag > 1e-15, "Matrix not positive definite (diag={})", diag);
                l[i][j] = diag.sqrt();
            } else {
                l[i][j] = (a[i][j] - sum) / l[j][j];
            }
        }
    }
    Ok(l)
}

/// Standard Gaussian PDF
fn gaussian_pdf(x: f64) -> f64 {
    (-0.5 * x * x).exp() / TAU.sqrt()
}

/// Standard Gaussian CDF (approximation using Abramowitz & Stegun 26.2.17)
fn gaussian_cdf(x: f64) -> f64 {
    if x < -8.0 {
        return 0.0;
    }
    if x > 8.0 {
        return 1.0;
    }
    let a1 = 0.254829592;
    let a2 = -0.284496736;
    let a3 = 1.421413741;
    let a4 = -1.453152027;
    let a5 = 1.061405429;
    let p = 0.3275911;
    let sign = if x < 0.0 { -1.0 } else { 1.0 };
    let x_abs = x.abs();
    let t = 1.0 / (1.0 + p * x_abs);
    let y = 1.0 - (((((a5 * t + a4) * t) + a3) * t + a2) * t + a1) * t * (-0.5 * x_abs * x_abs).exp();
    0.5 * (1.0 + sign * y)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gaussian_cdf() {
        // Known values: Φ(0) = 0.5, Φ(1) ≈ 0.8413, Φ(-1) ≈ 0.1587
        assert!((gaussian_cdf(0.0) - 0.5).abs() < 1e-4);
        assert!((gaussian_cdf(1.0) - 0.8413).abs() < 1e-3);
        assert!((gaussian_cdf(-1.0) - 0.1587).abs() < 1e-3);
    }

    #[test]
    fn test_gp_fit_and_predict() {
        let mut gp = GaussianProcess::new();
        let x = vec![0.0, 0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9, 1.0];
        let y: Vec<f64> = x.iter().map(|&x| (x * TAU).sin()).collect(); // sin wave

        gp.fit(&x, &y).unwrap();
        let preds = gp.predict(&[0.5, 0.55, 0.6]);
        // Predictions should follow the sine wave
        for &(mean, std) in &preds {
            assert!(std > 0.0);
            assert!(std < 2.0);
        }
    }

    #[test]
    fn test_ei_chooses_best_point() {
        let mut gp = GaussianProcess::new();
        // Bimodal-like data: best point is near x=0.7
        let x = vec![0.0, 0.2, 0.4, 0.6, 0.8, 1.0];
        let y = vec![0.1, 0.3, 0.5, 0.9, 0.7, 0.2];
        gp.fit(&x, &y).unwrap();

        let candidates: Vec<f64> = (0..=100).map(|i| i as f64 / 100.0).collect();
        let (best_x, _) = gp.expected_improvement(&candidates, 0.9);
        assert!((best_x - 0.7).abs() < 0.15, "best_x={} should be near 0.7", best_x);
    }
}
