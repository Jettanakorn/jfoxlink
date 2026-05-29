use aeroflow_core::SkillId;

pub struct SkillMatch {
    pub skill_id: SkillId,
    pub name: String,
    pub confidence: f64,
    pub flow_regime_key: String,
    pub hamming_distance: u32,
}

pub struct SkillMatcher;

impl Default for SkillMatcher {
    fn default() -> Self {
        Self::new()
    }
}

impl SkillMatcher {
    pub fn new() -> Self {
        Self
    }

    pub async fn find_matches(
        &self,
        _fingerprint: &super::GeometryFingerprint,
        _mach: f64,
        _reynolds: f64,
    ) -> Vec<SkillMatch> {
        // P0 stub: in P4 this will query PostgreSQL via SkillsDb
        // using Hamming distance on coarse hash + regime filtering
        vec![]
    }

    pub fn compute_flow_regime_key(mach: f64, reynolds: f64, flow_type: &str) -> String {
        let mach_bucket = if mach < 0.3 {
            "Ma0.0-0.3"
        } else if mach < 0.8 {
            "Ma0.3-0.8"
        } else if mach < 1.2 {
            "Ma0.8-1.2"
        } else if mach < 5.0 {
            "Ma1.2-5.0"
        } else {
            "Ma5.0+"
        };

        let re_exp = reynolds.log10().floor();
        let re_bucket = format!("Re1e{}", re_exp);

        format!("{}_{}_{}", flow_type, mach_bucket, re_bucket)
    }

    pub fn regime_ranges_match(
        skill_regime: &str,
        target_mach: f64,
        target_re: f64,
    ) -> bool {
        // Stub: parse and compare regime ranges
        let _ = (skill_regime, target_mach, target_re);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_flow_regime_key_subsonic() {
        let key = SkillMatcher::compute_flow_regime_key(0.2, 1e5, "external");
        assert_eq!(key, "external_Ma0.0-0.3_Re1e5");
    }

    #[test]
    fn test_compute_flow_regime_key_transonic() {
        let key = SkillMatcher::compute_flow_regime_key(1.0, 1e6, "external");
        assert_eq!(key, "external_Ma0.8-1.2_Re1e6");
    }

    #[test]
    fn test_compute_flow_regime_key_supersonic() {
        let key = SkillMatcher::compute_flow_regime_key(3.0, 1e7, "internal");
        assert_eq!(key, "internal_Ma1.2-5.0_Re1e7");
    }

    #[test]
    fn test_compute_flow_regime_key_hypersonic() {
        let key = SkillMatcher::compute_flow_regime_key(6.0, 1e5, "external");
        assert_eq!(key, "external_Ma5.0+_Re1e5");
    }

    #[test]
    fn test_compute_flow_regime_key_contains_substrings() {
        let key = SkillMatcher::compute_flow_regime_key(0.5, 5e5, "internal");
        assert!(key.contains("Ma0.3-0.8"));
        assert!(key.contains("Re1e5"));
        assert!(key.contains("internal"));
    }
}
