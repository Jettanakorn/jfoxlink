use crate::types::{GeoBounds, WindTunnelConfig, WindTunnelResult};

/// Sizes a digital-wind-tunnel domain around a given geometry bounding box,
/// using chord-based spacing with the model centred at the origin.
pub struct WindTunnelDomainSizer;

impl WindTunnelDomainSizer {
    /// Detect reference length from a bounding box.
    /// For external-aero geometries this is the chord (x-span).
    pub fn chord_from_bounds(bounds: &GeoBounds) -> f64 {
        let dx = bounds.max_x - bounds.min_x;
        let dy = bounds.max_y - bounds.min_y;
        let dz = bounds.max_z - bounds.min_z;
        dx.max(dy).max(dz).max(0.01)
    }

    /// Resolve a config reference to a value (clone-once or default).
    fn resolve_config(config: Option<&WindTunnelConfig>) -> WindTunnelConfig {
        config.cloned().unwrap_or_default()
    }

    /// Compute the domain extents as `(x_min, x_max, y_min, y_max, z_min, z_max)`.
    /// The model bounding box centre is placed at the origin.
    /// `chord` is the reference length (auto-detected if `None`).
    /// `config` provides the per-direction multipliers; uses defaults if `None`.
    pub fn domain_bounds(
        bounds: &GeoBounds,
        chord: f64,
        config: Option<&WindTunnelConfig>,
    ) -> (f64, f64, f64, f64, f64, f64) {
        let cfg = Self::resolve_config(config);
        let cx = (bounds.min_x + bounds.max_x) / 2.0;
        let cy = (bounds.min_y + bounds.max_y) / 2.0;
        let cz = (bounds.min_z + bounds.max_z) / 2.0;

        let up = cfg.upstream * chord;
        let dn = cfg.downstream * chord;
        let vt = cfg.vertical * chord;
        let lt = cfg.lateral * chord;

        (
            cx - up, cx + dn,
            cy - vt, cy + vt,
            cz - lt, cz + lt,
        )
    }

    /// Compute the blockage ratio (model frontal area / tunnel cross‑section)
    /// as a fraction [0, 1].
    pub fn blockage_ratio(bounds: &GeoBounds, chord: f64, config: Option<&WindTunnelConfig>) -> f64 {
        let cfg = Self::resolve_config(config);
        let tunnel_w = 2.0 * cfg.lateral * chord;
        let tunnel_h = 2.0 * cfg.vertical * chord;
        let tunnel_area = tunnel_w * tunnel_h;
        if tunnel_area <= 0.0 {
            return 0.0;
        }
        // Frontal area: use yz-plane projection of the model bbox
        let model_w = bounds.max_z - bounds.min_z;
        let model_h = bounds.max_y - bounds.min_y;
        let model_area = (model_w * model_h).max(0.0);
        model_area / tunnel_area
    }

    /// Apply blockage correction using the classic mask‑area method:
    /// `u_corrected = u_inf / (1 - blockage)`.
    pub fn corrected_velocity(u_inf: f64, blockage: f64) -> f64 {
        let corr = 1.0 - blockage;
        if corr <= 0.0 {
            u_inf
        } else {
            u_inf / corr
        }
    }

    /// Correct a force coefficient for blockage: `C_corrected = C_uncorrected * (1 - blockage)`.
    pub fn corrected_coefficient(c_uncorrected: f64, blockage: f64) -> f64 {
        c_uncorrected * (1.0 - blockage)
    }

