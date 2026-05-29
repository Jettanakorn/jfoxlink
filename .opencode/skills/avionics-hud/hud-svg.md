# HUD Reference — Combiner Glass Overlay, Flight Path Marker, SVS

## Table of Contents
1. [HUD Symbology Standard (MIL-STD-1787D)](#symbology)
2. [Flight Path Marker (Velocity Vector)](#fpm)
3. [Pitch Ladder (Conformal)](#pitch-ladder)
4. [HUD Renderer — WebGL 2.0](#webgl)
5. [Synthetic Vision System (SVS)](#svs)
6. [HUD Data Model](#data-model)
7. [UAV GCS HUD Overlay (SVG fast path)](#svg-hud)

---

## 1. HUD Symbology Standard {#symbology}

HUD symbology follows MIL-STD-1787D (fixed-wing) and adapts for GA / UAV use:

| Symbol | Shape | Color | Meaning |
|---|---|---|---|
| Flight Path Marker | Circle + 3 spokes (2 side + 1 down) | Green | Actual flight path vector |
| Pitch ladder | Horizontal bars, chevrons > ±30° | White | Inertial pitch ref |
| Horizon line | Solid line, wings | White | Level flight reference |
| Airspeed box | Left-side digital readout | White | IAS/CAS kts |
| Altitude box | Right-side digital readout | White | Barometric alt |
| Heading tape | Bottom strip | White | Magnetic heading |
| Ghost horizon | Dashed line at HUD edge | Amber | Unusual attitude warning |
| Velocity vector cage | Diamond around FPM | Amber | FD command |
| Stall warning chevron | Expanding brackets below FPM | Red | Approaching stall |
| Ground collision | Terrain pull-up bars | Red | GPWS/TAWS alert |

---

## 2. Flight Path Marker {#fpm}

The FPM represents the actual direction of travel through the air mass.
Computed from inertial velocity (NED), not attitude.

```rust
// avionics-hud/src/fpm.rs
use nalgebra::{Vector3, UnitQuaternion, Rotation3};

/// Compute HUD screen position of the Flight Path Marker
/// Returns (x_mrad, y_mrad) in milliradians from boresight
pub fn compute_fpm_position(
    velocity_ned: Vector3<f64>,  // m/s, NED frame
    attitude_quat: UnitQuaternion<f64>,
) -> (f64, f64) {
    let v = velocity_ned;
    let speed = v.magnitude();
    if speed < 1.0 { return (0.0, 0.0); } // stationary — park at center

    // Flight path angle (γ) and track angle (χ) in NED
    let gamma_rad = (-v.z / speed).asin();  // positive up
    let chi_rad   = v.y.atan2(v.x);        // track angle from North

    // Convert to body-frame look angles relative to current attitude
    let r = attitude_quat.inverse().to_rotation_matrix();
    let fpa_vec = Vector3::new(chi_rad.cos() * gamma_rad.cos(),
                                chi_rad.sin() * gamma_rad.cos(),
                                -gamma_rad.sin());
    let body_vec = r * fpa_vec;

    // Project to HUD screen plane (small-angle approximation, milliradians)
    let x_mrad = (body_vec.y / body_vec.x).atan() * 1000.0;
    let y_mrad = (body_vec.z / body_vec.x).atan() * 1000.0;
    (x_mrad, y_mrad)
}

/// Draw FPM circle + spokes on Canvas2D
pub fn draw_fpm(
    ctx: &web_sys::CanvasRenderingContext2d,
    screen_x: f64, screen_y: f64,
    radius: f64,
    active: bool,
) {
    let color = if active { "#4CAF50" } else { "#888888" };
    ctx.set_stroke_style_str(color);
    ctx.set_line_width(2.0);

    // Circle
    ctx.begin_path();
    ctx.arc(screen_x, screen_y, radius, 0.0, std::f64::consts::TAU).unwrap();
    ctx.stroke();

    // Left spoke
    ctx.begin_path();
    ctx.move_to(screen_x - radius, screen_y);
    ctx.line_to(screen_x - radius * 2.2, screen_y);
    ctx.stroke();

    // Right spoke
    ctx.begin_path();
    ctx.move_to(screen_x + radius, screen_y);
    ctx.line_to(screen_x + radius * 2.2, screen_y);
    ctx.stroke();

    // Down spoke
    ctx.begin_path();
    ctx.move_to(screen_x, screen_y + radius);
    ctx.line_to(screen_x, screen_y + radius * 1.8);
    ctx.stroke();
}
```

---

## 3. Pitch Ladder (Conformal) {#pitch-ladder}

HUD pitch ladder lines are conformal — they rotate and translate to remain aligned with the horizon.

```rust
// avionics-hud/src/pitch_ladder.rs

pub struct HudPitchLadder {
    pub cx: f64, pub cy: f64,
    pub mrad_to_px: f64,   // pixels per milliradian
    pub roll_deg: f64,
    pub pitch_deg: f64,
}

impl HudPitchLadder {
    pub fn render(&self, ctx: &web_sys::CanvasRenderingContext2d) {
        ctx.save();
        ctx.translate(self.cx, self.cy).unwrap();
        ctx.rotate(-self.roll_deg.to_radians()).unwrap();

        let pitch_offset_px = self.pitch_deg * (self.mrad_to_px * 17.4); // 17.4 mrad/deg

        ctx.set_stroke_style_str("#FFFFFF");

        // Horizon line (zero pitch)
        ctx.set_line_width(2.0);
        ctx.begin_path();
        ctx.move_to(-300.0, -pitch_offset_px);
        ctx.line_to(-60.0,  -pitch_offset_px);
        ctx.move_to(60.0,   -pitch_offset_px);
        ctx.line_to(300.0,  -pitch_offset_px);
        ctx.stroke();

        // Pitch ladder lines every 5°
        for i in -18i32..=18 {
            if i == 0 { continue; }
            let pitch_val = i as f64 * 5.0;
            let y = -pitch_offset_px - pitch_val * self.mrad_to_px * 17.4;
            let half_w = if i % 2 == 0 { 80.0 } else { 40.0 };

            ctx.set_line_width(if i % 2 == 0 { 1.5 } else { 1.0 });

            if pitch_val > 0.0 {
                // Positive pitch: solid lines
                ctx.begin_path();
                ctx.move_to(-half_w, y);
                ctx.line_to(-10.0, y);
                ctx.move_to(10.0, y);
                ctx.line_to(half_w, y);
                ctx.stroke();
            } else {
                // Negative pitch: dashed lines per MIL-STD-1787D
                ctx.set_line_dash(&js_sys::Array::of2(&JsValue::from(6.0), &JsValue::from(4.0))).unwrap();
                ctx.begin_path();
                ctx.move_to(-half_w, y);
                ctx.line_to(-10.0, y);
                ctx.move_to(10.0, y);
                ctx.line_to(half_w, y);
                ctx.stroke();
                ctx.set_line_dash(&js_sys::Array::new()).unwrap();
            }

            // Closing chevrons at ±30°, ±60° (unusual attitude warning)
            if i.abs() == 6 || i.abs() == 12 {
                let sign = if pitch_val > 0.0 { -1.0 } else { 1.0 };
                ctx.set_stroke_style_str(if i.abs() == 12 { "#F44336" } else { "#FFB300" });
                ctx.begin_path();
                ctx.move_to(-half_w, y);
                ctx.line_to(-half_w, y + sign * 10.0);
                ctx.move_to(half_w, y);
                ctx.line_to(half_w, y + sign * 10.0);
                ctx.stroke();
                ctx.set_stroke_style_str("#FFFFFF");
            }

            // Labels on major lines
            if i % 2 == 0 {
                ctx.set_fill_style_str("#FFFFFF");
                ctx.set_font("bold 12px monospace");
                ctx.set_text_align("right");
                let label = format!("{}", pitch_val.abs() as i32);
                ctx.fill_text(&label, -half_w - 6.0, y + 4.0).unwrap();
                ctx.set_text_align("left");
                ctx.fill_text(&label, half_w + 6.0, y + 4.0).unwrap();
            }
        }

        ctx.restore();
    }
}
```

---

## 4. HUD Renderer — WebGL 2.0 {#webgl}

For low-latency, GPU-accelerated HUD rendering (< 5ms frame budget):

```rust
// avionics-hud/src/webgl_hud.rs
use wasm_bindgen::prelude::*;
use web_sys::{WebGl2RenderingContext as GL, WebGlProgram, WebGlBuffer};

pub struct WebGlHudRenderer {
    gl: GL,
    program: WebGlProgram,
    vertex_buf: WebGlBuffer,
    width: f32, height: f32,
}

// HUD uses a simple 2D line renderer with instanced drawing
const VERT_SHADER: &str = r#"#version 300 es
layout(location=0) in vec2 a_pos;
uniform mat3 u_transform;
void main() {
    vec3 p = u_transform * vec3(a_pos, 1.0);
    gl_Position = vec4(p.xy, 0.0, 1.0);
}
"#;

const FRAG_SHADER: &str = r#"#version 300 es
precision mediump float;
uniform vec4 u_color;
out vec4 fragColor;
void main() { fragColor = u_color; }
"#;

impl WebGlHudRenderer {
    pub fn new(canvas: &web_sys::HtmlCanvasElement) -> Result<Self, JsValue> {
        let gl = canvas
            .get_context("webgl2")?.unwrap()
            .dyn_into::<GL>()?;

        gl.enable(GL::BLEND);
        gl.blend_func(GL::SRC_ALPHA, GL::ONE_MINUS_SRC_ALPHA);

        let program = Self::compile_program(&gl, VERT_SHADER, FRAG_SHADER)?;
        let vertex_buf = gl.create_buffer().ok_or("no buf")?;

        Ok(Self { gl, program, vertex_buf,
            width: canvas.width() as f32,
            height: canvas.height() as f32,
        })
    }

    /// Draw a line in HUD NDC space (-1..1)
    pub fn draw_line(&self, x1: f32, y1: f32, x2: f32, y2: f32, color: [f32;4], width: f32) {
        self.gl.use_program(Some(&self.program));
        // Upload vertices
        let verts: [f32; 4] = [x1, y1, x2, y2];
        let verts_js = unsafe { js_sys::Float32Array::view(&verts) };
        self.gl.bind_buffer(GL::ARRAY_BUFFER, Some(&self.vertex_buf));
        self.gl.buffer_data_with_array_buffer_view(GL::ARRAY_BUFFER, &verts_js, GL::DYNAMIC_DRAW);
        self.gl.vertex_attrib_pointer_with_i32(0, 2, GL::FLOAT, false, 0, 0);
        self.gl.enable_vertex_attrib_array(0);

        // Set uniforms
        let loc_color = self.gl.get_uniform_location(&self.program, "u_color");
        self.gl.uniform4fv_with_f32_array(loc_color.as_ref(), &color);

        let identity: [f32;9] = [1.,0.,0., 0.,1.,0., 0.,0.,1.];
        let loc_tx = self.gl.get_uniform_location(&self.program, "u_transform");
        self.gl.uniform_matrix3fv_with_f32_array(loc_tx.as_ref(), false, &identity);

        self.gl.line_width(width);
        self.gl.draw_arrays(GL::LINES, 0, 2);
    }

    fn compile_program(gl: &GL, vert_src: &str, frag_src: &str) -> Result<WebGlProgram, JsValue> {
        let vert = gl.create_shader(GL::VERTEX_SHADER).ok_or("no vert")?;
        gl.shader_source(&vert, vert_src);
        gl.compile_shader(&vert);
        let frag = gl.create_shader(GL::FRAGMENT_SHADER).ok_or("no frag")?;
        gl.shader_source(&frag, frag_src);
        gl.compile_shader(&frag);
        let prog = gl.create_program().ok_or("no prog")?;
        gl.attach_shader(&prog, &vert);
        gl.attach_shader(&prog, &frag);
        gl.link_program(&prog);
        Ok(prog)
    }
}
```

---

## 5. Synthetic Vision System (SVS) {#svs}

SVS renders a textured 3D terrain view behind the ADI, giving pilots terrain awareness without outside references.

```rust
// avionics-hud/src/svs.rs
// Integration: feed DEM tiles from gcs-airspace into WebGL terrain mesh

pub struct SvsRenderer {
    gl: WebGlHudRenderer,
    terrain_shader: WebGlProgram,
    dem_cache: std::collections::HashMap<(i32,i32), TerrainTile>,
}

// Terrain tile covers 1°×1° lat/lon at 30m SRTM resolution (3601×3601 posts)
pub struct TerrainTile {
    pub lat0: f64, pub lon0: f64,
    pub elevation_m: Vec<f32>,   // 3601×3601 grid, row-major
    pub gl_buf: Option<web_sys::WebGlBuffer>,
}

// Color scheme for SVS terrain (Garmin G1000 SVS palette)
pub fn elevation_to_color(elev_m: f32, aircraft_alt_m: f32) -> [f32; 4] {
    let separation = aircraft_alt_m - elev_m;
    if separation < 100.0 {
        [0.9, 0.1, 0.1, 1.0]         // red — imminent terrain
    } else if separation < 500.0 {
        [0.9, 0.5, 0.0, 1.0]         // amber — terrain caution
    } else if elev_m < 0.0 {
        [0.05, 0.2, 0.7, 1.0]         // deep blue — water
    } else if elev_m < 500.0 {
        [0.2, 0.55, 0.15, 1.0]        // green — lowland
    } else if elev_m < 2000.0 {
        [0.55, 0.42, 0.18, 1.0]       // brown — highland
    } else {
        [0.85, 0.85, 0.85, 1.0]       // grey-white — high mountain
    }
}
```

---

## 6. HUD Data Model {#data-model}

```rust
// avionics-hud/src/data.rs
#[derive(serde::Deserialize, Clone, Default)]
pub struct HudState {
    // Inertial / AHRS
    pub pitch_deg: f64,
    pub roll_deg: f64,
    pub heading_deg: f64,
    pub body_pitch_rate: f64,

    // Air data
    pub ias_kts: f64,
    pub tas_kts: f64,
    pub mach: f64,
    pub altitude_ft: f64,
    pub altitude_agl_ft: f64,
    pub vsi_fpm: f64,
    pub aoa_deg: f64,

    // Navigation
    pub track_deg: f64,
    pub flight_path_angle_deg: f64,
    pub cross_track_err_nm: f64,
    pub glideslope_dev: f64,     // dots ±2.5
    pub localizer_dev: f64,

    // Flight director commands
    pub fd_pitch_cmd_deg: f64,
    pub fd_roll_cmd_deg: f64,
    pub fd_active: bool,

    // Mode annunciations
    pub ap_mode: ApMode,
    pub at_mode: AtMode,
    pub nav_source: crate::hsi::NavSource,

    // Alerting
    pub gpws_pull_up: bool,
    pub stall_warning: bool,
    pub overspeed_warning: bool,
    pub ra_advisory: Option<String>,  // TCAS RA text
}

#[derive(serde::Deserialize, Clone, Default)]
pub enum ApMode {
    #[default] Off,
    Roll, Heading, Nav, Appr, Go,
    Pitch, Vs, Flch, AltHold,
    Vnav, Glideslope,
}

#[derive(serde::Deserialize, Clone, Default)]
pub enum AtMode {
    #[default] Off, Speed, Throttle, Toga,
}
```

---

## 7. UAV GCS HUD Overlay (SVG fast path) {#svg-hud}

For the JFOXGCS browser app, a lightweight SVG HUD overlaid on the Cesium 3D view:

```javascript
// frontend/src/hud/SvgHud.jsx (React component)
export function SvgHud({ state }) {
  const { pitch, roll, heading, ias, alt, vsi, fpmX, fpmY, fdActive } = state;

  // Convert mrad to SVG pixels (center 400,300 on 800×600 HUD)
  const MRAD_TO_PX = 2.5;
  const CX = 400, CY = 300;

  const pitchOffsetPx = pitch * 17.4 * MRAD_TO_PX; // 17.4 mrad/deg
  const rollRad = roll * Math.PI / 180;

  return (
    <svg
      viewBox="0 0 800 600"
      style={{ position: 'absolute', top: 0, left: 0, width: '100%', height: '100%',
               pointerEvents: 'none', fontFamily: 'monospace' }}
    >
      {/* Pitch ladder group — rotates with roll */}
      <g transform={`translate(${CX},${CY}) rotate(${-roll})`}>
        {/* Horizon line */}
        <line x1={-300} y1={-pitchOffsetPx} x2={300} y2={-pitchOffsetPx}
              stroke="white" strokeWidth={2} />
        {/* Pitch lines -30 to +30, every 5° */}
        {[-30,-25,-20,-15,-10,-5,5,10,15,20,25,30].map(p => {
          const y = -pitchOffsetPx - p * 17.4 * MRAD_TO_PX;
          const w = p % 10 === 0 ? 80 : 40;
          return (
            <g key={p}>
              <line x1={-w} y1={y} x2={-10} y2={y}
                    stroke="white" strokeWidth={p%10===0?1.5:1}
                    strokeDasharray={p < 0 ? "6,4" : undefined} />
              <line x1={10} y1={y} x2={w} y2={y}
                    stroke="white" strokeWidth={p%10===0?1.5:1}
                    strokeDasharray={p < 0 ? "6,4" : undefined} />
              {p%10===0 && <>
                <text x={-w-6} y={y+4} fill="white" fontSize={11}
                      textAnchor="end">{Math.abs(p)}</text>
                <text x={w+6} y={y+4} fill="white" fontSize={11}>{Math.abs(p)}</text>
              </>}
            </g>
          );
        })}
      </g>

      {/* Flight path marker */}
      <g transform={`translate(${CX + fpmX*MRAD_TO_PX}, ${CY - fpmY*MRAD_TO_PX})`}>
        <circle r={12} fill="none" stroke="#4CAF50" strokeWidth={2} />
        <line x1={-28} y1={0} x2={-12} y2={0} stroke="#4CAF50" strokeWidth={2} />
        <line x1={12}  y1={0} x2={28}  y2={0} stroke="#4CAF50" strokeWidth={2} />
        <line x1={0} y1={12} x2={0}  y2={22} stroke="#4CAF50" strokeWidth={2} />
      </g>

      {/* Flight director V-bar */}
      {fdActive && (
        <g transform={`translate(${CX},${CY}) rotate(${-roll})`}>
          <polyline
            points={`${-80},10 0,-20 80,10`}
            fill="none" stroke="#E040FB" strokeWidth={3} strokeLinejoin="round"
          />
        </g>
      )}

      {/* Airspeed box — left */}
      <rect x={20} y={260} width={70} height={30} fill="black" stroke="white" strokeWidth={1}/>
      <text x={55} y={281} fill="white" fontSize={18} fontWeight="bold" textAnchor="middle">
        {Math.round(ias)}
      </text>
      <text x={55} y={255} fill="#888" fontSize={11} textAnchor="middle">KTS</text>

      {/* Altitude box — right */}
      <rect x={710} y={260} width={70} height={30} fill="black" stroke="white" strokeWidth={1}/>
      <text x={745} y={281} fill="white" fontSize={18} fontWeight="bold" textAnchor="middle">
        {Math.round(alt)}
      </text>
      <text x={745} y={255} fill="#888" fontSize={11} textAnchor="middle">FT</text>

      {/* Heading tape — bottom */}
      <HeadingTape heading={heading} cx={CX} />

      {/* VSI digital */}
      <text x={760} y={310} fill={Math.abs(vsi)>1000 ? "#F44336" : "#FFFFFF"}
            fontSize={13} fontWeight="bold">
        {vsi >= 0 ? `+${Math.round(vsi)}` : `${Math.round(vsi)}`}
      </text>
    </svg>
  );
}

function HeadingTape({ heading, cx }) {
  const ticks = [];
  for (let d = -40; d <= 40; d += 5) {
    const x = cx + d * 5;
    const hdg = ((Math.round(heading / 5) * 5 + d) % 360 + 360) % 360;
    ticks.push(
      <g key={d}>
        <line x1={x} y1={565} x2={x} y2={d%10===0 ? 548 : 555}
              stroke="white" strokeWidth={1} />
        {d%10===0 && <text x={x} y={545} fill="white" fontSize={11}
                           textAnchor="middle">{hdg === 0 ? 'N' : hdg === 90 ? 'E' :
                             hdg === 180 ? 'S' : hdg === 270 ? 'W' : hdg/10}</text>}
      </g>
    );
  }
  return (
    <g>
      <rect x={0} y={540} width={800} height={60} fill="rgba(0,0,0,0.4)" />
      {ticks}
      {/* Current heading box */}
      <rect x={cx-24} y={564} width={48} height={20} fill="black" stroke="white" strokeWidth={1}/>
      <text x={cx} y={579} fill="white" fontSize={14} fontWeight="bold"
            textAnchor="middle">{String(Math.round(heading) % 360).padStart(3,'0')}</text>
    </g>
  );
}
```