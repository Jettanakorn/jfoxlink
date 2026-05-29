use aeroflow_core::{EventBus, SystemEvent};
use aeroflow_skills::{GeometryFingerprint, SkillsDb};
use notify::{Config, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::sync::watch;
use tracing::{error, info, warn};

/// State for deduplication: tracks recently imported STL files by path + mtime.
#[derive(Default)]
struct ImportTracker {
    seen: HashSet<(PathBuf, u64)>, // (canonical path, modified timestamp secs)
}

pub struct FileWatcher {
    event_bus: Option<EventBus>,
    db: Option<SkillsDb>,
    import_dir: PathBuf,
    workspace_dir: PathBuf,
    tracker: Arc<Mutex<ImportTracker>>,
    debounce: Duration,
}

impl FileWatcher {
    pub fn new(import_dir: PathBuf, workspace_dir: PathBuf) -> Self {
        Self {
            event_bus: None,
            db: None,
            import_dir,
            workspace_dir,
            tracker: Arc::new(Mutex::new(ImportTracker::default())),
            debounce: Duration::from_secs(2),
        }
    }

    pub fn with_event_bus(mut self, bus: EventBus) -> Self {
        self.event_bus = Some(bus);
        self
    }

    pub fn with_db(mut self, db: SkillsDb) -> Self {
        self.db = Some(db);
        self
    }

    /// Start watching in a background task. Returns a handle + shutdown sender.
    pub async fn start(
        self,
    ) -> tokio::sync::watch::Sender<bool> {
        let (shutdown_tx, shutdown_rx) = watch::channel(false);

        tokio::spawn(async move {
            // Ensure import directory exists
            std::fs::create_dir_all(&self.import_dir).ok();
            info!("File watcher monitoring {:?}", self.import_dir);

            let (tx, rx) = mpsc::channel::<Result<notify::Event, notify::Error>>();

            let mut watcher = match RecommendedWatcher::new(tx, Config::default()) {
                Ok(w) => w,
                Err(e) => {
                    error!("Failed to create file watcher: {}", e);
                    return;
                }
            };

            if let Err(e) = watcher.watch(&self.import_dir, RecursiveMode::Recursive) {
                error!("Failed to start watching {:?}: {}", self.import_dir, e);
                return;
            }

            // Scan existing files on startup
            self.scan_existing();

            // Event loop — run in spawn_blocking since mpsc is sync
            let tracker = self.tracker.clone();
            let debounce = self.debounce;
            let import_dir = self.import_dir.clone();
            let workspace_dir = self.workspace_dir.clone();
            let db = self.db.clone();
            let event_bus = self.event_bus.clone();

            tokio::task::spawn_blocking(move || {
                let mut last_event_time: Option<Instant> = None;

                for result in rx {
                    // Check shutdown
                    if *shutdown_rx.borrow() {
                        info!("File watcher shutting down gracefully");
                        break;
                    }

                    match result {
                        Ok(event) => {
                            // Debounce: ignore rapid repeated events
                            let now = Instant::now();
                            if let Some(last) = last_event_time
                                && now.duration_since(last) < debounce {
                                    continue;
                                }
                            last_event_time = Some(now);

                            // Only handle create/modify events
                            let is_import = matches!(
                                event.kind,
                                EventKind::Create(_) | EventKind::Modify(_)
                            );

                            if !is_import {
                                continue;
                            }

                            for path in &event.paths {
                                if path.extension().and_then(|e| e.to_str()) == Some("stl")
                                    && let Err(e) = Self::handle_stl_import(
                                        path, &import_dir, &workspace_dir, &db, &event_bus, &tracker,
                                    ) {
                                        warn!("STL import error for {:?}: {}", path, e);
                                    }
                            }
                        }
                        Err(e) => {
                            error!("File watch error: {}", e);
                        }
                    }
                }
                info!("File watcher stopped");
            });

            // Keep the watcher alive (it's moved into the closure via the mpsc channel)
            // Prevent dropping by holding it. Actually notify watcher is already
            // alive in the blocking task via the channel.
            let _ = watcher;
        });

        shutdown_tx
    }

    /// Scan import directory for existing STL files and import them
    fn scan_existing(&self) {
        let entries = match std::fs::read_dir(&self.import_dir) {
            Ok(e) => e,
            Err(_) => return,
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("stl") {
                info!("Found existing STL: {:?}", path);
                let _ = Self::handle_stl_import(
                    &path,
                    &self.import_dir,
                    &self.workspace_dir,
                    &self.db,
                    &self.event_bus,
                    &self.tracker,
                );
            }
        }
    }

    /// Handle a single STL file: fingerprint, deduplicate, insert, create case.
    fn handle_stl_import(
        path: &Path,
        _import_dir: &Path,
        workspace_dir: &Path,
        db: &Option<SkillsDb>,
        event_bus: &Option<EventBus>,
        tracker: &Arc<Mutex<ImportTracker>>,
    ) -> Result<(), anyhow::Error> {
        let canonical = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
        let mtime = std::fs::metadata(path)
            .and_then(|m| m.modified())
            .map(|t| t.duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs())
            .unwrap_or(0);

        // Deduplicate: skip if we've already imported this exact file
        {
            let mut tracker = tracker.lock().expect("poisoned");
            if !tracker.seen.insert((canonical.clone(), mtime)) {
                info!("Skipping already-imported STL: {:?}", path);
                return Ok(());
            }
        }

        info!("Importing STL: {:?}", path);

        // Compute fingerprint (SHA-256 + metrics)
        let fingerprint = GeometryFingerprint::from_stl(path)?;
        info!(
            "  Fingerprint: {} triangles, bbox={:.1}×{:.1}×{:.1}",
            fingerprint.num_triangles,
            fingerprint.bbox[3] - fingerprint.bbox[0],
            fingerprint.bbox[4] - fingerprint.bbox[1],
            fingerprint.bbox[5] - fingerprint.bbox[2],
        );

        // Emit event
        if let Some(bus) = event_bus {
            let _ = bus.send(SystemEvent::info(
                None,
                "import",
                format!("New STL detected: {:?} ({} triangles)", path, fingerprint.num_triangles),
            ));
        }

        // Persist to database
        if let Some(db) = db {
            let rt_handle = tokio::runtime::Handle::current();

            // Check for duplicate geometry by hash
            let geometry_id = match rt_handle.block_on(db.find_geometry_by_hash(&fingerprint.sha256_hash))? {
                Some(gid) => {
                    info!("  Geometry already exists: id={}", gid);
                    gid
                }
                None => {
                    let gid = rt_handle.block_on(db.insert_geometry(&fingerprint))?;
                    info!("  New geometry registered: id={}", gid);
                    gid
                }
            };

            // Generate case name from file name
            let stem = path.file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("imported");
            let case_name = format!("{}-{}", stem, chrono::Utc::now().format("%Y%m%d-%H%M%S"));

            let case_dir = workspace_dir.join("cases").join(&case_name);
            std::fs::create_dir_all(case_dir.join("0"))?;
            std::fs::create_dir_all(case_dir.join("constant"))?;
            std::fs::create_dir_all(case_dir.join("system"))?;
            std::fs::create_dir_all(case_dir.join("constant/triSurface"))?;
            std::fs::create_dir_all(case_dir.join("logs"))?;

            // Copy STL into case directory
            let stl_dest = case_dir.join("constant/triSurface").join(
                path.file_name().unwrap_or(std::ffi::OsStr::new("geometry.stl")),
            );
            std::fs::copy(path, &stl_dest)?;

            let solver = "simpleFoam";

            let case_id = rt_handle.block_on(db.create_case(
                &case_name,
                None,
                Some(geometry_id),
                solver,
                "External",
                &case_dir.to_string_lossy(),
            ))?;

            // Write manifest
            let manifest = serde_json::json!({
                "name": case_name,
                "case_id": case_id.to_string(),
                "geometry_id": geometry_id.to_string(),
                "solver": solver,
                "flow_type": "External",
                "auto_imported": true,
                "source_stl": path.to_string_lossy(),
                "created_at": chrono::Utc::now().to_rfc3339(),
            });
            std::fs::write(case_dir.join("manifest.json"), serde_json::to_string_pretty(&manifest)?)?;

            info!("  Case '{}' created (id={}) at {:?}", case_name, case_id, case_dir);

            if let Some(bus) = event_bus {
                let _ = bus.send(SystemEvent::info(
                    Some(case_id),
                    "import",
                    format!("Case '{}' auto-created from STL", case_name),
                ));
            }
        } else {
            info!("  STL imported (dry run, no DB): {:?}", path);
        }

        Ok(())
    }
}
