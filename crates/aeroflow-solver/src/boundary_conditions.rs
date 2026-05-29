use aeroflow_core::WindTunnelConfig;

/// Generates OpenFOAM boundary condition field files (0/U, 0/p, 0/k, 0/omega, 0/nut)
/// for digital wind tunnel cases using freestream BCs on farfield boundaries.
pub struct WindTunnelBcGenerator;

impl WindTunnelBcGenerator {
    /// Turbulent kinetic energy from turbulence intensity and velocity magnitude.
    pub fn k_from_intensity(u_mag: f64, ti: f64) -> f64 {
        1.5 * (u_mag * ti).powi(2)
    }

    /// Specific dissipation rate from k and turbulence viscosity ratio.
    pub fn omega_from_k(mut k: f64, viscosity_ratio: f64, nu: f64) -> f64 {
        if k <= 0.0 { k = 1e-10; }
        k.max(1e-10) / (viscosity_ratio * nu)
    }

    /// Turbulent viscosity nut from k and omega.
    pub fn nut_from_k_omega(k: f64, omega: f64) -> f64 {
        k / omega.max(1e-10)
    }

    /// Generate the content for `0/U` with freestreamVelocity on farfield boundaries.
    pub fn generate_u(
        u_mag: f64,
        direction: [f64; 3],
        wall_patches: &[String],
    ) -> String {
        let u_vec = format!("({} {} {})", direction[0]*u_mag, direction[1]*u_mag, direction[2]*u_mag);
        let wall_bc = wall_patches.iter()
            .map(|p| format!("    {}    {{ type fixedValue; value uniform (0 0 0); }}", p))
            .collect::<Vec<_>>()
            .join("\n");

        format!(
            r#"FoamFile {{ version 2.0; format ascii; class volVectorField; object U; }}

dimensions [0 1 -1 0 0 0 0];
internalField uniform {};

boundaryField
{{
    inlet
    {{
        type            freestreamVelocity;
        freestreamValue uniform {};
        value           $internalField;
    }}
    outlet
    {{
        type            inletOutlet;
        inletValue      uniform {};
        value           $internalField;
    }}
    top
    {{
        type            freestreamVelocity;
        freestreamValue uniform {};
        value           $internalField;
    }}
    bottom
    {{
        type            freestreamVelocity;
        freestreamValue uniform {};
        value           $internalField;
    }}
    front
    {{
        type            freestreamVelocity;
        freestreamValue uniform {};
        value           $internalField;
    }}
    back
    {{
        type            freestreamVelocity;
        freestreamValue uniform {};
        value           $internalField;
    }}
{}
    ".*"
    {{
        type            freestreamVelocity;
        freestreamValue uniform {};
        value           $internalField;
    }}
}}
"#,
            u_vec, u_vec, u_vec,
            u_vec, u_vec, u_vec, u_vec,
            if wall_bc.is_empty() { String::new() } else { format!("\n{}", wall_bc) },
            u_vec,
        )
    }

    /// Generate the content for `0/p` with freestreamPressure on farfield boundaries.
    pub fn generate_p(
        wall_patches: &[String],
    ) -> String {
        let wall_bc = wall_patches.iter()
            .map(|p| format!("    {}    {{ type zeroGradient; }}", p))
            .collect::<Vec<_>>()
            .join("\n");

        format!(
            r#"FoamFile {{ version 2.0; format ascii; class volScalarField; object p; }}

dimensions [0 2 -2 0 0 0 0];
internalField uniform 0;

boundaryField
{{
    inlet
    {{
        type            freestreamPressure;
        value           $internalField;
    }}
    outlet
    {{
        type            inletOutlet;
        inletValue      uniform 0;
        value           $internalField;
    }}
    top
    {{
        type            freestreamPressure;
        value           $internalField;
    }}
    bottom
    {{
        type            freestreamPressure;
        value           $internalField;
    }}
    front
    {{
        type            freestreamPressure;
        value           $internalField;
    }}
    back
    {{
        type            freestreamPressure;
        value           $internalField;
    }}
{}
    ".*"
    {{
        type            freestreamPressure;
        value           $internalField;
    }}
}}
"#,
            if wall_bc.is_empty() { String::new() } else { format!("\n{}", wall_bc) }
        )
    }

