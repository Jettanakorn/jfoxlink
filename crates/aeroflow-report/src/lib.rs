use aeroflow_core::{ForceCoefficients, MeshQualityMetrics, SolverStats};
use std::path::Path;
use tera::{Context, Tera};

pub struct ReportGenerator {
    tera: Tera,
}

impl ReportGenerator {
    /// Create a new report generator, auto-detecting the template directory.
    pub fn new() -> Result<Self, anyhow::Error> {
        // Try common locations for the template file
        let candidates = [
            "templates",
            "/build/templates",
            "../templates",
        ];
        let template_dir = candidates.iter()
            .find(|d| Path::new(d).join("report.html.tera").exists())
            .unwrap_or(&"templates");

        let pattern = format!("{}/*.html.tera", template_dir);
        let mut tera = Tera::new(&pattern)?;
        tera.autoescape_on(Vec::new());
        Ok(Self { tera })
    }

    /// Render the report HTML with the given data.
    pub fn render(
        &self,
        case_name: &str,
        case_id: &str,
        stage: &str,
        mesh: &MeshQualityMetrics,
        forces: &ForceCoefficients,
        solver: &SolverStats,
        viz_images: &[String],
    ) -> Result<String, anyhow::Error> {
        let mut ctx = Context::new();
        // Case info
        ctx.insert("case_name", case_name);
        ctx.insert("case_id", case_id);
        ctx.insert("stage", stage);
        ctx.insert("converged", &solver.converged);

        // Force coefficients
        ctx.insert("cl", &forces.cl);
        ctx.insert("cd", &forces.cd);
        ctx.insert("cm", &forces.cm);
        ctx.insert("cl_std", &forces.cl_std);
        ctx.insert("cd_std", &forces.cd_std);

        // Mesh quality
        ctx.insert("max_non_ortho", &mesh.max_non_orthogonality);
        ctx.insert("avg_non_ortho", &mesh.avg_non_orthogonality);
        ctx.insert("max_skewness", &mesh.max_skewness);
        ctx.insert("min_determinant", &mesh.min_determinant);
        ctx.insert("n_cells", &mesh.n_cells);

        // Solver
        ctx.insert("iterations", &solver.iterations);
        ctx.insert("wall_time_s", &solver.wall_time_s);
        ctx.insert("residual_p", &solver.residual_p);
        ctx.insert("residual_u", &solver.residual_u);

        // Visualization images
        ctx.insert("viz_images", viz_images);

        Ok(self.tera.render("report.html.tera", &ctx)?)
    }

    /// Generate an HTML report and write it to a file.
    pub fn generate_html_report(
        &self,
        case_name: &str,
        case_id: &str,
        stage: &str,
        mesh: &MeshQualityMetrics,
        forces: &ForceCoefficients,
        solver: &SolverStats,
        viz_images: &[String],
        output_dir: &Path,
    ) -> Result<(), anyhow::Error> {
        let html = self.render(case_name, case_id, stage, mesh, forces, solver, viz_images)?;
        std::fs::create_dir_all(output_dir)?;
        std::fs::write(output_dir.join("index.html"), html)?;
        tracing::info!("Report generated at {:?}", output_dir.join("index.html"));
        Ok(())
    }
}
