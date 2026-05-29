use std::path::Path;

/// Extracted MHD / electromagnetic metrics
#[derive(Debug, Clone)]
pub struct MhdMetrics {
    /// Hartmann number
    pub hartmann_number: f64,
    /// Magnetic Reynolds number
    pub magnetic_reynolds_number: f64,
    /// Pressure drop ratio vs non-MHD (ΔP / ΔP₀)
    pub pressure_drop_ratio: Option<f64>,
    /// Centreline velocity ratio U_max / U_bulk (1.0 = flat, 2.0 = laminar)
    pub velocity_profile_ratio: Option<f64>,
    /// Induced wall-jet velocity by DBD actuator (m/s)
    pub induced_velocity_m_s: Option<f64>,
}

/// Extractor for MHD and electromagnetic simulation results
pub struct MhdExtractor;

impl MhdExtractor {
    /// Compute Hartmann number from physical parameters
    pub fn hartmann_number(b0_tesla: f64, l_ref_m: f64, sigma_s_m: f64, mu_visc_pa_s: f64) -> f64 {
        b0_tesla * l_ref_m * (sigma_s_m / mu_visc_pa_s).sqrt()
    }

    /// Compute magnetic Reynolds number
    pub fn magnetic_reynolds_number(u_m_s: f64, l_ref_m: f64, sigma_s_m: f64) -> f64 {
        const MU0: f64 = 1.25663706212e-6;
        MU0 * sigma_s_m * u_m_s * l_ref_m
    }

    /// Hartmann layer thickness: δ_Ha = L / Ha
    pub fn hartmann_layer_thickness(l_ref_m: f64, ha: f64) -> f64 {
        if ha <= 0.0 { f64::INFINITY } else { l_ref_m / ha }
    }

