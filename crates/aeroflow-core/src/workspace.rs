use std::path::Path;

pub struct WorkspaceManager;

impl WorkspaceManager {
    pub fn create_layout(root: &str) -> std::io::Result<super::WorkspaceLayout> {
        let layout = super::WorkspaceLayout::new(root);
        layout.create_all()?;
        Ok(layout)
    }

    pub fn validate_disk_space(path: &Path, min_gb: u64) -> Result<(u64, bool), anyhow::Error> {
        let free = fs2::available_space(path)?;
        let free_gb = free / 1_073_741_824;
        let ok = free_gb >= min_gb;
        Ok((free_gb, ok))
    }

    pub fn settings_file(workspace_dir: &str) -> String {
        format!("{}/settings/aeroflow-settings.toml", workspace_dir)
    }

    pub fn global_settings_file() -> Option<String> {
        if let Ok(home) = std::env::var("HOME") {
            Some(format!("{}/.config/aeroflow/settings.toml", home))
        } else if let Ok(home) = std::env::var("USERPROFILE") {
            Some(format!("{}/.config/aeroflow/settings.toml", home))
        } else {
            None
        }
    }
}