    /// Build a full `WindTunnelResult` from the inputs.
    pub fn compute_result(
        bounds: &GeoBounds,
        chord: f64,
        config: Option<&WindTunnelConfig>,
        cl_uncorrected: f64,
        cd_uncorrected: f64,
    ) -> WindTunnelResult {
        let cfg = Self::resolve_config(config);
        let blockage = Self::blockage_ratio(bounds, chord, config);
        let u_corrected = Self::corrected_velocity(
            cfg.velocity_m_s.unwrap_or(10.0),
            blockage,
        );
        let (x_min, x_max, y_min, y_max, z_min, z_max) =
            Self::domain_bounds(bounds, chord, config);

        let tunnel_w = z_max - z_min;
        let tunnel_h = y_max - y_min;
        let tunnel_area = tunnel_w * tunnel_h;
        let model_w = bounds.max_z - bounds.min_z;
        let model_h = bounds.max_y - bounds.min_y;
        let model_area = (model_w * model_h).max(0.0);

        WindTunnelResult {
            blockage_pct: blockage * 100.0,
            u_corrected_m_s: u_corrected,
            cd_uncorrected,
            cd_corrected: Self::corrected_coefficient(cd_uncorrected, blockage),
            cl_uncorrected,
            cl_corrected: Self::corrected_coefficient(cl_uncorrected, blockage),
            tunnel_area_m2: tunnel_area,
            model_area_m2: model_area,
            upstream_m: x_min.abs(),
            downstream_m: x_max,
            vertical_m: y_max,
            lateral_m: z_max,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_bounds() -> GeoBounds {
        GeoBounds {
            min_x: -0.05, max_x: 0.05,
            min_y: -0.02, max_y: 0.02,
            min_z: -0.01, max_z: 0.01,
        }
    }

    #[test]
    fn test_chord_detection() {
        let b = sample_bounds();
        let chord = WindTunnelDomainSizer::chord_from_bounds(&b);
        assert!((chord - 0.10).abs() < 1e-6);
    }

    #[test]
    fn test_chord_minimum_clamp() {
        let b = GeoBounds {
            min_x: 0.0, max_x: 0.001,
            min_y: 0.0, max_y: 0.001,
            min_z: 0.0, max_z: 0.001,
        };
        let chord = WindTunnelDomainSizer::chord_from_bounds(&b);
        assert!((chord - 0.01).abs() < 1e-6);
    }

    #[test]
    fn test_domain_bounds_default() {
        let b = sample_bounds();
        let chord = 0.1;
        let (x_min, x_max, y_min, y_max, z_min, z_max) =
            WindTunnelDomainSizer::domain_bounds(&b, chord, None);
        // upstream=20c, downstream=40c, vertical=25c, lateral=25c
        assert!((x_min - (-2.0)).abs() < 1e-4);
        assert!((x_max - 4.0).abs() < 1e-4);
        assert!((y_min - (-2.5)).abs() < 1e-4);
        assert!((y_max - 2.5).abs() < 1e-4);
        assert!((z_min - (-2.5)).abs() < 1e-4);
        assert!((z_max - 2.5).abs() < 1e-4);
    }

    #[test]
    fn test_domain_bounds_custom_config() {
        let cfg = WindTunnelConfig {
            upstream: 10.0, downstream: 20.0,
            vertical: 15.0, lateral: 15.0,
            ..WindTunnelConfig::default()
        };
        let b = sample_bounds();
        let chord = 0.1;
        let (x_min, x_max, ..) =
            WindTunnelDomainSizer::domain_bounds(&b, chord, Some(&cfg));
        assert!((x_min - (-1.0)).abs() < 1e-4);
        assert!((x_max - 2.0).abs() < 1e-4);
    }

    #[test]
    fn test_blockage_ratio() {
        let b = sample_bounds();
        let chord = 0.1;
        // tunnel area = (2*25c)*(2*25c) = (5)*(5) = 25
        // model area = (z-span=0.02)*(y-span=0.04) = 0.0008
        // blockage = 0.0008 / 25.0 = 3.2e-5
        let blockage = WindTunnelDomainSizer::blockage_ratio(&b, chord, None);
        assert!((blockage - 3.2e-5).abs() < 1e-6);
    }

    #[test]
    fn test_corrected_velocity() {
        // u_inf=10, blockage=0.05 -> u_corr=10/0.95=10.526
        let u = WindTunnelDomainSizer::corrected_velocity(10.0, 0.05);
        assert!((u - 10.526315789).abs() < 1e-6);
    }

    #[test]
    fn test_corrected_velocity_no_blockage() {
        let u = WindTunnelDomainSizer::corrected_velocity(10.0, 0.0);
        assert!((u - 10.0).abs() < 1e-6);
    }

    #[test]
    fn test_corrected_coefficient() {
        let c = WindTunnelDomainSizer::corrected_coefficient(1.0, 0.05);
        assert!((c - 0.95).abs() < 1e-6);
    }

    #[test]
    fn test_compute_result() {
        let b = sample_bounds();
        let chord = 0.1;
        let result = WindTunnelDomainSizer::compute_result(&b, chord, None, 0.5, 0.02);
        assert!((result.blockage_pct - 3.2e-3).abs() < 1e-4);
        assert!(result.cd_uncorrected - 0.02 < 1e-6);
        assert!(result.upstream_m - 2.0 < 1e-4);
        assert!(result.downstream_m - 4.0 < 1e-4);
    }
}
