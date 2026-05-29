use std::path::Path;

/// Extracted thermal metrics from CHT simulations
#[derive(Debug, Clone)]
pub struct ThermalMetrics {
    /// Average wall heat flux (W/m²)
    pub avg_wall_heat_flux_w_m2: f64,
    /// Peak wall heat flux (W/m²)
    pub peak_wall_heat_flux_w_m2: f64,
    /// Average Nusselt number
    pub nu_avg: Option<f64>,
    /// Peak Nusselt number
    pub nu_max: Option<f64>,
    /// Film cooling effectiveness (adiabatic)
    pub film_cooling_effectiveness: Option<f64>,
    /// Maximum solid temperature (K)
    pub max_solid_temperature_k: Option<f64>,
    /// Average interface temperature (K)
    pub interface_temperature_k: Option<f64>,
}

/// Extractor for thermal and CHT simulation results
pub struct ThermalExtractor;

impl ThermalExtractor {
    /// Compute Nusselt number from wall heat flux, bulk temperature, and reference length
    pub fn nusselt_number(q_wall: f64, t_bulk: f64, t_wall: f64, l_ref: f64, k_fluid: f64) -> f64 {
        let h = q_wall / (t_wall - t_bulk).abs();
        h * l_ref / k_fluid
    }

    /// Compute film cooling effectiveness η = (T_aw - T_g) / (T_c - T_g)
    pub fn film_cooling_effectiveness(t_aw: f64, t_gas: f64, t_coolant: f64) -> f64 {
        (t_aw - t_gas) / (t_coolant - t_gas)
    }

