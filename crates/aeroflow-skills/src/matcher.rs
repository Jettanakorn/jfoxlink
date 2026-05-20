use aeroflow_core::SkillId;

pub struct SkillMatch {
    pub skill_id: SkillId,
    pub name: String,
    pub confidence: f64,
    pub flow_regime_key: String,
    pub hamming_distance: u32,
}

pub struct SkillMatcher;

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
