# MCAS, Autopilot & Flight Envelope Protection Reference

## Table of Contents
1. [MCAS Architecture](#mcas)
2. [Autopilot Mode Logic](#ap-modes)
3. [Autothrottle](#at)
4. [Flight Envelope Protection](#fep)
5. [Trim Runaway Detection](#trim-runaway)
6. [Autopilot Annunciator Rendering](#ap-annunciator)
7. [AFCS State Machine](#afcs-sm)

---

## 1. MCAS Architecture {#mcas}

MCAS (Maneuvering Characteristics Augmentation System) applies automatic horizontal
stabilizer trim to counteract pitch-up tendency at high AoA. Key constraints per
design intent (post-737 MAX review, DO-178C guidance):

**Authority limits:**
- Max single activation: 0.6° stabilizer movement
- Inhibit if: both AoA sensors disagree > 5.5°, airspeed disagree > 35 kts,
  altitude < 400 ft AGL, gear down, manual override on yoke

```rust
// avionics-mcas/src/mcas.rs
use anyhow::Result;

#[derive(Debug, Clone)]
pub struct McasState {
    pub enabled: bool,
    pub last_activation: Option<std::time::Instant>,
    pub total_trim_applied_deg: f64,
    pub inhibited: bool,
    pub inhibit_reason: Option<McasInhibitReason>,
}

#[derive(Debug, Clone)]
pub enum McasInhibitReason {
    AoaDisagreement { deg_diff: f64 },
    AirspeedDisagreement { kts_diff: f64 },
    BelowMinAltitude { agl_ft: f64 },
    GearDown,
    ManualOverride,
    AlreadyActivatedThisCycle,
}

pub struct McasConfig {
    pub aoa_trigger_deg: f64,        // Typically 13–15° AoA
    pub authority_per_act_deg: f64,  // 0.6° per activation
    pub trim_rate_deg_per_s: f64,    // Stabilizer travel rate
    pub min_alt_agl_ft: f64,         // 400 ft typically
    pub aoa_disagree_limit_deg: f64, // 5.5°
    pub spd_disagree_limit_kts: f64, // 35 kts
    pub cooldown_s: f64,             // Time between re-activations
}

impl McasConfig {
    pub fn default_737_style() -> Self {
        Self {
            aoa_trigger_deg: 13.5,
            authority_per_act_deg: 0.6,
            trim_rate_deg_per_s: 0.27,
            min_alt_agl_ft: 400.0,
            aoa_disagree_limit_deg: 5.5,
            spd_disagree_limit_kts: 35.0,
            cooldown_s: 5.0,
        }
    }
}

pub struct McasSystem {
    pub config: McasConfig,
    pub state: McasState,
}

impl McasSystem {
    pub fn evaluate(
        &mut self,
        aoa_left_deg: f64,
        aoa_right_deg: f64,
        airspeed_left_kts: f64,
        airspeed_right_kts: f64,
        altitude_agl_ft: f64,
        gear_down: bool,
        manual_override: bool,
        flaps_angle_deg: f64,
        dt_s: f64,
    ) -> McasOutput {
        // --- Inhibit logic ---
        if manual_override {
            self.state.inhibited = true;
            self.state.inhibit_reason = Some(McasInhibitReason::ManualOverride);
            return McasOutput::inhibited();
        }
        if (aoa_left_deg - aoa_right_deg).abs() > self.config.aoa_disagree_limit_deg {
            self.state.inhibited = true;
            self.state.inhibit_reason = Some(McasInhibitReason::AoaDisagreement {
                deg_diff: (aoa_left_deg - aoa_right_deg).abs()
            });
            return McasOutput::inhibited();
        }
        if (airspeed_left_kts - airspeed_right_kts).abs() > self.config.spd_disagree_limit_kts {
            self.state.inhibited = true;
            self.state.inhibit_reason = Some(McasInhibitReason::AirspeedDisagreement {
                kts_diff: (airspeed_left_kts - airspeed_right_kts).abs()
            });
            return McasOutput::inhibited();
        }
        if altitude_agl_ft < self.config.min_alt_agl_ft {
            return McasOutput::inhibited();
        }
        if gear_down {
            return McasOutput::inhibited();
        }
        if flaps_angle_deg > 0.5 {
            return McasOutput::inhibited(); // MCAS only active with flaps up
        }

        self.state.inhibited = false;
        self.state.inhibit_reason = None;

        // --- Activation logic ---
        let aoa_active = (aoa_left_deg + aoa_right_deg) / 2.0; // Use average
        if aoa_active < self.config.aoa_trigger_deg {
            return McasOutput::inactive();
        }

        // Cooldown check
        if let Some(last) = self.state.last_activation {
            if last.elapsed().as_secs_f64() < self.config.cooldown_s {
                self.state.inhibit_reason = Some(McasInhibitReason::AlreadyActivatedThisCycle);
                return McasOutput::inhibited();
            }
        }

        // Apply trim — nose-down (negative = leading edge down)
        let trim_cmd = -self.config.trim_rate_deg_per_s * dt_s;
        let applied = trim_cmd.max(-self.config.authority_per_act_deg);
        self.state.total_trim_applied_deg += applied;
        self.state.last_activation = Some(std::time::Instant::now());

        tracing::warn!(
            "MCAS ACTIVATED: AoA={:.1}° trim={:.3}° total={:.3}°",
            aoa_active, applied, self.state.total_trim_applied_deg
        );

        McasOutput { active: true, trim_cmd_deg: applied, inhibited: false }
    }
}

#[derive(Debug, Clone, Default)]
pub struct McasOutput {
    pub active: bool,
    pub trim_cmd_deg: f64,   // Negative = nose-down stabilizer
    pub inhibited: bool,
}

impl McasOutput {
    fn inhibited() -> Self { Self { inhibited: true, ..Default::default() } }
    fn inactive() -> Self { Self::default() }
}
```

---

## 2. Autopilot Mode Logic {#ap-modes}

```rust
// avionics-mcas/src/autopilot.rs

#[derive(Debug, Clone, PartialEq)]
pub enum RollMode {
    Off, RollHold(f64), HeadingSelect(f64), Nav, Approach, GoAround,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PitchMode {
    Off, PitchHold(f64), VerticalSpeed(f64), FlightLevelChange(f64),
    AltitudeHold(f64), VnavPath, Glideslope, GoAround,
}

pub struct AutopilotState {
    pub engaged: bool,
    pub roll_mode: RollMode,
    pub pitch_mode: PitchMode,
    pub armed_roll: Option<RollMode>,
    pub armed_pitch: Option<PitchMode>,
    pub flight_director_only: bool,
}

pub struct AutopilotComputer {
    pub state: AutopilotState,
    roll_pid: PidController,
    pitch_pid: PidController,
    vs_pid: PidController,
}

impl AutopilotComputer {
    pub fn compute_commands(
        &mut self,
        ahrs: &AhrsData,
        adc: &AdcData,
        nav: &NavData,
        dt_s: f64,
    ) -> ApCommands {
        let roll_cmd = match &self.state.roll_mode {
            RollMode::Off => 0.0,
            RollMode::RollHold(target) => {
                self.roll_pid.update(*target - ahrs.roll_deg, dt_s)
            },
            RollMode::HeadingSelect(target_hdg) => {
                let err = heading_error(*target_hdg, ahrs.heading_deg);
                let bank_cmd = (err * 2.0).clamp(-30.0, 30.0); // 2°/°
                self.roll_pid.update(bank_cmd - ahrs.roll_deg, dt_s)
            },
            RollMode::Nav => {
                // XTK error to roll command
                let bank_cmd = (nav.cross_track_err_nm * 10.0).clamp(-30.0, 30.0);
                self.roll_pid.update(bank_cmd - ahrs.roll_deg, dt_s)
            },
            _ => 0.0,
        };

        let pitch_cmd = match &self.state.pitch_mode {
            PitchMode::Off => 0.0,
            PitchMode::PitchHold(target) => {
                self.pitch_pid.update(*target - ahrs.pitch_deg, dt_s)
            },
            PitchMode::VerticalSpeed(target_fpm) => {
                let vs_err = *target_fpm - adc.vsi_fpm;
                self.vs_pid.update(vs_err, dt_s)
            },
            PitchMode::AltitudeHold(target_ft) => {
                let alt_err = *target_ft - adc.altitude_ft;
                let vs_cmd  = (alt_err * 0.8).clamp(-1000.0, 1000.0);
                let vs_err  = vs_cmd - adc.vsi_fpm;
                self.vs_pid.update(vs_err, dt_s)
            },
            PitchMode::Glideslope => {
                let gs_err = nav.glideslope_dev_dots * 0.5; // 0.5°/dot
                self.pitch_pid.update(gs_err, dt_s)
            },
            _ => 0.0,
        };

        ApCommands {
            roll_deflection: roll_cmd.clamp(-1.0, 1.0),
            pitch_deflection: pitch_cmd.clamp(-1.0, 1.0),
            fd_pitch_cmd_deg: pitch_cmd * 15.0,
            fd_roll_cmd_deg: roll_cmd * 30.0,
        }
    }
}

fn heading_error(target: f64, current: f64) -> f64 {
    let diff = ((target - current) + 540.0) % 360.0 - 180.0;
    diff
}

pub struct PidController {
    kp: f64, ki: f64, kd: f64,
    integral: f64, last_err: f64,
}

impl PidController {
    pub fn new(kp: f64, ki: f64, kd: f64) -> Self {
        Self { kp, ki, kd, integral: 0.0, last_err: 0.0 }
    }

    pub fn update(&mut self, error: f64, dt: f64) -> f64 {
        self.integral += error * dt;
        self.integral = self.integral.clamp(-1.0, 1.0); // anti-windup
        let derivative = (error - self.last_err) / dt;
        self.last_err = error;
        self.kp * error + self.ki * self.integral + self.kd * derivative
    }

    pub fn reset(&mut self) {
        self.integral = 0.0;
        self.last_err = 0.0;
    }
}
```

---

## 3. Autothrottle {#at}

```rust
// avionics-mcas/src/autothrottle.rs

#[derive(Debug, Clone, PartialEq)]
pub enum AtMode { Off, SpeedHold(f64), ThrottleHold(f64), Toga }

pub struct AutothrottleSystem {
    pub mode: AtMode,
    speed_pid: PidController,
}

impl AutothrottleSystem {
    pub fn compute_throttle(
        &mut self,
        cas_kts: f64,
        altitude_ft: f64,
        dt_s: f64,
    ) -> f64 {
        match &self.mode {
            AtMode::Off => f64::NAN, // no command
            AtMode::SpeedHold(target_kts) => {
                let err = *target_kts - cas_kts;
                let cmd = 0.5 + self.speed_pid.update(err, dt_s);
                cmd.clamp(0.0, 1.0)
            },
            AtMode::ThrottleHold(pos) => *pos,
            AtMode::Toga => 1.0,
        }
    }
}
```

---

## 4. Flight Envelope Protection {#fep}

```rust
// avionics-mcas/src/envelope_protection.rs

pub struct EnvelopeProtection {
    /// Aircraft performance limits
    pub vne_kts: f64,
    pub vmo_kts: f64,        // Max operating speed
    pub mmo: f64,            // Max Mach
    pub bank_limit_deg: f64, // Typically 67° for protection (normal: 30°)
    pub max_aoa_deg: f64,
    pub min_aoa_deg: f64,    // Negative for pusher protection
}

#[derive(Debug, Default)]
pub struct ProtectionOutput {
    pub overspeed_active: bool,
    pub stall_protection_active: bool,
    pub bank_limit_active: bool,
    pub pitch_limit_up_active: bool,
    pub pitch_limit_dn_active: bool,
    pub roll_correction_deg_s: f64,
    pub pitch_correction_deg_s: f64,
}

impl EnvelopeProtection {
    pub fn evaluate(
        &self,
        cas_kts: f64, mach: f64,
        bank_deg: f64, pitch_deg: f64, aoa_deg: f64,
    ) -> ProtectionOutput {
        let mut out = ProtectionOutput::default();

        // Overspeed — gentle pitch-up command
        if cas_kts > self.vmo_kts || mach > self.mmo {
            out.overspeed_active = true;
            out.pitch_correction_deg_s = 2.0; // nose up 2°/s
        }

        // Stall protection — nose-down pusher
        if aoa_deg > self.max_aoa_deg {
            out.stall_protection_active = true;
            let excess = aoa_deg - self.max_aoa_deg;
            out.pitch_correction_deg_s = -(excess * 3.0).min(5.0); // max 5°/s push
        }

        // Bank protection
        if bank_deg.abs() > self.bank_limit_deg {
            out.bank_limit_active = true;
            let excess = bank_deg.abs() - self.bank_limit_deg;
            out.roll_correction_deg_s = -bank_deg.signum() * (excess * 2.0).min(5.0);
        }

        out
    }
}
```

---

## 5. Trim Runaway Detection {#trim-runaway}

```rust
// avionics-mcas/src/trim_runaway.rs

pub struct TrimRunawayDetector {
    trim_change_history: std::collections::VecDeque<(std::time::Instant, f64)>,
    /// If trim moves more than this in window_s without pilot input → runaway
    pub threshold_deg: f64,
    pub window_s: f64,
}

impl TrimRunawayDetector {
    pub fn new() -> Self {
        Self {
            trim_change_history: std::collections::VecDeque::new(),
            threshold_deg: 2.5,
            window_s: 10.0,
        }
    }

    pub fn record_trim_change(&mut self, delta_deg: f64) {
        let now = std::time::Instant::now();
        self.trim_change_history.push_back((now, delta_deg));
        // Evict old entries
        while self.trim_change_history.front()
            .map_or(false, |(t, _)| now.duration_since(*t).as_secs_f64() > self.window_s) {
            self.trim_change_history.pop_front();
        }
    }

    pub fn is_runaway(&self) -> bool {
        let total: f64 = self.trim_change_history.iter().map(|(_, d)| d).sum();
        total.abs() > self.threshold_deg
    }
}
```

---

## 6. Autopilot Annunciator Rendering {#ap-annunciator}

G1000-style AP/FD mode annunciator strip across the top of the PFD:

```rust
// avionics-pfd/src/ap_annunciator.rs

pub fn render_ap_mode_strip(
    ctx: &web_sys::CanvasRenderingContext2d,
    x: f64, y: f64, w: f64,
    ap: &AutopilotState,
    at_mode: &AtMode,
) {
    // Background bar
    ctx.set_fill_style_str("#111111");
    ctx.fill_rect(x, y, w, 22.0);

    let mut col = x + 6.0;

    // AP engagement indicator
    if ap.engaged {
        ctx.set_fill_style_str("#4CAF50");
        ctx.set_font("bold 14px monospace");
        ctx.fill_text("AP", col, y + 15.0).unwrap();
        col += 30.0;
    } else if ap.flight_director_only {
        ctx.set_fill_style_str("#E040FB");
        ctx.fill_text("FD", col, y + 15.0).unwrap();
        col += 30.0;
    }

    // Roll mode
    let roll_text = match &ap.roll_mode {
        RollMode::Off => "",
        RollMode::RollHold(_)     => "ROL",
        RollMode::HeadingSelect(_) => "HDG",
        RollMode::Nav             => "NAV",
        RollMode::Approach        => "APPR",
        RollMode::GoAround        => "GA",
    };
    if !roll_text.is_empty() {
        ctx.set_fill_style_str("#4CAF50"); // active = green
        ctx.set_font("bold 13px monospace");
        ctx.fill_text(roll_text, col, y + 15.0).unwrap();
        col += roll_text.len() as f64 * 8.5 + 10.0;
    }

    // Armed roll mode (cyan)
    if let Some(armed) = &ap.armed_roll {
        let armed_text = match armed {
            RollMode::Nav  => "NAV",
            RollMode::Approach => "APPR",
            _ => "",
        };
        if !armed_text.is_empty() {
            ctx.set_fill_style_str("#00BCD4");
            ctx.fill_text(armed_text, col, y + 15.0).unwrap();
            col += armed_text.len() as f64 * 8.5 + 10.0;
        }
    }

    // Separator
    ctx.set_stroke_style_str("#444444");
    ctx.begin_path();
    ctx.move_to(col, y + 2.0);
    ctx.line_to(col, y + 20.0);
    ctx.stroke();
    col += 8.0;

    // Pitch mode
    let pitch_text = match &ap.pitch_mode {
        PitchMode::Off => "",
        PitchMode::PitchHold(_)     => "PIT",
        PitchMode::VerticalSpeed(v) => return render_vs_mode(ctx, col, y, *v),
        PitchMode::FlightLevelChange(s) => return render_flch_mode(ctx, col, y, *s),
        PitchMode::AltitudeHold(a)  => return render_alt_mode(ctx, col, y, *a),
        PitchMode::VnavPath         => "VPTH",
        PitchMode::Glideslope       => "GS",
        PitchMode::GoAround         => "GA",
    };
    if !pitch_text.is_empty() {
        ctx.set_fill_style_str("#4CAF50");
        ctx.fill_text(pitch_text, col, y + 15.0).unwrap();
    }

    // AT mode (rightmost)
    let at_text = match at_mode {
        AtMode::Off => "",
        AtMode::SpeedHold(s) => return render_at_speed(ctx, w - 60.0 + x, y, *s),
        AtMode::ThrottleHold(_) => "THR",
        AtMode::Toga            => "TOGA",
    };
    if !at_text.is_empty() {
        ctx.set_fill_style_str("#4CAF50");
        ctx.fill_text(at_text, x + w - 40.0, y + 15.0).unwrap();
    }
}

fn render_vs_mode(ctx: &web_sys::CanvasRenderingContext2d, x: f64, y: f64, vs: f64) {
    ctx.set_fill_style_str("#4CAF50");
    ctx.set_font("bold 13px monospace");
    let sign = if vs >= 0.0 { "+" } else { "" };
    ctx.fill_text(&format!("VS {}{:.0}", sign, vs), x, y + 15.0).unwrap();
}

fn render_alt_mode(ctx: &web_sys::CanvasRenderingContext2d, x: f64, y: f64, alt: f64) {
    ctx.set_fill_style_str("#4CAF50");
    ctx.fill_text(&format!("ALT {:.0}", alt), x, y + 15.0).unwrap();
}

fn render_flch_mode(ctx: &web_sys::CanvasRenderingContext2d, x: f64, y: f64, spd: f64) {
    ctx.set_fill_style_str("#4CAF50");
    ctx.fill_text(&format!("FLCH {:.0}", spd), x, y + 15.0).unwrap();
}

fn render_at_speed(ctx: &web_sys::CanvasRenderingContext2d, x: f64, y: f64, spd: f64) {
    ctx.set_fill_style_str("#4CAF50");
    ctx.fill_text(&format!("SPD {:.0}", spd), x, y + 15.0).unwrap();
}
```

---

## 7. AFCS State Machine {#afcs-sm}

```
AFCS (Automatic Flight Control System) transitions:

         [FD only]
             ↓
    [AP Engaged — ROL/PIT]
          ↙        ↘
  [HDG/NAV]      [VS/FLCH/ALT]
     ↓                 ↓
  [APPR arm]     [VNAV arm]
     ↓                 ↓
  [LOC/GS cap]  [VPTH/VFLTE]
     ↓
  [Disconnect on: manual override, large force, out-of-envelope, go-around]
```

```rust
pub enum AfcsTransition {
    EngageAp, DisengageAp,
    SelectRollMode(RollMode),
    SelectPitchMode(PitchMode),
    ArmNavMode,
    CaptureAlt(f64),
    CaptureGlideslope,
    GoAround,
    Disconnect(DisconnectReason),
}

pub enum DisconnectReason {
    PilotPushbutton,
    ManualOverride,
    OutOfEnvelope,
    SensorFail,
    Watchdog,
}
```