use aeroflow_core::ResourceUsage;
use chrono::Utc;

use crate::DockerClient;

pub struct StatsCollector {
    client: DockerClient,
}

impl StatsCollector {
    pub fn new(client: DockerClient) -> Self {
        Self { client }
    }

    pub async fn collect_stats(&self, container_id: &str) -> Result<ResourceUsage, anyhow::Error> {
        // P2: collect Docker stats via bollard streaming stats API
        let _ = container_id;
        // Placeholder for P0
        Ok(ResourceUsage {
            cpu_percent: 0.0,
            memory_used_bytes: 0,
            memory_limit_bytes: 0,
            disk_read_bytes: 0,
            disk_write_bytes: 0,
            network_rx_bytes: 0,
            network_tx_bytes: 0,
            timestamp: Utc::now(),
        })
    }
}
