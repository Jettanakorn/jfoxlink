# Cesium 3D Integration Reference

## Table of Contents
1. [Tauri ↔ Cesium IPC Bridge](#ipc-bridge)
2. [CZML Generation for Live Telemetry](#czml)
3. [Flight Path 3D Rendering](#flight-path)
4. [Airspace Polygon Overlay](#airspace-overlay)
5. [3D Tiles for Custom Terrain](#3d-tiles)
6. [Mission Planner — Interactive Editor](#mission-editor)
7. [Cesium Ion Token Management](#ion-token)
8. [Performance Optimization](#performance)

---

## 1. Tauri ↔ Cesium IPC Bridge {#ipc-bridge}

Rust backend → frontend Cesium communication uses Tauri v2 events and commands.

```rust
// gcs-ui/src/cesium_bridge.rs
use tauri::{AppHandle, Emitter};
use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CesiumCommand {
    UpdateVehiclePosition {
        vehicle_id: String,
        lat: f64,
        lon: f64,
        alt: f64,
        heading: f32,
        pitch: f32,
        roll: f32,
    },
    AddFlightPath {
        vehicle_id: String,
        waypoints: Vec<[f64; 3]>,  // [lon, lat, alt]
        color: [u8; 4],            // RGBA
    },
    ShowAirspace {
        airspace_id: String,
        geojson: serde_json::Value,
        color: [u8; 4],
        label: String,
    },
    FocusCamera {
        lat: f64,
        lon: f64,
        alt: f64,
        range_m: f64,
    },
    ClearVehicle { vehicle_id: String },
    ShowNotam {
        notam_id: String,
        geojson: serde_json::Value,
        severity: NotamSeverity,
        message: String,
    },
}

pub async fn emit_to_cesium(
    app: &AppHandle,
    cmd: CesiumCommand,
) -> anyhow::Result<()> {
    app.emit("cesium-command", &cmd)?;
    Ok(())
}

// Tauri command — frontend requests vehicle data
#[tauri::command]
pub async fn get_vehicle_state(
    vehicle_id: String,
    state: tauri::State<'_, GcsAppState>,
) -> Result<serde_json::Value, String> {
    let registry = state.vehicle_registry.read().await;
    registry
        .get_vehicle(&vehicle_id.parse().map_err(|e| format!("{e}"))?)
        .map(|v| serde_json::to_value(v).unwrap())
        .ok_or_else(|| "Vehicle not found".to_string())
}
```

### Frontend Cesium Event Listener
```javascript
// frontend/src/cesium/commandHandler.js
import { listen } from '@tauri-apps/api/event';

export async function initCesiumCommandHandler(viewer) {
  await listen('cesium-command', ({ payload }) => {
    switch (payload.type) {
      case 'update_vehicle_position':
        updateVehicleEntity(viewer, payload);
        break;
      case 'add_flight_path':
        renderFlightPath(viewer, payload);
        break;
      case 'show_airspace':
        renderAirspaceOverlay(viewer, payload);
        break;
      case 'focus_camera':
        flyToPosition(viewer, payload);
        break;
      case 'show_notam':
        renderNotamArea(viewer, payload);
        break;
    }
  });
}

function updateVehicleEntity(viewer, cmd) {
  const { vehicle_id, lat, lon, alt, heading, pitch, roll } = cmd;
  let entity = viewer.entities.getById(vehicle_id);

  const pos = Cesium.Cartesian3.fromDegrees(lon, lat, alt);
  const hpr = new Cesium.HeadingPitchRoll(
    Cesium.Math.toRadians(heading),
    Cesium.Math.toRadians(pitch),
    Cesium.Math.toRadians(roll)
  );
  const orientation = Cesium.Transforms.headingPitchRollQuaternion(pos, hpr);

  if (!entity) {
    entity = viewer.entities.add({
      id: vehicle_id,
      position: pos,
      orientation,
      model: {
        uri: '/assets/models/quadcopter.glb',
        minimumPixelSize: 32,
        maximumScale: 20000,
      },
      label: {
        text: vehicle_id.substring(0, 8),
        font: '12pt monospace',
        style: Cesium.LabelStyle.FILL_AND_OUTLINE,
        outlineWidth: 2,
        verticalOrigin: Cesium.VerticalOrigin.BOTTOM,
        pixelOffset: new Cesium.Cartesian2(0, -12),
      },
      path: {
        resolution: 1,
        material: new Cesium.PolylineGlowMaterialProperty({
          glowPower: 0.1,
          color: Cesium.Color.CYAN,
        }),
        width: 2,
        trailTime: 60, // seconds of trail
        leadTime: 0,
      },
    });
  } else {
    entity.position = new Cesium.ConstantPositionProperty(pos);
    entity.orientation = new Cesium.ConstantProperty(orientation);
  }
}
```

---

## 2. CZML Generation for Live Telemetry {#czml}

CZML is preferred for high-frequency telemetry replay and multi-vehicle animation.

```rust
// gcs-map/src/czml.rs
use serde_json::{json, Value};
use chrono::{DateTime, Utc};
use crate::core::types::{TelemetryFrame, GeoPoint};

pub struct CzmlBuilder {
    packets: Vec<Value>,
}

impl CzmlBuilder {
    pub fn new(name: &str) -> Self {
        let mut builder = Self { packets: Vec::new() };
        builder.packets.push(json!({
            "id": "document",
            "name": name,
            "version": "1.0",
        }));
        builder
    }

    pub fn add_vehicle_packet(&mut self, frames: &[TelemetryFrame]) -> &mut Self {
        if frames.is_empty() { return self; }
        let vehicle_id = frames[0].vehicle_id.to_string();

        // Build position and orientation time-sampled properties
        let mut position_times: Vec<String> = Vec::new();
        let mut position_values: Vec<f64> = Vec::new();
        let mut orientation_times: Vec<String> = Vec::new();
        let mut orientation_values: Vec<f64> = Vec::new();

        for frame in frames {
            let iso = frame.timestamp.to_rfc3339();
            let ecef = frame.position.to_ecef();

            position_times.push(iso.clone());
            position_values.extend_from_slice(&[ecef.x, ecef.y, ecef.z]);

            // Quaternion from heading/pitch/roll
            let q = attitude_to_quaternion(
                frame.attitude.heading_deg,
                frame.attitude.pitch_deg,
                frame.attitude.roll_deg,
            );
            orientation_times.push(iso);
            orientation_values.extend_from_slice(&[q.0, q.1, q.2, q.3]);
        }

        let first_ts = frames.first().unwrap().timestamp.to_rfc3339();
        let last_ts  = frames.last().unwrap().timestamp.to_rfc3339();

        self.packets.push(json!({
            "id": vehicle_id,
            "name": format!("UAV {}", &vehicle_id[..8]),
            "availability": format!("{}/{}", first_ts, last_ts),
            "position": {
                "interpolationAlgorithm": "LAGRANGE",
                "interpolationDegree": 3,
                "referenceFrame": "FIXED",
                "epoch": first_ts,
                "cartesian": position_values,
            },
            "orientation": {
                "interpolationAlgorithm": "LINEAR",
                "epoch": first_ts,
                "unitQuaternion": orientation_values,
            },
            "model": {
                "gltf": "/assets/models/quadcopter.glb",
                "scale": 2.0,
                "minimumPixelSize": 24,
            },
            "path": {
                "material": {
                    "polylineGlow": {
                        "color": { "rgba": [0, 255, 255, 200] },
                        "glowPower": 0.1,
                    }
                },
                "width": 2.0,
                "trailTime": 30.0,
                "leadTime": 0.0,
            }
        }));
        self
    }

    pub fn build(self) -> Vec<Value> {
        self.packets
    }

    pub fn to_json_string(self) -> String {
        serde_json::to_string(&self.build()).unwrap_or_default()
    }
}

fn attitude_to_quaternion(heading: f32, pitch: f32, roll: f32) -> (f64, f64, f64, f64) {
    use std::f64::consts::PI;
    let h = (heading as f64) * PI / 180.0;
    let p = (pitch as f64)   * PI / 180.0;
    let r = (roll as f64)    * PI / 180.0;
    // ZYX Euler to quaternion
    let (sh, ch) = ((h/2.0).sin(), (h/2.0).cos());
    let (sp, cp) = ((p/2.0).sin(), (p/2.0).cos());
    let (sr, cr) = ((r/2.0).sin(), (r/2.0).cos());
    (
        cr*cp*ch + sr*sp*sh,
        sr*cp*ch - cr*sp*sh,
        cr*sp*ch + sr*cp*sh,
        cr*cp*sh - sr*sp*ch,
    )
}
```

---

## 3. Flight Path 3D Rendering {#flight-path}

```javascript
// frontend/src/cesium/flightPath.js

export function renderMissionPath(viewer, waypoints, vehicleId) {
  // waypoints: [{lat, lon, alt, type}]
  const positions = waypoints.map(wp =>
    Cesium.Cartesian3.fromDegrees(wp.lon, wp.lat, wp.alt)
  );

  // Clamp-to-ground leg visualization
  viewer.entities.add({
    id: `${vehicleId}_ground_shadow`,
    polyline: {
      positions: positions.map(p => {
        const cart = Cesium.Cartographic.fromCartesian(p);
        return Cesium.Cartesian3.fromRadians(cart.longitude, cart.latitude, 0);
      }),
      width: 1,
      clampToGround: true,
      material: new Cesium.ColorMaterialProperty(Cesium.Color.WHITE.withAlpha(0.3)),
    }
  });

  // 3D flight path
  viewer.entities.add({
    id: `${vehicleId}_mission_path`,
    polyline: {
      positions,
      width: 3,
      material: new Cesium.PolylineDashMaterialProperty({
        color: Cesium.Color.YELLOW,
        dashLength: 16.0,
      }),
      arcType: Cesium.ArcType.NONE,
    }
  });

  // Waypoint markers
  waypoints.forEach((wp, i) => {
    viewer.entities.add({
      id: `${vehicleId}_wp_${i}`,
      position: Cesium.Cartesian3.fromDegrees(wp.lon, wp.lat, wp.alt),
      billboard: {
        image: waypointIcon(wp.type),
        verticalOrigin: Cesium.VerticalOrigin.BOTTOM,
        heightReference: Cesium.HeightReference.NONE,
        width: 28,
        height: 28,
      },
      label: {
        text: `WP${i + 1}\n${wp.alt.toFixed(0)}m`,
        font: '10pt sans-serif',
        fillColor: Cesium.Color.WHITE,
        outlineColor: Cesium.Color.BLACK,
        outlineWidth: 2,
        style: Cesium.LabelStyle.FILL_AND_OUTLINE,
        verticalOrigin: Cesium.VerticalOrigin.TOP,
        pixelOffset: new Cesium.Cartesian2(0, 4),
        showBackground: true,
        backgroundColor: Cesium.Color.fromCssColorString('#1a1a2e').withAlpha(0.8),
      }
    });
  });
}

// Vertical profile entity (side-view altitude band)
export function renderAltitudeProfile(viewer, waypoints, vehicleId) {
  const wallPositions = waypoints.map(wp =>
    Cesium.Cartesian3.fromDegrees(wp.lon, wp.lat, wp.alt)
  );
  viewer.entities.add({
    id: `${vehicleId}_alt_wall`,
    wall: {
      positions: wallPositions,
      minimumHeights: waypoints.map(() => 0),
      material: Cesium.Color.CYAN.withAlpha(0.05),
      outline: true,
      outlineColor: Cesium.Color.CYAN.withAlpha(0.3),
    }
  });
}
```

---

## 4. Airspace Polygon Overlay {#airspace-overlay}

```javascript
// frontend/src/cesium/airspaceOverlay.js
import * as Cesium from 'cesium';

const AIRSPACE_COLORS = {
  ClassA: [255, 0, 0, 100],
  ClassB: [0, 0, 255, 80],
  ClassC: [128, 0, 128, 80],
  ClassD: [0, 128, 255, 80],
  ClassE: [0, 200, 100, 60],
  ClassG: [200, 200, 200, 40],
  TFR:    [255, 0, 0, 150],
  CTR:    [255, 165, 0, 120],
  NOTAM:  [255, 255, 0, 100],
  Prohibited: [200, 0, 0, 180],
  Restricted: [255, 100, 0, 150],
  Danger:     [255, 50, 50, 130],
};

export function renderAirspace(viewer, airspace) {
  const { airspace_id, geojson, airspace_class, lower_alt_m, upper_alt_m, label } = airspace;
  const color = AIRSPACE_COLORS[airspace_class] || [150, 150, 150, 80];
  const cesiumColor = Cesium.Color.fromBytes(...color);

  if (geojson.geometry.type === 'Polygon') {
    const rings = geojson.geometry.coordinates;
    const outerRing = rings[0].flatMap(([lon, lat]) => [lon, lat]);

    viewer.entities.add({
      id: airspace_id,
      name: label,
      polygon: {
        hierarchy: new Cesium.PolygonHierarchy(
          Cesium.Cartesian3.fromDegreesArray(outerRing)
        ),
        extrudedHeight: upper_alt_m,
        height: lower_alt_m ?? 0,
        material: cesiumColor.withAlpha(0.12),
        outline: true,
        outlineColor: cesiumColor,
        outlineWidth: 2,
      },
      label: {
        text: `${label}\n${lower_alt_m ?? 'SFC'}–${upper_alt_m}m`,
        font: '10pt monospace',
        fillColor: cesiumColor,
        outlineColor: Cesium.Color.BLACK,
        outlineWidth: 1,
        style: Cesium.LabelStyle.FILL_AND_OUTLINE,
        show: false, // toggle on hover
      }
    });
  }
}

// Batch-render all airspaces from GeoJSON FeatureCollection
export function loadAirspaceLayer(viewer, featureCollection) {
  featureCollection.features.forEach(feature => {
    const props = feature.properties;
    renderAirspace(viewer, {
      airspace_id: props.id,
      geojson: feature,
      airspace_class: props.airspace_class,
      lower_alt_m: props.lower_limit_m,
      upper_alt_m: props.upper_limit_m,
      label: props.name,
    });
  });
}
```

---

## 5. 3D Tiles for Custom Terrain {#3d-tiles}

```javascript
// frontend/src/cesium/terrainSetup.js

export async function initializeCesiumViewer(containerId, ionToken) {
  Cesium.Ion.defaultAccessToken = ionToken;

  const viewer = new Cesium.Viewer(containerId, {
    terrainProvider: await Cesium.createWorldTerrainAsync({
      requestWaterMask: true,
      requestVertexNormals: true,
    }),
    baseLayerPicker: false,
    geocoder: false,
    homeButton: false,
    sceneModePicker: true,
    navigationHelpButton: false,
    animation: true,
    timeline: true,
    fullscreenButton: false,
    skyBox: new Cesium.SkyBox({ show: true }),
    shadows: true,
    terrainShadows: Cesium.ShadowMode.ENABLED,
  });

  // Enable depth testing against terrain (critical for low-altitude ops)
  viewer.scene.globe.depthTestAgainstTerrain = true;

  // Night-mode imagery for BVLOS
  viewer.scene.globe.enableLighting = true;

  // Add OSM Buildings for urban ops
  const osmBuildings = await Cesium.createOsmBuildingsAsync();
  viewer.scene.primitives.add(osmBuildings);

  return viewer;
}

// Load custom high-res terrain for operation area (e.g., Chiang Mai mountains)
export async function loadCustomTerrain(viewer, quantizedMeshUrl) {
  viewer.terrainProvider = await Cesium.CesiumTerrainProvider.fromUrl(
    quantizedMeshUrl,
    { requestVertexNormals: true }
  );
}
```

---

## 6. Mission Planner — Interactive Editor {#mission-editor}

```javascript
// frontend/src/cesium/missionEditor.js

export class MissionEditor {
  constructor(viewer, onWaypointChange) {
    this.viewer = viewer;
    this.onWaypointChange = onWaypointChange;
    this.waypoints = [];
    this.handler = new Cesium.ScreenSpaceEventHandler(viewer.scene.canvas);
    this._setupHandlers();
  }

  _setupHandlers() {
    // Left click → add waypoint
    this.handler.setInputAction((click) => {
      if (!this.active) return;
      const ray = this.viewer.camera.getPickRay(click.position);
      const position = this.viewer.scene.globe.pick(ray, this.viewer.scene);
      if (!position) return;

      const cartographic = Cesium.Cartographic.fromCartesian(position);
      const wp = {
        lat: Cesium.Math.toDegrees(cartographic.latitude),
        lon: Cesium.Math.toDegrees(cartographic.longitude),
        alt: this.currentAltitude,
        type: this.currentWaypointType,
        speed_mps: this.currentSpeed,
        hold_time_s: 0,
      };
      this.waypoints.push(wp);
      this._renderWaypoints();
      this.onWaypointChange(this.waypoints);
    }, Cesium.ScreenSpaceEventType.LEFT_CLICK);

    // Right click → remove last waypoint
    this.handler.setInputAction(() => {
      if (!this.active || this.waypoints.length === 0) return;
      this.waypoints.pop();
      this._renderWaypoints();
      this.onWaypointChange(this.waypoints);
    }, Cesium.ScreenSpaceEventType.RIGHT_CLICK);
  }

  setActive(active) { this.active = active; }
  setAltitude(alt)  { this.currentAltitude = alt; }
  setSpeed(spd)     { this.currentSpeed = spd; }

  exportMavlinkMission() {
    // Convert to MAVLink MISSION_ITEM_INT format for upload
    return this.waypoints.map((wp, i) => ({
      seq: i,
      frame: 3,   // MAV_FRAME_GLOBAL_RELATIVE_ALT
      command: wp.type === 'loiter' ? 17 : 16, // MAV_CMD_NAV_WAYPOINT or LOITER
      current: i === 0 ? 1 : 0,
      autocontinue: 1,
      param1: wp.hold_time_s,
      param2: 0.5,      // acceptance radius meters
      param3: 0,
      param4: 0,        // yaw (NaN = keep heading)
      x: Math.round(wp.lat * 1e7),
      y: Math.round(wp.lon * 1e7),
      z: wp.alt,
    }));
  }
}
```

---

## 7. Cesium Ion Token Management {#ion-token}

```rust
// gcs-ui/src/cesium_token.rs
// Never embed the token in source — load from env or encrypted config

pub fn get_cesium_token() -> anyhow::Result<String> {
    std::env::var("CESIUM_ION_TOKEN")
        .or_else(|_| {
            // Fallback: read from encrypted config file
            let config = crate::config::load_secure_config()?;
            config.cesium_ion_token.ok_or_else(|| anyhow::anyhow!("No Cesium token"))
        })
}
```

---

## 8. Performance Optimization {#performance}

- **Cluster entities** below zoom threshold using `Cesium.EntityCluster`
- **Limit path trail** to last N seconds using `trailTime` property
- **Use primitives** instead of entities for >1000 static airspace polygons
- **RequestAnimationFrame throttling**: update vehicle positions at max 30 Hz
- **Tile caching**: set `Cesium.RequestScheduler.maximumRequestsPerServer = 18`
- **LOD for 3D models**: use CesiumJS `maximumScale` and `minimumPixelSize`

```javascript
// Primitive-based approach for many airspace polygons (faster than entities)
export function renderAirspaceAsPrimitive(scene, polygons) {
  const instances = polygons.map(p => new Cesium.GeometryInstance({
    id: p.id,
    geometry: new Cesium.PolygonGeometry({
      polygonHierarchy: new Cesium.PolygonHierarchy(
        Cesium.Cartesian3.fromDegreesArray(p.coords)
      ),
      extrudedHeight: p.upper_alt,
      height: p.lower_alt,
    }),
    attributes: {
      color: Cesium.ColorGeometryInstanceAttribute.fromColor(
        Cesium.Color.fromBytes(...p.color)
      )
    }
  }));

  scene.primitives.add(new Cesium.Primitive({
    geometryInstances: instances,
    appearance: new Cesium.PerInstanceColorAppearance({
      translucent: true,
      closed: true,
    }),
    releaseGeometryInstances: false,
  }));
}
```