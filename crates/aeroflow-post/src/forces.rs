use aeroflow_core::ForceCoefficients;
use std::fs;
use std::path::Path;

pub struct ForceExtractor;

impl ForceExtractor {
    pub fn new() -> Self {
        Self
    }

    pub fn compute_directions(alpha_deg: f64) -> ([f64; 3], [f64; 3]) {
        let alpha = alpha_deg.to_radians();
        let lift_dir = [-alpha.sin(), 0.0, alpha.cos()];
        let drag_dir = [alpha.cos(), 0.0, alpha.sin()];
        (lift_dir, drag_dir)
    }

    /// Extract force coefficients from postProcessing/forceCoeffs/0/coefficient.dat
    pub fn extract_from_case(case_path: &str) -> Result<ForceCoefficients, anyhow::Error> {
        let path = Path::new(case_path)
            .join("postProcessing")
            .join("forceCoeffs")
            .join("0")
            .join("coefficient.dat");

        if !path.exists() {
            anyhow::bail!("Force coefficients file not found: {:?}", path);
        }

        let content = fs::read_to_string(&path)?;
        let mut cl_values = Vec::new();
        let mut cd_values = Vec::new();
        let mut cm_values = Vec::new();

        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            // Format: Time Cd Cd(f) Cd(r) Cl Cl(f) Cl(r) CmPitch CmRoll CmYaw Cs Cs(f) Cs(r)
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 8 {
                if let (Ok(cd), Ok(cl), Ok(cm)) = (
                    parts.get(1).unwrap_or(&"0").parse::<f64>(),
                    parts.get(4).unwrap_or(&"0").parse::<f64>(),
                    parts.get(7).unwrap_or(&"0").parse::<f64>(),
                ) {
                    // Skip initial transient (first 10% of data points)
                    // Filter unphysical values
                    if cl.abs() < 100.0 && cd.abs() < 100.0 {
                        cl_values.push(cl);
                        cd_values.push(cd);
                        cm_values.push(cm);
                    }
                }
            }
        }

        if cl_values.is_empty() {
            anyhow::bail!("No valid force coefficient data found in {:?}", path);
        }

        // Average last 50% of steady-state values
        let steady_start = cl_values.len() / 2;
        let steady_cl: Vec<f64> = cl_values[steady_start..].to_vec();
        let steady_cd: Vec<f64> = cd_values[steady_start..].to_vec();
        let steady_cm: Vec<f64> = cm_values[steady_start..].to_vec();

        let (cl_avg, cl_std) = Self::average_last_n(&cl_values, steady_cl.len());
        let (cd_avg, cd_std) = Self::average_last_n(&cd_values, steady_cd.len());
        let cm_avg = steady_cm.iter().sum::<f64>() / steady_cm.len() as f64;

        tracing::info!(
            "Force coefficients: Cl={:.4}±{:.4}, Cd={:.4}±{:.4}, Cm={:.4}",
            cl_avg, cl_std, cd_avg, cd_std, cm_avg
        );

        Ok(ForceCoefficients {
            cl: cl_avg,
            cd: cd_avg,
            cm: cm_avg,
            cl_std,
            cd_std,
        })
    }

    pub fn average_last_n(values: &[f64], n: usize) -> (f64, f64) {
        let n = n.min(values.len());
        let slice = &values[values.len() - n..];
        let mean = slice.iter().sum::<f64>() / n as f64;
        let variance = slice.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / n as f64;
        (mean, variance.sqrt())
    }
}
