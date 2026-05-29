use crate::reader::PostReader;
use std::error::Error;

pub struct PorousExtractor;

impl PorousExtractor {
    /// Extract pressure drop across porous zone from pressure field
    pub fn pressure_drop(
        reader: &PostReader,
        inlet_label: &str,
        outlet_label: &str,
    ) -> Result<f64, Box<dyn Error>> {
        let p_in = reader.read_scalar_boundary(inlet_label, "p")?;
        let p_out = reader.read_scalar_boundary(outlet_label, "p")?;
        let drop: Vec<f64> = p_in
            .iter()
            .zip(p_out.iter())
            .map(|(pi, po)| pi - po)
            .collect();
        Ok(if drop.is_empty() { 0.0 } else { drop.iter().sum::<f64>() / drop.len() as f64 })
    }

    /// Darcy number Da = K / L^2 based on pressure drop
    pub fn darcy_number(
        dp: f64,
        l: f64,
        mu: f64,
        u: f64,
    ) -> f64 {
        if u.abs() < 1e-15 || l <= 0.0 { return 0.0; }
        let k = mu * u * l / dp;
        k / (l * l)
    }
}

pub struct ParticleExtractor;

impl ParticleExtractor {
    /// Extract particle erosion rate at a boundary wall
    pub fn erosion_rate(reader: &PostReader, wall_label: &str) -> Result<f64, Box<dyn Error>> {
        let eros = reader.read_scalar_boundary(wall_label, "erosionRate")?;
        Ok(if eros.is_empty() { 0.0 } else { eros.iter().sum::<f64>() / eros.len() as f64 })
    }

    /// Extract particle deposition rate at a boundary wall
    pub fn deposition_rate(reader: &PostReader, wall_label: &str) -> Result<f64, Box<dyn Error>> {
        let dep = reader.read_scalar_boundary(wall_label, "depositionRate")?;
        Ok(if dep.is_empty() { 0.0 } else { dep.iter().sum::<f64>() / dep.len() as f64 })
    }
}

pub struct MultiphaseExtractor;

impl MultiphaseExtractor {
    /// Extract phase volume fraction on a patch
    pub fn phase_fraction(
        reader: &PostReader,
        phase_field: &str,
        patch_label: &str,
    ) -> Result<f64, Box<dyn Error>> {
        let alpha = reader.read_scalar_boundary(patch_label, phase_field)?;
        Ok(if alpha.is_empty() { 0.0 } else { alpha.iter().sum::<f64>() / alpha.len() as f64 })
    }

    /// Extract Sauter mean diameter d32 from interfacial area
    pub fn sauter_mean_diameter(
        reader: &PostReader,
        patch_label: &str,
    ) -> Result<f64, Box<dyn Error>> {
        let alpha = Self::phase_fraction(reader, "alpha.liquid", patch_label)?;
        let a_i = Self::phase_fraction(reader, "interfacialArea", patch_label)?;
        if a_i.abs() < 1e-15 { return Ok(0.0); }
        Ok(6.0 * alpha / a_i)
    }
}

pub struct NonNewtonianExtractor;

impl NonNewtonianExtractor {
    /// Extract apparent viscosity on a patch
    pub fn apparent_viscosity(
        reader: &PostReader,
        patch_label: &str,
    ) -> Result<f64, Box<dyn Error>> {
        let nu = reader.read_scalar_boundary(patch_label, "nu")?;
        Ok(if nu.is_empty() { 0.0 } else { nu.iter().sum::<f64>() / nu.len() as f64 })
    }

    /// Local shear rate from velocity gradient
    pub fn shear_rate(
        reader: &PostReader,
        patch_label: &str,
    ) -> Result<f64, Box<dyn Error>> {
        let u = reader.read_vector_boundary(patch_label, "U")?;
        if u.len() < 2 { return Ok(0.0); }
        let mag: Vec<f64> = u.chunks(3).map(|c| (c[0] * c[0] + c[1] * c[1] + c[2] * c[2]).sqrt()).collect();
        Ok(mag.iter().sum::<f64>() / mag.len() as f64)
    }
}

pub struct ViscoelasticExtractor;

impl ViscoelasticExtractor {
    /// Extract first normal stress difference N1 = tauxx - tauyy
    pub fn normal_stress_diff(
        reader: &PostReader,
        patch_label: &str,
    ) -> Result<f64, Box<dyn Error>> {
        let tau = reader.read_tensor_boundary(patch_label, "tau")?;
        if tau.len() < 9 { return Ok(0.0); }
        let n1: Vec<f64> = tau.chunks(9).map(|c| c[0] - c[4]).collect();
        Ok(n1.iter().sum::<f64>() / n1.len() as f64)
    }

    /// Wi = lambda * U / L for a given relaxation time, velocity, length
    pub fn weissenberg_number(lambda: f64, u: f64, l: f64) -> f64 {
        if l <= 0.0 { 0.0 } else { lambda * u / l }
    }
}

pub struct FsiExtractor;

