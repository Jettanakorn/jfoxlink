use std::path::Path;

/// Rotating machinery performance metrics
#[derive(Debug, Clone)]
pub struct RotatingMetrics {
    /// Thrust coefficient CT = T / (rho * n^2 * D^4)
    pub thrust_coefficient: f64,
    /// Torque coefficient CQ = Q / (rho * n^2 * D^5)
    pub torque_coefficient: f64,
    /// Power coefficient CP = 2*pi * CQ
    pub power_coefficient: f64,
    /// Propulsive efficiency eta = CT * J / CP
    pub efficiency: f64,
    /// Shaft power (W)
    pub shaft_power_w: f64,
    /// Thrust (N)
    pub thrust_n: f64,
}

#[derive(Debug, Clone)]
pub struct TurbomachineryMetrics {
    /// Pressure ratio
    pub pressure_ratio: f64,
    /// Flow coefficient phi = Cm / U_tip
    pub flow_coefficient: f64,
    /// Head coefficient psi = delta_H0 / U_tip^2
    pub head_coefficient: f64,
    /// Total-to-total efficiency
    pub efficiency_tt: f64,
}

pub struct RotatingExtractor;

impl Default for RotatingExtractor {
    fn default() -> Self {
        Self::new()
    }
}

impl RotatingExtractor {
    pub fn new() -> Self {
        Self
    }

    /// Extract propeller/rotor performance from forceCoeffs output
    pub fn extract_propeller(
        &self,
        case_path: &str,
        rpm: f64,
        diameter_m: f64,
        advance_ratio: f64,
        rho: f64,
    ) -> Result<RotatingMetrics, anyhow::Error> {
        let path = Path::new(case_path)
            .join("postProcessing")
            .join("propellerForces")
            .join("0")
            .join("coefficient.dat");

        if !path.exists() {
            // Fall back to standard forceCoeffs
            return self.extract_propeller_fallback(case_path, rpm, diameter_m, advance_ratio, rho);
        }

        let content = std::fs::read_to_string(&path)?;
        let mut cl_values: Vec<f64> = Vec::new();
        let mut cd_values: Vec<f64> = Vec::new();

        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') { continue; }
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 3 {
                let cl_val = parts.get(2).and_then(|s| s.parse::<f64>().ok()).unwrap_or(0.0);
                let cd_val = parts.get(1).and_then(|s| s.parse::<f64>().ok()).unwrap_or(0.0);
                cd_values.push(cd_val);
                cl_values.push(cl_val);
            }
        }

        if cl_values.is_empty() {
            return self.extract_propeller_fallback(case_path, rpm, diameter_m, advance_ratio, rho);
        }

        // Use last 50% of values for stable average
        let start = cl_values.len() / 2;
        let ct_avg = cl_values[start..].iter().copied().sum::<f64>() / (cl_values.len() - start) as f64;
        let cq_avg = cd_values[start..].iter().copied().sum::<f64>() / (cd_values.len() - start) as f64;

        let n = rpm / 60.0;
        let cp = 2.0 * std::f64::consts::PI * cq_avg;
        let eta = if cp > 0.0 { ct_avg * advance_ratio / cp } else { 0.0 };
        let thrust = ct_avg * rho * n * n * diameter_m.powi(4);
        let shaft_power = cp * rho * n.powi(3) * diameter_m.powi(5);

