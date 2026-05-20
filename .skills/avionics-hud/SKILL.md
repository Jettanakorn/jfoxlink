---
name: avionics-hud
description: >
  Expert-level AI agent skill for designing and building Garmin G1000/G3000-style glass cockpit
  avionics displays in Rust + WebAssembly + WebGL/Canvas. Covers Primary Flight Display (PFD),
  Multi-Function Display (MFD), Heads-Up Display (HUD), Electronic Flight Instrument System (EFIS),
  Engine Indication and Crew Alerting System (EICAS), Maneuvering Characteristics Augmentation
  System (MCAS), synthetic vision terrain, moving map, autopilot annunciator, and full avionics
  bus architecture (ARINC 429, CAN, RS-422). Trigger for ANY request involving: cockpit UI,
  flight instruments, attitude indicator, airspeed tape, altimeter, VSI, HSI, heading indicator,
  NAV/COM radio panels, engine gauges, caution/warning system, checklist system, avionics
  integration, UAV GCS instruments, or glass cockpit rendering — even if phrased as "make a
  flight display", "build an AHRS visualizer", or "show engine data like a real plane". L99.
compatibility:
  - Rust stable ≥ 1.78 (wasm32-unknown-unknown + native)
  - wasm-bindgen 0.2, wasm-pack
  - WebGL 2.0 (web-sys WebGl2RenderingContext) or Canvas2D (fast prototyping)
  - egui / eframe for embedded/desktop instrument panels
  - nalgebra for quaternion attitude math
  - ARINC 429 / RS-422 / CAN bus data ingestion
  - DO-178C Level C awareness (airborne software guidance)
  - DO-256 (EFIS design standard) awareness
---

# Avionics HUD & Glass Cockpit — Rust + WASM + WebGL

## Skill Overview

This skill makes Claude an L99 avionics display engineer — producing production-quality
Rust code for all glass cockpit instruments, rendering pipelines, data bus decoders,
failure logic, annunciator systems, and MCAS/autopilot integration layers.

**Design reference:** Garmin G1000 NXi / G3000 / G5000 philosophy — but architecture
is kept generic so it applies equally to UAV GCS instrument panels, military HUD,
and experimental aircraft EFIS.

---

## Quick Decision Tree

```
Request involves avionics display?
├── PFD (attitude, airspeed, alt, VSI, heading)
│     → Read references/pfd-instruments.md
├── MFD (moving map, engine page, checklist, wx)
│     → Read references/mfd-engine.md
├── HUD (combiner glass overlay, SVS, flight path marker)
│     → Read references/hud-svg.md
├── MCAS / Autopilot / Flight envelope protection
│     → Read references/mcas-autopilot.md
├── Avionics bus (ARINC 429, CAN, RS-422, AFDX)
│     → Read references/avionics-bus.md
└── Full glass cockpit scaffold (all instruments)
      → Read ALL reference files in order
```

---

## System Architecture

```
┌──────────────────────────────────────────────────────────────────┐
│                    GLASS COCKPIT RENDER SHELL                     │
│          WebGL 2.0 Canvas (browser) · egui (desktop/UAV)          │
├──────────────────────────────────────────────────────────────────┤
│  PFD Panel (left)          │   MFD Panel (right/center)           │
│  ┌────────────────────┐    │   ┌──────────────────────────────┐   │
│  │ Attitude (AHRS)    │    │   │ Moving Map (Cesium/SVG)      │   │
│  │ Airspeed tape      │    │   │ Engine Instruments (EICAS)   │   │
│  │ Altimeter tape     │    │   │ Traffic (TIS-B/ADS-B)        │   │
│  │ VSI                │    │   │ Weather overlay (Nexrad)     │   │
│  │ HSI / CDI          │    │   │ Checklist / procedures       │   │
│  │ Autopilot ann.     │    │   └──────────────────────────────┘   │
│  └────────────────────┘    │                                      │
├──────────────────────────────────────────────────────────────────┤
│                    HUD COMBINER OVERLAY                           │
│    Flight path marker · Speed · Altitude · Horizon line           │
├──────────────────────────────────────────────────────────────────┤
│              INSTRUMENT COMPUTE LAYER (Rust/WASM)                 │
│  AHRS engine │ Air data computer │ Navigation computer            │
│  Failure monitor │ Annunciator logic │ MCAS envelope              │
├──────────────────────────────────────────────────────────────────┤
│              AVIONICS DATA BUS (native Rust)                      │
│  ARINC 429 decoder │ CAN/UAVCAN │ RS-422 │ NMEA │ GDL 90         │
└──────────────────────────────────────────────────────────────────┘
```

---

## Reference Files