impl FsiExtractor {
    /// Extract von Mises stress from stress tensor on a patch
    pub fn von_mises_stress(reader: &PostReader, patch_label: &str) -> Result<f64, Box<dyn Error>> {
        let sigma = reader.read_tensor_boundary(patch_label, "sigma")?;
        if sigma.len() < 9 { return Ok(0.0); }
        let vm: Vec<f64> = sigma.chunks(9).map(|c| {
            let s11 = c[0]; let s22 = c[4]; let s33 = c[8];
            let s12 = c[1]; let s23 = c[5]; let s31 = c[2];
            let diff1 = s11 - s22;
            let diff2 = s22 - s33;
            let diff3 = s33 - s11;
            (0.5 * (diff1 * diff1 + diff2 * diff2 + diff3 * diff3
                + 6.0 * (s12 * s12 + s23 * s23 + s31 * s31))).sqrt() 
        }).collect();
        Ok(if vm.is_empty() { 0.0 } else { vm.iter().sum::<f64>() / vm.len() as f64 })
    }

    /// Extract displacement magnitude on a patch
    pub fn displacement(reader: &PostReader, patch_label: &str) -> Result<f64, Box<dyn Error>> {
        let d = reader.read_vector_boundary(patch_label, "D")?;
        if d.len() < 3 { return Ok(0.0); }
        let mag: Vec<f64> = d.chunks(3).map(|c| (c[0] * c[0] + c[1] * c[1] + c[2] * c[2]).sqrt()).collect();
        Ok(mag.iter().sum::<f64>() / mag.len() as f64)
    }

    /// Maximum principal stress (largest eigenvalue of stress tensor)
    pub fn max_principal_stress(reader: &PostReader, patch_label: &str) -> Result<f64, Box<dyn Error>> {
        let sigma = reader.read_tensor_boundary(patch_label, "sigma")?;
        if sigma.len() < 9 { return Ok(0.0); }
        let max_p: Vec<f64> = sigma.chunks(9).map(|c| {
            let s11 = c[0]; let s22 = c[4]; let s33 = c[8];
            let s12 = c[1]; let s13 = c[2]; let s23 = c[5];
            let i1 = s11 + s22 + s33;
            let i2 = s11 * s22 + s22 * s33 + s33 * s11 - s12 * s12 - s23 * s23 - s13 * s13;
            let i3 = s11 * s22 * s33 + 2.0 * s12 * s23 * s13 - s11 * s23 * s23 - s22 * s13 * s13 - s33 * s12 * s12;
            let q = (i1 * i1 - 3.0 * i2) / 9.0;
            let r = (2.0 * i1 * i1 * i1 - 9.0 * i1 * i2 + 27.0 * i3) / 54.0;
            let theta = (r / (q * q * q).sqrt()).acos();
            i1 / 3.0 + 2.0 * q.sqrt() * (theta / 3.0).cos()
        }).collect();
        Ok(max_p.iter().sum::<f64>() / max_p.len() as f64)
    }
}

pub struct CombustionExtractor;

impl CombustionExtractor {
    /// Extract flame temperature on a patch
    pub fn flame_temperature(reader: &PostReader, patch_label: &str) -> Result<f64, Box<dyn Error>> {
        let t = reader.read_scalar_boundary(patch_label, "T")?;
        Ok(if t.is_empty() { 0.0 } else { t.iter().sum::<f64>() / t.len() as f64 })
    }

    /// Extract heat release rate (HRR) on a patch
    pub fn heat_release_rate(reader: &PostReader, patch_label: &str) -> Result<f64, Box<dyn Error>> {
        let hrr = reader.read_scalar_boundary(patch_label, "Qdot")?;
        Ok(if hrr.is_empty() { 0.0 } else { hrr.iter().sum::<f64>() / hrr.len() as f64 })
    }

    /// Extract mixture fraction on a patch
    pub fn mixture_fraction(reader: &PostReader, patch_label: &str) -> Result<f64, Box<dyn Error>> {
        let mf = reader.read_scalar_boundary(patch_label, "f")?;
        Ok(if mf.is_empty() { 0.0 } else { mf.iter().sum::<f64>() / mf.len() as f64 })
    }

    /// Damkohler number approximation: Da = tau_flow / tau_chem
    pub fn damkohler_number(u: f64, l: f64, tau_chem: f64) -> f64 {
        if tau_chem <= 0.0 || u <= 0.0 { return 0.0; }
        let tau_flow = l / u;
        tau_flow / tau_chem
    }
}

pub struct CavitationExtractor;

impl CavitationExtractor {
    /// Extract cavitation number sigma = (p - p_v) / (0.5 * rho * U^2)
    pub fn cavitation_number(p_ref: f64, p_vap: f64, rho: f64, u_ref: f64) -> f64 {
        if rho <= 0.0 || u_ref <= 0.0 { return 0.0; }
        (p_ref - p_vap) / (0.5 * rho * u_ref * u_ref)
    }

    /// Extract vapor volume fraction on a patch
    pub fn vapor_fraction(reader: &PostReader, patch_label: &str) -> Result<f64, Box<dyn Error>> {
        let av = reader.read_scalar_boundary(patch_label, "alpha.vapor")?;
        Ok(if av.is_empty() { 0.0 } else { av.iter().sum::<f64>() / av.len() as f64 })
    }
}

pub struct SprayExtractor;

impl SprayExtractor {
    /// Extract Sauter mean diameter from spray
    pub fn sauter_mean_diameter(reader: &PostReader, patch_label: &str) -> Result<f64, Box<dyn Error>> {
        let d32 = reader.read_scalar_boundary(patch_label, "d")?;
        Ok(if d32.is_empty() { 0.0 } else { d32.iter().sum::<f64>() / d32.len() as f64 })
    }
}

