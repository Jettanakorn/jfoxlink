# Aircraft-Type Specific Reference

## Fixed-Wing

### Control Surface Allocation

```rust
pub struct FixedWingMixer {
    pub aileron_differential: f32,  // 0.0–1.0 (reduces adverse yaw)
    pub flaperon_mix: f32,          // flap + aileron blending
}

impl FixedWingMixer {
    /// Returns: [left_ail, right_ail, elevator, rudder, throttle]
    pub fn mix(&self, roll_cmd: f32, pitch_cmd: f32,
               yaw_cmd: f32, thrust_cmd: f32) -> [f32; 5]
    {
        let ail_diff = self.aileron_differential;
        let la = if roll_cmd > 0.0 { roll_cmd } else { roll_cmd * (1.0 + ail_diff) };
        let ra = -la;
        [ la.clamp(-1.0, 1.0), ra.clamp(-1.0, 1.0),
          pitch_cmd.clamp(-1.0, 1.0),
          yaw_cmd.clamp(-1.0, 1.0),
          thrust_cmd.clamp(0.0, 1.0) ]
    }
}
```

### TECS (Total Energy Control System)

```rust
/// TECS decouples speed and altitude control via total energy
pub struct Tecs {
    pub spdweight: f32,    // 1.0 = equal weight, 2.0 = prioritize speed
    pub throttle_pid: Pid,
    pub pitch_pid: Pid,
    pub energy_rate_filter: LowPassFilter,
}

impl Tecs {
    pub fn update(&mut self, airspeed_cmd: f32, altitude_cmd: f32,
                  airspeed: f32, altitude: f32, dt: f32) -> TecsOutput
    {
        // Specific total energy = V*V/(2g) + h
        let e_desired = airspeed_cmd.powi(2) / (2.0 * G) + altitude_cmd;
        let e_actual  = airspeed.powi(2) / (2.0 * G) + altitude;
        let e_error   = e_desired - e_actual;

        // Energy distribution error
        let ed_error = (altitude_cmd - altitude) * self.spdweight
                     - (airspeed_cmd - airspeed) * (2.0 - self.spdweight);

        TecsOutput {
            throttle: self.throttle_pid.update(e_error, dt),
            pitch_cmd: self.pitch_pid.update(ed_error, dt),
        }
    }
}
```

### L1 Navigation (Fixed-Wing Path Following)

```rust
/// L1 guidance: lateral acceleration command to follow path
/// Hardydev, Park et al. (2004)
pub struct L1Guidance {
    pub l1_period: f32,  // L1 period (s), typically 17–25s
    pub l1_damping: f32, // typically 0.75
    pub nu_max: f32,     // max lateral accel (m/s²)
}

impl L1Guidance {
    pub fn navigate_waypoint(&self, pos: Vector2<f32>, vel: Vector2<f32>,
                             wp_from: Vector2<f32>, wp_to: Vector2<f32>)
        -> f32  // lateral accel command (m/s²)
    {
        let v = vel.norm();
        let l1_dist = v * self.l1_period / core::f32::consts::TAU;
        let track = (wp_to - wp_from).normalize();
        let vec_a = pos - wp_from;
        let xtrack = track.perp_dot(&vec_a); // cross-track error

        let bearing_to_l1 = libm::atan2f(track.y, track.x)
                          - libm::atan2f(xtrack, l1_dist);
        let nu = 2.0 * v * v / l1_dist * libm::sinf(bearing_to_l1);
        nu.clamp(-self.nu_max, self.nu_max)
    }
}
```

---

## Multi-Rotor (Quadrotor / Hexarotor / Octorotor)

### Cascaded PID Controller

```rust
pub struct MultiRotorController {
    pub pos_pid:      [Pid; 3],   // x, y, z
    pub vel_pid:      [Pid; 3],
    pub att_pid:      [Pid; 3],   // roll, pitch, yaw
    pub rate_pid:     [Pid; 3],
    pub mixer:        MotorMixer,
}

impl MultiRotorController {
    pub fn update(&mut self, state: &AircraftState,
                  cmd: &PositionCommand, dt: f32) -> MotorOutputs
    {
        // Position → velocity cmd
        let vel_cmd = self.pos_pid.iter_mut()
            .zip(cmd.pos.iter().zip(state.pos.iter()))
            .map(|(pid, (c, m))| pid.update(c - m, dt))
            .collect::<Vec3>();

        // Velocity → attitude cmd
        let thrust = self.vel_pid[2].update(vel_cmd.z - state.vel.z, dt);
        let roll_cmd  = -self.vel_pid[1].update(vel_cmd.y - state.vel.y, dt);
        let pitch_cmd =  self.vel_pid[0].update(vel_cmd.x - state.vel.x, dt);

        // Attitude → rate cmd
        let rate_cmd = self.att_pid.iter_mut()
            .zip([roll_cmd, pitch_cmd, cmd.yaw].iter()
                 .zip(state.euler.iter()))
            .map(|(pid, (c, m))| pid.update(c - m, dt))
            .collect::<Vec3>();

        // Rate → motor cmd
        let torques = self.rate_pid.iter_mut()
            .zip(rate_cmd.iter().zip(state.omega.iter()))
            .map(|(pid, (c, m))| pid.update(c - m, dt))
            .collect::<Vec3>();

        self.mixer.mix(thrust, torques.x, torques.y, torques.z)
    }
}
```

