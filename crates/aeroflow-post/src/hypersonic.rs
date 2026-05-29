use std::path::Path;

/// Stagnation point heat flux (Fay-Riddell) and wall heat flux metrics
#[derive(Debug, Clone)]
pub struct AerothermalMetrics {
    /// Stagnation point heat flux from Fay-Riddell (W/m²)
    pub stagnation_heat_flux_w_m2: f64,
    /// Peak wall heat flux from simulation (W/m²)
    pub peak_wall_heat_flux_w_m2: Option<f64>,
    /// Stagnation temperature (K)
    pub stagnation_temperature_k: f64,
    /// Wall temperature (K)
    pub wall_temperature_k: f64,
    /// Shock stand-off distance (m) — for blunt bodies
    pub shock_stand_off_m: Option<f64>,
}

#[derive(Debug, Clone)]
pub struct SpeciesMetrics {
    pub mass_fractions: Vec<(String, f64)>,
}

pub struct HypersonicExtractor;

impl Default for HypersonicExtractor {
    fn default() -> Self {
        Self::new()
    }
}

impl HypersonicExtractor {
    pub fn new() -> Self {
        Self
    }

    /// Compute stagnation heat flux using Fay-Riddell correlation.
    /// Reference: Fay & Riddell (1958), "Theory of Stagnation Point Heat Transfer in Dissociated Air"
    pub fn fay_riddell_stagnation_heat_flux(
        &self,
        rho_inf: f64,
        u_inf: f64,
        nose_radius_m: f64,
        t_wall_k: f64,
    ) -> f64 {
        // Post-shock conditions (normal shock relations for calorically perfect gas at Ma>>1)
        let gamma = 1.4;
        let rho_s = rho_inf * (gamma + 1.0) / (gamma - 1.0); // density ratio across strong shock ≈ 6
        let h_0 = 0.5 * u_inf * u_inf + 1004.5 * 300.0; // stagnation enthalpy (cp ≈ 1004.5 for air)
        let h_w = 1004.5 * t_wall_k;
        let pr: f64 = 0.71;
        let mu_ref = 1.789e-5;
        let t_ref = 288.15;
        let t_s = u_inf * u_inf / (2.0 * 1004.5); // rough shock temp
        let mu_s = mu_ref * (t_s / t_ref).powf(0.7);
        let du_ds = (1.0 / nose_radius_m) * (2.0 * (rho_s - rho_inf) / rho_s * 9.81).sqrt();

        0.763 * pr.powf(-0.6) * (rho_s * mu_s).sqrt() * du_ds.sqrt() * (h_0 - h_w)
    }

