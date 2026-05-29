# Avionics Bus Reference — ARINC 429, CAN, RS-422, GDL 90, NMEA

## Table of Contents
1. [ARINC 429 Word Decoder](#arinc429)
2. [RS-422 AHRS Frame (Garmin GSU 75)](#rs422)
3. [GDL 90 ADS-B Decoder (Traffic)](#gdl90)
4. [NMEA 0183 / NMEA 2000](#nmea)
5. [CAN Avionics Bus](#can)
6. [Data Concentrator — Rust Actor](#concentrator)

---

## 1. ARINC 429 Word Decoder {#arinc429}

ARINC 429 is the standard avionics bus. Each word is 32 bits:

```
[Parity 1b][SSM 2b][Data 19b][SDI 2b][Label 8b]
```

**Label** (octal) defines the parameter. SSM = Sign/Status Matrix:
- 00 = Failure Warning
- 01 = No Computed Data
- 10 = Functional Test
- 11 = Normal Operation

```rust
// avionics-bus/src/arinc429.rs

#[derive(Debug, Clone)]
pub struct Arinc429Word(pub u32);

impl Arinc429Word {
    pub fn label(&self) -> u8 {
        // Label is octal, transmitted LSB first — reverse bit order
        let raw = (self.0 & 0xFF) as u8;
        raw.reverse_bits()
    }

    pub fn ssm(&self) -> u8 {
        ((self.0 >> 29) & 0x3) as u8
    }

    pub fn is_normal_ops(&self) -> bool { self.ssm() == 0b11 }
    pub fn is_failure_warn(&self) -> bool { self.ssm() == 0b00 }

    pub fn sdi(&self) -> u8 { ((self.0 >> 8) & 0x3) as u8 }

    /// Extract BNR (binary) data field — bits 11–29 (19 bits)
    pub fn bnr_value(&self) -> i32 {
        let raw = ((self.0 >> 10) & 0x7FFFF) as i32;
        // Sign-extend from bit 18
        if raw & (1 << 18) != 0 { raw | !0x7FFFF } else { raw }
    }

    /// BNR to engineering units given range
    pub fn bnr_to_eu(&self, full_scale: f64) -> Option<f64> {
        if !self.is_normal_ops() { return None; }
        // 19-bit two's complement, 1 LSB = full_scale / 2^18
        let lsb = full_scale / (1 << 18) as f64;
        Some(self.bnr_value() as f64 * lsb)
    }

    /// BCD digit extraction (for labels like altitude)
    pub fn bcd_value(&self) -> u32 {
        let d1 = (self.0 >> 28) & 0xF;
        let d2 = (self.0 >> 24) & 0xF;
        let d3 = (self.0 >> 20) & 0xF;
        let d4 = (self.0 >> 16) & 0xF;
        let d5 = (self.0 >> 12) & 0xF;
        d1 * 10000 + d2 * 1000 + d3 * 100 + d4 * 10 + d5
    }

    pub fn odd_parity_valid(&self) -> bool {
        self.0.count_ones() % 2 == 1
    }
}

/// Standard ARINC 429 label definitions (octal labels → parameter)
pub mod labels {
    pub const LABEL_ALTITUDE_BARO: u8    = 0o203; // 131 dec
    pub const LABEL_AIRSPEED_CAS: u8     = 0o206; // 134 dec
    pub const LABEL_AIRSPEED_TAS: u8     = 0o210; // 136 dec
    pub const LABEL_MACH: u8             = 0o205; // 133 dec
    pub const LABEL_PITCH_ANGLE: u8      = 0o324; // 212 dec
    pub const LABEL_ROLL_ANGLE: u8       = 0o325; // 213 dec
    pub const LABEL_HEADING_MAG: u8      = 0o320; // 208 dec
    pub const LABEL_LATITUDE: u8         = 0o310; // 200 dec
    pub const LABEL_LONGITUDE: u8        = 0o311; // 201 dec
    pub const LABEL_GROUNDSPEED: u8      = 0o312; // 202 dec
    pub const LABEL_TRACK_ANGLE: u8      = 0o313; // 203 dec
    pub const LABEL_VSI_BARO: u8         = 0o212; // 138 dec
    pub const LABEL_FUEL_FLOW: u8        = 0o166; // 118 dec
}

pub struct Arinc429Decoder {
    pub last_words: std::collections::HashMap<u8, Arinc429Word>,
}

impl Arinc429Decoder {
    pub fn ingest(&mut self, word: Arinc429Word) -> Option<DecodedParam> {
        if !word.odd_parity_valid() { return None; }
        let label = word.label();
        self.last_words.insert(label, word.clone());

        use labels::*;
        match label {
            LABEL_ALTITUDE_BARO => word.bnr_to_eu(131072.0).map(|v| DecodedParam::AltitudeFt(v)),
            LABEL_AIRSPEED_CAS  => word.bnr_to_eu(512.0).map(|v| DecodedParam::CasKts(v)),
            LABEL_AIRSPEED_TAS  => word.bnr_to_eu(512.0).map(|v| DecodedParam::TasKts(v)),
            LABEL_MACH          => word.bnr_to_eu(4.0).map(|v| DecodedParam::Mach(v)),
            LABEL_PITCH_ANGLE   => word.bnr_to_eu(180.0).map(|v| DecodedParam::PitchDeg(v)),
            LABEL_ROLL_ANGLE    => word.bnr_to_eu(180.0).map(|v| DecodedParam::RollDeg(v)),
            LABEL_HEADING_MAG   => word.bnr_to_eu(360.0).map(|v| DecodedParam::HeadingDeg(v)),
            LABEL_LATITUDE      => word.bnr_to_eu(90.0).map(|v| DecodedParam::LatitudeDeg(v)),
            LABEL_LONGITUDE     => word.bnr_to_eu(180.0).map(|v| DecodedParam::LongitudeDeg(v)),
            LABEL_VSI_BARO      => word.bnr_to_eu(16384.0).map(|v| DecodedParam::VsiFpm(v)),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub enum DecodedParam {
    AltitudeFt(f64), CasKts(f64), TasKts(f64), Mach(f64),
    PitchDeg(f64), RollDeg(f64), HeadingDeg(f64),
    LatitudeDeg(f64), LongitudeDeg(f64), VsiFpm(f64),
    FuelFlowGph(f64),
}
```

---

## 2. RS-422 AHRS Frame (Garmin GSU 75) {#rs422}

Garmin GSU 75/76 AHRS outputs RS-422 at 115,200 baud, 100 Hz.
Frame format: `$GPATT,pitch,roll,heading,rate_p,rate_q,rate_r,ax,ay,az*cs\r\n`

```rust
// avionics-bus/src/rs422_ahrs.rs
use tokio_serial::SerialPortBuilderExt;
use tokio::io::{AsyncBufReadExt, BufReader};

pub struct GarminAhrsDecoder {
    port: BufReader<tokio_serial::SerialStream>,
}

#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct AhrsFrame {
    pub pitch_deg: f64,
    pub roll_deg: f64,
    pub heading_deg: f64,
    pub rate_p_deg_s: f64,  // roll rate
    pub rate_q_deg_s: f64,  // pitch rate
    pub rate_r_deg_s: f64,  // yaw rate
    pub accel_x_g: f64,
    pub accel_y_g: f64,
    pub accel_z_g: f64,
    pub valid: bool,
}

impl GarminAhrsDecoder {
    pub async fn open(port_path: &str, baud: u32) -> anyhow::Result<Self> {
        let port = tokio_serial::new(port_path, baud)
            .open_native_async()?;
        Ok(Self { port: BufReader::new(port) })
    }

    pub async fn next_frame(&mut self) -> anyhow::Result<AhrsFrame> {
        let mut line = String::new();
        self.port.read_line(&mut line).await?;
        Self::parse_line(line.trim())
    }

    fn parse_line(line: &str) -> anyhow::Result<AhrsFrame> {
        // Validate checksum
        if !Self::nmea_checksum_valid(line) {
            anyhow::bail!("AHRS checksum fail: {}", line);
        }

        if !line.starts_with("$GPATT,") {
            anyhow::bail!("Not GPATT sentence");
        }

        let parts: Vec<&str> = line
            .trim_start_matches('$')
            .split('*').next().unwrap_or("")
            .split(',')
            .collect();

        if parts.len() < 10 {
            anyhow::bail!("GPATT too short: {} fields", parts.len());
        }

        Ok(AhrsFrame {
            pitch_deg: parts[1].parse().unwrap_or(0.0),
            roll_deg: parts[2].parse().unwrap_or(0.0),
            heading_deg: parts[3].parse().unwrap_or(0.0),
            rate_p_deg_s: parts[4].parse().unwrap_or(0.0),
            rate_q_deg_s: parts[5].parse().unwrap_or(0.0),
            rate_r_deg_s: parts[6].parse().unwrap_or(0.0),
            accel_x_g: parts[7].parse().unwrap_or(0.0),
            accel_y_g: parts[8].parse().unwrap_or(0.0),
            accel_z_g: parts[9].split('*').next().unwrap_or("0").parse().unwrap_or(0.0),
            valid: true,
        })
    }

    fn nmea_checksum_valid(sentence: &str) -> bool {
        if let Some(star_pos) = sentence.rfind('*') {
            let payload = &sentence[1..star_pos];
            let expected_cs = u8::from_str_radix(&sentence[star_pos+1..], 16).unwrap_or(0);
            let calc_cs = payload.bytes().fold(0u8, |acc, b| acc ^ b);
            calc_cs == expected_cs
        } else { false }
    }
}
```

---

## 3. GDL 90 ADS-B Decoder (Traffic) {#gdl90}

GDL 90 is Garmin's format for ADS-B traffic over UDP port 4000.

```rust
// avionics-bus/src/gdl90.rs

#[derive(Debug, Clone, serde::Serialize)]
pub struct TrafficReport {
    pub icao_addr: u32,
    pub callsign: String,
    pub lat: f64, pub lon: f64,
    pub altitude_ft: i32,
    pub ground_speed_kts: u16,
    pub track_deg: f64,
    pub vsi_fpm: i32,
    pub alert: bool,
    pub on_ground: bool,
    pub nic: u8,    // Navigation Integrity Category
    pub nacp: u8,   // Navigation Accuracy Category Position
}

pub struct Gdl90Decoder;

impl Gdl90Decoder {
    const FLAG_BYTE: u8 = 0x7E;
    const ESCAPE_BYTE: u8 = 0x7D;

    /// Parse a raw GDL 90 UDP datagram; returns all contained messages
    pub fn decode_datagram(data: &[u8]) -> Vec<Gdl90Message> {
        let mut messages = Vec::new();
        let mut i = 0;
        while i < data.len() {
            if data[i] == Self::FLAG_BYTE && i + 1 < data.len() {
                if let Some((msg, consumed)) = Self::parse_frame(&data[i..]) {
                    messages.push(msg);
                    i += consumed;
                    continue;
                }
            }
            i += 1;
        }
        messages
    }

    fn parse_frame(data: &[u8]) -> Option<(Gdl90Message, usize)> {
        if data.len() < 4 || data[0] != Self::FLAG_BYTE { return None; }

        // Unescape payload
        let mut payload = Vec::new();
        let mut i = 1;
        while i < data.len() && data[i] != Self::FLAG_BYTE {
            if data[i] == Self::ESCAPE_BYTE && i + 1 < data.len() {
                payload.push(data[i+1] ^ 0x20);
                i += 2;
            } else {
                payload.push(data[i]);
                i += 1;
            }
        }
        if payload.len() < 3 { return None; }

        let msg_id = payload[0];
        let body = &payload[1..payload.len()-2]; // strip CRC

        let msg = match msg_id {
            0x00 => Gdl90Message::Heartbeat(Self::parse_heartbeat(body)?),
            0x0A => Gdl90Message::OwnshipReport(Self::parse_traffic(body)?),
            0x14 => Gdl90Message::TrafficReport(Self::parse_traffic(body)?),
            0x1E => Gdl90Message::GeometricAltitude(Self::parse_geo_alt(body)?),
            _ => Gdl90Message::Unknown(msg_id),
        };
        Some((msg, i + 1))
    }

    fn parse_traffic(data: &[u8]) -> Option<TrafficReport> {
        if data.len() < 27 { return None; }

        let alert    = (data[0] >> 4) & 0x0F;
        let addr_type = data[0] & 0x0F;
        let icao_addr = (data[1] as u32) << 16 | (data[2] as u32) << 8 | data[3] as u32;

        // Latitude: 24-bit two's complement, 180/2^23 deg/LSB
        let lat_raw = i32::from(data[4]) << 16 | i32::from(data[5]) << 8 | i32::from(data[6]);
        let lat = lat_raw as f64 * (180.0 / 8_388_608.0);

        let lon_raw = i32::from(data[7]) << 16 | i32::from(data[8]) << 8 | i32::from(data[9]);
        let lon = lon_raw as f64 * (180.0 / 8_388_608.0);

        // Altitude: 12 bits, 25 ft res, -1000 ft offset
        let alt_raw = (data[10] as u16) << 4 | (data[11] as u16 >> 4);
        let altitude_ft = if alt_raw == 0xFFF { 0 } else { alt_raw as i32 * 25 - 1000 };

        let misc = data[11] & 0x0F;
        let on_ground = (misc & 0x08) != 0;

        let nic = (data[12] >> 4) & 0x0F;
        let nacp = data[12] & 0x0F;

        let speed_raw = (data[13] as u16) << 4 | (data[14] as u16 >> 4);
        let ground_speed_kts = if speed_raw == 0xFFF { 0 } else { speed_raw };

        let vsi_raw = i16::from(data[15]) << 4 | i16::from(data[16] >> 4);
        let vsi_fpm = vsi_raw as i32 * 64;

        let track_raw = data[17] as u16 * 360 / 256;
        let track_deg = track_raw as f64;

        let callsign: String = data[18..26].iter()
            .map(|&c| if c.is_ascii_alphanumeric() { c as char } else { ' ' })
            .collect::<String>()
            .trim()
            .to_string();

        Some(TrafficReport {
            icao_addr, callsign, lat, lon, altitude_ft, ground_speed_kts,
            track_deg, vsi_fpm, alert: alert != 0, on_ground, nic, nacp,
        })
    }

    fn parse_heartbeat(_data: &[u8]) -> Option<u8> { Some(0) }
    fn parse_geo_alt(data: &[u8]) -> Option<i32> {
        if data.len() < 2 { return None; }
        let raw = i16::from_be_bytes([data[0], data[1]]);
        Some(raw as i32 * 5) // 5 ft resolution
    }
}

#[derive(Debug, Clone)]
pub enum Gdl90Message {
    Heartbeat(u8),
    OwnshipReport(TrafficReport),
    TrafficReport(TrafficReport),
    GeometricAltitude(i32),
    Unknown(u8),
}
```

---

## 4. NMEA 0183 / NMEA 2000 {#nmea}

```rust
// avionics-bus/src/nmea.rs

pub struct NmeaParser;

impl NmeaParser {
    pub fn parse_gga(sentence: &str) -> Option<GgaFix> {
        if !sentence.starts_with("$GPGGA") && !sentence.starts_with("$GNGGA") {
            return None;
        }
        let parts: Vec<&str> = sentence.split('*').next()?.split(',').collect();
        if parts.len() < 15 { return None; }

        let fix_quality: u8 = parts[6].parse().unwrap_or(0);
        if fix_quality == 0 { return None; }

        Some(GgaFix {
            lat: Self::parse_ddmm(parts[2], parts[3])?,
            lon: Self::parse_ddmm(parts[4], parts[5])?,
            altitude_m: parts[9].parse().unwrap_or(0.0),
            fix_quality,
            satellites: parts[7].parse().unwrap_or(0),
            hdop: parts[8].parse().unwrap_or(99.9),
        })
    }

    pub fn parse_rmc(sentence: &str) -> Option<RmcData> {
        if !sentence.starts_with("$GPRMC") && !sentence.starts_with("$GNRMC") {
            return None;
        }
        let parts: Vec<&str> = sentence.split('*').next()?.split(',').collect();
        if parts.len() < 12 || parts[2] != "A" { return None; }

        Some(RmcData {
            lat: Self::parse_ddmm(parts[3], parts[4])?,
            lon: Self::parse_ddmm(parts[5], parts[6])?,
            speed_kts: parts[7].parse().unwrap_or(0.0),
            track_deg: parts[8].parse().unwrap_or(0.0),
            mag_var: {
                let mv: f64 = parts[10].parse().unwrap_or(0.0);
                if parts[11] == "W" { -mv } else { mv }
            },
        })
    }

    fn parse_ddmm(value: &str, hemi: &str) -> Option<f64> {
        if value.is_empty() { return None; }
        let dot = value.find('.')?;
        if dot < 2 { return None; }
        let deg: f64 = value[..dot-2].parse().ok()?;
        let min: f64 = value[dot-2..].parse().ok()?;
        let dd = deg + min / 60.0;
        Some(if hemi == "S" || hemi == "W" { -dd } else { dd })
    }
}

#[derive(Debug, Clone)]
pub struct GgaFix {
    pub lat: f64, pub lon: f64,
    pub altitude_m: f64,
    pub fix_quality: u8, pub satellites: u8, pub hdop: f64,
}

#[derive(Debug, Clone)]
pub struct RmcData {
    pub lat: f64, pub lon: f64,
    pub speed_kts: f64, pub track_deg: f64, pub mag_var: f64,
}
```

---

## 5. Data Concentrator — Rust Actor {#concentrator}

Aggregates all bus inputs into a single `AircraftState` at 50 Hz for the PFD/MFD/HUD renderers:

```rust
// avionics-bus/src/concentrator.rs
use tokio::sync::{broadcast, mpsc};

pub struct DataConcentrator {
    rx_arinc: mpsc::Receiver<DecodedParam>,
    rx_ahrs: mpsc::Receiver<AhrsFrame>,
    rx_nmea: mpsc::Receiver<GgaFix>,
    rx_gdl90: mpsc::Receiver<Gdl90Message>,
    state_tx: broadcast::Sender<AircraftState>,
    state: AircraftState,
}

#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct AircraftState {
    // ADC
    pub cas_kts: f64, pub tas_kts: f64, pub mach: f64,
    pub altitude_ft: f64, pub vsi_fpm: f64, pub baro_in_hg: f64,
    // AHRS
    pub pitch_deg: f64, pub roll_deg: f64, pub heading_deg: f64,
    pub slip_deg: f64,
    pub body_p: f64, pub body_q: f64, pub body_r: f64,
    // Navigation
    pub lat: f64, pub lon: f64,
    pub track_deg: f64, pub ground_speed_kts: f64,
    // Engine (first engine)
    pub rpm: f64, pub map_inhg: f64,
    pub oil_p_psi: f64, pub oil_t_f: f64,
    pub egts_f: Vec<f64>, pub chts_f: Vec<f64>,
    pub fuel_left_gal: f64, pub fuel_right_gal: f64,
    pub fuel_flow_gph: f64,
    // Electrical
    pub main_bus_v: f64, pub battery_a: f64,
    // Traffic
    pub traffic: Vec<TrafficReport>,
    // Data validity timestamps
    pub last_ahrs_ms: u64, pub last_adc_ms: u64, pub last_gps_ms: u64,
}

impl DataConcentrator {
    pub async fn run(mut self) {
        let mut interval = tokio::time::interval(std::time::Duration::from_millis(20)); // 50 Hz
        loop {
            tokio::select! {
                _ = interval.tick() => {
                    let _ = self.state_tx.send(self.state.clone());
                }
                Some(param) = self.rx_arinc.recv() => {
                    self.apply_arinc(param);
                }
                Some(frame) = self.rx_ahrs.recv() => {
                    self.apply_ahrs(frame);
                }
                Some(fix) = self.rx_nmea.recv() => {
                    self.state.lat = fix.lat;
                    self.state.lon = fix.lon;
                    self.state.last_gps_ms = now_ms();
                }
                Some(msg) = self.rx_gdl90.recv() => {
                    if let Gdl90Message::TrafficReport(t) = msg {
                        self.upsert_traffic(t);
                    }
                }
            }
        }
    }

    fn apply_arinc(&mut self, p: DecodedParam) {
        match p {
            DecodedParam::AltitudeFt(v)   => { self.state.altitude_ft = v; self.state.last_adc_ms = now_ms(); }
            DecodedParam::CasKts(v)       => self.state.cas_kts = v,
            DecodedParam::TasKts(v)       => self.state.tas_kts = v,
            DecodedParam::Mach(v)         => self.state.mach = v,
            DecodedParam::VsiFpm(v)       => self.state.vsi_fpm = v,
            DecodedParam::PitchDeg(v)     => self.state.pitch_deg = v,
            DecodedParam::RollDeg(v)      => self.state.roll_deg = v,
            DecodedParam::HeadingDeg(v)   => self.state.heading_deg = v,
            _ => {}
        }
    }

    fn apply_ahrs(&mut self, f: AhrsFrame) {
        self.state.pitch_deg = f.pitch_deg;
        self.state.roll_deg  = f.roll_deg;
        self.state.heading_deg = f.heading_deg;
        self.state.body_p = f.rate_p_deg_s;
        self.state.body_q = f.rate_q_deg_s;
        self.state.body_r = f.rate_r_deg_s;
        self.state.last_ahrs_ms = now_ms();
    }

    fn upsert_traffic(&mut self, t: TrafficReport) {
        if let Some(existing) = self.state.traffic.iter_mut()
            .find(|tr| tr.icao_addr == t.icao_addr) {
            *existing = t;
        } else {
            self.state.traffic.push(t);
        }
        // Expire stale traffic > 60s
        let now = now_ms();
        self.state.traffic.retain(|_| true); // TODO: add timestamp to TrafficReport
    }
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}
```