pub struct AeroacousticExtractor;

impl AeroacousticExtractor {
    /// Compute sound pressure level SPL = 20 * log10(p_rms / p_ref)
    pub fn sound_pressure_level(p_rms_pa: f64) -> f64 {
        let p_ref = 2e-5;
        if p_rms_pa <= 0.0 { return 0.0; }
        20.0 * (p_rms_pa / p_ref).log10()
    }

    /// Compute overall SPL from multiple frequencies
    pub fn overall_spl(spectrum: &[(f64, f64)]) -> f64 {
        let total: f64 = spectrum.iter().map(|(_, spl)| (spl / 10.0).exp()).sum();
        if total <= 0.0 { return 0.0; }
        10.0 * total.ln()
    }

    /// Strouhal number St = f * L / U
    pub fn strouhal_number(freq_hz: f64, l_m: f64, u_m_s: f64) -> f64 {
        if u_m_s <= 0.0 { 0.0 } else { freq_hz * l_m / u_m_s }
    }
}

pub struct WaveExtractor;

impl WaveExtractor {
    /// Extract wave height from free surface elevation
    pub fn wave_height(reader: &PostReader, patch_label: &str) -> Result<f64, Box<dyn Error>> {
        let eta = reader.read_scalar_boundary(patch_label, "alpha.water")?;
        Ok(if eta.is_empty() { 0.0 } else { eta.iter().sum::<f64>() / eta.len() as f64 })
    }

    /// Ursell number Ur = H * L^2 / d^3 for nonlinearity
    pub fn ursell_number(h_m: f64, l_m: f64, d_m: f64) -> f64 {
        if d_m <= 0.0 { 0.0 } else { h_m * l_m * l_m / (d_m * d_m * d_m) }
    }
}

pub struct PhaseChangeExtractor;

impl PhaseChangeExtractor {
    /// Extract liquid fraction on a patch
    pub fn liquid_fraction(reader: &PostReader, patch_label: &str) -> Result<f64, Box<dyn Error>> {
        let alpha_l = reader.read_scalar_boundary(patch_label, "alpha.liquid")?;
        Ok(if alpha_l.is_empty() { 0.0 } else { alpha_l.iter().sum::<f64>() / alpha_l.len() as f64 })
    }

    /// Stefan number Ste = Cp * (T - T_m) / L for phase change
    pub fn stefan_number(cp: f64, t_surface: f64, t_melt: f64, latent: f64) -> f64 {
        if latent <= 0.0 { return 0.0; }
        cp * (t_surface - t_melt) / latent
    }
}

pub struct WindExtractor;

impl WindExtractor {
    /// Power coefficient Cp (actual / ideal)
    pub fn actual_power_coefficient(power_w: f64, rho: f64, area_m2: f64, u_inf: f64) -> f64 {
        if rho <= 0.0 || area_m2 <= 0.0 || u_inf <= 0.0 { return 0.0; }
        power_w / (0.5 * rho * area_m2 * u_inf * u_inf * u_inf)
    }

    /// Thrust coefficient
    pub fn actual_thrust_coefficient(thrust_n: f64, rho: f64, area_m2: f64, u_inf: f64) -> f64 {
        if rho <= 0.0 || area_m2 <= 0.0 || u_inf <= 0.0 { return 0.0; }
        thrust_n / (0.5 * rho * area_m2 * u_inf * u_inf)
    }
}

pub struct ElectrostaticExtractor;

impl ElectrostaticExtractor {
    /// Electric field magnitude from potential gradient (estimate)
    pub fn electric_field_magnitude(potential_v: f64, gap_m: f64) -> f64 {
        if gap_m <= 0.0 { 0.0 } else { potential_v / gap_m }
    }
}

pub struct AblationExtractor;

impl AblationExtractor {
    /// Extract surface recession rate (m/s)
    pub fn recession_rate(reader: &PostReader, patch_label: &str) -> Result<f64, Box<dyn Error>> {
        let rr = reader.read_scalar_boundary(patch_label, "recessionRate")?;
        Ok(if rr.is_empty() { 0.0 } else { rr.iter().sum::<f64>() / rr.len() as f64 })
    }

    /// Extract char depth (m)
    pub fn char_depth(reader: &PostReader, patch_label: &str) -> Result<f64, Box<dyn Error>> {
        let cd = reader.read_scalar_boundary(patch_label, "charDepth")?;
        Ok(if cd.is_empty() { 0.0 } else { cd.iter().sum::<f64>() / cd.len() as f64 })
    }

    /// Non-dimensional heat of gasification B' = m_dot / (rho * U * St)
    pub fn blowing_parameter(mdot_kg_m2s: f64, rho: f64, u: f64, stanton: f64) -> f64 {
        if rho <= 0.0 || u <= 0.0 || stanton <= 0.0 { return 0.0; }
        mdot_kg_m2s / (rho * u * stanton)
    }
}

pub struct PropulsionExtractor;

