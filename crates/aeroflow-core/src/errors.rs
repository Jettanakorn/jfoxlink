use thiserror::Error;

#[derive(Error, Debug)]
pub enum AeroFlowError {
    #[error("Docker error: {0}")]
    Docker(String),

    #[error("Database error: {0}")]
    Database(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serialization(String),

    #[error("Pipeline error: {0}")]
    Pipeline(String),

    #[error("Mesh quality check failed: {0}")]
    MeshQuality(String),

    #[error("Solver diverged: {0}")]
    SolverDiverged(String),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Skill error: {0}")]
    Skill(String),
}

impl From<anyhow::Error> for AeroFlowError {
    fn from(e: anyhow::Error) -> Self {
        AeroFlowError::Pipeline(e.to_string())
    }
}
