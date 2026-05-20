# Aeronautical Databases Reference

## Table of Contents
1. [Database Schema Overview](#schema)
2. [AIXM 5.1 Ingestion](#aixm)
3. [NOTAM Processing](#notam)
4. [FAA NASR / Thailand AERIS Integration](#nasr)
5. [OpenAIP & OurAirports](#openair)
6. [PostGIS Spatial Queries](#postgis)
7. [Geofence Engine](#geofence)
8. [TFR / CTR Overlay Pipeline](#tfr-ctr)
9. [ICAO Airspace Classification](#icao)

---

## 1. Database Schema Overview {#schema}

```sql
-- SQLite with SpatiaLite extension (offline-capable)
-- Or PostgreSQL + PostGIS for server deployments

CREATE TABLE airspaces (
    id          TEXT PRIMARY KEY,      -- AIXM UUID or composite key
    name        TEXT NOT NULL,
    icao_class  TEXT,                  -- A/B/C/D/E/F/G
    airspace_type TEXT,                -- CTR/TMA/DANGER/RESTRICTED/PROHIBITED/FIR/UIR
    lower_limit_m   REAL,              -- MSL meters (NULL = SFC)
    upper_limit_m   REAL NOT NULL,
    lower_limit_ref TEXT DEFAULT 'MSL', -- MSL/AGL/FL
    upper_limit_ref TEXT DEFAULT 'MSL',
    country     TEXT,                  -- ISO 3166-1 alpha-2
    icao_region TEXT,                  -- e.g. VTBB (Bangkok FIR)
    valid_from  DATETIME,
    valid_to    DATETIME,
    source      TEXT,                  -- aixm/openair/faa/openairspace
    geom        GEOMETRY(POLYGON, 4326) NOT NULL
);
CREATE INDEX airspaces_geom_idx ON airspaces USING GIST(geom);

CREATE TABLE notams (
    id          TEXT PRIMARY KEY,      -- NOTAM number, e.g. A0123/24
    series      TEXT,                  -- A/B/C/D/V
    notam_type  TEXT,                  -- N/R/C
    q_line      TEXT,                  -- Q) field raw
    fir         TEXT,                  -- VTBB, VYYY, etc.
    subject     TEXT,                  -- Aerodrome, Navaid, Airspace, etc.
    condition   TEXT,                  -- Restrictions
    traffic     TEXT,                  -- IFR/VFR/NOTAM
    purpose     TEXT,
    scope       TEXT,
    lower_fl    INTEGER,               -- Lower flight level (0 = SFC)
    upper_fl    INTEGER,
    effective_from  DATETIME NOT NULL,
    effective_to    DATETIME,          -- NULL = PERM
    text_e      TEXT NOT NULL,         -- Freetext E) field
    source_raw  TEXT,
    geom        GEOMETRY(GEOMETRY, 4326) -- Point/Circle/Polygon
);
CREATE INDEX notams_from_idx  ON notams(effective_from);
CREATE INDEX notams_to_idx    ON notams(effective_to);
CREATE INDEX notams_geom_idx  ON notams USING GIST(geom);

CREATE TABLE waypoints (
    id          TEXT PRIMARY KEY,
    icao_id     TEXT,
    name        TEXT,
    waypoint_type TEXT, -- ICAO/NAV/AIRPORT/FIX/USER
    lat         REAL NOT NULL,
    lon         REAL NOT NULL,
    elevation_m REAL,
    country     TEXT,
    frequency   REAL,  -- MHz for navaids
    mag_var     REAL
);
CREATE INDEX waypoints_geo ON waypoints(lat, lon);

CREATE TABLE airports (
    icao        TEXT PRIMARY KEY,
    iata        TEXT,
    name        TEXT NOT NULL,
    lat         REAL NOT NULL,
    lon         REAL NOT NULL,
    elevation_m REAL,
    country     TEXT,
    type        TEXT, -- large_airport/medium_airport/small_airport/heliport
    scheduled_service BOOLEAN,
    home_link   TEXT,
    geom        GEOMETRY(POINT, 4326)
);

CREATE TABLE flight_restrictions (
    id          TEXT PRIMARY KEY,
    type        TEXT NOT NULL,     -- TFR/PROHIBITED/TEMPORARY/EXERCISE
    name        TEXT,
    reason      TEXT,
    lower_alt_m REAL,
    upper_alt_m REAL,
    effective_from DATETIME NOT NULL,
    effective_to   DATETIME,
    authority   TEXT,
    contact     TEXT,
    geom        GEOMETRY(GEOMETRY, 4326)
);
```

---

## 2. AIXM 5.1 Ingestion {#aixm}

AIXM (Aeronautical Information Exchange Model) uses GML/XML. Parse with `quick-xml`.

```rust
// gcs-airspace/src/aixm_parser.rs
use quick_xml::Reader;
use quick_xml::events::Event;
use serde::{Serialize, Deserialize};
use anyhow::{Result, Context};

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct AixmAirspace {
    pub identifier: String,
    pub name: Option<String>,
    pub icao_class: Option<String>,
    pub airspace_type: Option<String>,
    pub lower_limit: Option<AixmLimit>,
    pub upper_limit: Option<AixmLimit>,
    pub geometry: Option<AixmGeometry>,
    pub activation: Vec<AixmActivation>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct AixmLimit {
    pub value: f64,
    pub unit: String,    // FT/M/FL
    pub reference: String, // MSL/AGL/STD
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct AixmGeometry {
    pub coordinates: Vec<(f64, f64)>, // (lon, lat) pairs
}

pub struct AixmParser;

impl AixmParser {
    pub fn parse_airspace_file(xml_content: &str) -> Result<Vec<AixmAirspace>> {
        let mut reader = Reader::from_str(xml_content);
        reader.config_mut().trim_text(true);

        let mut airspaces: Vec<AixmAirspace> = Vec::new();
        let mut current: Option<AixmAirspace> = None;
        let mut in_element: Vec<String> = Vec::new();
        let mut buf = Vec::new();

        loop {
            match reader.read_event_into(&mut buf) {
                Ok(Event::Start(e)) => {
                    let name = std::str::from_utf8(e.name().as_ref())?.to_string();
                    in_element.push(name.clone());
                    if name.ends_with(":Airspace") || name == "Airspace" {
                        current = Some(AixmAirspace::default());
                    }
                },
                Ok(Event::Text(e)) => {
                    if let Some(ref mut a) = current {
                        let text = e.unescape()?.into_owned();
                        match in_element.last().map(|s| s.as_str()) {
                            Some(t) if t.ends_with("identifier") => a.identifier = text,
                            Some(t) if t.ends_with("txtName") => a.name = Some(text),
                            Some(t) if t.ends_with("codeClass") => a.icao_class = Some(text),
                            Some(t) if t.ends_with("codeType") => a.airspace_type = Some(text),
                            _ => {}
                        }
                    }
                },
                Ok(Event::End(e)) => {
                    let name = std::str::from_utf8(e.name().as_ref())?.to_string();
                    in_element.pop();
                    if (name.ends_with(":Airspace") || name == "Airspace") {
                        if let Some(a) = current.take() {
                            airspaces.push(a);
                        }
                    }
                },
                Ok(Event::Eof) => break,
                Err(e) => return Err(anyhow::anyhow!("XML parse error: {e}")),
                _ => {}
            }
            buf.clear();
        }
        Ok(airspaces)
    }

    pub fn limit_to_meters(limit: &AixmLimit) -> f64 {
        match limit.unit.as_str() {
            "FT"  => limit.value * 0.3048,
            "FL"  => limit.value * 100.0 * 0.3048,  // approximate FL to meters
            "M"   => limit.value,
            _     => limit.value,
        }
    }
}
```

---

## 3. NOTAM Processing {#notam}

```rust
// gcs-airspace/src/notam.rs
use chrono::{DateTime, Utc, NaiveDateTime};
use geo::{Point, Polygon, LineString};
use serde::{Serialize, Deserialize};
use regex::Regex;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Notam {
    pub id: String,
    pub fir: String,
    pub q_code: String,       // Q) VTBB/QRPCH/IV/NBO/W/000/999
    pub from: DateTime<Utc>,
    pub to: Option<DateTime<Utc>>,
    pub permanent: bool,
    pub text_e: String,
    pub geometry: Option<NotamGeometry>,
    pub lower_fl: i32,
    pub upper_fl: i32,
    pub parsed_severity: NotamSeverity,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NotamGeometry {
    Circle { center: (f64, f64), radius_nm: f64 },
    Polygon(Vec<(f64, f64)>),
    Point(f64, f64),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NotamSeverity { Info, Advisory, Warning, Critical }

pub struct NotamParser;

impl NotamParser {
    /// Parse a single NOTAM text block into structured form
    pub fn parse(raw: &str) -> anyhow::Result<Notam> {
        let id = Self::extract_field(raw, r"(?m)^([A-Z]\d{4}/\d{2})")
            .unwrap_or_else(|| "UNKNOWN".to_string());
        let fir = Self::extract_between(raw, "Q) ", "/")
            .unwrap_or_else(|| "XXXX".to_string());
        let q_code = Self::extract_field(raw, r"Q\) (\S+/\S+/\S+/\S+/\S+/\d+/\d+/[^)\n]+)")
            .unwrap_or_default();
        let text_e = Self::extract_between(raw, "E) ", "\nF)")
            .or_else(|| Self::extract_between(raw, "E) ", "\nG)"))
            .or_else(|| Self::extract_after(raw, "E) "))
            .unwrap_or_default();

        // Parse effective dates
        let from = Self::parse_notam_date(
            &Self::extract_between(raw, "B) ", "\n").unwrap_or_default()
        )?;
        let (to, permanent) = if raw.contains("C) PERM") {
            (None, true)
        } else {
            let to_str = Self::extract_between(raw, "C) ", "\n").unwrap_or_default();
            (Some(Self::parse_notam_date(&to_str)?), false)
        };

        // Parse altitudes from Q code: .../FL_LOW/FL_HIGH/...
        let (lower_fl, upper_fl) = Self::parse_q_altitudes(&q_code);

        // Try to extract geometry (circle pattern most common)
        let geometry = Self::extract_circle_geometry(&text_e)
            .or_else(|| Self::extract_polygon_geometry(&text_e));

        let severity = Self::classify_severity(&q_code, &text_e);

        Ok(Notam { id, fir, q_code, from, to, permanent, text_e, geometry, lower_fl, upper_fl, parsed_severity: severity })
    }

    fn parse_notam_date(s: &str) -> anyhow::Result<DateTime<Utc>> {
        // NOTAM format: 2401010000 = 2024-01-01 00:00 UTC
        let trimmed = s.trim();
        if trimmed.len() < 10 { anyhow::bail!("Date too short: {}", s); }
        let year = 2000 + trimmed[0..2].parse::<i32>()?;
        let month = trimmed[2..4].parse::<u32>()?;
        let day   = trimmed[4..6].parse::<u32>()?;
        let hour  = trimmed[6..8].parse::<u32>()?;
        let min   = trimmed[8..10].parse::<u32>()?;
        let naive = NaiveDateTime::parse_from_str(
            &format!("{:04}-{:02}-{:02} {:02}:{:02}:00", year, month, day, hour, min),
            "%Y-%m-%d %H:%M:%S"
        )?;
        Ok(DateTime::from_naive_utc_and_offset(naive, Utc))
    }

    fn extract_circle_geometry(text: &str) -> Option<NotamGeometry> {
        // Pattern: 1NM RADIUS CENTRED ON 1831N09837E
        let re = Regex::new(
            r"(?i)(\d+(?:\.\d+)?)\s*NM\s+RADIUS\s+CENT(?:RED|ER(?:ED)?)\s+(?:ON\s+)?(\d{4}[NS])(\d{5}[EW])"
        ).ok()?;
        let caps = re.captures(text)?;
        let radius_nm: f64 = caps[1].parse().ok()?;
        let lat = Self::dms_to_decimal(&caps[2], false)?;
        let lon = Self::dms_to_decimal(&caps[3], true)?;
        Some(NotamGeometry::Circle { center: (lon, lat), radius_nm })
    }

    fn dms_to_decimal(s: &str, is_lon: bool) -> Option<f64> {
        // 1831N → 18.52, 09837E → 98.62
        if is_lon {
            let deg: f64 = s[0..3].parse().ok()?;
            let min: f64 = s[3..5].parse().ok()?;
            let sign = if s.ends_with('E') { 1.0 } else { -1.0 };
            Some(sign * (deg + min / 60.0))
        } else {
            let deg: f64 = s[0..2].parse().ok()?;
            let min: f64 = s[2..4].parse().ok()?;
            let sign = if s.ends_with('N') { 1.0 } else { -1.0 };
            Some(sign * (deg + min / 60.0))
        }
    }

    fn classify_severity(q_code: &str, text: &str) -> NotamSeverity {
        let text_upper = text.to_uppercase();
        if q_code.contains("/QR") || text_upper.contains("PROHIBITED") ||
           text_upper.contains("DANGER") { return NotamSeverity::Critical; }
        if text_upper.contains("RESTRICTED") || text_upper.contains("TFR") ||
           q_code.contains("/QW") { return NotamSeverity::Warning; }
        if text_upper.contains("CAUTION") || text_upper.contains("UAS") {
            return NotamSeverity::Advisory;
        }
        NotamSeverity::Info
    }

    fn extract_field(text: &str, pattern: &str) -> Option<String> {
        Regex::new(pattern).ok()?.captures(text).map(|c| c[1].to_string())
    }
    fn extract_between(text: &str, start: &str, end: &str) -> Option<String> {
        let s = text.find(start)? + start.len();
        let e = text[s..].find(end)? + s;
        Some(text[s..e].trim().to_string())
    }
    fn extract_after(text: &str, start: &str) -> Option<String> {
        text.find(start).map(|i| text[i + start.len()..].trim().to_string())
    }
    fn extract_polygon_geometry(_text: &str) -> Option<NotamGeometry> { None }
    fn parse_q_altitudes(q: &str) -> (i32, i32) {
        let parts: Vec<&str> = q.split('/').collect();
        if parts.len() >= 7 {
            let lo = parts[5].parse().unwrap_or(0);
            let hi = parts[6].parse().unwrap_or(999);
            return (lo, hi);
        }
        (0, 999)
    }
}
```

---

## 4. Thailand Aeronautical Data Sources {#nasr}

For Thailand operations (JFOX context):

| Dataset | Source | Format | Update Cycle |
|---|---|---|---|
| Thailand AIP | AEROTHAI (aerothai.or.th) | PDF / AIXM | AIRAC 28-day |
| Bangkok FIR (VTBB) | ICAO WACAF | AIXM 5.1 | AIRAC |
| NOTAMs | ICAO NOTAM API / AFTN | ICAO Format | Real-time |
| OpenAIP Thailand | openAIP.net API | GeoJSON | Weekly |
| OurAirports | ourairports.com/data | CSV | Monthly |
| Digital Elevation | SRTM30 / ALOS DEM | GeoTIFF | Static |

```rust
// gcs-airspace/src/providers/openairspace.rs
use reqwest::Client;
use serde::Deserialize;

pub struct OpenAirspaceProvider {
    client: Client,
    api_key: String,
    base_url: String,
}

impl OpenAirspaceProvider {
    pub async fn fetch_airspaces_in_bbox(
        &self,
        min_lat: f64, min_lon: f64,
        max_lat: f64, max_lon: f64,
    ) -> anyhow::Result<serde_json::Value> {
        let url = format!(
            "{}/v1/airspaces?apiKey={}&from[lat]={}&from[lng]={}&to[lat]={}&to[lng]={}",
            self.base_url, self.api_key,
            min_lat, min_lon, max_lat, max_lon
        );
        let resp = self.client
            .get(&url)
            .timeout(std::time::Duration::from_secs(15))
            .send()
            .await?
            .json::<serde_json::Value>()
            .await?;
        Ok(resp)
    }

    pub async fn fetch_notams_for_region(&self, icao_prefix: &str) -> anyhow::Result<Vec<crate::notam::Notam>> {
        // Use ICAO NOTAM API or AFTN connection
        let url = format!("{}/v1/notams?fir={}", self.base_url, icao_prefix);
        let raw_notams: Vec<String> = self.client.get(&url)
            .send().await?
            .json().await?;
        raw_notams.iter()
            .map(|raw| crate::notam::NotamParser::parse(raw))
            .collect()
    }
}

// OurAirports CSV ingestion
pub async fn import_ourairports_csv(
    csv_path: &str,
    db: &sqlx::SqlitePool,
) -> anyhow::Result<usize> {
    let mut rdr = csv::Reader::from_path(csv_path)?;
    let mut count = 0;
    for result in rdr.deserialize::<OurAirportsRecord>() {
        let rec = result?;
        sqlx::query!(
            r#"INSERT OR REPLACE INTO airports (icao, iata, name, lat, lon, elevation_m, country, type)
               VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)"#,
            rec.ident, rec.iata_code, rec.name, rec.latitude_deg, rec.longitude_deg,
            rec.elevation_ft.map(|e| e * 0.3048), rec.iso_country, rec.type_,
        ).execute(db).await?;
        count += 1;
    }
    Ok(count)
}

#[derive(Deserialize)]
struct OurAirportsRecord {
    ident: String,
    iata_code: Option<String>,
    name: String,
    latitude_deg: f64,
    longitude_deg: f64,
    elevation_ft: Option<f64>,
    iso_country: String,
    #[serde(rename = "type")]
    type_: String,
}
```

---

## 5. PostGIS Spatial Queries {#postgis}

```rust
// gcs-airspace/src/spatial_queries.rs

pub struct AirspaceQueryEngine {
    db: sqlx::PgPool,  // PostGIS for server, switch to SpatiaLite for offline
}

impl AirspaceQueryEngine {
    /// Find all airspaces a position is inside (vertical + lateral check)
    pub async fn airspaces_containing(
        &self,
        lat: f64, lon: f64, alt_m: f64,
    ) -> anyhow::Result<Vec<AirspaceRecord>> {
        let records = sqlx::query_as!(
            AirspaceRecord,
            r#"SELECT id, name, icao_class, airspace_type,
                      lower_limit_m, upper_limit_m, lower_limit_ref
               FROM airspaces
               WHERE ST_Contains(geom, ST_SetSRID(ST_MakePoint($1, $2), 4326))
                 AND ($3 >= COALESCE(lower_limit_m, 0))
                 AND ($3 <= upper_limit_m)
               ORDER BY upper_limit_m - lower_limit_m ASC"#,
            lon, lat, alt_m
        ).fetch_all(&self.db).await?;
        Ok(records)
    }

    /// Find airspaces intersecting a flight path (linestring + altitude band)
    pub async fn airspaces_along_path(
        &self,
        waypoints: &[(f64, f64, f64)],  // (lon, lat, alt_m)
        buffer_m: f64,
    ) -> anyhow::Result<Vec<AirspaceRecord>> {
        let line_wkt = Self::waypoints_to_linestring_wkt(waypoints);
        let min_alt = waypoints.iter().map(|w| w.2).fold(f64::INFINITY, f64::min);
        let max_alt = waypoints.iter().map(|w| w.2).fold(f64::NEG_INFINITY, f64::max);

        let records = sqlx::query_as!(
            AirspaceRecord,
            r#"SELECT DISTINCT a.id, a.name, a.icao_class, a.airspace_type,
                      a.lower_limit_m, a.upper_limit_m, a.lower_limit_ref
               FROM airspaces a
               WHERE ST_Intersects(
                   a.geom,
                   ST_Buffer(ST_GeomFromText($1, 4326)::geography, $2)::geometry
               )
                 AND NOT (a.upper_limit_m < $3 OR COALESCE(a.lower_limit_m, 0) > $4)"#,
            line_wkt, buffer_m, min_alt, max_alt
        ).fetch_all(&self.db).await?;
        Ok(records)
    }

    /// Active NOTAMs within bbox + time window
    pub async fn active_notams_in_bbox(
        &self,
        min_lat: f64, min_lon: f64,
        max_lat: f64, max_lon: f64,
        at_time: chrono::DateTime<chrono::Utc>,
    ) -> anyhow::Result<Vec<NotamRecord>> {
        sqlx::query_as!(
            NotamRecord,
            r#"SELECT id, q_code, fir, text_e, effective_from, effective_to,
                      lower_fl, upper_fl, ST_AsGeoJSON(geom) as geom_json
               FROM notams
               WHERE ST_Intersects(geom, ST_MakeEnvelope($1, $2, $3, $4, 4326))
                 AND effective_from <= $5
                 AND (effective_to IS NULL OR effective_to >= $5)"#,
            min_lon, min_lat, max_lon, max_lat, at_time
        ).fetch_all(&self.db).await?;
        // Note: raw sqlx doesn't support chrono DateTime directly with SQLite
        // Use .map() to convert types as needed
        Ok(records)
    }

    fn waypoints_to_linestring_wkt(waypoints: &[(f64, f64, f64)]) -> String {
        let coords: Vec<String> = waypoints.iter()
            .map(|(lon, lat, _)| format!("{} {}", lon, lat))
            .collect();
        format!("LINESTRING({})", coords.join(", "))
    }
}
```

---

## 6. Geofence Engine {#geofence}

```rust
// gcs-geofence/src/engine.rs
use geo::{Contains, Polygon, Point, LineString, coord};
use crate::core::types::{GeoPoint, VehicleId};
use crate::core::events::{GcsEvent, EventBus};
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct GeofenceZone {
    pub id: Uuid,
    pub name: String,
    pub zone_type: GeofenceType,
    pub boundary: Vec<(f64, f64)>,   // (lon, lat) polygon
    pub min_alt_m: Option<f64>,
    pub max_alt_m: Option<f64>,
    pub action: GeofenceAction,
    pub active: bool,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum GeofenceType { Inclusion, Exclusion }

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum GeofenceAction { Alert, RTH, Land, HoldPosition }

pub struct GeofenceEngine {
    zones: Vec<GeofenceZone>,
    last_violation: HashMap<(VehicleId, Uuid), std::time::Instant>,
    alert_cooldown: std::time::Duration,
    event_bus: EventBus,
}

impl GeofenceEngine {
    pub fn new(event_bus: EventBus) -> Self {
        Self {
            zones: Vec::new(),
            last_violation: HashMap::new(),
            alert_cooldown: std::time::Duration::from_secs(5),
            event_bus,
        }
    }

    pub fn load_zones(&mut self, zones: Vec<GeofenceZone>) {
        self.zones = zones.into_iter().filter(|z| z.active).collect();
    }

    /// Check if a vehicle position violates any geofence
    pub fn check_position(&mut self, vehicle_id: VehicleId, pos: &GeoPoint) -> Vec<GeofenceViolation> {
        let point = Point::new(pos.lon, pos.lat);
        let mut violations = Vec::new();

        for zone in &self.zones {
            let poly = self.zone_to_polygon(zone);
            let inside = poly.contains(&point);

            // Altitude check
            let alt_ok = match (zone.min_alt_m, zone.max_alt_m) {
                (Some(min), Some(max)) => pos.alt >= min && pos.alt <= max,
                (Some(min), None) => pos.alt >= min,
                (None, Some(max)) => pos.alt <= max,
                (None, None) => true,
            };

            let violation = match zone.zone_type {
                GeofenceType::Exclusion => inside,
                GeofenceType::Inclusion => !inside,
            } && alt_ok;

            if violation {
                let key = (vehicle_id, zone.id);
                let now = std::time::Instant::now();
                let recently_alerted = self.last_violation.get(&key)
                    .map(|t| now.duration_since(*t) < self.alert_cooldown)
                    .unwrap_or(false);

                if !recently_alerted {
                    self.last_violation.insert(key, now);
                    tracing::warn!(
                        "Geofence violation: vehicle={} zone={} type={:?}",
                        vehicle_id, zone.name, zone.zone_type
                    );
                    let _ = self.event_bus.send(GcsEvent::GeofenceViolation {
                        vehicle_id,
                        zone_id: zone.id,
                    });
                    violations.push(GeofenceViolation {
                        zone_id: zone.id,
                        zone_name: zone.name.clone(),
                        action: zone.action.clone(),
                    });
                }
            }
        }
        violations
    }

    fn zone_to_polygon(&self, zone: &GeofenceZone) -> Polygon {
        Polygon::new(
            LineString::from(
                zone.boundary.iter().map(|(lon, lat)| coord! { x: *lon, y: *lat }).collect::<Vec<_>>()
            ),
            vec![],
        )
    }
}

#[derive(Debug, Clone)]
pub struct GeofenceViolation {
    pub zone_id: Uuid,
    pub zone_name: String,
    pub action: GeofenceAction,
}
```

---

## 7. TFR / CTR Overlay Pipeline {#tfr-ctr}

```
[AIXM/NOTAM Source] → [Parser] → [Normalizer] → [DB Insert]
                                                      ↓
                                             [Spatial Index]
                                                      ↓
                        [Query by BBox + Alt Band + Time]
                                                      ↓
                               [GeoJSON FeatureCollection]
                                                      ↓
                               [Cesium Airspace Overlay]
```

---

## 8. ICAO Airspace Classification {#icao}

| Class | IFR Sep | VFR Sep | ATC Service | Required |
|---|---|---|---|---|
| A | Yes | No IFR only | Yes | Clearance |
| B | Yes | Yes | Yes | Clearance |
| C | Yes | VFR/IFR sep | Yes | Clearance+squawk |
| D | Yes | Traffic info | Yes | Clearance |
| E | Yes | No | Yes (IFR) | No (VFR) |
| F | As available | No | Advisory | No |
| G | No | No | No | No |

**Thailand (VTBB FIR) specific:**
- Class A: FL195 and above (FL195–FL600)
- Class C: Major TMA around Bangkok/Chiang Mai
- Class D: CTR around regional airports (VTCC = Chiang Mai)
- Drone ops: require CAAT approval + NOTAM for Class C/D
- UAS below 30m AGL / 2kg: registration but no approval in Class G