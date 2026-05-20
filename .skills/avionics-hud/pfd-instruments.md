# PFD Instruments Reference

## Table of Contents
1. [ADI — Attitude Director Indicator](#adi)
2. [Airspeed Tape](#airspeed)
3. [Altimeter Tape](#altimeter)
4. [Vertical Speed Indicator](#vsi)
5. [HSI / Compass Rose](#hsi)
6. [Flight Director Bars](#fd)
7. [Slip/Skid Ball](#slip)
8. [Annunciator Strip](#annunciator)
9. [PFD Composite WASM Module](#pfd-wasm)

---

## 1. ADI — Attitude Director Indicator {#adi}

The ADI is the centerpiece of the PFD. It rotates a sky/ground split on **roll**
and translates vertically on **pitch**, with a fixed aircraft symbol at center.

### Rust/WASM rendering (Canvas 2D)

```rust
// avionics-pfd/src/adi.rs
use wasm_bindgen::prelude::*;
use web_sys::{CanvasRenderingContext2d, HtmlCanvasElement};

pub struct AdiRenderer {
    ctx: CanvasRenderingContext2d,
    cx: f64,   // center x
    cy: f64,   // center y
    radius: f64,
}

impl AdiRenderer {
    pub fn new(canvas: &HtmlCanvasElement) -> Result<Self, JsValue> {
        let ctx = canvas
            .get_context("2d")?
            .unwrap()
            .dyn_into::<CanvasRenderingContext2d>()?;
        Ok(Self { ctx, cx: 200.0, cy: 200.0, radius: 180.0 })
    }

    /// Render ADI given pitch (degrees, positive = nose up) and roll (degrees, positive = right bank)
    pub fn render(&self, pitch_deg: f64, roll_deg: f64, slip_deg: f64) {
        let ctx = &self.ctx;
        let roll_rad = roll_deg.to_radians();
        let pitch_px = pitch_deg * 8.0; // 8 pixels per degree of pitch

        ctx.save();
        // Clip to circular ADI boundary
        ctx.begin_path();
        ctx.arc(self.cx, self.cy, self.radius, 0.0, std::f64::consts::TAU).unwrap();
        ctx.clip();

        // Apply roll rotation around center
        ctx.translate(self.cx, self.cy).unwrap();
        ctx.rotate(-roll_rad).unwrap();

        // Draw sky (blue) — pitch offset applied
        ctx.set_fill_style_str("#1A6FA8");
        ctx.fill_rect(-self.radius * 2.0, -self.radius * 2.0 - pitch_px, self.radius * 4.0, self.radius * 2.0 + pitch_px);

        // Draw ground (brown)
        ctx.set_fill_style_str("#6B3D10");
        ctx.fill_rect(-self.radius * 2.0, -pitch_px, self.radius * 4.0, self.radius * 2.0);

        // Horizon line
        ctx.set_stroke_style_str("#FFFFFF");
        ctx.set_line_width(2.0);
        ctx.begin_path();
        ctx.move_to(-self.radius * 2.0, -pitch_px);
        ctx.line_to(self.radius * 2.0, -pitch_px);
        ctx.stroke();

        // Pitch ladder lines (every 5°, major every 10°)
        self.draw_pitch_ladder(ctx, pitch_px);

        ctx.restore();

        // Draw fixed aircraft symbol (over clip, not rotated)
        self.draw_aircraft_symbol();

        // Draw bank angle arc and pointer
        self.draw_bank_arc(roll_deg);

        // Slip/skid ball
        self.draw_slip_ball(slip_deg, roll_deg);
    }

    fn draw_pitch_ladder(&self, ctx: &CanvasRenderingContext2d, pitch_offset_px: f64) {
        ctx.set_stroke_style_str("#FFFFFF");
        ctx.set_fill_style_str("#FFFFFF");
        for i in -18i32..=18 {
            if i == 0 { continue; }
            let angle_deg = i as f64 * 5.0;
            let y = -pitch_offset_px - angle_deg * 8.0;
            let (half_width, line_w) = if i % 2 == 0 {
                (60.0, 1.5) // 10° major
            } else {
                (30.0, 1.0) // 5° minor
            };
            ctx.set_line_width(line_w);
            ctx.begin_path();
            ctx.move_to(-half_width, y);
            ctx.line_to(half_width, y);
            ctx.stroke();

            // Label major lines
            if i % 2 == 0 && i != 0 {
                let label = (angle_deg.abs() as i32).to_string();
                ctx.set_font("bold 12px monospace");
                ctx.set_text_align("right");
                ctx.fill_text(&label, -half_width - 4.0, y + 4.0).unwrap();
                ctx.set_text_align("left");
                ctx.fill_text(&label, half_width + 4.0, y + 4.0).unwrap();
            }
        }
    }

    fn draw_bank_arc(&self, roll_deg: f64) {
        let ctx = &self.ctx;
        ctx.save();
        ctx.translate(self.cx, self.cy).unwrap();

        // Bank arc (top of ADI circle)
        let arc_r = self.radius - 12.0;
        ctx.set_stroke_style_str("#FFFFFF");
        ctx.set_line_width(1.5);
        // Draw arc marks at 10/20/30/45/60°
        for &angle in &[-60.0_f64, -45.0, -30.0, -20.0, -10.0, 0.0, 10.0, 20.0, 30.0, 45.0, 60.0] {
            let a = (angle - 90.0).to_radians();
            let tick_len = if angle.abs() == 30.0 || angle == 0.0 { 12.0 } else { 7.0 };
            ctx.begin_path();
            ctx.move_to(arc_r * a.cos(), arc_r * a.sin());
            ctx.line_to((arc_r - tick_len) * a.cos(), (arc_r - tick_len) * a.sin());
            ctx.stroke();
        }

        // Bank angle pointer (moves with aircraft)
        let ptr_angle = (-roll_deg - 90.0_f64).to_radians();
        ctx.set_fill_style_str("#FFFFFF");
        ctx.begin_path();
        let px = arc_r * ptr_angle.cos();
        let py = arc_r * ptr_angle.sin();
        ctx.move_to(px, py);
        ctx.line_to(px - 6.0 * ptr_angle.sin(), py + 6.0 * ptr_angle.cos());
        ctx.line_to(px + 6.0 * ptr_angle.sin(), py - 6.0 * ptr_angle.cos());
        ctx.close_path();
        ctx.fill();
        ctx.restore();
    }

    fn draw_aircraft_symbol(&self) {
        let ctx = &self.ctx;
        ctx.save();
        ctx.translate(self.cx, self.cy).unwrap();
        ctx.set_stroke_style_str("#F9A825"); // aviation amber/yellow
        ctx.set_line_width(3.0);
        ctx.set_line_cap("round");
        // Left wing
        ctx.begin_path();
        ctx.move_to(-60.0, 0.0);
        ctx.line_to(-20.0, 0.0);
        ctx.line_to(-10.0, 8.0);
        ctx.stroke();
        // Right wing
        ctx.begin_path();
        ctx.move_to(60.0, 0.0);
        ctx.line_to(20.0, 0.0);
        ctx.line_to(10.0, 8.0);
        ctx.stroke();
        // Center dot
        ctx.begin_path();
        ctx.arc(0.0, 0.0, 4.0, 0.0, std::f64::consts::TAU).unwrap();
        ctx.set_fill_style_str("#F9A825");
        ctx.fill();
        ctx.restore();
    }

    fn draw_slip_ball(&self, slip_deg: f64, roll_deg: f64) {
        // Turn coordinator / slip-skid ball below ADI
        let ctx = &self.ctx;
        let ball_x = self.cx + slip_deg * 4.0; // 4px per degree deflection
        let ball_y = self.cy + self.radius + 20.0;
        ctx.set_fill_style_str("#FFFFFF");
        ctx.begin_path();
        ctx.arc(ball_x, ball_y, 7.0, 0.0, std::f64::consts::TAU).unwrap();
        ctx.fill();
        ctx.set_stroke_style_str("#888888");
        ctx.set_line_width(1.0);
        ctx.stroke();
    }
}
```

---

## 2. Airspeed Tape {#airspeed}

Garmin G1000 style: vertical tape, current airspeed in center window, colored arcs for Vs0/Vs1/Vfe/Vno/Vne.

```rust
// avionics-pfd/src/airspeed_tape.rs

pub struct AirspeedTape {
    ctx: web_sys::CanvasRenderingContext2d,
    x: f64, y: f64,
    width: f64, height: f64,
    px_per_kt: f64,       // pixels per knot
    v_speeds: VSpeedConfig,
}

#[derive(Clone)]
pub struct VSpeedConfig {
    pub vs0: f64,   // Stall, landing config (bottom of white arc)
    pub vs1: f64,   // Stall, clean (bottom of green arc)
    pub vfe: f64,   // Max flap extended (top of white arc)
    pub vno: f64,   // Max structural cruise (top of green, bottom of yellow)
    pub vne: f64,   // Never exceed (top of yellow, start of red)
    pub vr: f64,    // Rotate speed
    pub v2: f64,    // Takeoff safety
    pub vref: f64,  // Landing reference
}

impl AirspeedTape {
    const PX_PER_KT: f64 = 6.0;
    const TAPE_W: f64 = 70.0;

    pub fn render(&self, airspeed_kts: f64, trend_kts_per_sec: f64) {
        let ctx = &self.ctx;
        let center_y = self.y + self.height / 2.0;

        // Background
        ctx.set_fill_style_str("#1A1A1A");
        ctx.fill_rect(self.x, self.y, self.width, self.height);

        // Clip region
        ctx.save();
        ctx.begin_path();
        ctx.rect(self.x, self.y, self.width, self.height);
        ctx.clip();

        // Draw colored speed arcs (vertical bars on left edge of tape)
        self.draw_speed_arcs(ctx, airspeed_kts, center_y);

        // Tape tick marks and labels
        let first_tick = ((airspeed_kts - self.height / (2.0 * Self::PX_PER_KT)) / 10.0).floor() * 10.0;
        let last_tick  = ((airspeed_kts + self.height / (2.0 * Self::PX_PER_KT)) / 10.0).ceil()  * 10.0;

        let mut v = first_tick;
        while v <= last_tick {
            if v >= 0.0 {
                let y = center_y - (v - airspeed_kts) * Self::PX_PER_KT;
                ctx.set_stroke_style_str("#AAAAAA");
                ctx.set_line_width(if v % 20.0 == 0.0 { 1.5 } else { 1.0 });
                let tick_len = if v % 20.0 == 0.0 { 16.0 } else { 8.0 };
                ctx.begin_path();
                ctx.move_to(self.x + self.width, y);
                ctx.line_to(self.x + self.width - tick_len, y);
                ctx.stroke();
                if v % 20.0 == 0.0 {
                    ctx.set_fill_style_str("#FFFFFF");
                    ctx.set_font("bold 14px 'Courier New'");
                    ctx.set_text_align("right");
                    ctx.fill_text(&(v as u32).to_string(), self.x + self.width - 18.0, y + 5.0).unwrap();
                }
            }
            v += 10.0;
        }

        // Trend vector (6-second look-ahead, cyan line)
        if trend_kts_per_sec.abs() > 0.5 {
            let trend_px = trend_kts_per_sec * 6.0 * Self::PX_PER_KT;
            ctx.set_stroke_style_str("#00BCD4");
            ctx.set_line_width(2.0);
            ctx.begin_path();
            ctx.move_to(self.x + self.width - 5.0, center_y);
            ctx.line_to(self.x + self.width - 5.0, center_y - trend_px);
            ctx.stroke();
        }

        ctx.restore();

        // Current airspeed readout (cyan box in center)
        self.draw_airspeed_readout(ctx, airspeed_kts, center_y);
    }

    fn draw_speed_arcs(&self, ctx: &web_sys::CanvasRenderingContext2d, cas: f64, center_y: f64) {
        let v = &self.v_speeds;
        let arc_x = self.x + 6.0;
        let arc_w = 8.0;

        // White arc: Vs0 → Vfe (flaps operating)
        Self::draw_tape_arc(ctx, arc_x, arc_w, center_y, cas, v.vs0, v.vfe, "#FFFFFF", Self::PX_PER_KT);
        // Green arc: Vs1 → Vno (normal ops)
        Self::draw_tape_arc(ctx, arc_x + 10.0, arc_w, center_y, cas, v.vs1, v.vno, "#4CAF50", Self::PX_PER_KT);
        // Yellow arc: Vno → Vne (caution range)
        Self::draw_tape_arc(ctx, arc_x + 10.0, arc_w, center_y, cas, v.vno, v.vne, "#FFB300", Self::PX_PER_KT);
        // Red line: Vne
        let y_vne = center_y - (v.vne - cas) * Self::PX_PER_KT;
        ctx.set_fill_style_str("#F44336");
        ctx.fill_rect(arc_x + 10.0, y_vne - 2.0, arc_w, 4.0);
    }

    fn draw_tape_arc(
        ctx: &web_sys::CanvasRenderingContext2d,
        x: f64, w: f64, center_y: f64, cas: f64,
        v_low: f64, v_high: f64, color: &str, px_per_kt: f64
    ) {
        let y_top = center_y - (v_high - cas) * px_per_kt;
        let y_bot = center_y - (v_low  - cas) * px_per_kt;
        ctx.set_fill_style_str(color);
        ctx.fill_rect(x, y_top, w, y_bot - y_top);
    }

    fn draw_airspeed_readout(&self, ctx: &web_sys::CanvasRenderingContext2d, cas: f64, center_y: f64) {
        // Black box with white numbers — like G1000 center readout
        let box_w = 60.0; let box_h = 30.0;
        let bx = self.x + self.width - box_w;
        let by = center_y - box_h / 2.0;

        ctx.set_fill_style_str("#000000");
        ctx.fill_rect(bx - 2.0, by - 2.0, box_w + 4.0, box_h + 4.0);
        ctx.set_stroke_style_str("#FFFFFF");
        ctx.set_line_width(1.5);
        ctx.stroke_rect(bx - 2.0, by - 2.0, box_w + 4.0, box_h + 4.0);

        ctx.set_fill_style_str("#FFFFFF");
        ctx.set_font("bold 20px 'Courier New'");
        ctx.set_text_align("center");
        ctx.fill_text(&format!("{:3.0}", cas), bx + box_w / 2.0, by + box_h - 6.0).unwrap();
    }
}
```

---

## 3. Altimeter Tape {#altimeter}

```rust
// avionics-pfd/src/altimeter_tape.rs

pub struct AltimeterTape {
    ctx: web_sys::CanvasRenderingContext2d,
    x: f64, y: f64, width: f64, height: f64,
}

impl AltimeterTape {
    const PX_PER_FT: f64 = 0.06; // 100ft tape steps visible

    pub fn render(&self, altitude_ft: f64, selected_alt_ft: f64, baro_in_hg: f64, trend_fpm: f64) {
        let ctx = &self.ctx;
        let center_y = self.y + self.height / 2.0;

        ctx.set_fill_style_str("#1A1A1A");
        ctx.fill_rect(self.x, self.y, self.width, self.height);

        ctx.save();
        ctx.begin_path();
        ctx.rect(self.x, self.y, self.width, self.height);
        ctx.clip();

        // Tick marks every 100 ft, label every 500 ft
        let range_ft = self.height / Self::PX_PER_FT;
        let first = ((altitude_ft - range_ft / 2.0) / 100.0).floor() * 100.0;
        let last  = ((altitude_ft + range_ft / 2.0) / 100.0).ceil()  * 100.0;
        let mut alt = first;
        while alt <= last {
            let y = center_y + (altitude_ft - alt) * Self::PX_PER_FT;
            let is_major = alt % 500.0 == 0.0;
            ctx.set_stroke_style_str("#AAAAAA");
            ctx.set_line_width(if is_major { 1.5 } else { 1.0 });
            ctx.begin_path();
            ctx.move_to(self.x, y);
            ctx.line_to(self.x + if is_major { 16.0 } else { 8.0 }, y);
            ctx.stroke();
            if is_major {
                ctx.set_fill_style_str("#FFFFFF");
                ctx.set_font("bold 14px 'Courier New'");
                ctx.set_text_align("left");
                ctx.fill_text(&format!("{:5.0}", alt), self.x + 18.0, y + 5.0).unwrap();
            }
            alt += 100.0;
        }

        // Selected altitude bug (cyan)
        let bug_y = center_y + (altitude_ft - selected_alt_ft) * Self::PX_PER_FT;
        if bug_y >= self.y && bug_y <= self.y + self.height {
            ctx.set_fill_style_str("#00BCD4");
            ctx.fill_rect(self.x, bug_y - 2.0, self.width * 0.4, 4.0);
        }

        // Trend vector
        if trend_fpm.abs() > 100.0 {
            let trend_px = (trend_fpm / 60.0) * 6.0 * Self::PX_PER_FT; // 6-sec look-ahead
            ctx.set_stroke_style_str("#00BCD4");
            ctx.set_line_width(2.0);
            ctx.begin_path();
            ctx.move_to(self.x + self.width - 5.0, center_y);
            ctx.line_to(self.x + self.width - 5.0, center_y - trend_px);
            ctx.stroke();
        }

        ctx.restore();

        // Altimeter readout box
        self.draw_altitude_readout(ctx, altitude_ft, center_y);

        // Baro setting
        ctx.set_fill_style_str("#00BCD4");
        ctx.set_font("12px monospace");
        ctx.fill_text(&format!("B {:.2} IN", baro_in_hg),
            self.x + 4.0, self.y + self.height + 14.0).unwrap();
    }

    fn draw_altitude_readout(&self, ctx: &web_sys::CanvasRenderingContext2d, alt: f64, center_y: f64) {
        // Five-digit rolling drum (thousands + hundreds)
        let box_w = 80.0; let box_h = 30.0;
        let bx = self.x; let by = center_y - box_h / 2.0;
        ctx.set_fill_style_str("#000000");
        ctx.fill_rect(bx, by - 2.0, box_w + 4.0, box_h + 4.0);
        ctx.set_stroke_style_str("#FFFFFF");
        ctx.set_line_width(1.5);
        ctx.stroke_rect(bx, by - 2.0, box_w + 4.0, box_h + 4.0);
        ctx.set_fill_style_str("#FFFFFF");
        ctx.set_font("bold 20px 'Courier New'");
        ctx.set_text_align("center");
        ctx.fill_text(&format!("{:5.0}", alt), bx + box_w / 2.0, by + box_h - 6.0).unwrap();
    }
}
```

---

## 4. Vertical Speed Indicator {#vsi}

```rust
// avionics-pfd/src/vsi.rs
// Standard pointer VSI or tape-style; G1000 uses a vertical pointer on right side of ALT tape

pub fn render_vsi_pointer(
    ctx: &web_sys::CanvasRenderingContext2d,
    x: f64, y: f64, height: f64,
    vsi_fpm: f64,
) {
    // VSI scale: ±2000 fpm; non-linear (compressed near extremes)
    let max_fpm = 2000.0_f64;
    let clamped = vsi_fpm.clamp(-max_fpm, max_fpm);

    // Non-linear mapping: linear 0–1000, compressed 1000–2000
    let normalized = if clamped.abs() <= 1000.0 {
        clamped / max_fpm
    } else {
        let sign = clamped.signum();
        sign * (0.5 + (clamped.abs() - 1000.0) / 4000.0)
    };

    let pointer_y = y + height / 2.0 - normalized * height / 2.0;

    // Background bar
    ctx.set_fill_style_str("#1A1A1A");
    ctx.fill_rect(x, y, 40.0, height);

    // Pointer chevron
    let color = if vsi_fpm.abs() > 1500.0 { "#F44336" }
                else if vsi_fpm.abs() > 1000.0 { "#FFB300" }
                else { "#FFFFFF" };
    ctx.set_fill_style_str(color);
    ctx.begin_path();
    ctx.move_to(x, pointer_y);
    ctx.line_to(x + 20.0, pointer_y - 8.0);
    ctx.line_to(x + 35.0, pointer_y);
    ctx.line_to(x + 20.0, pointer_y + 8.0);
    ctx.close_path();
    ctx.fill();

    // Tick marks at ±500/1000/1500/2000
    ctx.set_stroke_style_str("#888888");
    ctx.set_line_width(1.0);
    for &fpm in &[-2000.0_f64, -1000.0, -500.0, 500.0, 1000.0, 2000.0] {
        let ny = if fpm.abs() <= 1000.0 { fpm / max_fpm }
                 else { fpm.signum() * (0.5 + (fpm.abs() - 1000.0) / 4000.0) };
        let ty = y + height / 2.0 - ny * height / 2.0;
        ctx.begin_path();
        ctx.move_to(x + 25.0, ty);
        ctx.line_to(x + 38.0, ty);
        ctx.stroke();
    }

    // Digital readout
    if vsi_fpm.abs() > 100.0 {
        ctx.set_fill_style_str("#FFFFFF");
        ctx.set_font("bold 12px monospace");
        ctx.set_text_align("center");
        let label = if vsi_fpm >= 0.0 {
            format!("+{:.0}", vsi_fpm)
        } else {
            format!("{:.0}", vsi_fpm)
        };
        ctx.fill_text(&label, x + 20.0, y + height + 14.0).unwrap();
    }
}
```

---

## 5. HSI / Compass Rose {#hsi}

```rust
// avionics-pfd/src/hsi.rs

pub struct HsiRenderer {
    ctx: web_sys::CanvasRenderingContext2d,
    cx: f64, cy: f64, radius: f64,
}

impl HsiRenderer {
    pub fn render(
        &self,
        heading_deg: f64,
        selected_hdg_deg: f64,
        course_deg: f64,         // OBS / active nav course
        course_deviation: f64,   // dots, ±2.5 full scale
        nav_source: NavSource,   // GPS / VOR / ILS / LOC
        bearing1_deg: Option<f64>, // pointer 1 (cyan)
        bearing2_deg: Option<f64>, // pointer 2 (white/green)
    ) {
        let ctx = &self.ctx;
        ctx.save();
        ctx.translate(self.cx, self.cy).unwrap();
        ctx.rotate(-heading_deg.to_radians()).unwrap();

        // Compass rose ring
        ctx.set_stroke_style_str("#888888");
        ctx.set_line_width(1.5);
        ctx.begin_path();
        ctx.arc(0.0, 0.0, self.radius, 0.0, std::f64::consts::TAU).unwrap();
        ctx.stroke();

        // Cardinal + 30° ticks and labels
        for i in 0..72 {
            let angle = (i as f64 * 5.0).to_radians();
            let tick = if i % 6 == 0 { 16.0 }  // 30° major
                       else if i % 2 == 0 { 8.0 } // 10° minor
                       else { 4.0 };              // 5° sub
            ctx.begin_path();
            ctx.move_to(self.radius * angle.sin(), -self.radius * angle.cos());
            ctx.line_to((self.radius - tick) * angle.sin(), -(self.radius - tick) * angle.cos());
            ctx.stroke();

            // Labels at 30° intervals
            if i % 6 == 0 {
                let deg = i * 5;
                let label = match deg {
                    0 => "N".to_string(), 90 => "E".to_string(),
                    180 => "S".to_string(), 270 => "W".to_string(),
                    _ => format!("{}", deg / 10),
                };
                ctx.save();
                ctx.rotate(angle).unwrap();
                ctx.set_fill_style_str(if deg == 0 { "#F44336" } else { "#FFFFFF" });
                ctx.set_font("bold 14px sans-serif");
                ctx.set_text_align("center");
                ctx.fill_text(&label, 0.0, -(self.radius - 22.0)).unwrap();
                ctx.restore();
            }
        }

        // Selected heading bug (cyan)
        let hdg_bug_angle = (selected_hdg_deg - heading_deg).to_radians();
        ctx.save();
        ctx.rotate(hdg_bug_angle).unwrap();
        ctx.set_fill_style_str("#00BCD4");
        ctx.begin_path();
        ctx.move_to(0.0, -self.radius + 2.0);
        ctx.line_to(-8.0, -self.radius + 14.0);
        ctx.line_to(8.0, -self.radius + 14.0);
        ctx.close_path();
        ctx.fill();
        ctx.restore();

        // Course needle (magenta for GPS, green for VOR)
        let course_color = match nav_source { NavSource::Gps => "#E040FB", _ => "#4CAF50" };
        let course_angle = (course_deg - heading_deg).to_radians();
        ctx.save();
        ctx.rotate(course_angle).unwrap();

        // Course needle line
        ctx.set_stroke_style_str(course_color);
        ctx.set_line_width(3.0);
        ctx.begin_path();
        ctx.move_to(0.0, -(self.radius - 20.0));
        ctx.line_to(0.0, -20.0);
        ctx.stroke();
        ctx.begin_path();
        ctx.move_to(0.0, 20.0);
        ctx.line_to(0.0, self.radius - 20.0);
        ctx.stroke();

        // CDI dots (± 1 and 2 dot positions at ±40px)
        for &dot_x in &[-40.0_f64, -20.0, 20.0, 40.0] {
            ctx.set_stroke_style_str("#888888");
            ctx.set_line_width(1.0);
            ctx.begin_path();
            ctx.arc(dot_x, 0.0, 3.0, 0.0, std::f64::consts::TAU).unwrap();
            ctx.stroke();
        }

        // CDI deviation indicator
        let dev_px = (course_deviation * 20.0).clamp(-40.0, 40.0);
        ctx.set_fill_style_str(course_color);
        ctx.begin_path();
        ctx.arc(dev_px, 0.0, 5.0, 0.0, std::f64::consts::TAU).unwrap();
        ctx.fill();

        ctx.restore(); // end course rotation

        // Bearing pointers (single/double bar)
        if let Some(b1) = bearing1_deg {
            self.draw_bearing_pointer(ctx, b1 - heading_deg, "#00BCD4", false);
        }
        if let Some(b2) = bearing2_deg {
            self.draw_bearing_pointer(ctx, b2 - heading_deg, "#FFFFFF", true);
        }

        ctx.restore(); // end heading rotation

        // Fixed aircraft symbol in center
        ctx.save();
        ctx.translate(self.cx, self.cy).unwrap();
        ctx.set_fill_style_str("#F9A825");
        ctx.begin_path();
        ctx.arc(0.0, 0.0, 5.0, 0.0, std::f64::consts::TAU).unwrap();
        ctx.fill();
        ctx.restore();

        // Heading readout (top of compass rose)
        ctx.set_fill_style_str("#000000");
        ctx.fill_rect(self.cx - 28.0, self.cy - self.radius - 26.0, 56.0, 22.0);
        ctx.set_stroke_style_str("#FFFFFF");
        ctx.set_line_width(1.0);
        ctx.stroke_rect(self.cx - 28.0, self.cy - self.radius - 26.0, 56.0, 22.0);
        ctx.set_fill_style_str("#FFFFFF");
        ctx.set_font("bold 16px monospace");
        ctx.set_text_align("center");
        ctx.fill_text(&format!("{:03.0}°", heading_deg), self.cx, self.cy - self.radius - 8.0).unwrap();
    }

    fn draw_bearing_pointer(
        &self, ctx: &web_sys::CanvasRenderingContext2d,
        relative_deg: f64, color: &str, double_bar: bool
    ) {
        ctx.save();
        ctx.rotate(relative_deg.to_radians()).unwrap();
        ctx.set_stroke_style_str(color);
        ctx.set_line_width(2.0);
        // Arrow head
        ctx.begin_path();
        ctx.move_to(0.0, -(self.radius - 18.0));
        ctx.line_to(-8.0, -(self.radius - 36.0));
        ctx.move_to(0.0, -(self.radius - 18.0));
        ctx.line_to(8.0, -(self.radius - 36.0));
        ctx.stroke();
        // Shaft
        ctx.begin_path();
        ctx.move_to(0.0, -(self.radius - 36.0));
        ctx.line_to(0.0, -10.0);
        ctx.stroke();
        if double_bar {
            ctx.begin_path();
            ctx.move_to(-6.0, -(self.radius - 46.0));
            ctx.line_to(6.0, -(self.radius - 46.0));
            ctx.stroke();
        }
        ctx.restore();
    }
}

pub enum NavSource { Gps, Vor, Ils, Loc }
```

---

## 6. Flight Director Bars {#fd}

```rust
// avionics-pfd/src/flight_director.rs
// Magenta V-bar or crosshair FD command bars overlaid on ADI

pub fn render_fd_crosshair(
    ctx: &web_sys::CanvasRenderingContext2d,
    cx: f64, cy: f64,
    pitch_cmd_deg: f64,   // positive = pitch up
    roll_cmd_deg: f64,    // positive = roll right
) {
    let pitch_px = pitch_cmd_deg * 8.0;
    let roll_px  = roll_cmd_deg * 3.0;

    ctx.set_stroke_style_str("#E040FB"); // magenta
    ctx.set_line_width(3.0);
    ctx.set_line_cap("round");

    // Pitch bar (horizontal)
    ctx.begin_path();
    ctx.move_to(cx - 60.0, cy - pitch_px);
    ctx.line_to(cx + 60.0, cy - pitch_px);
    ctx.stroke();

    // Roll bar (vertical, offset by roll command)
    ctx.begin_path();
    ctx.move_to(cx + roll_px, cy - 40.0);
    ctx.line_to(cx + roll_px, cy + 40.0);
    ctx.stroke();
}

// V-bar FD (Garmin G1000 default style)
pub fn render_fd_vbar(
    ctx: &web_sys::CanvasRenderingContext2d,
    cx: f64, cy: f64,
    pitch_cmd_deg: f64,
    roll_cmd_deg: f64,
) {
    let pitch_px = pitch_cmd_deg * 8.0;
    let roll_rad = roll_cmd_deg.to_radians();
    ctx.save();
    ctx.translate(cx, cy).unwrap();
    ctx.rotate(-roll_rad).unwrap();
    ctx.set_stroke_style_str("#E040FB");
    ctx.set_line_width(3.5);
    ctx.set_line_join("round");
    ctx.begin_path();
    ctx.move_to(-80.0, 10.0 - pitch_px);
    ctx.line_to(0.0,   -20.0 - pitch_px);
    ctx.line_to(80.0,  10.0 - pitch_px);
    ctx.stroke();
    ctx.restore();
}
```

---

## 7. Annunciator Strip {#annunciator}

```rust
// avionics-pfd/src/annunciator.rs

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AnnunciatorLevel { Warning, Caution, Advisory, Status }

#[derive(Debug, Clone)]
pub struct Annunciator {
    pub id: &'static str,
    pub text: &'static str,
    pub level: AnnunciatorLevel,
    pub active: bool,
    pub acknowledged: bool,
}

pub struct AnnunciatorSystem {
    alerts: Vec<Annunciator>,
    flash_phase: bool,
}

impl AnnunciatorSystem {
    pub fn render(&self, ctx: &web_sys::CanvasRenderingContext2d, x: f64, y: f64) {
        let active: Vec<&Annunciator> = self.alerts.iter()
            .filter(|a| a.active)
            .collect();

        for (i, ann) in active.iter().enumerate() {
            let ax = x + (i as f64 % 3.0) * 110.0;
            let ay = y + (i as f64 / 3.0).floor() * 20.0;
            let color = match ann.level {
                AnnunciatorLevel::Warning  => "#F44336",
                AnnunciatorLevel::Caution  => "#FFB300",
                AnnunciatorLevel::Advisory => "#FFFFFF",
                AnnunciatorLevel::Status   => "#4CAF50",
            };
            // Flash warnings/cautions until acknowledged
            let show = if !ann.acknowledged && matches!(ann.level, AnnunciatorLevel::Warning | AnnunciatorLevel::Caution) {
                self.flash_phase
            } else { true };
            if show {
                ctx.set_fill_style_str(color);
                ctx.set_font("bold 11px sans-serif");
                ctx.fill_text(ann.text, ax, ay).unwrap();
            }
        }
    }
}
```

---

## 8. PFD Composite WASM Module {#pfd-wasm}

```rust
// apps/jfoxgcs-wasm/src/pfd.rs
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub struct PfdDisplay {
    adi: crate::adi::AdiRenderer,
    airspeed: crate::airspeed_tape::AirspeedTape,
    altimeter: crate::altimeter_tape::AltimeterTape,
    hsi: crate::hsi::HsiRenderer,
    ann: crate::annunciator::AnnunciatorSystem,
}

#[wasm_bindgen]
impl PfdDisplay {
    #[wasm_bindgen(constructor)]
    pub fn new(canvas_id: &str) -> Result<PfdDisplay, JsValue> {
        // Bind sub-renderers to canvas regions
        todo!("bind canvas context, partition into instrument zones")
    }

    /// Called from JS on every telemetry frame (≥ 30 Hz)
    pub fn update(&self, data_json: &str) -> Result<(), JsValue> {
        let data: PfdData = serde_json::from_str(data_json)
            .map_err(|e| JsValue::from_str(&e.to_string()))?;
        self.adi.render(data.pitch, data.roll, data.slip);
        self.airspeed.render(data.cas_kts, data.cas_trend);
        self.altimeter.render(data.alt_ft, data.sel_alt_ft, data.baro, data.vsi_fpm);
        // ... HSI, FD, annunciators
        Ok(())
    }
}

#[derive(serde::Deserialize)]
pub struct PfdData {
    pub pitch: f64, pub roll: f64, pub slip: f64,
    pub cas_kts: f64, pub cas_trend: f64,
    pub alt_ft: f64, pub sel_alt_ft: f64, pub baro: f64, pub vsi_fpm: f64,
    pub heading_deg: f64, pub sel_hdg_deg: f64,
    pub course_deg: f64, pub course_dev: f64,
    pub fd_pitch: f64, pub fd_roll: f64,
    pub fd_active: bool,
}
```