impl PropulsionExtractor {
    /// Specific impulse Isp = thrust / (mdot * g0)
    pub fn specific_impulse(thrust_n: f64, mdot_kg_s: f64) -> f64 {
        if mdot_kg_s <= 0.0 { return 0.0; }
        thrust_n / (mdot_kg_s * 9.80665)
    }

    /// Thrust coefficient Cf = thrust / (p0 * At)
    pub fn thrust_coefficient(thrust_n: f64, p0_pa: f64, at_m2: f64) -> f64 {
        if p0_pa <= 0.0 || at_m2 <= 0.0 { return 0.0; }
        thrust_n / (p0_pa * at_m2)
    }

    /// Characteristic velocity c* = p0 * At / mdot
    pub fn characteristic_velocity(p0_pa: f64, at_m2: f64, mdot_kg_s: f64) -> f64 {
        if mdot_kg_s <= 0.0 { return 0.0; }
        p0_pa * at_m2 / mdot_kg_s
    }
}

pub struct NuclearExtractor;

impl NuclearExtractor {
    /// Extract neutron flux on a patch
    pub fn neutron_flux(reader: &PostReader, patch_label: &str) -> Result<f64, Box<dyn Error>> {
        let flux = reader.read_scalar_boundary(patch_label, "neutronFlux")?;
        Ok(if flux.is_empty() { 0.0 } else { flux.iter().sum::<f64>() / flux.len() as f64 })
    }

    /// Extract reaction rate (fissions per second) on a patch
    pub fn reaction_rate(reader: &PostReader, patch_label: &str) -> Result<f64, Box<dyn Error>> {
        let rr = reader.read_scalar_boundary(patch_label, "reactionRate")?;
        Ok(if rr.is_empty() { 0.0 } else { rr.iter().sum::<f64>() / rr.len() as f64 })
    }

    /// Multiplication factor k_eff from fission/source ratio
    pub fn multiplication_factor(fission_rate: f64, source_rate: f64) -> f64 {
        if source_rate <= 0.0 { return 0.0; }
        fission_rate / source_rate
    }
}

pub struct MarineExtractor;

impl MarineExtractor {
    /// Extract wave resistance coefficient Cw = Rw / (0.5 * rho * U^2 * S)
    pub fn wave_resistance_coefficient(resistance_n: f64, rho: f64, u_m_s: f64, wetted_area_m2: f64) -> f64 {
        if rho <= 0.0 || u_m_s <= 0.0 || wetted_area_m2 <= 0.0 { return 0.0; }
        resistance_n / (0.5 * rho * u_m_s * u_m_s * wetted_area_m2)
    }

    /// Advance coefficient J = Va / (n * D) for propellers
    pub fn advance_coefficient(va_m_s: f64, revs_per_s: f64, d_m: f64) -> f64 {
        if revs_per_s <= 0.0 || d_m <= 0.0 { return 0.0; }
        va_m_s / (revs_per_s * d_m)
    }

