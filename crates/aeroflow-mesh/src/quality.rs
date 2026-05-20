use aeroflow_core::{MeshQualityMetrics, MeshQualityThresholds};

#[derive(Debug)]
pub struct QualityVerdict {
    pub passed: bool,
    pub metrics: MeshQualityMetrics,
    pub warnings: Vec<String>,
    pub failures: Vec<String>,
    pub recommendations: Vec<String>,
}

pub struct MeshQualityEngine;

impl MeshQualityEngine {
    pub fn new() -> Self {
        Self
    }

    pub fn check_mesh(
        &self,
        metrics: &MeshQualityMetrics,
        thresholds: &MeshQualityThresholds,
    ) -> QualityVerdict {
        let mut warnings = Vec::new();
        let mut failures = Vec::new();
        let mut recommendations = Vec::new();

        // Non-orthogonality
        if metrics.max_non_orthogonality >= thresholds.max_non_orthogonality_fail {
            failures.push(format!(
                "Max non-orthogonality {:.1}° exceeds fail threshold {:.0}°",
                metrics.max_non_orthogonality, thresholds.max_non_orthogonality_fail
            ));
        } else if metrics.max_non_orthogonality >= thresholds.max_non_orthogonality_warn {
            warnings.push(format!(
                "Max non-orthogonality {:.1}° exceeds warn threshold {:.0}°",
                metrics.max_non_orthogonality, thresholds.max_non_orthogonality_warn
            ));
            recommendations.push("Increase nCellsBetweenLevels or add more refinement layers".into());
        }

        // Skewness
        if metrics.max_skewness >= thresholds.max_skewness_fail {
            failures.push(format!(
                "Max skewness {:.1} exceeds fail threshold {:.0}",
                metrics.max_skewness, thresholds.max_skewness_fail
            ));
        } else if metrics.max_skewness >= thresholds.max_skewness_warn {
            warnings.push(format!(
                "Max skewness {:.1} exceeds warn threshold {:.0}",
                metrics.max_skewness, thresholds.max_skewness_warn
            ));
        }

        // Determinant
        if metrics.min_determinant <= thresholds.min_determinant_fail {
            failures.push(format!(
                "Min determinant {:.4} below fail threshold {:.4}",
                metrics.min_determinant, thresholds.min_determinant_fail
            ));
            recommendations.push("Reduce snappy refinement levels or check STL quality".into());
        }

        // Aspect ratio
        if metrics.max_aspect_ratio >= thresholds.max_aspect_ratio_fail {
            failures.push(format!(
                "Max aspect ratio {:.0}:1 exceeds fail threshold {:.0}:1",
                metrics.max_aspect_ratio, thresholds.max_aspect_ratio_fail
            ));
        }

        // Failed cells
        if metrics.n_failed_cells > 0 {
            failures.push(format!(
                "{} cells failed quality check",
                metrics.n_failed_cells
            ));
        }

        QualityVerdict {
            passed: failures.is_empty(),
            metrics: metrics.clone(),
            warnings,
            failures,
            recommendations,
        }
    }
}