    /// Extract peak wall heat flux from wallHeatFlux postProcessing output
    pub fn extract_peak_heat_flux(&self, case_path: &str) -> Option<f64> {
        let path = Path::new(case_path)
            .join("postProcessing")
            .join("heatFlux")
            .join("0")
            .join("wallHeatFlux.dat");
        if !path.exists() { return None; }
        let content = std::fs::read_to_string(path).ok()?;
        let mut peak = 0.0_f64;
        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') { continue; }
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2
                && let Ok(val) = parts[1].parse::<f64>()
                    && val > peak { peak = val; }
        }
        Some(peak)
    }

    /// Compute stagnation temperature for a given Mach number
    pub fn stagnation_temperature(&self, t_inf: f64, mach: f64) -> f64 {
        t_inf * (1.0 + (1.4 - 1.0) / 2.0 * mach * mach)
    }

    /// Estimate shock stand-off distance for a sphere (or sphere-cone)
    /// Reference: Lobb (1964) empirical correlation for bow shock
    pub fn shock_stand_off(&self, nose_radius_m: f64, rho_inf: f64, rho_2: f64) -> f64 {
        // d = 0.19 * R_nose * (rho_inf / rho_2)
        // rho_2 is post-shock density ≈ 6 * rho_inf for Ma >> 1
        0.19 * nose_radius_m * rho_inf / rho_2
    }

    /// Extract species mass fractions from postProcessing data (hy2Foam output)
    pub fn extract_species(&self, case_path: &str, time_step: &str) -> Vec<(String, f64)> {
        let mut results = Vec::new();
        for species in &["N2", "O2", "NO", "N", "O"] {
            let path = Path::new(case_path)
                .join("postProcessing")
                .join("species")
                .join(time_step)
                .join(format!("{}.dat", species));
            if let Ok(content) = std::fs::read_to_string(&path)
                && let Some(last) = content.lines().rfind(|l| !l.starts_with('#')) {
                    let parts: Vec<&str> = last.split_whitespace().collect();
                    if parts.len() >= 2
                        && let Ok(val) = parts[1].parse::<f64>() {
                            results.push((species.to_string(), val));
                        }
                }
        }
        results
    }

    /// Aggregate all hypersonic metrics
    #[allow(clippy::too_many_arguments)]
    pub fn extract_aerothermal(
        &self,
        case_path: &str,
        rho_inf: f64,
        u_inf: f64,
        mach_inf: f64,
        t_inf: f64,
        nose_radius_m: Option<f64>,
        t_wall_k: Option<f64>,
    ) -> AerothermalMetrics {
        let t_wall = t_wall_k.unwrap_or(1500.0);
        let r_nose = nose_radius_m.unwrap_or(0.05);
        let t_stag = self.stagnation_temperature(t_inf, mach_inf);
        let q_stag = self.fay_riddell_stagnation_heat_flux(rho_inf, u_inf, r_nose, t_wall);
        let peak_q = self.extract_peak_heat_flux(case_path);
        let rho_2 = rho_inf * 6.0; // strong shock limit
        let delta = self.shock_stand_off(r_nose, rho_inf, rho_2);

        AerothermalMetrics {
            stagnation_heat_flux_w_m2: q_stag,
            peak_wall_heat_flux_w_m2: peak_q,
            stagnation_temperature_k: t_stag,
            wall_temperature_k: t_wall,
            shock_stand_off_m: Some(delta),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stagnation_temperature() {
        let ext = HypersonicExtractor::new();
        // Ma=10, T=300K → T0 = 300 * (1 + 0.2 * 100) = 6300 K
        let t = ext.stagnation_temperature(300.0, 10.0);
        assert!((t - 6300.0).abs() < 1.0);
    }

    #[test]
    fn test_stagnation_temperature_subsonic() {
        let ext = HypersonicExtractor::new();
        // Ma=0.5, T=300K → T0 = 300 * (1 + 0.2 * 0.25) = 315 K
        let t = ext.stagnation_temperature(300.0, 0.5);
        assert!((t - 315.0).abs() < 1.0);
    }

    #[test]
    fn test_fay_riddell_positive() {
        let ext = HypersonicExtractor::new();
        let q = ext.fay_riddell_stagnation_heat_flux(1.225, 3403.0, 0.05, 1000.0);
        // Ma=10 at sea level should produce MW/m² range heat flux
        assert!(q > 1e5);
        assert!(q < 1e9);
    }

    #[test]
    fn test_fay_riddell_scales_with_rho() {
        let ext = HypersonicExtractor::new();
        let q_high = ext.fay_riddell_stagnation_heat_flux(1.225, 3403.0, 0.05, 1000.0);
        let q_low = ext.fay_riddell_stagnation_heat_flux(0.01, 3403.0, 0.05, 1000.0);
        // Lower density → lower heat flux
        assert!(q_low < q_high);
    }

    #[test]
    fn test_fay_riddell_colder_wall_higher_flux() {
        let ext = HypersonicExtractor::new();
        let q_cold = ext.fay_riddell_stagnation_heat_flux(1.225, 3403.0, 0.05, 300.0);
        let q_hot = ext.fay_riddell_stagnation_heat_flux(1.225, 3403.0, 0.05, 2000.0);
        // Cold wall → higher heat flux
        assert!(q_cold > q_hot);
    }

    #[test]
    fn test_shock_stand_off_positive() {
        let ext = HypersonicExtractor::new();
        let d = ext.shock_stand_off(0.05, 0.01, 0.06);
        // d ≈ 0.19 * 0.05 * (0.01/0.06) ≈ 0.00158
        assert!((d - 0.001583).abs() < 1e-5);
    }

    #[test]
    fn test_extract_peak_heat_flux_no_file() {
        let ext = HypersonicExtractor::new();
        let dir = std::env::temp_dir().join("aeroflow_hypersonic_nonexistent");
        assert!(ext.extract_peak_heat_flux(dir.to_str().unwrap()).is_none());
    }

    #[test]
    fn test_extract_peak_heat_flux_from_fixture() {
        let dir = std::env::temp_dir().join("aeroflow_hypersonic_flux_test");
        let _ = std::fs::remove_dir_all(&dir);
        let post_dir = dir.join("postProcessing").join("heatFlux").join("0");
        std::fs::create_dir_all(&post_dir).unwrap();
        let data = "# Time  heatFlux\n0.001  500000.0\n0.002  1200000.0\n0.003  800000.0\n";
        std::fs::write(post_dir.join("wallHeatFlux.dat"), data).unwrap();

        let ext = HypersonicExtractor::new();
        let peak = ext.extract_peak_heat_flux(dir.to_str().unwrap());
        assert!((peak.unwrap() - 1_200_000.0).abs() < 1.0);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_extract_aerothermal_aggregate() {
        let dir = std::env::temp_dir().join("aeroflow_hypersonic_aggregate");
        let _ = std::fs::remove_dir_all(&dir);

        let ext = HypersonicExtractor::new();
        let metrics = ext.extract_aerothermal(dir.to_str().unwrap(), 0.01, 3403.0, 10.0, 300.0, Some(0.05), Some(1500.0));
        assert!(metrics.stagnation_temperature_k > 6000.0);
        assert!(metrics.stagnation_heat_flux_w_m2 > 0.0);
        assert!(metrics.shock_stand_off_m.unwrap() > 0.0);
    }
}