    /// Generate the content for `0/k` (turbulent kinetic energy).
    pub fn generate_k(k_value: f64, wall_patches: &[String]) -> String {
        let k_str = format!("uniform {:.8e}", k_value);
        let wall_bc = wall_patches.iter()
            .map(|p| format!("    {}    {{ type zeroGradient; }}", p))
            .collect::<Vec<_>>()
            .join("\n");

        format!(
            r#"FoamFile {{ version 2.0; format ascii; class volScalarField; object k; }}

dimensions [0 2 -2 0 0 0 0];
internalField {};

boundaryField
{{
    inlet
    {{
        type            freestreamVelocity;
        freestreamValue {};
        value           $internalField;
    }}
    outlet
    {{
        type            inletOutlet;
        inletValue      {};
        value           $internalField;
    }}
    top
    {{
        type            freestreamVelocity;
        freestreamValue {};
        value           $internalField;
    }}
    bottom
    {{
        type            freestreamVelocity;
        freestreamValue {};
        value           $internalField;
    }}
    front
    {{
        type            freestreamVelocity;
        freestreamValue {};
        value           $internalField;
    }}
    back
    {{
        type            freestreamVelocity;
        freestreamValue {};
        value           $internalField;
    }}
{}
    ".*"
    {{
        type            freestreamVelocity;
        freestreamValue {};
        value           $internalField;
    }}
}}
"#,
            k_str, k_str, k_str,
            k_str, k_str, k_str, k_str,
            if wall_bc.is_empty() { String::new() } else { format!("\n{}", wall_bc) },
            k_str,
        )
    }

    /// Generate the content for `0/omega` (specific dissipation rate).
    pub fn generate_omega(omega_value: f64, wall_patches: &[String]) -> String {
        let o_str = format!("uniform {:.8e}", omega_value);
        let wall_bc = wall_patches.iter()
            .map(|p| format!("    {}    {{ type zeroGradient; }}", p))
            .collect::<Vec<_>>()
            .join("\n");

        format!(
            r#"FoamFile {{ version 2.0; format ascii; class volScalarField; object omega; }}

dimensions [0 0 -1 0 0 0 0];
internalField {};

boundaryField
{{
    inlet
    {{
        type            freestreamVelocity;
        freestreamValue {};
        value           $internalField;
    }}
    outlet
    {{
        type            inletOutlet;
        inletValue      {};
        value           $internalField;
    }}
    top
    {{
        type            freestreamVelocity;
        freestreamValue {};
        value           $internalField;
    }}
    bottom
    {{
        type            freestreamVelocity;
        freestreamValue {};
        value           $internalField;
    }}
    front
    {{
        type            freestreamVelocity;
        freestreamValue {};
        value           $internalField;
    }}
    back
    {{
        type            freestreamVelocity;
        freestreamValue {};
        value           $internalField;
    }}
{}
    ".*"
    {{
        type            freestreamVelocity;
        freestreamValue {};
        value           $internalField;
    }}
}}
"#,
            o_str, o_str, o_str,
            o_str, o_str, o_str, o_str,
            if wall_bc.is_empty() { String::new() } else { format!("\n{}", wall_bc) },
            o_str,
        )
    }

    /// Generate the content for `0/nut` (turbulent viscosity).
    pub fn generate_nut(nut_value: f64, wall_patches: &[String]) -> String {
        let n_str = format!("uniform {:.8e}", nut_value);
        let wall_bc = wall_patches.iter()
            .map(|p| format!("    {}    {{ type nutUSpaldingWallFunction; value {}; }}", p, n_str))
            .collect::<Vec<_>>()
            .join("\n");

        format!(
            r#"FoamFile {{ version 2.0; format ascii; class volScalarField; object nut; }}

dimensions [0 2 -1 0 0 0 0];
internalField {};

boundaryField
{{
    inlet
    {{
        type            freestreamVelocity;
        freestreamValue {};
        value           $internalField;
    }}
    outlet
    {{
        type            inletOutlet;
        inletValue      {};
        value           $internalField;
    }}
    top
    {{
        type            freestreamVelocity;
        freestreamValue {};
        value           $internalField;
    }}
    bottom
    {{
        type            freestreamVelocity;
        freestreamValue {};
        value           $internalField;
    }}
    front
    {{
        type            freestreamVelocity;
        freestreamValue {};
        value           $internalField;
    }}
    back
    {{
        type            freestreamVelocity;
        freestreamValue {};
        value           $internalField;
    }}
{}
    ".*"
    {{
        type            freestreamVelocity;
        freestreamValue {};
        value           $internalField;
    }}
}}
"#,
            n_str, n_str, n_str,
            n_str, n_str, n_str, n_str,
            if wall_bc.is_empty() { String::new() } else { format!("\n{}", wall_bc) },
            n_str,
        )
    }

