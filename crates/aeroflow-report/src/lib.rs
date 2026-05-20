use aeroflow_core::{CaseMeta, ForceCoefficients, MeshQualityMetrics, SolverStats};
use std::path::Path;
use tera::{Context, Tera};

pub struct ReportGenerator {
    tera: Tera,
}

impl ReportGenerator {
    pub fn new(template_dir: &Path) -> Result<Self, anyhow::Error> {
        let template_pattern = template_dir.join("**/*.html.tera").to_string_lossy().to_string();
        let tera = Tera::new(&template_pattern)?;
        Ok(Self { tera })
    }

    pub fn generate_html_report(
        &self,
        case: &CaseMeta,
        mesh: &MeshQualityMetrics,
        forces: &ForceCoefficients,
        solver: &SolverStats,
        output_dir: &Path,
    ) -> Result<(), anyhow::Error> {
        let mut ctx = Context::new();
        ctx.insert("case_name", &case.name);
        ctx.insert("case_id", &case.id.to_string());
        ctx.insert("stage", &case.stage.label());
        ctx.insert("max_non_ortho", &mesh.max_non_orthogonality);
        ctx.insert("avg_non_ortho", &mesh.avg_non_orthogonality);
        ctx.insert("max_skewness", &mesh.max_skewness);
        ctx.insert("min_determinant", &mesh.min_determinant);
        ctx.insert("n_cells", &mesh.n_cells);
        ctx.insert("cl", &forces.cl);
        ctx.insert("cd", &forces.cd);
        ctx.insert("cm", &forces.cm);
        ctx.insert("cl_std", &forces.cl_std);
        ctx.insert("cd_std", &forces.cd_std);
        ctx.insert("iterations", &solver.iterations);
        ctx.insert("wall_time_s", &solver.wall_time_s);
        ctx.insert("residual_p", &solver.residual_p);
        ctx.insert("residual_u", &solver.residual_u);
        ctx.insert("converged", &solver.converged);

        let html = self.tera.render("report.html.tera", &ctx)?;

        std::fs::create_dir_all(output_dir)?;
        std::fs::write(output_dir.join("index.html"), html)?;

        tracing::info!("Report generated at {:?}", output_dir.join("index.html"));
        Ok(())
    }
}
