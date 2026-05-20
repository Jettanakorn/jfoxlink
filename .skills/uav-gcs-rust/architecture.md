# UAV GCS — Rust Architecture Reference

## Table of Contents
1. [Workspace Layout](#workspace)
2. [Actor Model with Tokio](#actor)
3. [Crate Topology](#crates)
4. [Real-Time Task Graph](#rt-graph)
5. [State Machine: Vehicle](#state-machine)
6. [Safety Patterns](#safety)
7. [Configuration System](#config)
8. [Plugin Architecture](#plugins)

---

## 1. Workspace Layout {#workspace}

```
gcs-workspace/
├── Cargo.toml                  # Workspace manifest
├── crates/
│   ├── gcs-core/               # Domain types, Vehicle state, events
│   ├── gcs-comms/              # MAVLink, UAVCAN, WebSocket transport
│   ├── gcs-telemetry/          # Telemetry ingestion, ring buffers, metrics
│   ├── gcs-planner/            # Mission planning, path algorithms
│   ├── gcs-geofence/           # Geofence engine, spatial constraints
│   ├── gcs-airspace/           # AIXM, NOTAM, TFR, AIP processing
│   ├── gcs-map/                # Cesium bridge, CZML generator, 3D tiles
│   ├── gcs-ui/                 # Tauri commands, IPC, frontend bridge
│   ├── gcs-db/                 # SQLite/PostGIS, migrations, queries
│   └── gcs-swarm/              # Multi-vehicle coordination, consensus
├── apps/
│   ├── gcs-desktop/            # Tauri v2 desktop app
│   └── gcs-headless/           # CLI / server mode for BVLOS ops
├── frontend/                   # CesiumJS + React/Svelte UI
└── config/
    ├── default.toml
    └── vehicles/               # Per-vehicle config profiles
```

### Root Cargo.toml
```toml
[workspace]
members = [
    "crates/*",
    "apps/*",
]
resolver = "2"

[workspace.dependencies]
tokio      = { version = "1.38", features = ["full"] }
serde      = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
anyhow     = "1.0"
thiserror  = "1.0"
tracing    = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
uuid       = { version = "1.8", features = ["v4", "serde"] }
chrono     = { version = "0.4", features = ["serde"] }
nalgebra   = "0.33"           # Linear algebra for navigation math
geo        = "0.28"           # Geometric types (Point, Polygon, LineString)
geojson    = "0.24"
mavlink    = { version = "0.14", features = ["common"] }
canadensis = "0.5"            # OpenCyphal / UAVCAN
rusqlite   = { version = "0.31", features = ["bundled", "column_decltype"] }
sqlx       = { version = "0.7", features = ["runtime-tokio", "sqlite", "postgres", "chrono", "uuid"] }
tauri      = { version = "2", features = [] }
axum       = "0.7"
tonic      = "0.11"
config     = "0.14"
```

---

## 2. Actor Model with Tokio {#actor}

GCS uses a message-passing actor model. Each subsystem owns a `tokio::task`, communicates
via `tokio::sync::mpsc` channels, and never shares mutable state across tasks.

```rust
// gcs-core/src/actor.rs

use tokio::sync::{mpsc, oneshot};

/// Generic actor handle — wraps an mpsc sender
pub struct Actor<M> {
    sender: mpsc::Sender<M>,
}

impl<M: Send + 'static> Actor<M> {
    pub async fn send(&self, msg: M) -> anyhow::Result<()> {
        self.sender.send(msg).await
            .map_err(|_| anyhow::anyhow!("Actor channel closed"))
    }
}

/// Request-response pattern via oneshot inside the message
pub struct Request<Req, Res> {
    pub payload: Req,
    pub reply: oneshot::Sender<Res>,
}

impl<Req, Res> Request<Req, Res> {
    pub fn new(payload: Req) -> (Self, oneshot::Receiver<Res>) {
        let (tx, rx) = oneshot::channel();
        (Self { payload, reply: tx }, rx)
    }
}
```

### Core Event Bus
```rust
// gcs-core/src/events.rs
use crate::types::{VehicleId, GeoPoint, Attitude, BatteryState, FlightMode};
use chrono::{DateTime, Utc};
use serde::{Serialize, Deserialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelemetryFrame {
    pub id: Uuid,
    pub vehicle_id: VehicleId,
    pub timestamp: DateTime<Utc>,
    pub position: GeoPoint,
    pub attitude: Attitude,
    pub battery: BatteryState,
    pub flight_mode: FlightMode,
    pub airspeed_mps: f32,
    pub groundspeed_mps: f32,
    pub altitude_msl_m: f64,
    pub altitude_agl_m: Option<f64>,
    pub gps_fix: GpsFix,
    pub hdop: f32,
    pub vdop: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GcsEvent {
    TelemetryReceived(TelemetryFrame),
    GeofenceViolation { vehicle_id: VehicleId, zone_id: Uuid },
    AirspaceAlert { vehicle_id: VehicleId, airspace_id: String, alert_type: AirspaceAlertType },
    MissionUploadComplete { vehicle_id: VehicleId, mission_id: Uuid },
    VehicleConnected(VehicleId),
    VehicleDisconnected(VehicleId),
    NotamReceived(crate::airspace::Notam),
    SwarmCommandIssued { swarm_id: Uuid, command: SwarmCommand },
}

// Broadcast bus using tokio broadcast channel
pub type EventBus = tokio::sync::broadcast::Sender<GcsEvent>;
```

---

## 3. Crate Topology {#crates}

### gcs-core — Domain Types
```rust
// gcs-core/src/types.rs
use nalgebra::{Vector3, Matrix3};
use serde::{Serialize, Deserialize};

pub type VehicleId = uuid::Uuid;

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct GeoPoint {
    pub lat: f64,   // degrees, WGS84
    pub lon: f64,   // degrees, WGS84
    pub alt: f64,   // meters MSL
}

impl GeoPoint {
    /// Convert to ECEF (Earth-Centered Earth-Fixed) coordinates
    pub fn to_ecef(&self) -> Vector3<f64> {
        const A: f64 = 6_378_137.0;          // WGS84 semi-major axis
        const E2: f64 = 6.694_379_990_14e-3; // first eccentricity squared
        let lat_r = self.lat.to_radians();
        let lon_r = self.lon.to_radians();
        let n = A / (1.0 - E2 * lat_r.sin().powi(2)).sqrt();
        Vector3::new(
            (n + self.alt) * lat_r.cos() * lon_r.cos(),
            (n + self.alt) * lat_r.cos() * lon_r.sin(),
            (n * (1.0 - E2) + self.alt) * lat_r.sin(),
        )
    }

    /// Great-circle distance in meters (Haversine)
    pub fn distance_to(&self, other: &GeoPoint) -> f64 {
        const R: f64 = 6_371_000.0;
        let dlat = (other.lat - self.lat).to_radians();
        let dlon = (other.lon - self.lon).to_radians();
        let a = (dlat / 2.0).sin().powi(2)
            + self.lat.to_radians().cos()
            * other.lat.to_radians().cos()
            * (dlon / 2.0).sin().powi(2);
        2.0 * R * a.sqrt().asin()
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Attitude {
    pub roll_deg: f32,
    pub pitch_deg: f32,
    pub yaw_deg: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FlightMode {
    Manual, Stabilize, AltHold, Loiter, Auto, Guided,
    Acro, Land, ReturnToLaunch, PosHold, Brake, Smart_RTL,
    SystemId(u32), // unknown/custom mode
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatteryState {
    pub voltage_v: f32,
    pub current_a: Option<f32>,
    pub remaining_pct: Option<u8>,
    pub consumed_mah: Option<f32>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum GpsFix { NoFix, Fix2D, Fix3D, DGps, RtkFloat, RtkFixed }
```

---

## 4. Real-Time Task Graph {#rt-graph}

```
[Serial/UDP RX] ──→ [MAVLink Parser] ──→ [Telemetry Actor]
                                               │
                    ┌──────────────────────────┼─────────────────────┐
                    ↓                          ↓                     ↓
             [Geofence Engine]      [Airspace Monitor]      [Map Bridge Actor]
                    │                          │                     │
                    └──────→ [Event Bus] ←─────┘           [Cesium IPC Channel]
                                   │
                    ┌──────────────┼───────────────┐
                    ↓              ↓               ↓
           [Alert Manager]   [DB Writer]    [gRPC/WS Relay]
```

**Priority Assignment (tokio task priority approximation):**
```rust
// High-priority task: telemetry ingestion + geofence
tokio::task::Builder::new()
    .name("telemetry-ingestion")
    .spawn(telemetry_loop(rx, event_bus.clone()))?;

// Medium-priority: airspace monitor (500ms polling acceptable)
tokio::task::Builder::new()
    .name("airspace-monitor")
    .spawn(airspace_monitor_loop(event_bus.clone(), db.clone()))?;

// Low-priority: DB write, logging
tokio::task::Builder::new()
    .name("db-writer")
    .spawn(db_writer_loop(event_rx, db.clone()))?;
```

---

## 5. Vehicle State Machine {#state-machine}

```rust
// gcs-core/src/vehicle_sm.rs
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum VehicleState {
    Offline,
    Connecting,
    Online { armed: bool },
    MissionActive { mission_id: uuid::Uuid },
    ReturnToHome,
    Landing,
    Emergency,
}

#[derive(Debug, Clone)]
pub enum VehicleTransition {
    Connected,
    Disconnected,
    Armed,
    Disarmed,
    MissionStarted(uuid::Uuid),
    MissionCompleted,
    RthTriggered,
    LandingInitiated,
    EmergencyDeclared(String),
    Recovered,
}

pub struct VehicleStateMachine {
    pub state: VehicleState,
    pub history: Vec<(chrono::DateTime<chrono::Utc>, VehicleState)>,
}

impl VehicleStateMachine {
    pub fn transition(&mut self, t: VehicleTransition) -> Result<(), String> {
        let next = match (&self.state, t) {
            (VehicleState::Offline, VehicleTransition::Connected) =>
                VehicleState::Connecting,
            (VehicleState::Connecting, VehicleTransition::Armed) =>
                VehicleState::Online { armed: true },
            (VehicleState::Online { .. }, VehicleTransition::MissionStarted(id)) =>
                VehicleState::MissionActive { mission_id: id },
            (VehicleState::MissionActive { .. }, VehicleTransition::RthTriggered) =>
                VehicleState::ReturnToHome,
            (VehicleState::ReturnToHome, VehicleTransition::LandingInitiated) =>
                VehicleState::Landing,
            (_, VehicleTransition::EmergencyDeclared(reason)) => {
                tracing::error!("EMERGENCY: {}", reason);
                VehicleState::Emergency
            }
            (_, VehicleTransition::Disconnected) =>
                VehicleState::Offline,
            (s, t) => return Err(format!("Invalid transition {:?} from {:?}", t, s)),
        };
        self.history.push((chrono::Utc::now(), self.state.clone()));
        self.state = next;
        Ok(())
    }
}
```

---

## 6. Safety Patterns {#safety}

### Never `unwrap()` on flight-critical paths
```rust
// BAD — will panic if telemetry malformed
let lat = frame.position.unwrap().lat;

// GOOD — explicit error path
let lat = frame.position
    .ok_or(TelemetryError::MissingPosition)?
    .lat;
```

### Watchdog Pattern
```rust
pub struct Watchdog {
    deadline: tokio::time::Instant,
    timeout: std::time::Duration,
    vehicle_id: VehicleId,
    event_bus: EventBus,
}

impl Watchdog {
    pub fn reset(&mut self) {
        self.deadline = tokio::time::Instant::now() + self.timeout;
    }

    pub async fn monitor(mut self) {
        loop {
            tokio::time::sleep_until(self.deadline).await;
            tracing::warn!("Watchdog timeout for vehicle {}", self.vehicle_id);
            let _ = self.event_bus.send(GcsEvent::VehicleDisconnected(self.vehicle_id));
            break;
        }
    }
}
```

### Ring Buffer for Telemetry History
```rust
use std::collections::VecDeque;

pub struct TelemetryRing {
    buffer: VecDeque<TelemetryFrame>,
    capacity: usize,
}

impl TelemetryRing {
    pub fn new(capacity: usize) -> Self {
        Self { buffer: VecDeque::with_capacity(capacity), capacity }
    }

    pub fn push(&mut self, frame: TelemetryFrame) {
        if self.buffer.len() == self.capacity {
            self.buffer.pop_front();
        }
        self.buffer.push_back(frame);
    }

    pub fn last_n(&self, n: usize) -> impl Iterator<Item = &TelemetryFrame> {
        self.buffer.iter().rev().take(n)
    }
}
```

---

## 7. Configuration System {#config}

```toml
# config/default.toml
[gcs]
name = "JFOX GCS"
version = "1.0.0"
max_vehicles = 32

[comms]
mavlink_udp_port = 14550
mavlink_serial_baud = 57600
heartbeat_interval_ms = 1000
connection_timeout_ms = 5000

[map]
cesium_ion_token = "${CESIUM_ION_TOKEN}"
default_lat = 18.7883  # Chiang Mai
default_lon = 98.9853
default_altitude_m = 2000.0
terrain_provider = "cesium_world_terrain"

[airspace]
notam_poll_interval_secs = 300
aixm_db_path = "./data/aixm_thailand.sqlite"
openairspace_api_url = "https://api.openairspace.org/v1"
faa_nasr_path = "./data/nasr"

[db]
url = "sqlite:./gcs_data.db"
max_connections = 10

[safety]
geofence_alert_margin_m = 50.0
watchdog_timeout_ms = 3000
max_telemetry_gap_ms = 2000
```

---

## 8. Plugin Architecture {#plugins}

Use Rust's trait objects for extensible vehicle backends and map providers:

```rust
// gcs-core/src/plugin.rs
use async_trait::async_trait;

#[async_trait]
pub trait VehicleBackend: Send + Sync {
    async fn connect(&mut self) -> anyhow::Result<()>;
    async fn disconnect(&mut self) -> anyhow::Result<()>;
    async fn send_command(&self, cmd: VehicleCommand) -> anyhow::Result<()>;
    async fn upload_mission(&self, mission: &Mission) -> anyhow::Result<()>;
    fn vehicle_type(&self) -> VehicleType;
}

#[async_trait]
pub trait AirspaceProvider: Send + Sync {
    async fn query_airspaces(&self, bbox: BoundingBox) -> anyhow::Result<Vec<Airspace>>;
    async fn fetch_notams(&self, icao_region: &str) -> anyhow::Result<Vec<Notam>>;
    fn provider_name(&self) -> &str;
}

pub struct PluginRegistry {
    vehicles: std::collections::HashMap<VehicleId, Box<dyn VehicleBackend>>,
    airspace_providers: Vec<Box<dyn AirspaceProvider>>,
}
```