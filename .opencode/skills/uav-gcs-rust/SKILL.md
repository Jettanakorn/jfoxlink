---
name: uav-gcs-rust
description: >
  Expert-level AI agent skill for designing, architecting, and building production-grade
  UAV Ground Control Stations (GCS) using Rust, CesiumJS/Cesium Native for 3D geospatial
  visualization, and aeronautical databases (AIXM, AIP, ICAO, OpenAIP, FAA NASR, NOTAM).
  Use for ANY task involving: GCS software design, UAV command-and-control architecture,
  MAVLink/UAVCAN/DDS-XRCE protocol stacks, real-time telemetry pipelines, 3D terrain/airspace
  rendering, geofencing, flight path planning, ATC integration, drone swarm coordination, or
  safety-critical embedded/desktop hybrid systems in Rust. Also covers PX4 autopilot: flight
  mode decoding, uXRCE-DDS (Zenoh) telemetry bridge, parameter protocol, offboard control,
  VTOL transitions, and ULOG log download. Trigger for "drone software", "flight controller UI",
  "telemetry dashboard", "airspace map", "PX4", "offboard mode", or "ulog". Full stack from
  hardware abstraction to cloud backend. L99 expertise level.
compatibility: "Rust ≥1.78/tokio, CesiumJS ≥1.118/Cesium Native, Tauri v2/egui, MAVLink 2.0 (mavlink crate), PX4 v1.13+/v1.14+ uXRCE-DDS, ArduPilot, UAVCAN/OpenCyphal (canadensis), PostGIS/SpatiaLite, AIXM 5.1/AIP/NOTAM/FAA NASR/OpenAIP, DDS-XRCE/Zenoh, gRPC (tonic)"
---

# UAV Ground Control Station — Rust + Cesium + Aeronautical Databases

## Skill Overview

This skill transforms Claude into an L99 GCS architect — capable of generating
production-ready Rust code, system architecture diagrams, protocol implementations,
geospatial query engines, and aeronautical database schemas for UAV / UAS operations.

**Complexity level:** L99 — Full-stack safety-critical system design, DO-178C/DO-326A
awareness, aerospace software patterns, real-time constraints, geospatial intelligence.

---

## Quick Decision Tree

```
User request involves GCS?
├── Architecture / system design            → Read references/architecture.md
├── 3D map / Cesium / terrain / flight path → Read references/cesium-3d-integration.md
├── Airspace data / NOTAM / AIXM / TFR      → Read references/aeronautic-databases.md
├── MAVLink / telemetry / protocols         → Read references/rust-protocols.md
├── PX4 flight controller / autopilot
│   ├── PX4 flight modes / mode switching   → Read references/px4-flight-controller.md §2–3
│   ├── PX4 uXRCE-DDS / micro-ROS (v1.14+) → Read references/px4-flight-controller.md §4
│   ├── PX4 parameters / offboard control   → Read references/px4-flight-controller.md §5–6
│   ├── PX4 missions / geofence             → Read references/px4-flight-controller.md §7–8
│   ├── PX4 ULOG log download               → Read references/px4-flight-controller.md §9
│   ├── VTOL transition commands            → Read references/px4-flight-controller.md §10
│   └── PX4 vs ArduPilot comparison         → Read references/px4-flight-controller.md (end)
└── Full GCS application scaffold           → Read ALL reference files in order
```

**Always read the relevant reference before generating code or architecture.**

---

## Core GCS Architecture Layers

```
┌─────────────────────────────────────────────────────────────┐
│                    GCS APPLICATION SHELL                     │
│          Tauri v2 (desktop) or egui/eframe (embedded)        │
├─────────────────────────────────────────────────────────────┤
│                   PRESENTATION LAYER                         │
│    CesiumJS/WebGL (3D Map) │ HUD Widgets │ Mission Planner   │
├─────────────────────────────────────────────────────────────┤
│                   DOMAIN SERVICES LAYER                      │
│  Telemetry Engine │ Flight Path Planner │ Geofence Engine    │
│  Airspace Query   │ Swarm Coordinator  │ Alerts & Events     │
├─────────────────────────────────────────────────────────────┤
│                 COMMUNICATION LAYER                          │
│  MAVLink 2.0  │ UAVCAN/OpenCyphal │ DDS-XRCE │ WebSocket     │
├─────────────────────────────────────────────────────────────┤
│               AERONAUTICAL DATA LAYER                        │
│  AIXM Engine  │ NOTAM Processor │ TFR/CTR Overlay │ AIP DB   │
├─────────────────────────────────────────────────────────────┤
│                PERSISTENCE LAYER                             │
│  PostGIS / SpatiaLite │ SQLite (offline) │ TimescaleDB       │
└─────────────────────────────────────────────────────────────┘
```

---

## Reference Files — When to Read Each