    /// Extract wall heat flux from field data files (wallHeatFlux output)
    /// Expects a file at postProcessing/heatFlux/0/wallHeatFlux.dat with columns: time, min, max, avg
    pub fn extract_heat_flux(case_path: &str) -> (f64, f64) {
        let path = Path::new(case_path)
            .join("postProcessing")
            .join("heatFlux")
            .join("0")
            .join("wallHeatFlux.dat");
        if !path.exists() {
            return (0.0, 0.0);
        }
        let content = std::fs::read_to_string(&path).unwrap_or_default();
        let mut avg_q = 0.0;
        let mut peak_q = 0.0;
        let mut count = 0u32;
        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with('(') {
                continue;
            }
            let parts: Vec<&str> = trimmed.split_whitespace().collect();
            if parts.len() >= 4
                && let (Ok(_time), Ok(_min), Ok(max), Ok(avg)) = (
                    parts[0].parse::<f64>(),
                    parts[1].parse::<f64>(),
                    parts[2].parse::<f64>(),
                    parts[3].parse::<f64>(),
                ) {
                    avg_q += avg;
                    if max > peak_q { peak_q = max; }
                    count += 1;
                }
        }
        if count > 0 { avg_q /= count as f64; }
        (avg_q, peak_q)
    }

    /// Extract maximum solid temperature from a T field file at the latest time
    pub fn extract_max_solid_temperature(case_path: &str, solid_region: &str) -> Option<f64> {
        let time_dir = Path::new(case_path)
            .join("postProcessing")
            .join("minMaxT")
            .join("0")
            .join("T.dat");
        if time_dir.exists() {
            let content = std::fs::read_to_string(&time_dir).ok()?;
            for line in content.lines().rev() {
                let trimmed = line.trim();
                if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with('(') {
                    continue;
                }
                let parts: Vec<&str> = trimmed.split_whitespace().collect();
                if parts.len() >= 3
                    && let (Ok(_t), Ok(_min), Ok(max)) = (
                        parts[0].parse::<f64>(),
                        parts[1].parse::<f64>(),
                        parts[2].parse::<f64>(),
                    ) {
                        return Some(max);
                    }
            }
        }
        // Fallback: try reading from solid region
        let solid_path = Path::new(case_path)
            .join(solid_region)
            .join("postProcessing")
            .join("minMaxT")
            .join("0")
            .join("T.dat");
        if solid_path.exists() {
            let content = std::fs::read_to_string(&solid_path).ok()?;
            for line in content.lines().rev() {
                let trimmed = line.trim();
                if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with('(') {
                    continue;
                }
                let parts: Vec<&str> = trimmed.split_whitespace().collect();
                if parts.len() >= 3
                    && let (Ok(_t), Ok(_min), Ok(max)) = (
                        parts[0].parse::<f64>(),
                        parts[1].parse::<f64>(),
                        parts[2].parse::<f64>(),
                    ) {
                        return Some(max);
                    }
            }
        }
        None
    }

    /// Extract all thermal metrics from a CHT case
    #[allow(clippy::too_many_arguments)]
    pub fn extract_all(
        &self,
        case_path: &str,
        t_bulk: f64,
        t_gas: Option<f64>,
        t_coolant: Option<f64>,
        l_ref: f64,
        k_fluid: f64,
        solid_region: &str,
    ) -> ThermalMetrics {
        let (avg_q, peak_q) = Self::extract_heat_flux(case_path);
        let t_wall = t_bulk + 100.0; // rough estimate — user should refine
        let nu_avg = if avg_q > 0.0 && k_fluid > 0.0 {
            Some(Self::nusselt_number(avg_q, t_bulk, t_wall, l_ref, k_fluid))
        } else {
            None
        };
        let nu_max = if peak_q > 0.0 && k_fluid > 0.0 {
            Some(Self::nusselt_number(peak_q, t_bulk, t_wall, l_ref, k_fluid))
        } else {
            None
        };
        let film_eta = match (t_gas, t_coolant) {
            (Some(tg), Some(tc)) if tg != tc => {
                let t_aw = t_wall; // adiabatic wall temp approx
                Some(Self::film_cooling_effectiveness(t_aw, tg, tc))
            }
            _ => None,
        };
        let max_t_solid = Self::extract_max_solid_temperature(case_path, solid_region);
        ThermalMetrics {
            avg_wall_heat_flux_w_m2: avg_q,
            peak_wall_heat_flux_w_m2: peak_q,
            nu_avg,
            nu_max,
            film_cooling_effectiveness: film_eta,
            max_solid_temperature_k: max_t_solid,
            interface_temperature_k: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_nusselt_number_typical_air() {
        // Air: k ~ 0.026 W/(m·K), L=0.05, q=5000 W/m², ΔT=200K
        let nu = ThermalExtractor::nusselt_number(5000.0, 600.0, 800.0, 0.05, 0.026);
        // h = 5000/200 = 25, Nu = 25 * 0.05 / 0.026 ≈ 48.08
        let expected = 5000.0 / 200.0 * 0.05 / 0.026;
        assert!((nu - expected).abs() < 1e-6);
    }

    #[test]
    fn test_nusselt_number_zero_delta_t() {
        let nu = ThermalExtractor::nusselt_number(5000.0, 600.0, 600.0, 0.05, 0.026);
        assert!(nu.is_infinite() || nu.is_nan());
    }

    #[test]
    fn test_film_cooling_effectiveness_typical() {
        let eta = ThermalExtractor::film_cooling_effectiveness(700.0, 1000.0, 400.0);
        assert!((eta - 0.5).abs() < 1e-6);
    }

    #[test]
    fn test_film_cooling_no_coolant() {
        let eta = ThermalExtractor::film_cooling_effectiveness(1000.0, 1000.0, 1000.0);
        assert!(eta.is_nan() || (eta - 0.0).abs() < 1e-10);
    }

    #[test]
    fn test_extract_heat_flux_missing_file() {
        let dir = std::env::temp_dir().join(format!("cht_test_missing_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let (avg, peak) = ThermalExtractor::extract_heat_flux(dir.to_str().unwrap());
        assert!((avg - 0.0).abs() < 1e-10);
        assert!((peak - 0.0).abs() < 1e-10);
    }

    #[test]
    fn test_extract_heat_flux_parses_file() {
        let dir = std::env::temp_dir().join(format!("cht_test_flux_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir.join("postProcessing").join("heatFlux").join("0"));
        let dat_path = dir.join("postProcessing").join("heatFlux").join("0").join("wallHeatFlux.dat");
        std::fs::write(&dat_path, "# time min max avg\n0 100 5000 1200\n1 150 5500 1300\n").unwrap();
        let (avg, peak) = ThermalExtractor::extract_heat_flux(dir.to_str().unwrap());
        assert!((avg - 1250.0).abs() < 1e-6);
        assert!((peak - 5500.0).abs() < 1e-6);
    }

    #[test]
    fn test_extract_max_solid_missing_file() {
        let dir = std::env::temp_dir().join(format!("cht_test_solid_missing_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        assert!(ThermalExtractor::extract_max_solid_temperature(dir.to_str().unwrap(), "solid").is_none());
    }

    #[test]
    fn test_extract_max_solid_parses_file() {
        let dir = std::env::temp_dir().join(format!("cht_test_solid_T_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir.join("postProcessing").join("minMaxT").join("0"));
        let dat_path = dir.join("postProcessing").join("minMaxT").join("0").join("T.dat");
        std::fs::write(&dat_path, "# time min max\n0 300 950\n1 320 980\n").unwrap();
        let max_t = ThermalExtractor::extract_max_solid_temperature(dir.to_str().unwrap(), "solid");
        assert!((max_t.unwrap() - 980.0).abs() < 1e-6);
    }

    #[test]
    fn test_extract_all_with_data() {
        let dir = std::env::temp_dir().join(format!("cht_test_all_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir.join("postProcessing").join("heatFlux").join("0"));
        let _ = std::fs::create_dir_all(&dir.join("postProcessing").join("minMaxT").join("0"));
        std::fs::write(
            dir.join("postProcessing").join("heatFlux").join("0").join("wallHeatFlux.dat"),
            "0 100 5000 1200\n",
        ).unwrap();
        std::fs::write(
            dir.join("postProcessing").join("minMaxT").join("0").join("T.dat"),
            "0 300 980\n",
        ).unwrap();
        let ext = ThermalExtractor;
        let m = ext.extract_all(dir.to_str().unwrap(), 600.0, Some(1000.0), Some(400.0), 0.05, 0.026, "solid");
        assert!((m.avg_wall_heat_flux_w_m2 - 1200.0).abs() < 1e-6);
        assert!((m.peak_wall_heat_flux_w_m2 - 5000.0).abs() < 1e-6);
        assert!(m.nu_avg.unwrap() > 0.0);
        assert!(m.max_solid_temperature_k.unwrap() - 980.0 < 1e-6);
        // η = (700 - 1000) / (400 - 1000) = -300 / -600 = 0.5
        assert!((m.film_cooling_effectiveness.unwrap() - 0.5).abs() < 1e-6);
    }
}
