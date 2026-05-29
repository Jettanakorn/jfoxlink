use aeroflow_core::{AeroflowSettings, SettingsManager, WorkspaceManager, OpenFOAMFormat};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_settings_path_with_explicit_path() {
        let custom = "custom/path".to_string();
        let path = get_settings_path(Some(&custom));
        assert_eq!(path, "custom/path");
    }

    #[test]
    fn get_settings_path_none_returns_string() {
        let path = get_settings_path(None);
        assert!(!path.is_empty());
    }
}

fn get_settings_path(action_arg: Option<&String>) -> String {
    if let Some(p) = action_arg {
        return p.clone();
    }

    if let Ok(ws) = std::env::var("AEROFLOW_WORKSPACE_DIR") {
        return WorkspaceManager::settings_file(&ws);
    }

    if let Some(global) = WorkspaceManager::global_settings_file() {
        return global;
    }

    "aeroflow-settings.toml".to_string()
}

pub async fn execute(action: &crate::SettingsAction) -> anyhow::Result<()> {
    match action {
        crate::SettingsAction::Show => {
            let mgr = SettingsManager::load();
            let s = &mgr.settings;

            println!("\n=== AeroFlow Settings ===\n");

            println!("[Workspace]");
            println!("  workspace_dir:     {}", s.workspace_dir);
            println!("  data_dir:          {}", s.data_dir);
            println!();

            println!("[Database]");
            println!("  database_url:      {}", s.database_url);
            println!();

            println!("[Docker / Container]");
            println!("  docker_socket:     {}", s.docker_socket);
            println!("  openfoam_image:    {}", s.openfoam_image);
            println!("  max_concurrent:    {}", s.max_concurrent_cases);
            println!("  default_cores:     {}", s.default_cores);
            println!("  default_memory_gb: {} GB", s.default_memory_gb);
            println!();

            println!("[OpenFOAM]");
            println!("  write_format:      {}", s.openfoam_format.label());
            println!();

            println!("[Solver Defaults]");
            println!("  min_iterations:    {}", s.solver.min_iterations);
            println!("  max_iterations:    {}", s.solver.max_iterations);
            println!("  write_interval:    {}", s.solver.write_interval);
            println!("  convergence_res:   {:.0e}", s.solver.convergence_residual);
            println!();

            println!("[User Management]");
            println!("  session_ttl_hours: {}", s.session_ttl_hours);
            println!("  allow_registration: {}", s.allow_registration);
            println!();

            if let Some(path) = &mgr.config_path {
                println!("Settings file: {:?}", path);
            } else {
                println!("Settings file: (defaults only, not saved)");
            }
        }

        crate::SettingsAction::Set { key, value } => {
            let path = get_settings_path(None);
            let mut mgr = match std::fs::exists(&path) {
                Ok(true) => SettingsManager::from_file(std::path::Path::new(&path))?,
                _ => SettingsManager::load(),
            };

            let mut changed = false;
            match key.as_str() {
                "workspace_dir" => {
                    mgr.settings.workspace_dir = value.clone();
                    mgr.settings.data_dir = value.clone();
                    changed = true;
                }
                "database_url" => { mgr.settings.database_url = value.clone(); changed = true; }
                "docker_socket" => { mgr.settings.docker_socket = value.clone(); changed = true; }
                "openfoam_image" => { mgr.settings.openfoam_image = value.clone(); changed = true; }
                "max_concurrent_cases" => {
                    if let Ok(n) = value.parse() { mgr.settings.max_concurrent_cases = n; changed = true; }
                }
                "default_cores" => {
                    if let Ok(n) = value.parse() { mgr.settings.default_cores = n; changed = true; }
                }
                "default_memory_gb" => {
                    if let Ok(n) = value.parse() { mgr.settings.default_memory_gb = n; changed = true; }
                }
                "write_format" => {
                    match value.to_lowercase().as_str() {
                        "binary" => { mgr.settings.openfoam_format = OpenFOAMFormat::Binary; changed = true; }
                        "ascii" => { mgr.settings.openfoam_format = OpenFOAMFormat::Ascii; changed = true; }
                        _ => { println!("Invalid format: {}. Use 'binary' or 'ascii'.", value); }
                    }
                }
                "session_ttl_hours" => {
                    if let Ok(n) = value.parse() { mgr.settings.session_ttl_hours = n; changed = true; }
                }
                "allow_registration" => {
                    match value.to_lowercase().as_str() {
                        "true" | "yes" | "1" => { mgr.settings.allow_registration = true; changed = true; }
                        "false" | "no" | "0" => { mgr.settings.allow_registration = false; changed = true; }
                        _ => { println!("Invalid value. Use 'true' or 'false'."); }
                    }
                }
                _ => { println!("Unknown setting key: {}", key); }
            }

            if changed {
                mgr.save_to_file(std::path::Path::new(&path))?;
                println!("✓ Set {} = {}", key, value);
                println!("  Saved to: {}", path);
            }
        }

        crate::SettingsAction::Init { path } => {
            let root = path.clone().unwrap_or_else(|| {
                "/data".to_string()
            });

            let layout = WorkspaceManager::create_layout(&root)?;
            println!("\n=== AeroFlow Workspace Initialization ===\n");
            println!("  Root:           {}", root);
            println!("  Cases:          {}", layout.cases);
            println!("  Import:         {}", layout.import);
            println!("  Reports:        {}", layout.reports);
            println!("  Skills:         {}", layout.skills);
            println!("  Settings:       {}", layout.settings);
            println!("  Temp:           {}", layout.temp);
            println!("  Logs:           {}", layout.logs);
            println!();

            let settings = SettingsManager::load();
            let settings_file = WorkspaceManager::settings_file(&root);
            settings.save_to_file(std::path::Path::new(&settings_file))?;
            println!("✓ Workspace initialized at: {}", root);
            println!("  Settings saved to: {}", settings_file);
            println!("\n  Tip: Set AEROFLOW_WORKSPACE_DIR={} in your environment", root);
            println!("       or run: aeroflow settings set workspace_dir {}", root);
        }

        crate::SettingsAction::Reset => {
            let confirm = dialoguer::Confirm::new()
                .with_prompt("Reset all settings to defaults?")
                .default(false)
                .interact()?;

            if confirm {
                let mut mgr = SettingsManager::load();
                let s = AeroflowSettings::default();
                mgr.settings = s;
                let path = get_settings_path(None);
                mgr.save_to_file(std::path::Path::new(&path))?;
                println!("✓ Settings reset to defaults.");
            } else {
                println!("Cancelled.");
            }
        }

        crate::SettingsAction::Path => {
            let path = get_settings_path(None);
            println!("{}", path);
        }
    }

    Ok(())
}
