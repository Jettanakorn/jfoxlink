# MFD & Engine Instruments Reference (EICAS/EIS)

## Table of Contents
1. [Engine Page Layout](#engine-page)
2. [RPM / Manifold Pressure Arc Gauges](#rpm-mp)
3. [EGT/CHT Bar Gauges](#egt-cht)
4. [Fuel Quantity & Flow](#fuel)
5. [Oil Pressure & Temperature](#oil)
6. [Electrical Bus Monitor](#electrical)
7. [MFD Moving Map Integration](#moving-map)
8. [EICAS Alerting Logic](#eicas-alerts)

---

## 1. Engine Page Layout (G1000-style) {#engine-page}

```
┌──────────────────────────────────────────────────────┐
│         ENG 1                     ENG 2 (if twin)    │
│   RPM [arc]   MAP [arc]     RPM [arc]   MAP [arc]    │
│   ○────────○  ○────────○                             │
├──────────────────────────────────────────────────────┤
│   EGT/CHT bar columns (6 cylinders typical)          │
│   |||  |||  |||  |||  |||  |||                       │
├──────────────────────────────────────────────────────┤
│   FUEL L [qty+flow]   FUEL R [qty+flow]              │
│   OIL P/T  │  VOLTS  │  AMPS  │  VACUUM              │
└──────────────────────────────────────────────────────┘
```

---

## 2. RPM / Manifold Pressure Arc Gauges {#rpm-mp}

```rust
// avionics-mfd/src/arc_gauge.rs
use web_sys::CanvasRenderingContext2d;
use std::f64::consts::PI;

pub struct ArcGaugeConfig {
    pub cx: f64, pub cy: f64, pub radius: f64,
    pub start_angle_deg: f64,    // e.g. 225° (lower-left)
    pub sweep_deg: f64,          // e.g. 270° full sweep
    pub min_val: f64,
    pub max_val: f64,
    pub green_lo: f64, pub green_hi: f64,
    pub yellow_lo: Option<f64>, pub yellow_hi: Option<f64>,
    pub red_hi: Option<f64>,
    pub label: &'static str,
    pub unit: &'static str,
}

impl ArcGaugeConfig {
    /// Standard RPM gauge config for Lycoming IO-360
    pub fn rpm_lycoming() -> Self {
        Self {
            cx: 100.0, cy: 100.0, radius: 70.0,
            start_angle_deg: 220.0, sweep_deg: 260.0,
            min_val: 0.0, max_val: 3000.0,
            green_lo: 600.0, green_hi: 2700.0,
            yellow_lo: Some(2700.0), yellow_hi: Some(2800.0),
            red_hi: Some(2800.0),
            label: "RPM", unit: "×100",
        }
    }

    /// Manifold Pressure (inches Hg)
    pub fn manifold_pressure() -> Self {
        Self {
            cx: 260.0, cy: 100.0, radius: 70.0,
            start_angle_deg: 220.0, sweep_deg: 260.0,
            min_val: 10.0, max_val: 35.0,
            green_lo: 12.0, green_hi: 29.5,
            yellow_lo: Some(29.5), yellow_hi: Some(31.0),
            red_hi: Some(31.0),
            label: "MAP", unit: "IN",
        }
    }
}

pub fn render_arc_gauge(ctx: &CanvasRenderingContext2d, cfg: &ArcGaugeConfig, value: f64) {
    let start_rad = (cfg.start_angle_deg - 90.0).to_radians();
    let sweep_rad = cfg.sweep_deg.to_radians();

    // Background arc
    ctx.set_stroke_style_str("#333333");
    ctx.set_line_width(8.0);
    ctx.begin_path();
    ctx.arc(cfg.cx, cfg.cy, cfg.radius, start_rad, start_rad + sweep_rad).unwrap();
    ctx.stroke();

    // Green arc
    let green_start = start_rad + ((cfg.green_lo - cfg.min_val) / (cfg.max_val - cfg.min_val)) * sweep_rad;
    let green_end   = start_rad + ((cfg.green_hi - cfg.min_val) / (cfg.max_val - cfg.min_val)) * sweep_rad;
    ctx.set_stroke_style_str("#4CAF50");
    ctx.set_line_width(7.0);
    ctx.begin_path();
    ctx.arc(cfg.cx, cfg.cy, cfg.radius, green_start, green_end).unwrap();
    ctx.stroke();

    // Yellow arc
    if let (Some(yl), Some(yh)) = (cfg.yellow_lo, cfg.yellow_hi) {
        let y_start = start_rad + ((yl - cfg.min_val) / (cfg.max_val - cfg.min_val)) * sweep_rad;
        let y_end   = start_rad + ((yh - cfg.min_val) / (cfg.max_val - cfg.min_val)) * sweep_rad;
        ctx.set_stroke_style_str("#FFB300");
        ctx.begin_path();
        ctx.arc(cfg.cx, cfg.cy, cfg.radius, y_start, y_end).unwrap();
        ctx.stroke();
    }

    // Red radial line at Vne/max
    if let Some(rhi) = cfg.red_hi {
        let r_angle = start_rad + ((rhi - cfg.min_val) / (cfg.max_val - cfg.min_val)) * sweep_rad;
        ctx.set_stroke_style_str("#F44336");
        ctx.set_line_width(3.0);
        ctx.begin_path();
        ctx.move_to(cfg.cx + (cfg.radius - 12.0) * r_angle.cos(),
                    cfg.cy + (cfg.radius - 12.0) * r_angle.sin());
        ctx.line_to(cfg.cx + (cfg.radius + 4.0) * r_angle.cos(),
                    cfg.cy + (cfg.radius + 4.0) * r_angle.sin());
        ctx.stroke();
    }

    // Needle
    let clamped = value.clamp(cfg.min_val, cfg.max_val);
    let needle_angle = start_rad + ((clamped - cfg.min_val) / (cfg.max_val - cfg.min_val)) * sweep_rad;
    let needle_color = if cfg.red_hi.map_or(false, |r| value >= r) { "#F44336" }
                       else if cfg.yellow_hi.map_or(false, |y| value >= cfg.yellow_lo.unwrap_or(y)) { "#FFB300" }
                       else { "#FFFFFF" };
    ctx.set_stroke_style_str(needle_color);
    ctx.set_line_width(2.5);
    ctx.set_line_cap("round");
    ctx.begin_path();
    ctx.move_to(cfg.cx - 10.0 * needle_angle.cos(), cfg.cy - 10.0 * needle_angle.sin());
    ctx.line_to(cfg.cx + (cfg.radius - 8.0) * needle_angle.cos(),
                cfg.cy + (cfg.radius - 8.0) * needle_angle.sin());
    ctx.stroke();

    // Center cap
    ctx.set_fill_style_str("#888888");
    ctx.begin_path();
    ctx.arc(cfg.cx, cfg.cy, 6.0, 0.0, PI * 2.0).unwrap();
    ctx.fill();

    // Digital readout
    let dg_color = if cfg.red_hi.map_or(false, |r| value >= r) { "#F44336" }
                   else { "#FFFFFF" };
    ctx.set_fill_style_str(dg_color);
    ctx.set_font("bold 15px 'Courier New'");
    ctx.set_text_align("center");
    ctx.fill_text(&format!("{:.0}", value), cfg.cx, cfg.cy + cfg.radius * 0.6).unwrap();

    // Label
    ctx.set_fill_style_str("#AAAAAA");
    ctx.set_font("11px sans-serif");
    ctx.fill_text(cfg.label, cfg.cx, cfg.cy + cfg.radius + 18.0).unwrap();
}
```

---

## 3. EGT/CHT Bar Gauges {#egt-cht}

```rust
// avionics-mfd/src/egt_cht.rs
// 6-cylinder EGT and CHT bar gauges — twin-column per cylinder

pub struct EgtChtConfig {
    pub egt_max: f64,       // Typically 1650°F Lycoming
    pub egt_caution: f64,   // 1500°F
    pub cht_max: f64,       // 400°F
    pub cht_caution: f64,   // 380°F
    pub num_cylinders: usize,
}

pub fn render_egt_cht(
    ctx: &web_sys::CanvasRenderingContext2d,
    cfg: &EgtChtConfig,
    egts: &[f64],
    chts: &[f64],
    x0: f64, y0: f64,
    bar_w: f64, bar_h: f64,
    spacing: f64,
) {
    for (i, (&egt, &cht)) in egts.iter().zip(chts.iter()).enumerate() {
        let bx = x0 + i as f64 * (bar_w * 2.0 + spacing);

        // EGT bar (left of pair)
        let egt_h = (egt / cfg.egt_max).clamp(0.0, 1.0) * bar_h;
        let egt_color = if egt >= cfg.egt_max     { "#F44336" }
                        else if egt >= cfg.egt_caution { "#FFB300" }
                        else { "#4CAF50" };
        ctx.set_fill_style_str("#1A1A1A");
        ctx.fill_rect(bx, y0, bar_w, bar_h);
        ctx.set_fill_style_str(egt_color);
        ctx.fill_rect(bx, y0 + bar_h - egt_h, bar_w, egt_h);

        // CHT bar (right of pair)
        let cx_ = bx + bar_w + 2.0;
        let cht_h = (cht / cfg.cht_max).clamp(0.0, 1.0) * bar_h;
        let cht_color = if cht >= cfg.cht_max     { "#F44336" }
                        else if cht >= cfg.cht_caution { "#FFB300" }
                        else { "#00BCD4" }; // cyan for CHT per G1000
        ctx.set_fill_style_str("#1A1A1A");
        ctx.fill_rect(cx_, y0, bar_w, bar_h);
        ctx.set_fill_style_str(cht_color);
        ctx.fill_rect(cx_, y0 + bar_h - cht_h, bar_w, cht_h);

        // Cylinder label
        ctx.set_fill_style_str("#AAAAAA");
        ctx.set_font("10px monospace");
        ctx.set_text_align("center");
        ctx.fill_text(&(i + 1).to_string(), bx + bar_w, y0 + bar_h + 12.0).unwrap();
    }

    // Caution line
    let caution_y = y0 + bar_h - (cfg.egt_caution / cfg.egt_max) * bar_h;
    ctx.set_stroke_style_str("#FFB300");
    ctx.set_line_width(1.0);
    ctx.set_line_dash(&js_sys::Array::of2(&JsValue::from(4.0), &JsValue::from(2.0))).unwrap();
    ctx.begin_path();
    ctx.move_to(x0, caution_y);
    ctx.line_to(x0 + (bar_w * 2.0 + spacing) * cfg.num_cylinders as f64, caution_y);
    ctx.stroke();
    ctx.set_line_dash(&js_sys::Array::new()).unwrap();
}
```

---

## 4. Fuel Quantity & Flow {#fuel}

```rust
// avionics-mfd/src/fuel.rs

pub fn render_fuel_gauge(
    ctx: &web_sys::CanvasRenderingContext2d,
    x: f64, y: f64,
    fuel_gal: f64, capacity_gal: f64,
    flow_gph: f64,
    label: &str,
) {
    // Fuel quantity arc (semicircle)
    let cx = x + 40.0; let cy = y + 55.0; let r = 40.0;
    let frac = (fuel_gal / capacity_gal).clamp(0.0, 1.0);

    // Background
    ctx.set_stroke_style_str("#333333");
    ctx.set_line_width(8.0);
    ctx.begin_path();
    ctx.arc(cx, cy, r, PI, 0.0).unwrap(); // bottom half-circle
    ctx.stroke();

    // Fill arc — green (>25%), amber (10-25%), red (<10%)
    let color = if frac > 0.25 { "#4CAF50" }
                else if frac > 0.10 { "#FFB300" }
                else { "#F44336" };
    ctx.set_stroke_style_str(color);
    ctx.set_line_width(7.0);
    let end_angle = PI + frac * PI;
    ctx.begin_path();
    ctx.arc(cx, cy, r, PI, end_angle).unwrap();
    ctx.stroke();

    // E and F labels
    ctx.set_fill_style_str("#FFFFFF");
    ctx.set_font("bold 11px monospace");
    ctx.fill_text("E", cx - r - 8.0, cy + 6.0).unwrap();
    ctx.fill_text("F", cx + r,       cy + 6.0).unwrap();

    // Digital quantity
    let qty_color = if frac < 0.10 { "#F44336" } else { "#FFFFFF" };
    ctx.set_fill_style_str(qty_color);
    ctx.set_font("bold 14px 'Courier New'");
    ctx.set_text_align("center");
    ctx.fill_text(&format!("{:.1}", fuel_gal), cx, cy + 14.0).unwrap();
    ctx.set_fill_style_str("#AAAAAA");
    ctx.set_font("10px monospace");
    ctx.fill_text("GAL", cx, cy + 26.0).unwrap();

    // Flow rate
    ctx.set_fill_style_str("#00BCD4");
    ctx.set_font("bold 12px monospace");
    ctx.fill_text(&format!("{:.1} GPH", flow_gph), cx, cy + 40.0).unwrap();

    // Label
    ctx.set_fill_style_str("#FFFFFF");
    ctx.fill_text(label, cx, y + 8.0).unwrap();
}
```

---

## 5. Oil Pressure & Temperature {#oil}

```rust
// avionics-mfd/src/oil.rs
// Horizontal bar gauges for oil P/T, vacuum, volts, amps

pub struct HorizBarGauge {
    pub label: &'static str,
    pub unit: &'static str,
    pub min: f64, pub max: f64,
    pub green_lo: f64, pub green_hi: f64,
    pub red_lo: Option<f64>, pub red_hi: Option<f64>,
}

pub fn render_horiz_bar(
    ctx: &web_sys::CanvasRenderingContext2d,
    gauge: &HorizBarGauge,
    value: f64,
    x: f64, y: f64, w: f64, h: f64,
) {
    // Background bar
    ctx.set_fill_style_str("#1A1A1A");
    ctx.fill_rect(x, y, w, h);

    // Green zone
    let green_x = x + (gauge.green_lo - gauge.min) / (gauge.max - gauge.min) * w;
    let green_w  = (gauge.green_hi  - gauge.green_lo)  / (gauge.max - gauge.min) * w;
    ctx.set_fill_style_str("#1A3D1A");
    ctx.fill_rect(green_x, y, green_w, h);

    // Value fill
    let fill_w = (value.clamp(gauge.min, gauge.max) - gauge.min) / (gauge.max - gauge.min) * w;
    let color = if gauge.red_lo.map_or(false, |r| value < r) ||
                   gauge.red_hi.map_or(false, |r| value > r) { "#F44336" }
                else if value < gauge.green_lo || value > gauge.green_hi { "#FFB300" }
                else { "#4CAF50" };
    ctx.set_fill_style_str(color);
    ctx.fill_rect(x, y, fill_w, h);

    // Border
    ctx.set_stroke_style_str("#555555");
    ctx.set_line_width(1.0);
    ctx.stroke_rect(x, y, w, h);

    // Label and value
    ctx.set_fill_style_str("#AAAAAA");
    ctx.set_font("10px monospace");
    ctx.fill_text(gauge.label, x, y - 4.0).unwrap();
    ctx.set_fill_style_str(color);
    ctx.set_font("bold 13px 'Courier New'");
    ctx.set_text_align("right");
    ctx.fill_text(&format!("{:.0} {}", value, gauge.unit), x + w, y + h + 13.0).unwrap();
    ctx.set_text_align("left");
}

/// Standard GA piston engine gauges
pub fn oil_pressure_gauge() -> HorizBarGauge {
    HorizBarGauge { label: "OIL P", unit: "PSI",
        min: 0.0, max: 115.0,
        green_lo: 25.0, green_hi: 100.0,
        red_lo: Some(10.0), red_hi: Some(115.0) }
}

pub fn oil_temp_gauge() -> HorizBarGauge {
    HorizBarGauge { label: "OIL T", unit: "°F",
        min: 75.0, max: 245.0,
        green_lo: 100.0, green_hi: 220.0,
        red_lo: None, red_hi: Some(245.0) }
}

pub fn bus_voltage_gauge() -> HorizBarGauge {
    HorizBarGauge { label: "VOLTS", unit: "V",
        min: 0.0, max: 32.0,
        green_lo: 13.0, green_hi: 14.8,
        red_lo: Some(11.5), red_hi: Some(15.5) }
}
```

---

## 6. Electrical Bus Monitor {#electrical}

```rust
// avionics-mfd/src/electrical.rs

#[derive(Clone, serde::Deserialize)]
pub struct ElectricalBusState {
    pub main_bus_v: f64,
    pub essential_bus_v: f64,
    pub battery_v: f64,
    pub battery_a: f64,      // positive = charging
    pub alternator1_a: f64,
    pub alternator2_a: Option<f64>,
    pub av_bus_v: f64,        // avionics bus
}

pub fn render_electrical_page(
    ctx: &web_sys::CanvasRenderingContext2d,
    state: &ElectricalBusState,
    x0: f64, y0: f64,
) {
    let gauges = [
        (bus_voltage_gauge(), state.main_bus_v, "MAIN BUS"),
        (bus_voltage_gauge(), state.essential_bus_v, "ESS BUS"),
        (bus_voltage_gauge(), state.battery_v, "BATTERY"),
    ];

    for (i, (g, val, lbl)) in gauges.iter().enumerate() {
        render_horiz_bar(ctx, g, *val, x0, y0 + i as f64 * 36.0, 160.0, 14.0);
        ctx.set_fill_style_str("#AAAAAA");
        ctx.set_font("10px monospace");
        ctx.fill_text(lbl, x0 + 165.0, y0 + i as f64 * 36.0 + 10.0).unwrap();
    }

    // Ampere indicators
    ctx.set_fill_style_str("#AAAAAA");
    ctx.set_font("10px monospace");
    ctx.fill_text("BAT", x0, y0 + 120.0).unwrap();
    let bat_color = if state.battery_a < -5.0 { "#F44336" } else { "#4CAF50" };
    ctx.set_fill_style_str(bat_color);
    ctx.set_font("bold 14px monospace");
    ctx.fill_text(&format!("{:+.0}A", state.battery_a), x0 + 28.0, y0 + 120.0).unwrap();
}
```

---

## 7. MFD Moving Map Integration {#moving-map}

The MFD moving map reuses the Cesium integration from `uav-gcs-rust` skill, with aviation-specific overlays:

```javascript
// frontend/src/mfd/MovingMap.jsx
import { useEffect, useRef } from 'react';
import { loadAirspaceLayer } from '../gcs/wasmBridge';

// Aviation overlay layers on Cesium MFD page
export const MFD_LAYERS = {
  airports: {
    icon: '/assets/airport_symbol.svg',
    minZoomAlt: 50000,   // show when below 50,000 ft camera altitude
  },
  navaids: {
    icon: '/assets/vor_symbol.svg',
    minZoomAlt: 30000,
  },
  airspace: {
    types: ['CTR', 'TMA', 'ClassB', 'ClassC', 'ClassD', 'TFR', 'Prohibited'],
    colors: { CTR: '#FF6600', TFR: '#FF0000', ClassB: '#0000FF', ClassC: '#CC00CC' },
  },
  nexrad: {
    url: 'https://mesonet.agron.iastate.edu/cgi-bin/wms/nexrad/n0r.cgi',
    opacity: 0.5,
  },
};

export function initMfdOverlays(viewer, airspaceGeoJson) {
  loadAirspaceLayer(viewer, airspaceGeoJson);

  // NEXRAD weather (WMS layer)
  viewer.imageryLayers.addImageryProvider(
    new Cesium.WebMapServiceImageryProvider({
      url: MFD_LAYERS.nexrad.url,
      layers: 'nexrad-n0r',
      parameters: { transparent: true, format: 'image/png' },
    })
  );
}
```

---

## 8. EICAS Alerting Logic {#eicas-alerts}

```rust
// avionics-mfd/src/eicas.rs

/// ARP4761 alert priority levels
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum AlertPriority {
    Warning  = 3,  // Immediate action required — red
    Caution  = 2,  // Timely action required — amber
    Advisory = 1,  // Awareness only — white
    Status   = 0,  // Information — cyan
}

#[derive(Debug, Clone)]
pub struct EicasAlert {
    pub id: &'static str,
    pub text: &'static str,
    pub priority: AlertPriority,
    pub inhibit_on_ground: bool,
    pub inhibit_takeoff: bool,
}

pub static ENGINE_ALERTS: &[EicasAlert] = &[
    EicasAlert { id: "ENG_OIL_LOW_P", text: "ENG OIL PRESSURE LO", priority: AlertPriority::Warning, inhibit_on_ground: false, inhibit_takeoff: false },
    EicasAlert { id: "ENG_OIL_HI_T",  text: "ENG OIL TEMP HI",     priority: AlertPriority::Caution, inhibit_on_ground: false, inhibit_takeoff: false },
    EicasAlert { id: "ENG_CHT_HI",    text: "CHT HIGH",             priority: AlertPriority::Caution, inhibit_on_ground: false, inhibit_takeoff: false },
    EicasAlert { id: "FUEL_LOW_L",    text: "FUEL QTY LO LEFT",     priority: AlertPriority::Caution, inhibit_on_ground: true,  inhibit_takeoff: false },
    EicasAlert { id: "FUEL_LOW_R",    text: "FUEL QTY LO RIGHT",    priority: AlertPriority::Caution, inhibit_on_ground: true,  inhibit_takeoff: false },
    EicasAlert { id: "ALT_FAIL",      text: "ALTERNATOR FAIL",      priority: AlertPriority::Warning, inhibit_on_ground: false, inhibit_takeoff: false },
    EicasAlert { id: "VOLTS_LO",      text: "LOW VOLTS",            priority: AlertPriority::Warning, inhibit_on_ground: false, inhibit_takeoff: false },
];

pub struct EicasSystem {
    active_alerts: Vec<EicasAlert>,
    is_on_ground: bool,
}

impl EicasSystem {
    pub fn evaluate(&mut self, data: &MfdData) {
        self.active_alerts.clear();
        for alert in ENGINE_ALERTS {
            if alert.inhibit_on_ground && self.is_on_ground { continue; }
            let triggered = match alert.id {
                "ENG_OIL_LOW_P" => data.oil_pressure_psi < 25.0,
                "ENG_OIL_HI_T"  => data.oil_temp_f > 235.0,
                "ENG_CHT_HI"    => data.chts.iter().any(|&c| c > 390.0),
                "FUEL_LOW_L"    => data.fuel_left_gal < 3.0,
                "FUEL_LOW_R"    => data.fuel_right_gal < 3.0,
                "ALT_FAIL"      => data.alternator_a < 1.0 && !self.is_on_ground,
                "VOLTS_LO"      => data.main_bus_v < 12.5,
                _ => false,
            };
            if triggered {
                self.active_alerts.push((*alert).clone());
            }
        }
        // Sort by priority descending
        self.active_alerts.sort_by(|a, b| b.priority.cmp(&a.priority));
    }
}
```