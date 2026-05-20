use aeroflow_core::AeroFlowConfig;
use bollard::Docker;

#[derive(Clone)]
pub struct DockerClient {
    pub docker: Docker,
    pub config: AeroFlowConfig,
}

impl DockerClient {
    pub async fn connect(config: &AeroFlowConfig) -> Result<Self, anyhow::Error> {
        let docker = Docker::connect_with_local_defaults()?;
        docker.ping().await?;
        tracing::info!("Docker daemon reachable");
        Ok(Self {
            docker,
            config: config.clone(),
        })
    }

    pub async fn ping(&self) -> Result<bool, anyhow::Error> {
        self.docker.ping().await?;
        Ok(true)
    }

    pub async fn version(&self) -> Result<String, anyhow::Error> {
        let v = self.docker.version().await?;
        Ok(format!("{:?}", v))
    }
}