### Actuator Fault Tolerance (Hexarotor)

```rust
/// Pseudo-inverse allocation with motor failure handling
pub struct FaultTolerantMixer {
    pub effectiveness_matrix: [[f32; 6]; 4], // 4 outputs × 6 motors
    pub failed_motors: [bool; 6],
}

impl FaultTolerantMixer {
    pub fn recompute_allocation(&mut self) {
        // Remove columns for failed motors
        // Recompute Moore-Penrose pseudo-inverse of reduced matrix
        // Hexarotor can tolerate 1 motor failure; reconfigures automatically
        let active: heapless::Vec<usize, 6> = (0..6)
            .filter(|&i| !self.failed_motors[i])
            .collect();
        // ... SVD-based reallocation
    }

    pub fn report_motor_failure(&mut self, motor_idx: usize) {
        self.failed_motors[motor_idx] = true;
        self.recompute_allocation();
    }
}
```

---

## eVTOL (Tiltrotor / Tailsitter)

### Tiltrotor — Unified Control Allocation

```rust
/// Bell V-280 / AW609 style tiltrotor
pub struct TiltrotorController {
    pub tilt_angle: f32,          // 0 = helicopter, 90° = airplane
    pub rotor_ctrl: RotorControl,
    pub surface_ctrl: SurfaceControl,
    pub transition_state: TransitionState,
}

impl TiltrotorController {
    pub fn update(&mut self, state: &AircraftState,
                  cmd: &FlightCmd, dt: f32) -> TiltrotorOutput
    {
        let alpha = self.transition_blend_factor(state.airspeed);

        // Rotor contributions (helicopter-style)
        let rotor_out = self.rotor_ctrl.update(state, cmd, alpha, dt);

        // Surface contributions (airplane-style)
        let surf_out = self.surface_ctrl.update(state, cmd, alpha, dt);

        // Nacelle tilt rate limiting
        let tilt_cmd = if state.airspeed > 15.0 { 90.0_f32.to_radians() }
                       else { 0.0_f32 };
        let max_tilt_rate = 5.0_f32.to_radians(); // 5°/s
        self.tilt_angle += (tilt_cmd - self.tilt_angle)
                          .clamp(-max_tilt_rate * dt, max_tilt_rate * dt);

        TiltrotorOutput { rotor_out, surf_out, tilt_angle: self.tilt_angle }
    }
}
```

### Tailsitter (Single / Dual Prop)

```rust
pub struct TailsitterController {
    pub mode: TailsitterMode,
    pub vtol_att_ctrl: AttitudeController,  // hover: body-frame pitch = yaw
    pub fw_ctrl: FixedWingController,
    pub blend_scheduler: BlendScheduler,
}

#[derive(Clone, Copy)]
pub enum TailsitterMode {
    HoverAscent,
    HoverStation,
    TransitionPushover,  // nose pitches from vertical to horizontal
    FixedWing,
    TransitionFlare,     // nose pitches from horizontal to vertical
    HoverDescent,
}
```

---

## Flight Envelope Protection

### Angle of Attack Limiter (Fixed-Wing)

```rust
pub struct EnvelopeProtection {
    pub alpha_max:     f32,   // e.g., 15° for most GA aircraft
    pub alpha_warn:    f32,   // stall warning onset
    pub bank_max:      f32,   // max bank angle
    pub load_factor_max: f32, // g limit
    pub low_speed_limit: f32, // 1.3 × Vs
}

impl EnvelopeProtection {
    pub fn apply(&self, pilot_cmd: PilotCmd,
                 state: &AircraftState) -> PilotCmd
    {
        let mut cmd = pilot_cmd;

        // Alpha limiting: reduce pitch-up command near stall
        if state.alpha > self.alpha_warn {
            let margin = (self.alpha_max - state.alpha) /
                         (self.alpha_max - self.alpha_warn);
            let limit = margin.clamp(0.0, 1.0);
            if cmd.pitch > 0.0 { cmd.pitch *= limit; }
        }

        // Bank angle limit
        if state.bank.abs() > self.bank_max.to_radians() {
            if cmd.roll * state.bank.signum() > 0.0 {
                cmd.roll *= 0.0; // block further roll in that direction
            }
        }

        // G limiting
        if state.load_factor > self.load_factor_max {
            if cmd.pitch > 0.0 { cmd.pitch = 0.0; }
        }

        cmd
    }
}
```

---

## Payload / Mass Properties

```rust
/// Dynamic CoG tracking for variable payload (e.g., delivery UAV)
pub struct MassProperties {
    pub empty_mass: f32,
    pub empty_cog: Vector3<f32>,
    pub payload_mass: f32,
    pub payload_cog: Vector3<f32>,
}

impl MassProperties {
    pub fn total_mass(&self) -> f32 {
        self.empty_mass + self.payload_mass
    }

    pub fn composite_cog(&self) -> Vector3<f32> {
        let m_tot = self.total_mass();
        (self.empty_cog * self.empty_mass + self.payload_cog * self.payload_mass) / m_tot
    }

    pub fn compute_inertia(&self) -> Matrix3<f32> {
        // Parallel-axis theorem from component inertias
        // I_total = I_empty + m_empty*(d_empty²) + I_payload + m_payload*(d_payload²)
        todo!("compute from geometry")
    }
}
```