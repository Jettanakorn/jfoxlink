use aeroflow_core::{IntakeConfig, OpenFOAMFormat, Priority};

pub struct SolverConfigGen {
    write_format: OpenFOAMFormat,
}

impl SolverConfigGen {
    pub fn new() -> Self {
        Self {
            write_format: OpenFOAMFormat::Binary,
        }
    }

    pub fn with_format(format: OpenFOAMFormat) -> Self {
        Self { write_format: format }
    }

    /// Select solver based on flow regime (from skills/openfoam-aerospace SKILL.md Step 1)
    pub fn select_solver(&self, intake: &IntakeConfig) -> &'static str {
        match () {
            _ if intake.mach_number < 0.3 => "simpleFoam",
            _ if intake.mach_number < 0.8 => "rhoSimpleFoam",
            _ if intake.mach_number < 1.2 => "rhoCentralFoam",
            _ => "rhoCentralFoam",
        }
    }

    /// Select turbulence model (from SKILL.md Step 3)
    pub fn select_turbulence_model(&self, intake: &IntakeConfig) -> &'static str {
        match (intake.reynolds_number, intake.mach_number, &intake.priority) {
            (re, _, Priority::Speed) if re > 1e6 => "SpalartAllmaras",
            (re, _, Priority::Accuracy) if re > 1e6 => "kOmegaSST",
            (_, ma, _) if ma > 0.5 => "kOmegaSST",
            _ => "kOmegaSST",
        }
    }

    /// Compute relaxation factors from priority (from SKILL.md Rule P5)
    pub fn relaxation_factors(&self, intake: &IntakeConfig) -> (f64, f64) {
        match intake.priority {
            Priority::Speed => (0.8, 0.4),
            Priority::Balanced => (0.7, 0.3),
            Priority::Accuracy => (0.5, 0.2),
        }
    }

    /// Generate controlDict content with binary format (saves huge disk space)
    pub fn generate_control_dict(&self, intake: &IntakeConfig) -> String {
        let solver = self.select_solver(intake);
        let format = self.write_format.label();
        let iterations = match intake.priority {
            Priority::Speed => 1500,
            Priority::Balanced => 3000,
            Priority::Accuracy => 5000,
        };
        format!(
            r#"FoamFile {{ version 2.0; format {}; class dictionary; object controlDict; }}
application     {};
startFrom       startTime;
startTime       0;
stopAt          endTime;
endTime         {};
deltaT          1;
writeControl    timeStep;
writeInterval   500;
purgeWrite      3;
"#, format, solver, iterations)
    }
}