        Ok(RotatingMetrics {
            thrust_coefficient: ct_avg,
            torque_coefficient: cq_avg,
            power_coefficient: cp,
            efficiency: eta,
            shaft_power_w: shaft_power,
            thrust_n: thrust,
        })
    }

    fn extract_propeller_fallback(
        &self,
        case_path: &str,
        rpm: f64,
        diameter_m: f64,
        advance_ratio: f64,
        rho: f64,
    ) -> Result<RotatingMetrics, anyhow::Error> {
        // Read from standard forceCoeffs output — thrust = liftDir, torque = dragDir
        let path = Path::new(case_path)
            .join("postProcessing")
            .join("forceCoeffs")
            .join("0")
            .join("coefficient.dat");

        let content = std::fs::read_to_string(&path)?;
        let mut cd_sum = 0.0_f64;
        let mut cl_sum = 0.0_f64;
        let mut count = 0u64;

        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') { continue; }
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 3 {
                let cl_val = parts.get(2).and_then(|s| s.parse::<f64>().ok()).unwrap_or(0.0);
                let cd_val = parts.get(1).and_then(|s| s.parse::<f64>().ok()).unwrap_or(0.0);
                cd_sum += cd_val;
                cl_sum += cl_val;
                count += 1;
            }
        }

        if count == 0 { anyhow::bail!("No force data found"); }

        let avg_ct = cl_sum / count as f64;
        let avg_cq = cd_sum / count as f64;

        let n = rpm / 60.0;
        let cp = 2.0 * std::f64::consts::PI * avg_cq;
        let eta = if cp > 0.0 { avg_ct * advance_ratio / cp } else { 0.0 };
        let thrust = avg_ct * rho * n * n * diameter_m.powi(4);
        let shaft_power = cp * rho * n.powi(3) * diameter_m.powi(5);

        Ok(RotatingMetrics {
            thrust_coefficient: avg_ct,
            torque_coefficient: avg_cq,
            power_coefficient: cp,
            efficiency: eta,
            shaft_power_w: shaft_power,
            thrust_n: thrust,
        })
    }

    /// Extract turbomachinery performance (pressure ratio, flow coefficient, etc.)
    pub fn extract_turbomachinery(
        &self,
        case_path: &str,
        rpm: f64,
        tip_diameter_m: f64,
    ) -> Result<TurbomachineryMetrics, anyhow::Error> {
        let case_dir = Path::new(case_path);

        // Read inlet/outlet average pressure from postProcessing
        let p_in = self.read_patch_average(case_dir, "inlet", "p")?;
        let p_out = self.read_patch_average(case_dir, "outlet", "p")?;
        let pressure_ratio = if p_in > 0.0 { p_out / p_in } else { 1.0 };

        let u_tip = rpm / 60.0 * std::f64::consts::PI * tip_diameter_m;

        // Estimate flow coefficient from inlet velocity
        let u_in = self.read_patch_average(case_dir, "inlet", "U_mag")?;
        let flow_coefficient = if u_tip > 0.0 { u_in / u_tip } else { 0.0 };

        Ok(TurbomachineryMetrics {
            pressure_ratio,
            flow_coefficient,
            head_coefficient: 0.0, // requires total enthalpy data
            efficiency_tt: 0.0,
        })
    }

    /// Read averaged field value on a patch from postProcessing data
    fn read_patch_average(&self, case_dir: &Path, patch: &str, field: &str) -> Result<f64, anyhow::Error> {
        // Try fieldValueAverage output first
        let field_file = if field == "U_mag" { "U.dat".to_string() } else { format!("{}.dat", field) };
        let pattern = format!("postProcessing/{}/0/{}", patch, field_file);
        let path = case_dir.join(&pattern);
        if path.exists() {
            let content = std::fs::read_to_string(&path)?;
            if let Some(last) = content.lines().rfind(|l| !l.starts_with('#')) {
                let parts: Vec<&str> = last.split_whitespace().collect();
                if !parts.is_empty() {
                    if field == "U_mag" { return Ok(parts.last().and_then(|s| s.parse::<f64>().ok()).unwrap_or(0.0)); }
                    if parts.len() >= 2 { return Ok(parts[1].parse::<f64>().unwrap_or(0.0)); }
                }
            }
        }
        Ok(0.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Write a forceCoeffs coefficient.dat fixture and run extract_propeller
    fn run_with_fixture(name: &str, lines: &[&str], rpm: f64, diam: f64, adv: f64, rho: f64) -> RotatingMetrics {
        let dir = std::env::temp_dir().join(format!("aeroflow_test_rotating_{}", name));
        // Start fresh each time
        let _ = std::fs::remove_dir_all(&dir);
        let post_dir = dir.join("postProcessing").join("forceCoeffs").join("0");
        std::fs::create_dir_all(&post_dir).unwrap();

        let path = post_dir.join("coefficient.dat");
        let content = lines.join("\n");
        std::fs::write(&path, &content).unwrap();

        let result = RotatingExtractor::new()
            .extract_propeller(dir.to_str().unwrap(), rpm, diam, adv, rho)
            .expect("extract_propeller should succeed");

        // Cleanup
        std::fs::remove_dir_all(&dir).ok();

        result
    }

    #[test]
    fn test_extract_propeller_basic() {
        let data = &[
            "# Time  Cd  Cl  Cm  ...",
            "0.001  0.05  0.12  -0.01  0  0  0  0",
            "0.002  0.05  0.11  -0.01  0  0  0  0",
            "0.003  0.04  0.13  -0.01  0  0  0  0",
        ];
        let m = run_with_fixture("basic", data, 2500.0, 0.5, 0.5, 1.225);
        assert!((m.thrust_coefficient - 0.12).abs() < 0.01);
        assert!((m.power_coefficient - 2.0 * std::f64::consts::PI * 0.046666).abs() < 0.01);
        assert!(m.shaft_power_w > 0.0);
        assert!(m.thrust_n > 0.0);
    }

    #[test]
    fn test_extract_propeller_efficiency() {
        let data = &[
            "# Time  Cd  Cl  ...",
            "0.001  0.02  0.10  0  0  0  0  0",
            "0.002  0.02  0.10  0  0  0  0  0",
        ];
        let m = run_with_fixture("eff", data, 2500.0, 0.5, 0.5, 1.225);
        assert!((m.efficiency - 0.398).abs() < 0.01);
    }

    #[test]
    fn test_extract_propeller_no_data_fails() {
        let dir = std::env::temp_dir().join("aeroflow_test_rotating_empty");
        let _ = std::fs::remove_dir_all(&dir);
        let result = RotatingExtractor::new()
            .extract_propeller(dir.to_str().unwrap(), 2500.0, 0.5, 0.5, 1.225);
        assert!(result.is_err());
    }

    #[test]
    fn test_rotating_metrics_sanity() {
        let data = &[
            "# Time  Cd  Cl  ...",
            "0.001  0.10  0.50  0  0  0  0  0",
            "0.002  0.10  0.50  0  0  0  0  0",
            "0.003  0.10  0.50  0  0  0  0  0",
        ];
        let m = run_with_fixture("sanity", data, 3000.0, 0.8, 0.3, 1.225);
        assert!(m.power_coefficient > 0.0);
        assert!(m.efficiency > 0.0 && m.efficiency < 1.0);
        assert!(m.shaft_power_w > 100.0);
        assert!(m.thrust_n > 10.0);
    }
}
