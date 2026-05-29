# PX4 Flight Controller — GCS Integration Reference

## Table of Contents
1. [PX4 Identification & Autopilot Detection](#detection)
2. [PX4 Flight Modes](#flight-modes)
3. [MAVLink Connection to PX4](#mavlink-connection)
4. [uXRCE-DDS / micro-ROS Interface (PX4 v1.14+)](#uxrce-dds)
5. [Parameter Protocol](#parameters)
6. [Offboard Control Mode](#offboard)
7. [PX4-Specific Mission Items](#mission)
8. [Geofence Upload (PX4 style)](#geofence)
9. [ULOG Download & Parsing](#ulog)
10. [VTOL Support](#vtol)
11. [PX4 Cargo.toml Dependencies](#cargo)
12. [VehicleBackend trait implementation for PX4](#backend-impl)

---

## 1. PX4 Identification & Autopilot Detection {#detection}

PX4 identifies itself in the MAVLink `HEARTBEAT` message with:
```
autopilot == MAV_AUTOPILOT_PX4 (12)
```

Detect PX4 at connection time and select the correct backend:

```rust
// gcs-comms/src/autopilot_detect.rs
use mavlink::common::{MavMessage, MavAutopilot};

#[derive(Debug, Clone, PartialEq)]
pub enum AutopilotFirmware {
    Px4,
    ArduPilot,
    Unknown(u8),
}

pub fn detect_from_heartbeat(hb: &mavlink::common::HEARTBEAT_DATA) -> AutopilotFirmware {
    match hb.autopilot {
        MavAutopilot::MAV_AUTOPILOT_PX4       => AutopilotFirmware::Px4,
        MavAutopilot::MAV_AUTOPILOT_ARDUPILOTMEGA => AutopilotFirmware::ArduPilot,
        other => AutopilotFirmware::Unknown(other as u8),
    }
}
```

Register a one-shot listener on first HEARTBEAT before routing to the appropriate `VehicleBackend` implementation.

---

## 2. PX4 Flight Modes {#flight-modes}

PX4 encodes flight modes in `HEARTBEAT.custom_mode` (u32). Unlike ArduPilot, PX4 uses a
structured bitfield. Decode with the `PX4_CUSTOM_MAIN_MODE` enum. The top 8 bits are the
main mode; the lower 8 bits (when main mode is AUTO) are the sub-mode.

```rust
// gcs-core/src/px4_modes.rs

/// PX4 main mode — from top byte of custom_mode
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Px4MainMode {
    Manual      = 1,
    Altctl      = 2,
    Posctl      = 3,
    Auto        = 4,
    Acro        = 5,
    Offboard    = 6,
    Stabilized  = 7,
    Rattitude   = 8,
    Simple      = 9,  // deprecated
    Unknown(u8),
}

/// PX4 auto sub-mode — from second byte of custom_mode when main==Auto
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Px4AutoSubMode {
    Ready        = 1,
    Takeoff      = 2,
    Loiter       = 3,
    Mission      = 4,
    Rtl          = 5,
    Land         = 6,
    RtlUnderway  = 7,
    Idle         = 8,
    FollowTarget = 9,
    Precland     = 10,
    Orbit        = 11,
    Vtol_Takeoff = 12,
    Unknown(u8),
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Px4FlightMode {
    pub main: Px4MainMode,
    pub sub: Option<Px4AutoSubMode>,
}

pub fn decode_px4_custom_mode(custom_mode: u32) -> Px4FlightMode {
    let main_raw = ((custom_mode >> 16) & 0xFF) as u8;
    let sub_raw  = ((custom_mode >> 8)  & 0xFF) as u8;

    let main = match main_raw {
        1 => Px4MainMode::Manual,
        2 => Px4MainMode::Altctl,
        3 => Px4MainMode::Posctl,
        4 => Px4MainMode::Auto,
        5 => Px4MainMode::Acro,
        6 => Px4MainMode::Offboard,
        7 => Px4MainMode::Stabilized,
        8 => Px4MainMode::Rattitude,
        9 => Px4MainMode::Simple,
        n => Px4MainMode::Unknown(n),
    };

    let sub = if main == Px4MainMode::Auto {
        Some(match sub_raw {
            1  => Px4AutoSubMode::Ready,
            2  => Px4AutoSubMode::Takeoff,
            3  => Px4AutoSubMode::Loiter,
            4  => Px4AutoSubMode::Mission,
            5  => Px4AutoSubMode::Rtl,
            6  => Px4AutoSubMode::Land,
            7  => Px4AutoSubMode::RtlUnderway,
            8  => Px4AutoSubMode::Idle,
            9  => Px4AutoSubMode::FollowTarget,
            10 => Px4AutoSubMode::Precland,
            11 => Px4AutoSubMode::Orbit,
            12 => Px4AutoSubMode::Vtol_Takeoff,
            n  => Px4AutoSubMode::Unknown(n),
        })
    } else {
        None
    };

    Px4FlightMode { main, sub }
}

/// Map to GCS generic FlightMode for display
impl From<Px4FlightMode> for crate::core::types::FlightMode {
    fn from(m: Px4FlightMode) -> Self {
        match (&m.main, &m.sub) {
            (Px4MainMode::Manual,     _)                               => Self::Manual,
            (Px4MainMode::Stabilized, _)                               => Self::Stabilize,
            (Px4MainMode::Altctl,     _)                               => Self::AltHold,
            (Px4MainMode::Posctl,     _)                               => Self::Loiter,
            (Px4MainMode::Auto, Some(Px4AutoSubMode::Mission))         => Self::Auto,
            (Px4MainMode::Auto, Some(Px4AutoSubMode::Loiter))          => Self::Loiter,
            (Px4MainMode::Auto, Some(Px4AutoSubMode::Rtl))             => Self::ReturnToLaunch,
            (Px4MainMode::Auto, Some(Px4AutoSubMode::Land))            => Self::Land,
            (Px4MainMode::Auto, Some(Px4AutoSubMode::Takeoff))         => Self::Guided,
            (Px4MainMode::Offboard, _)                                 => Self::Guided,
            (Px4MainMode::Acro, _)                                     => Self::Acro,
            _                                                          => Self::SystemId(m.main as u32),
        }
    }
}
```

---

## 3. MAVLink Connection to PX4 {#mavlink-connection}

PX4's default MAVLink ports:
- **UDP 14550** — GCS link (QGroundControl default; use this)
- **UDP 14540** — companion computer link (onboard ROS2 bridge)
- **Serial** — TELEM1/TELEM2 ports at 57600 baud

PX4 sends `HEARTBEAT` at 1 Hz on every active link. It also sends `EXTENDED_SYS_STATE`
which carries VTOL state and landing gear — parse this alongside the standard heartbeat.

```rust
// gcs-comms/src/px4_connection.rs
use mavlink::common::{MavMessage, EXTENDED_SYS_STATE_DATA, MavVtolState, MavLandedState};

#[derive(Debug, Clone, Default)]
pub struct Px4ExtendedState {
    pub vtol_state:   MavVtolState,
    pub landed_state: MavLandedState,
}

pub fn parse_extended_sys_state(msg: &EXTENDED_SYS_STATE_DATA) -> Px4ExtendedState {
    Px4ExtendedState {
        vtol_state:   msg.vtol_state,
        landed_state: msg.landed_state,
    }
}

/// PX4 uses STATUSTEXT with severity levels — map to tracing levels
pub fn handle_statustext(st: &mavlink::common::STATUSTEXT_DATA) {
    let text: String = st.text.iter()
        .take_while(|&&c| c != 0)
        .map(|&c| c as char)
        .collect();
    use mavlink::common::MavSeverity::*;
    match st.severity {
        MAV_SEVERITY_EMERGENCY | MAV_SEVERITY_ALERT | MAV_SEVERITY_CRITICAL
            => tracing::error!("[PX4] {}", text),
        MAV_SEVERITY_ERROR
            => tracing::error!("[PX4] {}", text),
        MAV_SEVERITY_WARNING
            => tracing::warn!("[PX4] {}", text),
        MAV_SEVERITY_NOTICE | MAV_SEVERITY_INFO
            => tracing::info!("[PX4] {}", text),
        MAV_SEVERITY_DEBUG
            => tracing::debug!("[PX4] {}", text),
        _   => tracing::trace!("[PX4] {}", text),
    }
}
```

**PX4 mode-set command** (via `COMMAND_LONG` / `MAV_CMD_DO_SET_MODE`):

```rust
use mavlink::common::{MavMessage, COMMAND_LONG_DATA, MavCmd, MavMode};

/// Set PX4 to POSCTL (Position Control) mode
pub fn cmd_set_posctl() -> MavMessage {
    // param1 = MAV_MODE_FLAG_CUSTOM_MODE_ENABLED (1)
    // param2 = custom_mode upper word: main_mode=3 (POSCTL), sub_mode=0
    let custom_mode = (3u32 << 16) as f32; // POSCTL main mode
    MavMessage::COMMAND_LONG(COMMAND_LONG_DATA {
        param1: MavMode::MAV_MODE_FLAG_CUSTOM_MODE_ENABLED as i32 as f32,
        param2: custom_mode,
        param3: 0.0, param4: 0.0, param5: 0.0, param6: 0.0, param7: 0.0,
        command: MavCmd::MAV_CMD_DO_SET_MODE,
        target_system: 1,
        target_component: 1,
        confirmation: 0,
    })
}

/// Set PX4 to AUTO MISSION mode
pub fn cmd_set_auto_mission() -> MavMessage {
    // main_mode=4 (AUTO), sub_mode=4 (MISSION)
    let custom_mode = ((4u32 << 16) | (4u32 << 8)) as f32;
    MavMessage::COMMAND_LONG(COMMAND_LONG_DATA {
        param1: MavMode::MAV_MODE_FLAG_CUSTOM_MODE_ENABLED as i32 as f32,
        param2: custom_mode,
        param3: 0.0, param4: 0.0, param5: 0.0, param6: 0.0, param7: 0.0,
        command: MavCmd::MAV_CMD_DO_SET_MODE,
        target_system: 1,
        target_component: 1,
        confirmation: 0,
    })
}
```

---

## 4. uXRCE-DDS / micro-ROS Interface (PX4 v1.14+) {#uxrce-dds}

PX4 v1.14+ exposes a native **uXRCE-DDS** bridge that publishes uORB topics as ROS2/DDS
messages on the companion computer link (UDP 8888 by default, or serial).
This is the preferred high-bandwidth interface for autonomous operation.

Use **Zenoh** on the GCS side as a DDS-compatible bridge (simpler than raw DDS):

```toml
[dependencies]
zenoh        = { version = "0.11", features = ["transport_udp", "transport_serial"] }
zenoh-config = "0.11"
```

Key PX4 uXRCE-DDS topics (mapped to Zenoh key expressions):

| uORB Topic | Zenoh Key | Rate |
|---|---|---|
| `vehicle_global_position` | `/fmu/out/vehicle_global_position` | 5–50 Hz |
| `vehicle_local_position` | `/fmu/out/vehicle_local_position` | 50 Hz |
| `vehicle_attitude` | `/fmu/out/vehicle_attitude` | 50–250 Hz |
| `vehicle_status` | `/fmu/out/vehicle_status` | 1 Hz |
| `battery_status` | `/fmu/out/battery_status` | 1 Hz |
| `sensor_combined` | `/fmu/out/sensor_combined` | 250 Hz |
| `vehicle_gps_position` | `/fmu/out/vehicle_gps_position` | 5 Hz |
| `TrajectorySetpoint` (in) | `/fmu/in/TrajectorySetpoint` | ≤50 Hz |
| `VehicleCommand` (in) | `/fmu/in/VehicleCommand` | on demand |

```rust
// gcs-comms/src/px4_uxrce.rs
use zenoh::prelude::r#async::*;
use serde::{Deserialize, Serialize};

/// Mirrors PX4's vehicle_global_position uORB struct (CDR-encoded via DDS)
/// Use the px4_msgs crate or define manually matching PX4's IDL
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct VehicleGlobalPosition {
    pub timestamp: u64,       // microseconds since boot
    pub lat: f64,             // degrees, WGS84
    pub lon: f64,             // degrees, WGS84
    pub alt: f32,             // meters MSL
    pub alt_ellipsoid: f32,   // meters above WGS84 ellipsoid
    pub delta_alt: f32,
    pub lat_lon_reset_counter: u8,
    pub alt_reset_counter: u8,
    pub eph: f32,             // horizontal position uncertainty (m)
    pub epv: f32,             // vertical position uncertainty (m)
    pub terrain_alt: f32,
    pub terrain_alt_valid: bool,
    pub dead_reckoning: bool,
}

pub async fn subscribe_px4_position(
    event_bus: crate::core::events::EventBus,
    vehicle_id: crate::core::types::VehicleId,
) -> anyhow::Result<()> {
    let session = zenoh::open(zenoh::config::default()).res().await
        .map_err(|e| anyhow::anyhow!("Zenoh open failed: {}", e))?;

    let sub = session
        .declare_subscriber("/fmu/out/vehicle_global_position")
        .res().await
        .map_err(|e| anyhow::anyhow!("Zenoh subscribe failed: {}", e))?;

    tracing::info!("PX4 uXRCE-DDS position subscriber active");

    loop {
        match sub.recv_async().await {
            Ok(sample) => {
                // Deserialize CDR payload from sample.value.payload
                let payload = sample.value.payload.contiguous();
                // Skip 4-byte CDR header, then deserialize
                if payload.len() > 4 {
                    if let Ok(pos) = cdr::deserialize::<VehicleGlobalPosition>(&payload[4..]) {
                        let frame = build_telemetry_frame(vehicle_id, &pos);
                        let _ = event_bus.send(
                            crate::core::events::GcsEvent::TelemetryReceived(frame)
                        );
                    }
                }
            }
            Err(e) => {
                tracing::error!("Zenoh receive error: {}", e);
                break;
            }
        }
    }
    Ok(())
}

fn build_telemetry_frame(
    vehicle_id: crate::core::types::VehicleId,
    pos: &VehicleGlobalPosition,
) -> crate::core::events::TelemetryFrame {
    crate::core::events::TelemetryFrame {
        id: uuid::Uuid::new_v4(),
        vehicle_id,
        timestamp: chrono::Utc::now(),
        position: crate::core::types::GeoPoint {
            lat: pos.lat,
            lon: pos.lon,
            alt: pos.alt as f64,
        },
        ..Default::default()
    }
}
```

Add CDR serialization support:
```toml
cdr = "0.2"   # CDR (Common Data Representation) for DDS messages
```

---

## 5. Parameter Protocol {#parameters}

PX4 uses the standard MAVLink parameter protocol. Critical PX4 parameters for GCS:

| Parameter | Description | Typical Value |
|---|---|---|
| `SYS_AUTOSTART` | Airframe ID | 4001 (quadrotor) |
| `COM_RC_LOSS_T` | RC loss timeout (s) | 0.5 |
| `COM_DL_LOSS_T` | Datalink loss timeout (s) | 10.0 |
| `NAV_ACC_RAD` | Waypoint acceptance radius (m) | 10.0 |
| `MIS_TAKEOFF_ALT` | Default takeoff altitude (m) | 5.0 |
| `RTL_RETURN_ALT` | RTL return altitude (m) | 30.0 |
| `GF_ACTION` | Geofence breach action | 1 (warning) / 2 (hold) / 3 (RTL) |
| `GF_MAX_HOR_DIST` | Max horizontal geofence dist (m) | 600.0 |
| `GF_MAX_VER_DIST` | Max vertical geofence altitude (m) | 200.0 |
| `EKF2_GPS_MASK` | EKF2 GPS aiding mask | 7 |

```rust
// gcs-comms/src/px4_params.rs
use mavlink::common::{MavMessage, PARAM_REQUEST_LIST_DATA, PARAM_SET_DATA, MavParamType};
use std::collections::HashMap;

pub struct Px4ParamStore {
    params: HashMap<String, f32>,
    pending: std::collections::HashSet<String>,
}

impl Px4ParamStore {
    pub fn new() -> Self {
        Self { params: HashMap::new(), pending: Default::default() }
    }

    pub fn request_all(target_system: u8) -> MavMessage {
        MavMessage::PARAM_REQUEST_LIST(PARAM_REQUEST_LIST_DATA {
            target_system,
            target_component: 1,
        })
    }

    pub fn handle_param_value(
        &mut self,
        pv: &mavlink::common::PARAM_VALUE_DATA,
    ) {
        let name: String = pv.param_id.iter()
            .take_while(|&&c| c != 0)
            .map(|&c| c as char)
            .collect();
        self.params.insert(name.clone(), pv.param_value);
        self.pending.remove(&name);
        tracing::debug!("PX4 param {} = {} ({}/{})", name, pv.param_value, pv.param_index + 1, pv.param_count);
    }

    pub fn set_param(name: &str, value: f32, param_type: MavParamType) -> MavMessage {
        let mut param_id = [0i8; 16];
        for (i, c) in name.chars().take(16).enumerate() {
            param_id[i] = c as i8;
        }
        MavMessage::PARAM_SET(PARAM_SET_DATA {
            param_value: value,
            target_system: 1,
            target_component: 1,
            param_id,
            param_type,
        })
    }

    pub fn get(&self, name: &str) -> Option<f32> {
        self.params.get(name).copied()
    }

    pub fn is_complete(&self) -> bool {
        self.pending.is_empty()
    }
}
```

---

## 6. Offboard Control Mode {#offboard}

PX4 Offboard mode allows the GCS (or companion) to stream position/velocity setpoints.
**Critical:** PX4 requires setpoints at ≥2 Hz or it exits Offboard mode automatically.

```rust
// gcs-comms/src/px4_offboard.rs
use mavlink::common::{
    MavMessage, SET_POSITION_TARGET_LOCAL_NED_DATA,
    SET_ATTITUDE_TARGET_DATA, MavFrame,
    PositionTargetTypemask,
};

/// Stream a NED position setpoint at ≥2 Hz
/// type_mask: bitmask of IGNORED fields — set bits to IGNORE, clear to USE
pub fn position_setpoint_ned(
    north_m: f32, east_m: f32, down_m: f32,
    yaw_rad: f32,
) -> MavMessage {
    MavMessage::SET_POSITION_TARGET_LOCAL_NED(SET_POSITION_TARGET_LOCAL_NED_DATA {
        time_boot_ms: 0,
        target_system: 1,
        target_component: 1,
        coordinate_frame: MavFrame::MAV_FRAME_LOCAL_NED,
        // Ignore velocity, acceleration, yaw_rate — use position + yaw only
        type_mask: (PositionTargetTypemask::POSITION_TARGET_TYPEMASK_VX_IGNORE
                  | PositionTargetTypemask::POSITION_TARGET_TYPEMASK_VY_IGNORE
                  | PositionTargetTypemask::POSITION_TARGET_TYPEMASK_VZ_IGNORE
                  | PositionTargetTypemask::POSITION_TARGET_TYPEMASK_AX_IGNORE
                  | PositionTargetTypemask::POSITION_TARGET_TYPEMASK_AY_IGNORE
                  | PositionTargetTypemask::POSITION_TARGET_TYPEMASK_AZ_IGNORE
                  | PositionTargetTypemask::POSITION_TARGET_TYPEMASK_YAW_RATE_IGNORE).bits(),
        x: north_m, y: east_m, z: down_m,
        vx: 0.0, vy: 0.0, vz: 0.0,
        afx: 0.0, afy: 0.0, afz: 0.0,
        yaw: yaw_rad,
        yaw_rate: 0.0,
    })
}

/// Heartbeat-style offboard keepalive task — must run at ≥2 Hz
pub async fn offboard_keepalive_loop(
    socket: std::sync::Arc<tokio::net::UdpSocket>,
    target_addr: std::net::SocketAddr,
    setpoint_rx: tokio::sync::watch::Receiver<(f32, f32, f32, f32)>,
) -> anyhow::Result<()> {
    let mut interval = tokio::time::interval(std::time::Duration::from_millis(100)); // 10 Hz
    loop {
        interval.tick().await;
        let (north, east, down, yaw) = *setpoint_rx.borrow();
        let msg = position_setpoint_ned(north, east, down, yaw);
        let mut buf = Vec::new();
        mavlink::write_v2_msg(&mut buf, mavlink::MavHeader {
            system_id: 255, component_id: 0, sequence: 0,
        }, &msg)?;
        socket.send_to(&buf, target_addr).await?;
    }
}
```

**To enter Offboard mode** — send at least one setpoint BEFORE mode switch, then:
```rust
// Switch to Offboard (main_mode=6)
let custom_mode = (6u32 << 16) as f32;
let cmd = cmd_set_mode_custom(custom_mode);
```

---

## 7. PX4-Specific Mission Items {#mission}

PX4 supports the standard MAVLink mission protocol (see `rust-protocols.md` §8) with these
PX4-specific `MAV_CMD` codes worth knowing:

| MAV_CMD | Code | Use |
|---|---|---|
| `MAV_CMD_NAV_WAYPOINT` | 16 | Standard waypoint with hold time |
| `MAV_CMD_NAV_TAKEOFF` | 22 | Takeoff to altitude (param7 = alt_m) |
| `MAV_CMD_NAV_LAND` | 21 | Land at current position |
| `MAV_CMD_NAV_RETURN_TO_LAUNCH` | 20 | RTL |
| `MAV_CMD_NAV_LOITER_TIME` | 19 | Loiter for N seconds |
| `MAV_CMD_DO_CHANGE_SPEED` | 178 | param1=0 (airspeed), param2=speed_mps |
| `MAV_CMD_DO_SET_RELAY` | 181 | Trigger payload relay |
| `MAV_CMD_DO_DIGICAM_CONTROL` | 203 | Camera shutter trigger |
| `MAV_CMD_DO_VTOL_TRANSITION` | 3000 | VTOL ↔ FW transition (param1=3=FW, 4=MC) |

PX4 mission IMPORTANT notes:
- First item should always be `MAV_CMD_NAV_TAKEOFF` for multicopters
- Use `MAV_FRAME_GLOBAL_RELATIVE_ALT_INT` for all altitude values (relative to home)
- PX4 respects `param1` of `WAYPOINT` as minimum loiter time at waypoint
- PX4 will reject missions if `MIS_TAKEOFF_ALT` < first waypoint alt when no explicit TAKEOFF item

---

## 8. Geofence Upload (PX4 style) {#geofence}

PX4 v1.13+ supports MAVLink fence protocol (`FENCE_FETCH_POINT`, `FENCE_POINT`, `FENCE_STATUS`).
PX4 also reads geofences from the SD card as GeoJSON-like structures.

For MAVLink-based fence upload:

```rust
// gcs-comms/src/px4_fence.rs
use mavlink::common::{MavMessage, FENCE_FETCH_POINT_DATA, FENCE_POINT_DATA};

/// Upload a circular geofence centered on home position
/// PX4 GF_ACTION parameter controls breach behavior
pub async fn upload_circular_fence(
    socket: &tokio::net::UdpSocket,
    target_addr: &str,
    center_lat: f32,
    center_lon: f32,
    radius_m: f32,
) -> anyhow::Result<()> {
    // PX4 circular fence: single point with radius
    let fence_pt = MavMessage::FENCE_POINT(FENCE_POINT_DATA {
        lat: center_lat,
        lng: center_lon,
        radius: radius_m,
        count: 1,
        idx: 0,
        target_system: 1,
        target_component: 1,
    });
    let mut buf = Vec::new();
    mavlink::write_v2_msg(&mut buf, mavlink::MavHeader {
        system_id: 255, component_id: 0, sequence: 0,
    }, &fence_pt)?;
    socket.send_to(&buf, target_addr).await?;
    tracing::info!("PX4 circular geofence uploaded: radius={}m", radius_m);
    Ok(())
}

/// Parse FENCE_STATUS to check for breach
pub fn handle_fence_status(
    fs: &mavlink::common::FENCE_STATUS_DATA,
    vehicle_id: crate::core::types::VehicleId,
    event_bus: &crate::core::events::EventBus,
) {
    if fs.breach_status != 0 {
        tracing::warn!("PX4 geofence breach: type={:?}", fs.breach_type);
        let _ = event_bus.send(crate::core::events::GcsEvent::GeofenceViolation {
            vehicle_id,
            zone_id: uuid::Uuid::nil(), // PX4 doesn't give a zone UUID
        });
    }
}
```

---

## 9. ULOG Download & Parsing {#ulog}

PX4 uses the **ULOG** binary format for flight logs. Download via MAVLink:

```rust
// gcs-comms/src/px4_log_download.rs
use mavlink::common::{
    MavMessage, LOG_REQUEST_LIST_DATA, LOG_REQUEST_DATA,
    LOG_ENTRY_DATA, LOG_DATA_DATA,
};
use std::path::PathBuf;
use tokio::io::AsyncWriteExt;

pub struct Px4LogDownloader {
    log_list: Vec<LogEntry>,
}

#[derive(Debug, Clone)]
pub struct LogEntry {
    pub id:       u16,
    pub size:     u32,
    pub time_utc: u32,
    pub num_logs: u16,
    pub last_log_num: u16,
}

impl Px4LogDownloader {
    /// Step 1: Request log list
    pub fn request_list() -> MavMessage {
        MavMessage::LOG_REQUEST_LIST(LOG_REQUEST_LIST_DATA {
            start: 0,
            end: 0xFFFF,
            target_system: 1,
            target_component: 1,
        })
    }

    pub fn handle_log_entry(&mut self, entry: &LOG_ENTRY_DATA) {
        self.log_list.push(LogEntry {
            id: entry.id,
            size: entry.size,
            time_utc: entry.time_utc,
            num_logs: entry.num_logs,
            last_log_num: entry.last_log_num,
        });
        tracing::info!("Log #{}: {} bytes, UTC={}", entry.id, entry.size, entry.time_utc);
    }

    /// Step 2: Request specific log by ID
    pub fn request_log(id: u16, offset: u32, count: u32) -> MavMessage {
        MavMessage::LOG_REQUEST_DATA(LOG_REQUEST_DATA {
            id,
            ofs: offset,
            count,
            target_system: 1,
            target_component: 1,
        })
    }

    /// Step 3: Accumulate LOG_DATA packets into file
    pub async fn handle_log_data(
        file: &mut tokio::fs::File,
        data: &LOG_DATA_DATA,
    ) -> anyhow::Result<()> {
        let chunk = &data.data[..data.count as usize];
        file.write_all(chunk).await?;
        Ok(())
    }
}
```

Parse ULOG files with the `ulog-rs` crate (or stream-parse for real-time analysis):
```toml
ulog-rs = "0.3"   # ULOG binary format parser
```

---

## 10. VTOL Support {#vtol}

For PX4 VTOL vehicles (relevant for JFOX fixed-wing/multirotor hybrid UAVs):

```rust
// Parse VTOL state from EXTENDED_SYS_STATE
use mavlink::common::MavVtolState;

pub fn describe_vtol_state(state: MavVtolState) -> &'static str {
    match state {
        MavVtolState::MAV_VTOL_STATE_UNDEFINED     => "undefined",
        MavVtolState::MAV_VTOL_STATE_TRANSITION_TO_FW => "transitioning → fixed-wing",
        MavVtolState::MAV_VTOL_STATE_TRANSITION_TO_MC => "transitioning → multicopter",
        MavVtolState::MAV_VTOL_STATE_MC            => "multicopter",
        MavVtolState::MAV_VTOL_STATE_FW            => "fixed-wing",
        _                                          => "unknown",
    }
}

/// Command VTOL transition
pub fn cmd_vtol_transition(to_fixed_wing: bool) -> mavlink::common::MavMessage {
    use mavlink::common::{MavMessage, COMMAND_LONG_DATA, MavCmd, MavVtolState};
    let target_state = if to_fixed_wing {
        MavVtolState::MAV_VTOL_STATE_FW as u8 as f32
    } else {
        MavVtolState::MAV_VTOL_STATE_MC as u8 as f32
    };
    MavMessage::COMMAND_LONG(COMMAND_LONG_DATA {
        param1: target_state,
        param2: 0.0, param3: 0.0, param4: 0.0,
        param5: 0.0, param6: 0.0, param7: 0.0,
        command: MavCmd::MAV_CMD_DO_VTOL_TRANSITION,
        target_system: 1,
        target_component: 1,
        confirmation: 0,
    })
}
```

---

## 11. PX4 Cargo.toml Dependencies {#cargo}

```toml
# Add to workspace Cargo.toml [workspace.dependencies]
mavlink    = { version = "0.14", features = ["common", "async"] }  # existing
zenoh      = { version = "0.11", features = ["transport_udp"] }    # uXRCE-DDS bridge
cdr        = "0.2"     # CDR serialization for DDS payloads
ulog-rs    = "0.3"     # PX4 ULOG flight log parser

# Add to gcs-comms/Cargo.toml
[dependencies]
mavlink.workspace = true
zenoh.workspace   = true
cdr.workspace     = true
ulog-rs.workspace = true
```

---

## 12. VehicleBackend Trait Implementation for PX4 {#backend-impl}

Wire PX4 into the GCS plugin architecture (see `architecture.md` §8):

```rust
// gcs-comms/src/px4_backend.rs
use async_trait::async_trait;
use crate::core::plugin::{VehicleBackend, VehicleType};
use crate::core::types::{VehicleCommand, Mission};

pub struct Px4Backend {
    socket: std::sync::Arc<tokio::net::UdpSocket>,
    target_addr: std::net::SocketAddr,
    param_store: crate::px4_params::Px4ParamStore,
    extended_state: crate::px4_connection::Px4ExtendedState,
}

#[async_trait]
impl VehicleBackend for Px4Backend {
    async fn connect(&mut self) -> anyhow::Result<()> {
        // Send initial param request to confirm link
        let msg = crate::px4_params::Px4ParamStore::request_all(1);
        self.send_mavlink(&msg).await?;
        tracing::info!("PX4 backend connected to {}", self.target_addr);
        Ok(())
    }

    async fn disconnect(&mut self) -> anyhow::Result<()> {
        tracing::info!("PX4 backend disconnecting");
        Ok(())
    }

    async fn send_command(&self, cmd: VehicleCommand) -> anyhow::Result<()> {
        use VehicleCommand::*;
        let mav_msg = match cmd {
            Arm                  => self.arm_disarm_cmd(true),
            Disarm               => self.arm_disarm_cmd(false),
            ReturnToHome         => self.set_mode_auto_rtl(),
            Land                 => self.set_mode_auto_land(),
            SetMode(mode_str)    => self.mode_from_str(&mode_str)?,
            Takeoff { alt_m }    => self.takeoff_cmd(alt_m),
        };
        self.send_mavlink(&mav_msg).await
    }

    async fn upload_mission(&self, mission: &Mission) -> anyhow::Result<()> {
        // Delegate to MissionUploader from rust-protocols.md §8
        // Prepend TAKEOFF item if first item is not already TAKEOFF
        let items = self.prepare_px4_mission(mission);
        let mut uploader = crate::mission_uploader::MissionUploader::new(
            self.socket.clone(), self.target_addr,
        );
        uploader.upload(&items).await
    }

    fn vehicle_type(&self) -> VehicleType {
        VehicleType::Px4
    }
}

impl Px4Backend {
    fn arm_disarm_cmd(&self, arm: bool) -> mavlink::common::MavMessage {
        use mavlink::common::{MavMessage, COMMAND_LONG_DATA, MavCmd};
        MavMessage::COMMAND_LONG(COMMAND_LONG_DATA {
            param1: if arm { 1.0 } else { 0.0 },
            param2: 0.0, param3: 0.0, param4: 0.0,
            param5: 0.0, param6: 0.0, param7: 0.0,
            command: MavCmd::MAV_CMD_COMPONENT_ARM_DISARM,
            target_system: 1,
            target_component: 1,
            confirmation: 0,
        })
    }

    fn prepare_px4_mission(&self, mission: &Mission) -> Vec<crate::planner::MissionItem> {
        let mut items = mission.items.clone();
        // PX4 requires TAKEOFF as first item for multicopters
        if !items.first().map(|i| i.is_takeoff).unwrap_or(false) {
            let home = mission.home_position;
            items.insert(0, crate::planner::MissionItem {
                is_takeoff: true,
                lat: home.lat,
                lon: home.lon,
                alt_m: self.param_store.get("MIS_TAKEOFF_ALT").unwrap_or(5.0),
                ..Default::default()
            });
        }
        items
    }

    async fn send_mavlink(&self, msg: &mavlink::common::MavMessage) -> anyhow::Result<()> {
        let mut buf = Vec::new();
        mavlink::write_v2_msg(&mut buf, mavlink::MavHeader {
            system_id: 255, component_id: 0, sequence: 0,
        }, msg)?;
        self.socket.send_to(&buf, self.target_addr).await?;
        Ok(())
    }
}
```

---

## PX4 vs ArduPilot Quick Comparison

| Feature | PX4 | ArduPilot |
|---|---|---|
| Autopilot ID in HEARTBEAT | `MAV_AUTOPILOT_PX4` (12) | `MAV_AUTOPILOT_ARDUPILOTMEGA` (3) |
| Flight mode encoding | `custom_mode` bitfield (main+sub) | `custom_mode` flat enum |
| Primary hi-rate interface | uXRCE-DDS (v1.14+) | MAVLink over serial/UDP |
| VTOL support | Native (`MAV_CMD_DO_VTOL_TRANSITION`) | Limited (Plane/Copter combo) |
| Offboard control | `SET_POSITION_TARGET_LOCAL_NED` at ≥2 Hz | `GUIDED` mode + `SET_POSITION_TARGET` |
| Log format | ULOG (.ulg) | DataFlash (.bin) |
| Geofence upload | `FENCE_POINT` protocol | `FENCE_FETCH_POINT` protocol |
| Parameter protocol | Standard MAVLink PARAM_* | Standard MAVLink PARAM_* |
| Mission protocol | Standard MAVLink + requires TAKEOFF item | Standard MAVLink |