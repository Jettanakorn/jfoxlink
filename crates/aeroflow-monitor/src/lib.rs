use aeroflow_core::ResourceUsage;
use chrono::Utc;
use sysinfo::{CpuRefreshKind, Disks, MemoryRefreshKind, RefreshKind, System};

pub struct ResourceMonitor {
    system: System,
}

impl ResourceMonitor {
    pub fn new() -> Self {
        Self {
            system: System::new_with_specifics(
                RefreshKind::nothing()
                    .with_cpu(CpuRefreshKind::nothing())
                    .with_memory(MemoryRefreshKind::nothing())
            ),
        }
    }

    pub fn host_usage(&mut self) -> ResourceUsage {
        self.system.refresh_cpu_specifics(CpuRefreshKind::everything());
        self.system.refresh_memory_specifics(MemoryRefreshKind::everything());

        let cpu_percent = self.system.global_cpu_usage() as f64;
        let total_mem = self.system.total_memory() as u64;
        let used_mem = self.system.used_memory() as u64;

        ResourceUsage {
            cpu_percent,
            memory_used_bytes: used_mem,
            memory_limit_bytes: total_mem,
            disk_read_bytes: 0,
            disk_write_bytes: 0,
            network_rx_bytes: 0,
            network_tx_bytes: 0,
            timestamp: Utc::now(),
        }
    }

    pub fn disk_free_bytes(&mut self) -> u64 {
        Disks::new_with_refreshed_list()
            .iter()
            .map(|d| d.available_space())
            .sum()
    }

    pub fn cpu_count(&self) -> usize {
        self.system.physical_core_count().unwrap_or(0)
    }
}