| Reference File | Read When... |
|---|---|
| `references/architecture.md` | Designing Rust GCS crate structure, async task graphs, actor models, real-time safety patterns |
| `references/cesium-3d-integration.md` | Implementing 3D map, terrain, flight path rendering, CZML, 3D Tiles, WGS84/ECEF math |
| `references/aeronautic-databases.md` | Working with AIXM, NOTAM, FAA NASR, TFR, AIP, geofencing, airspace classification |
| `references/rust-protocols.md` | MAVLink 2.0, UAVCAN/OpenCyphal, DDS-XRCE, telemetry deserialization, serial/UDP/WebSocket |
| `references/px4-flight-controller.md` | PX4 autopilot detection, flight mode decoding, uXRCE-DDS bridge, parameter protocol, offboard control, VTOL, ULOG download |

---

## Claude's Behavioral Contract for This Skill

When invoked, Claude MUST:

1. **Identify the exact GCS sub-domain** from the user's request (see Quick Decision Tree).
2. **Read the relevant reference file(s)** before generating any code.
3. **Generate production-quality Rust code** — no pseudocode unless explicitly asked. Use:
   - `tokio` for async runtime
   - `thiserror` / `anyhow` for error handling
   - `serde` / `serde_json` for serialization
   - `tracing` for structured logging
   - Proper lifetime annotations and ownership patterns
4. **Apply aerospace safety patterns** — never suggest `unwrap()` in flight-critical paths; use `Result<T, E>` chains, fallback handlers, and watchdog patterns.
5. **Ground all aeronautical facts in the reference files** — do not hallucinate airspace classes, ICAO procedures, or regulatory requirements.
6. **Provide architecture diagrams** using ASCII or Mermaid when system design is requested.
7. **Call out real-time constraints** when they exist — specify task priorities, jitter budgets, and scheduling model.

---

## Skill Invocation Examples

| User Says | Claude Does |
|---|---|
| "Set up the Rust workspace for a GCS project" | Reads architecture.md → generates `Cargo.toml` workspace with crate topology |
| "Show telemetry pipeline from MAVLink to Cesium" | Reads rust-protocols.md + cesium-3d-integration.md → generates end-to-end pipeline |
| "Overlay Thailand's CTR and TFR on the 3D map" | Reads aeronautic-databases.md + cesium-3d-integration.md → AIXM query + CZML overlay |
| "Implement geofencing for a swarm mission in Chiang Mai area" | Reads all refs → spatial query + constraint solver + MAVLink fence upload |
| "Build the mission planner UI with flight path editor" | Reads cesium-3d-integration.md + architecture.md → Tauri + Cesium mission editor |
| "Connect the GCS to a PX4 flight controller" | Reads px4-flight-controller.md + rust-protocols.md → autopilot detection + Px4Backend impl |
| "Decode PX4 flight modes from MAVLink heartbeat" | Reads px4-flight-controller.md §2 → `decode_px4_custom_mode()` with full enum |
| "Stream PX4 telemetry via uXRCE-DDS" | Reads px4-flight-controller.md §4 → Zenoh subscriber + CDR deserialization |
| "Upload a geofence and read PX4 parameters" | Reads px4-flight-controller.md §5+8 → `Px4ParamStore` + `upload_circular_fence()` |
| "Control the drone in offboard mode from GCS" | Reads px4-flight-controller.md §6 → setpoint streaming loop at ≥2 Hz |
| "Download and parse the PX4 flight log" | Reads px4-flight-controller.md §9 → MAVLink LOG_REQUEST + ulog-rs parsing |

---

## Glossary

| Term | Definition |
|---|---|
| AIXM | Aeronautical Information Exchange Model (XML schema for airspace data) |
| AIP | Aeronautical Information Publication (country-level airspace charts & procedures) |
| CZML | Cesium's JSON-based dynamic scene description language |
| CTR | Control Zone — controlled airspace around an airport |
| TFR | Temporary Flight Restriction |
| NOTAM | Notice to Air Missions |
| MAVLink | Micro Air Vehicle Link — UAV communication protocol |
| UAVCAN | CAN-bus protocol for UAV intra-vehicle communication (now OpenCyphal) |
| DDS-XRCE | Data Distribution Service for eXtremely Resource-Constrained Environments |
| WGS84 | World Geodetic System 1984 — GPS reference ellipsoid |
| ECEF | Earth-Centered, Earth-Fixed coordinate frame |
| ENU | East-North-Up local tangent plane coordinates |
| GeoJSON | JSON format for geographic features (used in PostGIS, Cesium) |
| 3D Tiles | Cesium's spatial data format for large-scale 3D geospatial datasets |
| PostGIS | PostgreSQL extension for geospatial queries (ST_* functions) |
| PX4 | Open-source UAV autopilot firmware (Linux Foundation Dronecode project) |
| uXRCE-DDS | Micro XRCE-DDS — PX4 v1.14+ native DDS bridge for companion computer comms |
| uORB | PX4's internal publish-subscribe message bus (topics exposed via uXRCE-DDS) |
| ULOG | PX4 binary flight log format (.ulg files, downloaded via MAVLink LOG_REQUEST) |
| Offboard | PX4 flight mode where GCS/companion streams setpoints at ≥2 Hz |
| VTOL | Vertical Take-Off and Landing — aircraft combining multirotor + fixed-wing |
| CDR | Common Data Representation — serialization format used by DDS/XRCE messages |