| File | Read When... |
|---|---|
| `references/pfd-instruments.md` | Building attitude indicator, airspeed tape, altimeter, VSI, HSI, slip/skid ball, compass rose, FD bars |
| `references/mfd-engine.md` | Building engine page (RPM/MP/fuel/EGT/CHT), moving map, traffic, weather overlay, checklist |
| `references/hud-svg.md` | Designing HUD symbology, flight path marker, velocity vector, conformal overlays, SVG/WebGL HUD renderer |
| `references/mcas-autopilot.md` | Implementing MCAS trim logic, autopilot modes (AP/FD/AT), flight envelope protection, trim runaway detection |
| `references/avionics-bus.md` | ARINC 429 word decode, CAN avionics, RS-422 AHRS/ADC frames, GDL 90 ADS-B, NMEA 0183/2000 |

---

## Claude's Behavioral Contract

When invoked, Claude MUST:

1. **Identify the instrument/subsystem** and read the matching reference file first.
2. **Generate production Rust/WASM code** — real implementations, not placeholder stubs.
   Use `wasm-bindgen`, `web-sys`, `nalgebra`, `serde`. No `unwrap()` on flight-data paths.
3. **Respect DO-178C Level C patterns** — redundant data validation, range clamping, failed-flag rendering (red X flag over instrument on data loss), never panic in WASM flight path.
4. **Apply Garmin G1000 visual conventions** — black background, aviation amber/green/cyan/magenta/white color semantics, Cheltenham/sans tape fonts, correct tape scaling per FAR/EASA standards.
5. **Provide pixel-accurate geometry** for instrument arcs, tapes, and needles — specify SVG paths or WebGL vertex data with correct angular extents.
6. **Annotate safety-critical logic** — e.g., MCAS authority limits, envelope protection thresholds, annunciator priority levels per ARP4761.

---

## Garmin G1000 Color Conventions (MUST follow)

| Color | Hex (approx) | Used for |
|---|---|---|
| Instrument background | `#000000` | All instrument backgrounds |
| Sky (attitude) | `#1A6FA8` | ADI sky half |
| Ground (attitude) | `#6B3D10` | ADI ground half |
| Horizon line | `#FFFFFF` | ADI horizon, pitch ladder |
| Magenta | `#E040FB` | Active nav source (GPS/VLOC), FD command bars |
| Cyan | `#00BCD4` | Selected values, preselected altitude, MCP bugs |
| Green | `#4CAF50` | Normal operating range arcs, confirmed data |
| White | `#FFFFFF` | Current values, tape numbers |
| Amber | `#FFB300` | Caution, near limits, EICAS advisory |
| Red | `#F44336` | Warning, out-of-limit, failed instrument flag |
| Gray | `#616161` | Inoperative range, unavailable |
| Aviation Blue | `#1565C0` | Sky fill variation per instrument |

---

## Garmin G1000 Standard Instrument Scales

| Instrument | Range | Tape scale |
|---|---|---|
| Airspeed (GA piston) | 0–200 kts | 10 kts / major tick; 20 px/kt typical |
| Airspeed (turboprop) | 0–400 kts | 20 kts / major tick |
| Altitude | –1000 to +50000 ft | 100 ft / major tick |
| VSI | ±2000 fpm (GA) | Linear; pointer only, no tape |
| Heading | 0–360° | Compass rose, full 360° arc |
| Bank angle | ±60° + 65° slip | Arc with 10/20/30/45/60° marks |
| Pitch | ±90° | Ladder, 5° lines, 10° chevrons |

---

## Glossary

| Term | Definition |
|---|---|
| ADI | Attitude Director Indicator — artificial horizon |
| AHRS | Attitude and Heading Reference System |
| ADC | Air Data Computer — provides airspeed, altitude, VSI |
| FD | Flight Director — magenta command bars on ADI |
| AP | Autopilot |
| AT | Autothrottle |
| HSI | Horizontal Situation Indicator — compass rose with nav needles |
| CDI | Course Deviation Indicator |
| VSI / VVI | Vertical Speed Indicator / Vertical Velocity Indicator |
| PFD | Primary Flight Display |
| MFD | Multi-Function Display |
| EFIS | Electronic Flight Instrument System |
| EICAS | Engine Indication and Crew Alerting System |
| MCAS | Maneuvering Characteristics Augmentation System |
| SVS | Synthetic Vision System — 3D terrain on PFD |
| HUD | Heads-Up Display |
| FPM | Flight Path Marker (HUD) |
| PNI | Pictorial Navigation Indicator (HSI variant) |
| ARINC 429 | Avionics data bus standard (12.5 / 100 kbps) |
| GDL 90 | Garmin ADS-B data link format |
| TIS-B | Traffic Information Service — Broadcast |
| Vne | Never-exceed speed |
| Vno | Maximum structural cruising speed |
| Vfe | Maximum flap extended speed |
| Vs0 | Stall speed in landing configuration |