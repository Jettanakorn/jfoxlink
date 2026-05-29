use aeroflow_core::{ForceCoefficients, MeshQualityMetrics, ScoringWeights, SolverStats};

#[derive(Default)]
pub struct RewardFunction {
    weights: ScoringWeights,
}


impl RewardFunction {
    pub fn new(weights: ScoringWeights) -> Self {
        Self { weights }
    }

    /// Compute composite reward score (lower = better, target = 0.0)
    pub fn compute(
        &self,
        forces: &ForceCoefficients,
        mesh: &MeshQualityMetrics,
        solver: &SolverStats,
        target_cl: Option<f64>,
        target_cd_max: Option<f64>,
        target_yplus: f64,
    ) -> f64 {
        let cl_err = match target_cl {
            Some(t) => ((forces.cl - t).abs() / t).min(1.0),
            None => 0.0,
        };

        let cd_excess = match target_cd_max {
            Some(t) => (forces.cd - t).max(0.0) / t,
            None => 0.0,
        };

        let yplus_penalty = {
            let yp = forces.cl_std.max(0.1);  // place holder — real y+ from yPlus field
            if yp <= target_yplus {
                0.0
            } else {
                ((yp / target_yplus).sqrt() - 1.0).max(0.0)
            }
        };

        let residual_penalty = {
            let r = solver.residual_p.max(1e-15);
            r.log10().abs() / 5.0
        };

        let mesh_quality = {
            let ortho = (mesh.max_non_orthogonality / 70.0).min(1.0);
            let skew = (mesh.max_skewness / 4.0).min(1.0);
            (ortho + skew) / 2.0
        };

        let raw = self.weights.w_cl * cl_err
            + self.weights.w_cd * cd_excess
            + self.weights.w_yplus * yplus_penalty
            + self.weights.w_residual * residual_penalty;

        // Normalize to [0, 1] with mesh quality bonus
        (raw + mesh_quality * 0.05).min(1.0)
    }
}
