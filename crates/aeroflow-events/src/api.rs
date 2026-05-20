/// Web API module — moved to aeroflow-api crate for P3
pub struct WebApi;

impl WebApi {
    pub fn new() -> Self {
        Self
    }

    pub async fn start(self, port: u16) -> Result<(), anyhow::Error> {
        let _ = port;
        tracing::info!("Web API stub — port {}", port);
        Ok(())
    }
}