    /// Cavitation inception speed based on depth and cavitation margin
    pub fn cavitation_inception_speed(depth_m: f64, margin: f64, rho: f64) -> f64 {
        if margin <= 1.0 { return f64::INFINITY; }
        let p_inf = 101325.0 + rho * 9.81 * depth_m;
        let p_vap = 2340.0;
        ((p_inf - p_vap) / (0.5 * rho * (margin - 1.0))).sqrt()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_darcy_number_zero_when_no_flow() {
        assert_eq!(PorousExtractor::darcy_number(100.0, 1.0, 0.001, 0.0), 0.0);
    }

    #[test]
    fn test_darcy_number_zero_when_zero_length() {
        assert_eq!(PorousExtractor::darcy_number(100.0, 0.0, 0.001, 10.0), 0.0);
    }

    #[test]
    fn test_darcy_number_computable() {
        let da = PorousExtractor::darcy_number(100.0, 1.0, 0.001, 10.0);
        assert!((da - 1e-4).abs() < 1e-15);
    }

    #[test]
    fn test_weissenberg_number_zero_when_zero_length() {
        assert_eq!(ViscoelasticExtractor::weissenberg_number(0.1, 10.0, 0.0), 0.0);
    }

    #[test]
    fn test_weissenberg_number_computable() {
        let wi = ViscoelasticExtractor::weissenberg_number(0.5, 2.0, 0.1);
        assert!((wi - 10.0).abs() < 1e-15);
    }

    #[test]
    fn test_damkohler_number_zero_when_no_flow() {
        assert_eq!(CombustionExtractor::damkohler_number(0.0, 1.0, 0.1), 0.0);
    }

    #[test]
    fn test_damkohler_number_computable() {
        let da = CombustionExtractor::damkohler_number(1.0, 1.0, 0.5);
        assert!((da - 2.0).abs() < 1e-15);
    }

    #[test]
    fn test_cavitation_number_zero_when_no_flow() {
        assert_eq!(CavitationExtractor::cavitation_number(1e5, 2.34e3, 1000.0, 0.0), 0.0);
    }

    #[test]
    fn test_cavitation_number_computable() {
        let sigma = CavitationExtractor::cavitation_number(1e5, 2.34e3, 1000.0, 10.0);
        let expected = (1e5 - 2.34e3) / (0.5 * 1000.0 * 100.0);
        assert!((sigma - expected).abs() < 1e-10);
    }

    #[test]
    fn test_spl_zero_for_negative_pressure() {
        assert_eq!(AeroacousticExtractor::sound_pressure_level(0.0), 0.0);
    }

    #[test]
    fn test_spl_computable() {
        let spl = AeroacousticExtractor::sound_pressure_level(2e-5);
        assert!((spl - 0.0).abs() < 1e-10);
    }

    #[test]
    fn test_strouhal_number_zero_when_no_flow() {
        assert_eq!(AeroacousticExtractor::strouhal_number(100.0, 0.1, 0.0), 0.0);
    }

    #[test]
    fn test_strouhal_number_computable() {
        let st = AeroacousticExtractor::strouhal_number(100.0, 0.1, 10.0);
        assert!((st - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_ursell_number_zero_when_no_depth() {
        assert_eq!(WaveExtractor::ursell_number(2.0, 10.0, 0.0), 0.0);
    }

    #[test]
    fn test_ursell_number_computable() {
        let ur = WaveExtractor::ursell_number(2.0, 10.0, 5.0);
        assert!((ur - 1.6).abs() < 1e-10);
    }

    #[test]
    fn test_stefan_number_zero_when_no_latent() {
        assert_eq!(PhaseChangeExtractor::stefan_number(500.0, 1000.0, 800.0, 0.0), 0.0);
    }

    #[test]
    fn test_stefan_number_computable() {
        let ste = PhaseChangeExtractor::stefan_number(500.0, 900.0, 800.0, 2.5e5);
        assert!((ste - 0.2).abs() < 1e-10);
    }

    #[test]
    fn test_wind_power_coefficient_zero_when_no_flow() {
        assert_eq!(WindExtractor::actual_power_coefficient(1e6, 1.225, 10000.0, 0.0), 0.0);
    }

    #[test]
    fn test_wind_thrust_coefficient_zero_when_no_flow() {
        assert_eq!(WindExtractor::actual_thrust_coefficient(1e5, 1.225, 10000.0, 0.0), 0.0);
    }

    #[test]
    fn test_electric_field_zero_when_no_gap() {
        assert_eq!(ElectrostaticExtractor::electric_field_magnitude(1e4, 0.0), 0.0);
    }

    #[test]
    fn test_electric_field_computable() {
        let ef = ElectrostaticExtractor::electric_field_magnitude(1e4, 0.1);
        assert!((ef - 1e5).abs() < 1e-10);
    }

    #[test]
    fn test_blowing_parameter_zero_when_no_flow() {
        assert_eq!(AblationExtractor::blowing_parameter(0.1, 1.225, 0.0, 0.01), 0.0);
    }

    #[test]
    fn test_blowing_parameter_computable() {
        let bp = AblationExtractor::blowing_parameter(0.1, 1.225, 10.0, 0.01);
        assert!((bp - 0.8163265).abs() < 1e-6);
    }

    #[test]
    fn test_specific_impulse_zero_when_no_mass_flow() {
        assert_eq!(PropulsionExtractor::specific_impulse(1e6, 0.0), 0.0);
    }

    #[test]
    fn test_specific_impulse_computable() {
        let isp = PropulsionExtractor::specific_impulse(1e6, 100.0);
        // 1e6 / (100 * 9.80665) = 1019.72
        assert!((isp - 1019.72).abs() < 0.01);
    }

    #[test]
    fn test_thrust_coefficient_zero_when_no_area() {
        assert_eq!(PropulsionExtractor::thrust_coefficient(1e6, 7e6, 0.0), 0.0);
    }

    #[test]
    fn test_characteristic_velocity_computable() {
        let cstar = PropulsionExtractor::characteristic_velocity(7e6, 0.01, 100.0);
        assert!((cstar - 700.0).abs() < 1e-10);
    }

    #[test]
    fn test_multiplication_factor_zero_when_no_source() {
        assert_eq!(NuclearExtractor::multiplication_factor(1.0, 0.0), 0.0);
    }

    #[test]
    fn test_multiplication_factor_computable() {
        let k = NuclearExtractor::multiplication_factor(2.0, 1.0);
        assert!((k - 2.0).abs() < 1e-10);
    }

    #[test]
    fn test_advance_coefficient_zero_when_no_revs() {
        assert_eq!(MarineExtractor::advance_coefficient(10.0, 0.0, 5.0), 0.0);
    }

    #[test]
    fn test_advance_coefficient_computable() {
        let j = MarineExtractor::advance_coefficient(10.0, 20.0, 5.0);
        assert!((j - 0.1).abs() < 1e-10);
    }

    #[test]
    fn test_cavitation_inception_speed_infinite_when_margin_one() {
        assert!(MarineExtractor::cavitation_inception_speed(5.0, 1.0, 1000.0).is_infinite());
    }

    #[test]
    fn test_cavitation_inception_speed_finite_when_margin_greater() {
        let u = MarineExtractor::cavitation_inception_speed(5.0, 2.0, 1000.0);
        assert!(u > 0.0 && u < 1e6);
    }

    // ── ML Surrogate Extractor Tests ────────────────────────────

    #[test]
    fn test_ei_positive_when_best_better_than_mean() {
        let ei = MlSurrogateExtractor::expected_improvement(0.0, 1.0, 1.0);
        assert!(ei > 0.0);
    }

    #[test]
    fn test_ei_zero_when_std_zero() {
        assert_eq!(MlSurrogateExtractor::expected_improvement(0.0, 0.0, 1.0), 0.0);
    }

    #[test]
    fn test_ucb_greater_than_mean() {
        let ucb = MlSurrogateExtractor::upper_confidence_bound(0.0, 1.0, 2.0);
        assert!((ucb - 1.41421356).abs() < 1e-6);
    }

    #[test]
    fn test_pi_between_zero_and_one() {
        let pi = MlSurrogateExtractor::probability_of_improvement(0.0, 1.0, 1.0);
        assert!(pi > 0.5 && pi < 1.0);
    }

    #[test]
    fn test_pi_zero_when_std_zero_and_mean_above_best() {
        let pi = MlSurrogateExtractor::probability_of_improvement(2.0, 0.0, 1.0);
        assert_eq!(pi, 0.0);
    }

    // ── PEMFC Extractor Tests ───────────────────────────────────

    #[test]
    fn test_nernst_potential_std() {
        let e = PemfcExtractor::nernst_potential(353.15, 1.0, 1.0, 1.0);
        assert!((e - 1.18).abs() < 0.05, "Nernst potential {e} not near 1.18 V");
    }

    #[test]
    fn test_nernst_default_returns_e0() {
        let e = PemfcExtractor::nernst_potential(353.15, 0.0, 1.0, 1.0);
        assert!((e - 1.229).abs() < 1e-6);
    }

    #[test]
    fn test_cell_voltage_below_nernst() {
        let v = PemfcExtractor::cell_voltage(1.2, 0.1, 0.05, 0.05, 0.02, 0.02);
        assert!((v - 0.96).abs() < 1e-6);
    }

    #[test]
    fn test_cell_voltage_non_negative() {
        let v = PemfcExtractor::cell_voltage(1.0, 2.0, 0.0, 0.0, 0.0, 0.0);
        assert!(v >= 0.0);
    }

    #[test]
    fn test_power_density_zero_current() {
        let p = PemfcExtractor::power_density(0.0, 0.0);
        assert!(p.abs() < 1e-12);
    }

    #[test]
    fn test_power_density_typical() {
        let p = PemfcExtractor::power_density(0.7, 1e4);
        assert!((p - 7000.0).abs() < 1e-6);
    }

    #[test]
    fn test_efficiency_below_one() {
        let eff = PemfcExtractor::efficiency(0.7, true);
        assert!(eff > 0.0 && eff < 1.0);
        assert!((eff - 0.7 / 1.482).abs() < 1e-6);
    }

    #[test]
    fn test_ohmic_loss_linear() {
        let eta = PemfcExtractor::ohmic_loss(1e4, 10.0, 50e-6);
        assert!((eta - 0.05).abs() < 1e-8);
    }

    #[test]
    fn test_activation_loss_increases_with_i() {
        let eta1 = PemfcExtractor::activation_loss(353.15, 1e3, 100.0, 0.5, 1.0);
        let eta2 = PemfcExtractor::activation_loss(353.15, 1e4, 100.0, 0.5, 1.0);
        assert!(eta2 > eta1, "Activation loss should increase with current");
    }

    #[test]
    fn test_concentration_loss_form() {
        let eta = PemfcExtractor::concentration_loss(353.15, 5000.0, 10000.0, 2.0);
        assert!(eta > 0.0, "Concentration loss should be positive");
    }

    #[test]
    fn test_ecsa_loss_fraction() {
        let loss = PemfcExtractor::ecsa_loss(80.0, 60.0);
        assert!((loss - 0.25).abs() < 1e-6);
    }

    #[test]
    fn test_membrane_lifetime_finite() {
        let hours = PemfcExtractor::membrane_lifetime(50e-6, 1e-12);
        assert!(hours > 10000.0 && hours < 15000.0);
    }

    #[test]
    fn test_membrane_lifetime_no_degradation() {
        let hours = PemfcExtractor::membrane_lifetime(50e-6, 0.0);
        assert!(hours > 1e5);
    }
}

pub struct MlSurrogateExtractor;

impl MlSurrogateExtractor {
    /// Compute expected improvement at a point given GP prediction
    pub fn expected_improvement(mean: f64, std: f64, best_so_far: f64) -> f64 {
        if std <= 0.0 { return 0.0; }
        let z = (best_so_far - mean) / std;
        let cdf = gaussian_cdf(z);
        let pdf = gaussian_pdf(z);
        (best_so_far - mean) * cdf + std * pdf
    }

    /// Upper confidence bound acquisition
    pub fn upper_confidence_bound(mean: f64, std: f64, beta: f64) -> f64 {
        mean + beta.sqrt() * std
    }

    /// Probability of improvement
    pub fn probability_of_improvement(mean: f64, std: f64, best_so_far: f64) -> f64 {
        if std <= 0.0 { return if mean < best_so_far { 1.0 } else { 0.0 }; }
        let z = (best_so_far - mean) / std;
        gaussian_cdf(z)
    }
}

fn gaussian_cdf(x: f64) -> f64 {
    // Hart's algorithm 5666 approximation (accurate to ~1.5e-7)
    let t = 1.0 / (1.0 + 0.2316419 * x.abs());
    let d = 0.3989422804 * (-0.5 * x * x).exp();
    let p = d * t * (0.319381530 + t * (-0.356563782 + t * (1.781477937 + t * (-1.821255978 + t * 1.330274429))));
    if x >= 0.0 { 1.0 - p } else { p }
}

fn gaussian_pdf(x: f64) -> f64 {
    use std::f64::consts::{FRAC_1_SQRT_2, PI};
    FRAC_1_SQRT_2 / PI.sqrt() * (-0.5 * x * x).exp()
}


pub struct PemfcExtractor;

impl PemfcExtractor {
    // Physical constants
    const R: f64 = 8.314462618;        // J/(mol·K)
    const F: f64 = 96485.33212;        // C/mol
    const N_ELECTRONS: f64 = 2.0;      // e⁻ per H₂ molecule

    /// Mean current density from field
    pub fn current_density(reader: &PostReader, patch: &str) -> Result<f64, Box<dyn Error>> {
        let i = reader.read_scalar_boundary(patch, "i")?;
        Ok(if i.is_empty() { 0.0 } else { i.iter().sum::<f64>() / i.len() as f64 })
    }

    /// Membrane potential drop across anode and cathode CL interfaces
    pub fn membrane_potential_drop(reader: &PostReader, anode_patch: &str, cathode_patch: &str) -> Result<f64, Box<dyn Error>> {
        let eps_a = reader.read_scalar_boundary(anode_patch, "epsilon")?;
        let eps_c = reader.read_scalar_boundary(cathode_patch, "epsilon")?;
        let mean_a = if eps_a.is_empty() { 0.0 } else { eps_a.iter().sum::<f64>() / eps_a.len() as f64 };
        let mean_c = if eps_c.is_empty() { 0.0 } else { eps_c.iter().sum::<f64>() / eps_c.len() as f64 };
        Ok((mean_c - mean_a).abs())
    }

    /// Water crossover flux from membrane
    pub fn water_crossover_flux(reader: &PostReader, patch: &str) -> Result<f64, Box<dyn Error>> {
        let flux = reader.read_scalar_boundary(patch, "waterFlux")?;
        Ok(if flux.is_empty() { 0.0 } else { flux.iter().sum::<f64>() / flux.len() as f64 })
    }

    /// Liquid water saturation at a patch
    pub fn liquid_saturation(reader: &PostReader, patch: &str) -> Result<f64, Box<dyn Error>> {
        let s = reader.read_scalar_boundary(patch, "s_l")?;
        Ok(if s.is_empty() { 0.0 } else { s.iter().sum::<f64>() / s.len() as f64 })
    }

    /// Electrochemical surface area from degradation field
    pub fn ecsa(reader: &PostReader, patch: &str) -> Result<f64, Box<dyn Error>> {
        let e = reader.read_scalar_boundary(patch, "ECSA")?;
        Ok(if e.is_empty() { 0.0 } else { e.iter().sum::<f64>() / e.len() as f64 })
    }

    // ── Pure function diagnostics ─────────────────────────────

    /// Nernst potential (V)
    /// E = E⁰ + (RT/nF) ln(p_H₂ · p_O₂^0.5 / p_H₂O)
    pub fn nernst_potential(t_k: f64, p_h2_bar: f64, p_o2_bar: f64, p_h2o_bar: f64) -> f64 {
        let e0 = 1.229; // standard potential at 298K, 1 bar
        if p_h2_bar <= 0.0 || p_o2_bar <= 0.0 || p_h2o_bar <= 0.0 {
            return e0;
        }
        let rt_nf = (Self::R * t_k) / (Self::N_ELECTRONS * Self::F);
        e0 + rt_nf * (p_h2_bar * p_o2_bar.sqrt() / p_h2o_bar).ln()
    }

    /// Cell voltage accounting for losses:
    /// V_cell = E_Nernst - η_ohmic - η_act - η_conc
    pub fn cell_voltage(e_nernst: f64, eta_ohmic: f64, eta_act_anode: f64, eta_act_cathode: f64, eta_conc_anode: f64, eta_conc_cathode: f64) -> f64 {
        (e_nernst - eta_ohmic - eta_act_anode - eta_act_cathode - eta_conc_anode - eta_conc_cathode).max(0.0)
    }

    /// Power density (W/m²)
    pub fn power_density(v_cell: f64, i_avg: f64) -> f64 {
        v_cell * i_avg
    }

    /// Voltage efficiency (HHV = 1.482 V, LHV = 1.253 V)
    pub fn efficiency(v_cell: f64, hhv: bool) -> f64 {
        let ref_voltage = if hhv { 1.482 } else { 1.253 };
        if ref_voltage <= 0.0 { return 0.0; }
        (v_cell / ref_voltage).clamp(0.0, 1.0)
    }

    /// Ohmic loss: η = i · t_mem / σ_mem
    pub fn ohmic_loss(i_avg: f64, sigma_mem: f64, t_mem_m: f64) -> f64 {
        if sigma_mem <= 0.0 { return 0.0; }
        i_avg * t_mem_m / sigma_mem
    }

    /// Activation loss from Butler-Volmer:
    /// η_act = (RT / αF) · asinh(i / (2·i₀))
    pub fn activation_loss(t_k: f64, i_avg: f64, i0: f64, alpha: f64, n: f64) -> f64 {
        if i0 <= 0.0 || alpha <= 0.0 { return 0.0; }
        let arg = i_avg / (2.0 * i0);
        (Self::R * t_k) / (alpha * n * Self::F) * arg.asinh()
    }

    /// Concentration loss:
    /// η_conc = -(RT/nF) · ln(1 - i / i_lim)
    pub fn concentration_loss(t_k: f64, i_avg: f64, i_lim: f64, n: f64) -> f64 {
        if i_lim <= 0.0 || i_avg >= i_lim { return 0.0; }
        -(Self::R * t_k) / (n * Self::F) * (1.0 - i_avg / i_lim).ln()
    }

    /// ECSA loss fraction after degradation
    pub fn ecsa_loss(ecsa_initial: f64, ecsa_current: f64) -> f64 {
        if ecsa_initial <= 0.0 { return 0.0; }
        (1.0 - ecsa_current / ecsa_initial).clamp(0.0, 1.0)
    }

    /// Membrane lifetime estimate (hours) based on thinning rate
    pub fn membrane_lifetime(t_mem_initial_m: f64, thinning_rate_m_s: f64) -> f64 {
        if thinning_rate_m_s <= 0.0 { return 1e6; } // effectively infinite
        t_mem_initial_m / thinning_rate_m_s / 3600.0
    }
}

/// Post-processing extractor for digital wind tunnel results.
/// Computes blockage-corrected coefficients and tunnel geometry metrics.
pub struct WindTunnelExtractor;

impl WindTunnelExtractor {
    /// Blockage ratio from model and tunnel dimensions.
    pub fn blockage_ratio(model_area_m2: f64, tunnel_area_m2: f64) -> f64 {
        if tunnel_area_m2 <= 0.0 { 0.0 } else { model_area_m2 / tunnel_area_m2 }
    }

    /// Mask-area corrected velocity: u' = u / (1 - BR)
    pub fn corrected_velocity(u_inf_m_s: f64, blockage: f64) -> f64 {
        let corr = 1.0 - blockage;
        if corr <= 0.0 { u_inf_m_s } else { u_inf_m_s / corr }
    }

    /// Mask-area corrected force coefficient: C' = C * (1 - BR)
    pub fn corrected_coefficient(c_uncorrected: f64, blockage: f64) -> f64 {
        c_uncorrected * (1.0 - blockage)
    }

    /// Tunnel cross-sectional area.
    pub fn tunnel_area(span_m: f64, height_m: f64) -> f64 {
        span_m * height_m
    }

    /// Model frontal area from bounding box extents.
    pub fn model_frontal_area(width_m: f64, height_m: f64) -> f64 {
        (width_m * height_m).max(0.0)
    }

    /// Report wind tunnel metrics as a formatted string.
    pub fn report(
        blockage: f64,
        u_inf: f64,
        cl: f64,
        cd: f64,
        tunnel_area: f64,
        model_area: f64,
    ) -> String {
        let u_corr = Self::corrected_velocity(u_inf, blockage);
        let cl_corr = Self::corrected_coefficient(cl, blockage);
        let cd_corr = Self::corrected_coefficient(cd, blockage);
        format!(
            r#"
Blockage:           {:.4}% ({:.4} fraction)
Tunnel area:        {:.4} m²
Model area:         {:.4} m²
u_inf (corrected):  {:.4} m/s  ({:.4} uncorrected)
Cl (corrected):     {:.4}      ({:.4} uncorrected)
Cd (corrected):     {:.4}      ({:.4} uncorrected)
"#,
            blockage * 100.0, blockage,
            tunnel_area, model_area,
            u_corr, u_inf,
            cl_corr, cl,
            cd_corr, cd,
        )
    }
}

#[cfg(test)]
mod wind_tunnel_tests {
    use super::*;

    #[test]
    fn test_blockage_ratio() {
        let br = WindTunnelExtractor::blockage_ratio(0.1, 10.0);
        assert!((br - 0.01).abs() < 1e-6);
    }

    #[test]
    fn test_blockage_ratio_zero_area() {
        let br = WindTunnelExtractor::blockage_ratio(0.1, 0.0);
        assert!((br - 0.0).abs() < 1e-6);
    }

    #[test]
    fn test_corrected_velocity() {
        let u = WindTunnelExtractor::corrected_velocity(10.0, 0.05);
        assert!((u - 10.526315789).abs() < 1e-6);
    }

    #[test]
    fn test_corrected_velocity_no_blockage() {
        let u = WindTunnelExtractor::corrected_velocity(10.0, 0.0);
        assert!((u - 10.0).abs() < 1e-6);
    }

    #[test]
    fn test_corrected_coefficient() {
        let c = WindTunnelExtractor::corrected_coefficient(1.0, 0.05);
        assert!((c - 0.95).abs() < 1e-6);
    }

    #[test]
    fn test_tunnel_area() {
        let a = WindTunnelExtractor::tunnel_area(5.0, 5.0);
        assert!((a - 25.0).abs() < 1e-6);
    }

    #[test]
    fn test_report_contains_values() {
        let r = WindTunnelExtractor::report(0.03, 10.0, 0.5, 0.02, 25.0, 0.75);
        assert!(r.contains("Blockage"));
        assert!(r.contains("3.0000%"));
        assert!(r.contains("0.5000"));
        assert!(r.contains("0.0200"));
    }
}