    /// Extract pressure drop ratio from field data files
    /// Expects file at postProcessing/pressureDrop/0/p.dat with columns: time, p_in, p_out
    pub fn extract_pressure_drop_ratio(case_path: &str, p0_drop: f64) -> Option<f64> {
        let path = Path::new(case_path)
            .join("postProcessing")
            .join("pressureDrop")
            .join("0")
            .join("p.dat");
        if !path.exists() { return None; }
        let content = std::fs::read_to_string(&path).ok()?;
        for line in content.lines().rev() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with('(') {
                continue;
            }
            let parts: Vec<&str> = trimmed.split_whitespace().collect();
            if parts.len() >= 3
                && let (Ok(_t), Ok(p_in), Ok(p_out)) = (
                    parts[0].parse::<f64>(),
                    parts[1].parse::<f64>(),
                    parts[2].parse::<f64>(),
                ) {
                    let drop = p_in - p_out;
                    if p0_drop > 0.0 { return Some(drop / p0_drop); }
                }
        }
        None
    }

    /// Extract centreline velocity ratio U_max / U_bulk from profile data
    /// Expects file at postProcessing/velocityProfile/0/U_profile.dat
    pub fn extract_velocity_profile_ratio(case_path: &str) -> Option<f64> {
        let path = Path::new(case_path)
            .join("postProcessing")
            .join("velocityProfile")
            .join("0")
            .join("U_profile.dat");
        if !path.exists() { return None; }
        let content = std::fs::read_to_string(&path).ok()?;
        let mut u_center = 0.0_f64;
        let mut u_sum = 0.0_f64;
        let mut count = 0u32;
        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with('(') {
                continue;
            }
            // Format: time, y, Ux, Uy, Uz
            let parts: Vec<&str> = trimmed.split_whitespace().collect();
            if parts.len() >= 4
                && let (Ok(_t), Ok(y), Ok(ux), ..) = (
                    parts[0].parse::<f64>(),
                    parts[1].parse::<f64>(),
                    parts[2].parse::<f64>(),
                ) {
                    u_sum += ux;
                    count += 1;
                    // Centreline assumption: y=0 or minimum y deviates from wall
                    if y.abs() < 0.01 { u_center = ux; }
                }
        }
        if count > 0 {
            let u_bulk = u_sum / count as f64;
            if u_bulk > 0.0 { Some(u_center / u_bulk) } else { None }
        } else { None }
    }

    /// Extract DBD induced wall-jet peak velocity from profile data
    /// Expects file at postProcessing/inducedVelocity/0/U_induced.dat with columns: time, y, Ux
    pub fn extract_induced_velocity(case_path: &str) -> Option<f64> {
        let path = Path::new(case_path)
            .join("postProcessing")
            .join("inducedVelocity")
            .join("0")
            .join("U_induced.dat");
        if !path.exists() { return None; }
        let content = std::fs::read_to_string(&path).ok()?;
        let mut max_u = 0.0_f64;
        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with('(') {
                continue;
            }
            let parts: Vec<&str> = trimmed.split_whitespace().collect();
            if parts.len() >= 3
                && let (Ok(_t), Ok(_y), Ok(ux)) = (
                    parts[0].parse::<f64>(),
                    parts[1].parse::<f64>(),
                    parts[2].parse::<f64>(),
                )
                    && ux > max_u { max_u = ux; }
        }
        if max_u > 0.0 { Some(max_u) } else { None }
    }

    /// Extract all MHD metrics from a case directory
    #[allow(clippy::too_many_arguments)]
    pub fn extract_all(
        &self,
        b0_tesla: f64,
        l_ref_m: f64,
        sigma_s_m: f64,
        mu_visc_pa_s: f64,
        u_ref_m_s: f64,
        p0_drop: f64,
        case_path: &str,
    ) -> MhdMetrics {
        let ha = Self::hartmann_number(b0_tesla, l_ref_m, sigma_s_m, mu_visc_pa_s);
        let rm = Self::magnetic_reynolds_number(u_ref_m_s, l_ref_m, sigma_s_m);
        let pressure_ratio = Self::extract_pressure_drop_ratio(case_path, p0_drop);
        let velocity_ratio = Self::extract_velocity_profile_ratio(case_path);
        let induced = Self::extract_induced_velocity(case_path);
        MhdMetrics {
            hartmann_number: ha,
            magnetic_reynolds_number: rm,
            pressure_drop_ratio: pressure_ratio,
            velocity_profile_ratio: velocity_ratio,
            induced_velocity_m_s: induced,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hartmann_number_liquid_sodium() {
        // Na at 400K: sigma=1e7, mu=2.34e-4, B=0.1T, L=0.01m
        let ha = MhdExtractor::hartmann_number(0.1, 0.01, 1.0e7, 0.000234);
        // Ha = 0.1 * 0.01 * sqrt(1e7/0.000234) ≈ 0.001 * 206758 ≈ 207
        assert!(ha > 100.0 && ha < 500.0);
    }

    #[test]
    fn test_hartmann_number_zero_field() {
        assert!((MhdExtractor::hartmann_number(0.0, 0.01, 1.0e7, 0.000234) - 0.0).abs() < 1e-10);
    }

    #[test]
    fn test_magnetic_reynolds_number() {
        let rm = MhdExtractor::magnetic_reynolds_number(1.0, 0.01, 1.0e7);
        assert!(rm > 0.0 && rm < 1.0);
    }

    #[test]
    fn test_hartmann_layer_thickness() {
        let delta = MhdExtractor::hartmann_layer_thickness(0.01, 100.0);
        assert!((delta - 0.0001).abs() < 1e-10);
    }

    #[test]
    fn test_hartmann_layer_thickness_zero_ha() {
        assert!(MhdExtractor::hartmann_layer_thickness(0.01, 0.0).is_infinite());
    }

    #[test]
    fn test_extract_pressure_drop_missing_file() {
        let dir = std::env::temp_dir().join(format!("mhd_test_pdrop_{}", std::process::id()));
        assert!(MhdExtractor::extract_pressure_drop_ratio(dir.to_str().unwrap(), 100.0).is_none());
    }

    #[test]
    fn test_extract_pressure_drop_parses_file() {
        let dir = std::env::temp_dir().join(format!("mhd_test_pdrop2_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir.join("postProcessing").join("pressureDrop").join("0"));
        std::fs::write(
            dir.join("postProcessing").join("pressureDrop").join("0").join("p.dat"),
            "# time p_in p_out\n0 150 50\n1 160 55\n",
        ).unwrap();
        let ratio = MhdExtractor::extract_pressure_drop_ratio(dir.to_str().unwrap(), 105.0);
        // Last row: drop = 160 - 55 = 105, ratio = 105 / 105 = 1.0
        assert!(ratio.is_some());
        assert!((ratio.unwrap() - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_extract_velocity_profile_missing_file() {
        let dir = std::env::temp_dir().join(format!("mhd_test_vel_{}", std::process::id()));
        assert!(MhdExtractor::extract_velocity_profile_ratio(dir.to_str().unwrap()).is_none());
    }

    #[test]
    fn test_extract_velocity_profile_parses_file() {
        let dir = std::env::temp_dir().join(format!("mhd_test_vel2_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir.join("postProcessing").join("velocityProfile").join("0"));
        std::fs::write(
            dir.join("postProcessing").join("velocityProfile").join("0").join("U_profile.dat"),
            "# time y Ux Uy Uz\n0 -0.01 0.5 0 0\n0 0.00 1.8 0 0\n0 0.01 0.5 0 0\n",
        ).unwrap();
        let ratio = MhdExtractor::extract_velocity_profile_ratio(dir.to_str().unwrap());
        // U_bulk = (0.5 + 1.8 + 0.5) / 3 = 0.933, U_center = 1.8
        // ratio = 1.8 / 0.933 ≈ 1.928
        assert!(ratio.unwrap() > 1.9);
    }

    #[test]
    fn test_extract_induced_velocity_missing_file() {
        let dir = std::env::temp_dir().join(format!("mhd_test_ind_{}", std::process::id()));
        assert!(MhdExtractor::extract_induced_velocity(dir.to_str().unwrap()).is_none());
    }

    #[test]
    fn test_extract_induced_velocity_parses_file() {
        let dir = std::env::temp_dir().join(format!("mhd_test_ind2_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir.join("postProcessing").join("inducedVelocity").join("0"));
        std::fs::write(
            dir.join("postProcessing").join("inducedVelocity").join("0").join("U_induced.dat"),
            "# time y Ux\n0 0.001 2.5\n0 0.002 5.0\n0 0.003 4.0\n",
        ).unwrap();
        let u = MhdExtractor::extract_induced_velocity(dir.to_str().unwrap());
        assert!((u.unwrap() - 5.0).abs() < 1e-6);
    }
}
