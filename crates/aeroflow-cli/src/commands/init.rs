use aeroflow_core::SettingsManager;
use aeroflow_skills::{GeometryFingerprint, SkillsDb};
use dialoguer::{Confirm, Input, Select};
use std::path::PathBuf;
use tracing::info;

pub async fn execute(name: Option<String>) -> anyhow::Result<()> {
    let case_name = match name {
        Some(n) => n,
        None => {
            Input::new()
                .with_prompt("Case name")
                .default("my-case".into())
                .interact_text()?
        }
    };

    let settings = SettingsManager::load();
    let workspace_root = if settings.settings.workspace_dir == "/data" || settings.settings.workspace_dir.is_empty() {
        let default_root = std::env::current_dir()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| "/data".to_string());

        let root: String = Input::new()
            .with_prompt("Workspace directory (all case data will be stored here)")
            .default(default_root)
            .interact_text()?;

        let layout = aeroflow_core::WorkspaceLayout::new(&root);
        layout.create_all()?;
        info!("Workspace initialized at: {}", root);
        root
    } else {
        settings.settings.workspace_dir.clone()
    };

    println!("\n=== AeroFlow Agent — New Case Setup ===\n");
    println!("  Workspace: {}", workspace_root);

    // --- STL Geometry Import ---
    let stl_path: String = Input::new()
        .with_prompt("STL geometry file path")
        .interact_text()?;

    let stl_path = PathBuf::from(&stl_path);
    if !stl_path.is_file() {
        anyhow::bail!("STL file not found: {:?}", stl_path);
    }

    println!("\n  Reading STL geometry...");
    let fingerprint = GeometryFingerprint::from_stl(&stl_path)?;
    println!("  Triangles: {}", fingerprint.num_triangles);
    println!("  Bounding box: [{:.3}, {:.3}, {:.3}] × [{:.3}, {:.3}, {:.3}]",
             fingerprint.bbox[0], fingerprint.bbox[1], fingerprint.bbox[2],
             fingerprint.bbox[3], fingerprint.bbox[4], fingerprint.bbox[5]);
    println!("  Surface area: {:.3}", fingerprint.surface_area);
    println!("  Volume: {:.3}", fingerprint.volume);

    // --- Persist to Database ---
    let db_url = settings.settings.database_url.clone();
    let db = SkillsDb::connect(&db_url).await?;

    let existing = db.find_geometry_by_hash(&fingerprint.sha256_hash).await?;
    let geometry_id = match existing {
        Some(gid) => {
            println!("  ✓ Geometry already registered (id={})", gid);
            gid
        }
        None => {
            let gid = db.insert_geometry(&fingerprint).await?;
            println!("  ✓ Geometry registered (id={})", gid);
            gid
        }
    };

    // --- Case Configuration ---
    let flow_opts = &["External", "Internal", "External (Digital Wind Tunnel)"];
    let flow_idx = Select::new()
        .with_prompt("Flow type?")
        .items(flow_opts)
        .default(0)
        .interact()?;

    let _fluid_idx = Select::new()
        .with_prompt("Fluid?")
        .items(&["Air", "Water", "Custom"])
        .default(0)
        .interact()?;

    let velocity: f64 = Input::new()
        .with_prompt("Velocity (m/s)")
        .default(50.0)
        .interact_text()?;

    let comp_opts = &["Incompressible", "Subsonic", "Transonic", "Supersonic", "Hypersonic"];
    let comp_idx = Select::new()
        .with_prompt("Compressibility?")
        .items(comp_opts)
        .default(0)
        .interact()?;

    let acc_opts = &["Draft", "Standard", "High", "Aerospace-grade"];
    let acc_idx = Select::new()
        .with_prompt("Accuracy level?")
        .items(acc_opts)
        .default(1)
        .interact()?;

    let solver = match comp_idx {
        0 | 1 => "simpleFoam",
        _ => "rhoCentralFoam",
    };

    let turb_model = match acc_idx {
        0 | 1 => "kOmegaSST",
        _ => "kOmegaSST (low-Re)",
    };

    let flow_type = flow_opts[flow_idx];

    // Digital Wind Tunnel configuration (optional)
    let wt_upstream: f64;
    let wt_downstream: f64;
    let wt_vertical: f64;
    let wt_lateral: f64;
    if flow_idx == 2 {
        println!("\n  ── Digital Wind Tunnel Configuration ──");
        wt_upstream = Input::new()
            .with_prompt("  Upstream distance (chord multiples)")
            .default(20.0)
            .interact_text()?;
        wt_downstream = Input::new()
            .with_prompt("  Downstream distance (chord multiples)")
            .default(40.0)
            .interact_text()?;
        wt_vertical = Input::new()
            .with_prompt("  Vertical half-height (chord multiples)")
            .default(25.0)
            .interact_text()?;
        wt_lateral = Input::new()
            .with_prompt("  Lateral half-width (chord multiples)")
            .default(25.0)
            .interact_text()?;
    } else {
        wt_upstream = 20.0;
        wt_downstream = 40.0;
        wt_vertical = 25.0;
        wt_lateral = 25.0;
    }

    println!("\n  Solver:          {solver}");
    println!("  Turbulence:      {turb_model}");
    println!("  Flow type:       {flow_type}");
    println!("  Velocity:        {velocity} m/s");
    println!("  Compressibility: {}", comp_opts[comp_idx]);
    println!("  Accuracy:        {}", acc_opts[acc_idx]);
    if flow_idx == 2 {
        println!("  Wind tunnel:     {}c up, {}c down, {}c vert, {}c lat",
            wt_upstream, wt_downstream, wt_vertical, wt_lateral);
    }
    println!();

    let confirm = Confirm::new()
        .with_prompt("Create case?")
        .default(true)
        .interact()?;

    if confirm {
        let case_dir = PathBuf::from(&workspace_root)
            .join("cases")
            .join(&case_name);

        std::fs::create_dir_all(case_dir.join("0"))?;
        std::fs::create_dir_all(case_dir.join("constant"))?;
        std::fs::create_dir_all(case_dir.join("system"))?;
        std::fs::create_dir_all(case_dir.join("constant/triSurface"))?;
        std::fs::create_dir_all(case_dir.join("logs"))?;

        // Copy STL into triSurface/
        let stl_dest = case_dir.join("constant/triSurface").join(
            stl_path.file_name().unwrap_or(std::ffi::OsStr::new("geometry.stl"))
        );
        std::fs::copy(&stl_path, &stl_dest)?;
        info!("STL copied to {:?}", stl_dest);

        let case_path_str = case_dir.to_string_lossy().to_string();

        // Create case record in database
        let case_id = db.create_case(
            &case_name,
            None,
            Some(geometry_id),
            solver,
            flow_type,
            &case_path_str,
        ).await?;
        info!("Case '{}' created in DB (id={})", case_name, case_id);

        // Build wind_tunnel config for manifest
        let wind_tunnel = if flow_idx == 2 {
            Some(serde_json::json!({
                "upstream": wt_upstream,
                "downstream": wt_downstream,
                "vertical": wt_vertical,
                "lateral": wt_lateral,
                "velocity_m_s": velocity,
            }))
        } else {
            None
        };

        // Save manifest
        let manifest = serde_json::json!({
            "name": case_name,
            "case_id": case_id.to_string(),
            "geometry_id": geometry_id.to_string(),
            "solver": solver,
            "turbulence_model": turb_model,
            "flow_type": flow_type,
            "velocity": velocity,
            "compressibility": comp_opts[comp_idx],
            "accuracy": acc_opts[acc_idx],
            "wind_tunnel": wind_tunnel,
            "workspace": workspace_root,
            "stl_file": stl_dest.to_string_lossy(),
            "created_at": chrono::Utc::now().to_rfc3339(),
        });
        std::fs::write(
            case_dir.join("manifest.json"),
            serde_json::to_string_pretty(&manifest)?,
        )?;

        info!("Case '{}' created at {:?}", case_name, case_dir);
        println!("\n✓ Case '{}' created successfully", case_name);
        println!("  Location: {:?}", case_dir);
        println!("  Case ID:  {}", case_id);
        println!("  Next: aeroflow run {:?}", case_dir);
    } else {
        println!("Cancelled.");
    }

    Ok(())
}
