# Rust Protocols Reference — MAVLink, UAVCAN, DDS-XRCE, WebSocket

## Table of Contents
1. [MAVLink 2.0 Stack](#mavlink)
2. [UAVCAN / OpenCyphal (CAN Bus)](#uavcan)
3. [DDS-XRCE (micro-ROS)](#dds)
4. [WebSocket Telemetry Relay](#websocket)
5. [gRPC Service Definitions](#grpc)
6. [Serial / UDP Transport Abstraction](#transport)
7. [Telemetry Deserialization Pipeline](#telemetry-pipeline)
8. [MAVLink Mission Upload](#mission-upload)
9. [Swarm Command Protocol](#swarm)

---

## 1. MAVLink 2.0 Stack {#mavlink}

```toml
# gcs-comms/Cargo.toml
[dependencies]
mavlink = { version = "0.14", features = ["common", "ardupilotmega", "async"] }
tokio   = { version = "1", features = ["full"] }
tokio-serial = "5.4"
```

```rust
// gcs-comms/src/mavlink_connection.rs
use mavlink::{
    MavlinkVersion, Message,
    common::{MavMessage, HEARTBEAT_DATA, GLOBAL_POSITION_INT_DATA,
             ATTITUDE_DATA, SYS_STATUS_DATA, BATTERY_STATUS_DATA,
             GPS_RAW_INT_DATA, VFR_HUD_DATA, STATUSTEXT_DATA},
};
use tokio::sync::broadcast;
use tokio_serial::SerialPortBuilderExt;

pub struct MavlinkConnection {
    vehicle_id: crate::core::types::VehicleId,
    event_bus: broadcast::Sender<crate::core::events::GcsEvent>,
    config: MavlinkConfig,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct MavlinkConfig {
    pub connection_type: ConnectionType,
    pub baud_rate: u32,
    pub system_id: u8,    // GCS system ID (typically 255)
    pub component_id: u8, // GCS component (typically 0)
}

#[derive(Debug, Clone, serde::Deserialize)]
pub enum ConnectionType {
    Serial { port: String },
    Udp    { host: String, port: u16 },
    Tcp    { host: String, port: u16 },
}

impl MavlinkConnection {
    pub async fn connect_udp(
        &self,
        bind_addr: &str,
    ) -> anyhow::Result<()> {
        let socket = tokio::net::UdpSocket::bind(bind_addr).await?;
        tracing::info!("MAVLink UDP listening on {}", bind_addr);

        let mut buf = [0u8; 280]; // Max MAVLink 2 packet size
        let event_bus = self.event_bus.clone();
        let vehicle_id = self.vehicle_id;

        loop {
            match socket.recv_from(&mut buf).await {
                Ok((n, _addr)) => {
                    if let Ok((_, msg)) = mavlink::read_v2_msg::<MavMessage, _>(&mut &buf[..n]) {
                        if let Some(event) = Self::mavlink_to_event(vehicle_id, &msg) {
                            let _ = event_bus.send(event);
                        }
                    }
                }
                Err(e) => {
                    tracing::error!("UDP receive error: {}", e);
                    break;
                }
            }
        }
        Ok(())
    }

    fn mavlink_to_event(
        vehicle_id: crate::core::types::VehicleId,
        msg: &MavMessage,
    ) -> Option<crate::core::events::GcsEvent> {
        match msg {
            MavMessage::GLOBAL_POSITION_INT(gpi) => {
                use crate::core::types::{GeoPoint, TelemetryFrame};
                // GPI lat/lon are in 1e7 degrees
                let frame = TelemetryFrame {
                    id: uuid::Uuid::new_v4(),
                    vehicle_id,
                    timestamp: chrono::Utc::now(),
                    position: GeoPoint {
                        lat: gpi.lat as f64 / 1e7,
                        lon: gpi.lon as f64 / 1e7,
                        alt: gpi.alt as f64 / 1000.0, // mm → m MSL
                    },
                    altitude_agl_m: if gpi.relative_alt >= 0 {
                        Some(gpi.relative_alt as f64 / 1000.0)
                    } else { None },
                    groundspeed_mps: ((gpi.vx.pow(2) + gpi.vy.pow(2)) as f64).sqrt() / 100.0,
                    // Fill remaining fields from other messages
                    ..Default::default()
                };
                Some(crate::core::events::GcsEvent::TelemetryReceived(frame))
            }
            MavMessage::STATUSTEXT(st) => {
                let text: String = st.text.iter()
                    .take_while(|&&c| c != 0)
                    .map(|&c| c as char)
                    .collect();
                tracing::info!("[Vehicle {}] STATUS: {}", vehicle_id, text);
                None
            }
            _ => None,
        }
    }

    /// Send a MAVLink command long
    pub async fn send_command(
        &self,
        socket: &tokio::net::UdpSocket,
        target_addr: &str,
        command: u16,
        params: [f32; 7],
    ) -> anyhow::Result<()> {
        use mavlink::common::{COMMAND_LONG_DATA, MavCmd};
        let msg = MavMessage::COMMAND_LONG(COMMAND_LONG_DATA {
            param1: params[0], param2: params[1],
            param3: params[2], param4: params[3],
            param5: params[4], param6: params[5],
            param7: params[6],
            command: unsafe { std::mem::transmute(command as u32) },
            target_system: 1,
            target_component: 1,
            confirmation: 0,
        });
        let mut buf = Vec::new();
        mavlink::write_v2_msg(&mut buf, mavlink::MavHeader {
            system_id: 255,
            component_id: 0,
            sequence: 0,
        }, &msg)?;
        socket.send_to(&buf, target_addr).await?;
        Ok(())
    }
}
```

---

## 2. UAVCAN / OpenCyphal {#uavcan}

```toml
[dependencies]
canadensis       = { version = "0.5", features = ["can", "socketcan"] }
canadensis-pnp   = "0.5"
socketcan        = "3.3"
```

```rust
// gcs-comms/src/uavcan_monitor.rs
// Monitor OpenCyphal messages on CAN bus (for intra-vehicle data via SLCAN adapter)
use canadensis::Node;
use canadensis::transport::can::CanTransport;

pub async fn start_uavcan_monitor(
    interface: &str,
    event_bus: crate::core::events::EventBus,
) -> anyhow::Result<()> {
    let socket = socketcan::CANSocket::open(interface)?;
    tracing::info!("OpenCyphal monitor on {}", interface);

    // Subscribe to battery status (UAVCAN v1 subject ID 4097)
    // Subscribe to GPS fix (subject ID 1060)
    // Subscribe to IMU (subject ID 1040)

    loop {
        if let Ok(frame) = socket.read_frame() {
            // Parse CAN frame header for transfer ID, source node ID
            let node_id = (frame.id() & 0x7F) as u8;
            let subject_id = ((frame.id() >> 8) & 0x1FFF) as u16;

            tracing::debug!("UAVCAN frame: node={} subject={}", node_id, subject_id);

            match subject_id {
                4097 => { /* parse BatteryInfo */ }
                1060 => { /* parse GnssFix */ }
                _    => {}
            }
        }
        tokio::task::yield_now().await;
    }
}
```

---

## 3. DDS-XRCE (micro-ROS) {#dds}

For vehicles running micro-ROS on embedded MCU communicating with GCS:

```toml
[dependencies]
zenoh = "0.10"   # Zenoh is DDS-compatible and much simpler for GCS side
```

```rust
// gcs-comms/src/zenoh_bridge.rs
// Use Zenoh as DDS-XRCE bridge for micro-ROS vehicles
use zenoh::prelude::r#async::*;

pub async fn start_zenoh_bridge(
    event_bus: crate::core::events::EventBus,
) -> anyhow::Result<()> {
    let session = zenoh::open(zenoh::config::default()).res().await?;

    // Subscribe to micro-ROS telemetry topics
    let subscriber = session
        .declare_subscriber("/uav/telemetry/position")
        .res()
        .await?;

    loop {
        if let Ok(sample) = subscriber.recv_async().await {
            let payload = sample.value.payload.contiguous();
            // Deserialize CDR-encoded ROS2 NavSatFix message
            if let Ok(pos) = decode_navsatfix_cdr(&payload) {
                tracing::debug!("Zenoh position: {:?}", pos);
            }
        }
    }
}

fn decode_navsatfix_cdr(data: &[u8]) -> anyhow::Result<crate::core::types::GeoPoint> {
    // CDR encoding: skip 4-byte header, then doubles lat/lon/alt
    if data.len() < 28 { anyhow::bail!("CDR too short"); }
    let lat = f64::from_le_bytes(data[4..12].try_into()?);
    let lon = f64::from_le_bytes(data[12..20].try_into()?);
    let alt = f64::from_le_bytes(data[20..28].try_into()?);
    Ok(crate::core::types::GeoPoint { lat, lon, alt })
}
```

---

## 4. WebSocket Telemetry Relay {#websocket}

```rust
// gcs-comms/src/ws_relay.rs
// Relay telemetry to external consumers (web dashboards, BVLOS operators)
use axum::{Router, extract::WebSocketUpgrade, response::IntoResponse};
use axum::extract::ws::{WebSocket, Message};
use tokio::sync::broadcast;

pub fn ws_router(bus: broadcast::Sender<crate::core::events::GcsEvent>) -> Router {
    Router::new()
        .route("/ws/telemetry", axum::routing::get(
            move |ws: WebSocketUpgrade| ws_handler(ws, bus.clone())
        ))
        .route("/ws/commands", axum::routing::get(
            move |ws: WebSocketUpgrade| command_ws_handler(ws, bus.clone())
        ))
}

async fn ws_handler(
    ws: WebSocketUpgrade,
    bus: broadcast::Sender<crate::core::events::GcsEvent>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, bus.subscribe()))
}

async fn handle_socket(
    mut socket: WebSocket,
    mut event_rx: broadcast::Receiver<crate::core::events::GcsEvent>,
) {
    loop {
        tokio::select! {
            event = event_rx.recv() => {
                match event {
                    Ok(crate::core::events::GcsEvent::TelemetryReceived(frame)) => {
                        if let Ok(json) = serde_json::to_string(&frame) {
                            if socket.send(Message::Text(json)).await.is_err() {
                                break;
                            }
                        }
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                    _ => {}
                }
            }
            msg = socket.recv() => {
                match msg {
                    Some(Ok(Message::Close(_))) | None => break,
                    Some(Ok(Message::Text(cmd))) => {
                        tracing::info!("WS command received: {}", cmd);
                        // Parse and dispatch command
                    }
                    _ => {}
                }
            }
        }
    }
    tracing::info!("WebSocket client disconnected");
}
```

---

## 5. gRPC Service Definitions {#grpc}

```protobuf
// proto/gcs.proto
syntax = "proto3";
package gcs;

import "google/protobuf/timestamp.proto";

service GcsService {
    rpc StreamTelemetry(TelemetryRequest) returns (stream TelemetryFrame);
    rpc UploadMission(MissionUploadRequest) returns (MissionUploadResponse);
    rpc SendCommand(VehicleCommand) returns (CommandAck);
    rpc GetAirspaces(AirspaceBboxRequest) returns (AirspaceList);
    rpc GetActiveNotams(NotamRequest) returns (NotamList);
}

message TelemetryFrame {
    string vehicle_id = 1;
    google.protobuf.Timestamp timestamp = 2;
    double lat = 3;
    double lon = 4;
    double alt_msl = 5;
    double alt_agl = 6;
    float heading = 7;
    float pitch = 8;
    float roll = 9;
    float airspeed = 10;
    float groundspeed = 11;
    BatteryState battery = 12;
    string flight_mode = 13;
    GpsFix gps_fix = 14;
}

message Waypoint {
    int32 seq = 1;
    double lat = 2;
    double lon = 3;
    double alt = 4;
    int32 command = 5;
    float param1 = 6;
    float param2 = 7;
    float speed = 8;
    bool autocontinue = 9;
}

enum GpsFix { NO_FIX=0; FIX_2D=1; FIX_3D=2; DGPS=3; RTK_FLOAT=4; RTK_FIXED=5; }
```

---

## 6. Transport Abstraction {#transport}

```rust
// gcs-comms/src/transport.rs
use async_trait::async_trait;

#[async_trait]
pub trait Transport: Send + Sync {
    async fn read_packet(&mut self) -> anyhow::Result<Vec<u8>>;
    async fn write_packet(&mut self, data: &[u8]) -> anyhow::Result<()>;
    fn is_connected(&self) -> bool;
}

pub struct UdpTransport {
    socket: tokio::net::UdpSocket,
    remote: std::net::SocketAddr,
    buf: [u8; 280],
}

#[async_trait]
impl Transport for UdpTransport {
    async fn read_packet(&mut self) -> anyhow::Result<Vec<u8>> {
        let (n, _) = self.socket.recv_from(&mut self.buf).await?;
        Ok(self.buf[..n].to_vec())
    }
    async fn write_packet(&mut self, data: &[u8]) -> anyhow::Result<()> {
        self.socket.send_to(data, self.remote).await?;
        Ok(())
    }
    fn is_connected(&self) -> bool { true } // UDP is connectionless
}

pub struct SerialTransport {
    port: tokio_serial::SerialStream,
    buf: bytes::BytesMut,
}

#[async_trait]
impl Transport for SerialTransport {
    async fn read_packet(&mut self) -> anyhow::Result<Vec<u8>> {
        use tokio::io::AsyncReadExt;
        let mut tmp = [0u8; 280];
        let n = self.port.read(&mut tmp).await?;
        Ok(tmp[..n].to_vec())
    }
    async fn write_packet(&mut self, data: &[u8]) -> anyhow::Result<()> {
        use tokio::io::AsyncWriteExt;
        self.port.write_all(data).await?;
        Ok(())
    }
    fn is_connected(&self) -> bool { true }
}
```

---

## 7. Telemetry Deserialization Pipeline {#telemetry-pipeline}

```
Serial/UDP bytes
     ↓
[MAVLink framer: detect STX 0xFD, validate CRC]
     ↓
[Message dispatcher: match msg_id to handler]
     ↓
[State aggregator: merge GLOBAL_POS + ATTITUDE + SYS_STATUS]
     ↓
[TelemetryFrame builder: assemble complete snapshot]
     ↓
[Event bus broadcast]
     ↓
[Subscribers: DB writer, Cesium bridge, geofence, WS relay]
```

```rust
// gcs-comms/src/telemetry_aggregator.rs

/// Aggregate partial state from multiple MAVLink messages into full TelemetryFrame
#[derive(Default)]
pub struct VehicleTelemetryState {
    pub lat: Option<f64>,
    pub lon: Option<f64>,
    pub alt_msl_m: Option<f64>,
    pub alt_agl_m: Option<f64>,
    pub heading_deg: Option<f32>,
    pub pitch_deg: Option<f32>,
    pub roll_deg: Option<f32>,
    pub airspeed_mps: Option<f32>,
    pub groundspeed_mps: Option<f32>,
    pub battery_v: Option<f32>,
    pub battery_pct: Option<u8>,
    pub flight_mode: Option<crate::core::types::FlightMode>,
    pub gps_fix: Option<crate::core::types::GpsFix>,
    pub last_update: Option<std::time::Instant>,
}

impl VehicleTelemetryState {
    pub fn try_build_frame(&self, vehicle_id: crate::core::types::VehicleId) -> Option<crate::core::events::TelemetryFrame> {
        // Only emit if we have at minimum position data
        let lat = self.lat?;
        let lon = self.lon?;
        let alt = self.alt_msl_m.unwrap_or(0.0);

        Some(crate::core::events::TelemetryFrame {
            id: uuid::Uuid::new_v4(),
            vehicle_id,
            timestamp: chrono::Utc::now(),
            position: crate::core::types::GeoPoint { lat, lon, alt },
            attitude: crate::core::types::Attitude {
                roll_deg: self.roll_deg.unwrap_or(0.0),
                pitch_deg: self.pitch_deg.unwrap_or(0.0),
                yaw_deg: self.heading_deg.unwrap_or(0.0),
            },
            battery: crate::core::types::BatteryState {
                voltage_v: self.battery_v.unwrap_or(0.0),
                current_a: None,
                remaining_pct: self.battery_pct,
                consumed_mah: None,
            },
            flight_mode: self.flight_mode.clone().unwrap_or(crate::core::types::FlightMode::Manual),
            airspeed_mps: self.airspeed_mps.unwrap_or(0.0),
            groundspeed_mps: self.groundspeed_mps.unwrap_or(0.0),
            altitude_msl_m: alt,
            altitude_agl_m: self.alt_agl_m,
            gps_fix: self.gps_fix.unwrap_or(crate::core::types::GpsFix::NoFix),
            hdop: 0.0,
            vdop: 0.0,
        })
    }
}
```

---

## 8. MAVLink Mission Upload {#mission-upload}

```rust
// gcs-comms/src/mission_uploader.rs

pub struct MissionUploader<'a> {
    transport: &'a mut dyn crate::transport::Transport,
    system_id: u8,
    target_system: u8,
    target_component: u8,
}

impl<'a> MissionUploader<'a> {
    pub async fn upload(&mut self, items: &[crate::planner::MissionItem]) -> anyhow::Result<()> {
        use mavlink::common::*;

        // Step 1: Send MISSION_COUNT
        self.send_msg(&MavMessage::MISSION_COUNT(MISSION_COUNT_DATA {
            count: items.len() as u16,
            target_system: self.target_system,
            target_component: self.target_component,
            mission_type: mavlink::common::MavMissionType::MAV_MISSION_TYPE_MISSION,
        })).await?;

        // Step 2: Wait for MISSION_REQUEST_INT, respond with items
        for _ in 0..items.len() {
            let req = self.wait_for_mission_request(std::time::Duration::from_secs(5)).await?;
            let item = &items[req as usize];
            self.send_msg(&MavMessage::MISSION_ITEM_INT(MISSION_ITEM_INT_DATA {
                seq: req,
                frame: mavlink::common::MavFrame::MAV_FRAME_GLOBAL_RELATIVE_ALT_INT,
                command: mavlink::common::MavCmd::MAV_CMD_NAV_WAYPOINT,
                current: if req == 0 { 1 } else { 0 },
                autocontinue: 1,
                param1: item.hold_time_s,
                param2: item.acceptance_radius_m,
                param3: 0.0,
                param4: f32::NAN,
                x: (item.lat * 1e7) as i32,
                y: (item.lon * 1e7) as i32,
                z: item.alt_m,
                target_system: self.target_system,
                target_component: self.target_component,
                mission_type: mavlink::common::MavMissionType::MAV_MISSION_TYPE_MISSION,
            })).await?;
        }

        // Step 3: Wait for MISSION_ACK
        let ack = self.wait_for_mission_ack(std::time::Duration::from_secs(10)).await?;
        if ack != mavlink::common::MavMissionResult::MAV_MISSION_ACCEPTED {
            anyhow::bail!("Mission rejected: {:?}", ack);
        }
        tracing::info!("Mission uploaded successfully ({} items)", items.len());
        Ok(())
    }

    async fn send_msg(&mut self, msg: &mavlink::common::MavMessage) -> anyhow::Result<()> {
        let mut buf = Vec::new();
        mavlink::write_v2_msg(&mut buf, mavlink::MavHeader {
            system_id: self.system_id,
            component_id: 0,
            sequence: 0,
        }, msg)?;
        self.transport.write_packet(&buf).await
    }

    async fn wait_for_mission_request(&mut self, timeout: std::time::Duration) -> anyhow::Result<u16> {
        let deadline = tokio::time::Instant::now() + timeout;
        loop {
            if tokio::time::Instant::now() >= deadline {
                anyhow::bail!("Timeout waiting for MISSION_REQUEST_INT");
            }
            let pkt = self.transport.read_packet().await?;
            if let Ok((_, mavlink::common::MavMessage::MISSION_REQUEST_INT(req))) =
                mavlink::read_v2_msg::<mavlink::common::MavMessage, _>(&mut pkt.as_slice()) {
                return Ok(req.seq);
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    }

    async fn wait_for_mission_ack(&mut self, timeout: std::time::Duration) -> anyhow::Result<mavlink::common::MavMissionResult> {
        let deadline = tokio::time::Instant::now() + timeout;
        loop {
            if tokio::time::Instant::now() >= deadline {
                anyhow::bail!("Timeout waiting for MISSION_ACK");
            }
            let pkt = self.transport.read_packet().await?;
            if let Ok((_, mavlink::common::MavMessage::MISSION_ACK(ack))) =
                mavlink::read_v2_msg::<mavlink::common::MavMessage, _>(&mut pkt.as_slice()) {
                return Ok(ack.type_);
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    }
}
```

---

## 9. Swarm Command Protocol {#swarm}

```rust
// gcs-swarm/src/coordinator.rs
use std::collections::HashMap;
use crate::core::types::VehicleId;
use crate::core::events::{EventBus, GcsEvent, SwarmCommand};

pub struct SwarmCoordinator {
    vehicles: HashMap<VehicleId, VehicleRole>,
    formation: Formation,
    event_bus: EventBus,
}

#[derive(Debug, Clone)]
pub enum VehicleRole { Lead, Follower(usize), Scout, Observer }

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum Formation {
    Line { spacing_m: f64 },
    VShape { spacing_m: f64, angle_deg: f64 },
    Circle { radius_m: f64 },
    Grid { rows: u8, cols: u8, spacing_m: f64 },
    Custom(Vec<(f64, f64, f64)>),  // relative offsets (north, east, up) in meters
}

impl SwarmCoordinator {
    /// Compute absolute waypoints for each follower based on lead position
    pub fn compute_formation_waypoints(
        &self,
        lead_pos: &crate::core::types::GeoPoint,
        lead_heading_deg: f32,
    ) -> HashMap<VehicleId, crate::core::types::GeoPoint> {
        let mut result = HashMap::new();
        let offsets = self.formation_offsets();

        for (vehicle_id, role) in &self.vehicles {
            if let VehicleRole::Follower(idx) = role {
                if let Some(offset) = offsets.get(*idx) {
                    let (n, e, u) = Self::rotate_ned_by_heading(*offset, lead_heading_deg);
                    let pos = Self::ned_offset_to_geo(lead_pos, n, e, u);
                    result.insert(*vehicle_id, pos);
                }
            }
        }
        result
    }

    fn formation_offsets(&self) -> Vec<(f64, f64, f64)> {
        match &self.formation {
            Formation::Line { spacing_m } => (1..8)
                .map(|i| (-(*spacing_m) * i as f64, 0.0, 0.0))
                .collect(),
            Formation::VShape { spacing_m, angle_deg } => {
                let half_ang = angle_deg.to_radians() / 2.0;
                (1..8).map(|i| {
                    let d = *spacing_m * i as f64;
                    let side = if i % 2 == 0 { 1.0 } else { -1.0 };
                    (-d * half_ang.cos(), side * d * half_ang.sin(), 0.0)
                }).collect()
            }
            Formation::Custom(offsets) => offsets.clone(),
            _ => Vec::new(),
        }
    }

    fn rotate_ned_by_heading(ned: (f64, f64, f64), heading_deg: f32) -> (f64, f64, f64) {
        let h = (heading_deg as f64).to_radians();
        let (n, e, u) = ned;
        (n * h.cos() - e * h.sin(), n * h.sin() + e * h.cos(), u)
    }

    fn ned_offset_to_geo(
        origin: &crate::core::types::GeoPoint,
        north_m: f64, east_m: f64, up_m: f64,
    ) -> crate::core::types::GeoPoint {
        const R: f64 = 6_371_000.0;
        let lat = origin.lat + (north_m / R).to_degrees();
        let lon = origin.lon + (east_m / (R * origin.lat.to_radians().cos())).to_degrees();
        crate::core::types::GeoPoint { lat, lon, alt: origin.alt + up_m }
    }
}
```