    /// Generate all initial field files and return a map of filename → content.
    pub fn generate_all(
        config: &WindTunnelConfig,
        wall_patches: &[String],
    ) -> Vec<(String, String)> {
        let u_mag = config.velocity_m_s.unwrap_or(10.0);
        let ti = config.turbulence_intensity;
        let k = Self::k_from_intensity(u_mag, ti);
        let omega = Self::omega_from_k(k, config.turbulence_viscosity_ratio, 1.5e-5);
        let nut = Self::nut_from_k_omega(k, omega);

        vec![
            ("U".into(), Self::generate_u(u_mag, [1.0, 0.0, 0.0], wall_patches)),
            ("p".into(), Self::generate_p(wall_patches)),
            ("k".into(), Self::generate_k(k, wall_patches)),
            ("omega".into(), Self::generate_omega(omega, wall_patches)),
            ("nut".into(), Self::generate_nut(nut, wall_patches)),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> WindTunnelConfig {
        WindTunnelConfig {
            velocity_m_s: Some(10.0),
            turbulence_intensity: 0.01,
            turbulence_viscosity_ratio: 10.0,
            ..WindTunnelConfig::default()
        }
    }

    #[test]
    fn test_k_from_intensity() {
        let k = WindTunnelBcGenerator::k_from_intensity(10.0, 0.01);
        assert!((k - 0.015).abs() < 1e-6);
    }

    #[test]
    fn test_omega_from_k() {
        let omega = WindTunnelBcGenerator::omega_from_k(0.015, 10.0, 1.5e-5);
        assert!((omega - 100.0).abs() < 1.0);
    }

    #[test]
    fn test_nut_from_k_omega() {
        let nut = WindTunnelBcGenerator::nut_from_k_omega(0.015, 100.0);
        assert!((nut - 1.5e-4).abs() < 1e-6);
    }

    #[test]
    fn test_generate_u_freestream() {
        let u = WindTunnelBcGenerator::generate_u(10.0, [1.0, 0.0, 0.0], &[]);
        assert!(u.contains("freestreamVelocity"));
        assert!(u.contains("10"));
        assert!(u.contains("inletOutlet"));
    }

    #[test]
    fn test_generate_p_freestream() {
        let p = WindTunnelBcGenerator::generate_p(&[]);
        assert!(p.contains("freestreamPressure"));
        assert!(p.contains("inletOutlet"));
    }

    #[test]
    fn test_generate_all() {
        let cfg = test_config();
        let files = WindTunnelBcGenerator::generate_all(&cfg, &["blade".to_string()]);
        assert_eq!(files.len(), 5);
        for (name, content) in &files {
            assert!(content.contains("FoamFile"), "Missing FoamFile in {}", name);
            match name.as_str() {
                "U" => assert!(content.contains("freestreamVelocity")),
                "p" => assert!(content.contains("freestreamPressure")),
                "k" => assert!(content.contains("freestreamVelocity")),
                "omega" => assert!(content.contains("freestreamVelocity")),
                "nut" => assert!(content.contains("freestreamVelocity")),
                _ => {}
            }
            assert!(content.contains("blade"), "Missing wall patch in {}", name);
        }
    }

    #[test]
    fn test_generate_all_no_walls() {
        let cfg = test_config();
        let files = WindTunnelBcGenerator::generate_all(&cfg, &[]);
        assert_eq!(files.len(), 5);
    }
}
