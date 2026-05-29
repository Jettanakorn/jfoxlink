# Flight Dynamics Models Reference

## 6-DOF Rigid Body Equations of Motion

### State Vector

```
x = [u, v, w,          ← body-frame velocities (m/s)
     p, q, r,          ← body-frame angular rates (rad/s)
     φ, θ, ψ,          ← Euler angles (rad) OR quaternion q
     x_e, y_e, z_e]    ← Earth-frame position (m, NED)
```

### Translational Dynamics (Body Frame, NED)

```
m·(u̇ + q·w - r·v) = F_x  (surge)
m·(v̇ + r·u - p·w) = F_y  (sway)
m·(ẇ + p·v - q·u) = F_z  (heave)
```

Where total forces F = F_aero + F_thrust + R_be · [0, 0, m·g]ᵀ

### Rotational Dynamics (Euler's Equations)

```
I_xx·ṗ - (I_yy - I_zz)·q·r - I_xz·(ṙ + p·q) = L  (roll moment)
I_yy·q̇ - (I_zz - I_xx)·p·r - I_xz·(p² - r²)  = M  (pitch moment)
I_zz·ṙ - (I_xx - I_yy)·p·q + I_xz·(ṗ - q·r)  = N  (yaw moment)
```

### Quaternion Kinematics (Preferred over Euler)

```
q̇ = 0.5 · q ⊗ [0, p, q, r]ᵀ

Where q = [q0, q1, q2, q3] (scalar-first convention)
Constraint: |q| = 1  (enforce by normalization every step)
```

### Rust Implementation (6-DOF Integrator)

```rust
// fc-core/src/dynamics/rigid_body.rs
use nalgebra::{Vector3, Matrix3, UnitQuaternion, Quaternion};

pub struct RigidBodyState {
    pub vel_body:   Vector3<f32>,    // [u, v, w]
    pub omega_body: Vector3<f32>,    // [p, q, r]
    pub attitude:   UnitQuaternion<f32>,
    pub pos_ned:    Vector3<f32>,
    pub mass:       f32,
    pub inertia:    Matrix3<f32>,
    pub inv_inertia: Matrix3<f32>,
}

impl RigidBodyState {
    pub fn integrate(&mut self, forces: Vector3<f32>,
                     moments: Vector3<f32>, dt: f32) {
        // Translational (body frame)
        let v = self.vel_body;
        let w = self.omega_body;
        let vdot = forces / self.mass - w.cross(&v);
        self.vel_body += vdot * dt;

        // Rotational
        let wdot = self.inv_inertia
            * (moments - w.cross(&(self.inertia * w)));
        self.omega_body += wdot * dt;

        // Quaternion kinematics
        let omega_quat = Quaternion::new(0.0, w.x, w.y, w.z);
        let qdot = self.attitude.quaternion() * omega_quat * 0.5;
        let new_q = self.attitude.quaternion() + qdot * dt;
        self.attitude = UnitQuaternion::from_quaternion(new_q); // normalizes

        // Position integration (NED)
        let vel_ned = self.attitude * self.vel_body;
        self.pos_ned += vel_ned * dt;
    }
}
```

---

## Fixed-Wing Aerodynamic Model

### Linear Aerodynamics (Small-Perturbation, Stability Axes)

```
C_L = C_L0 + C_Lα·α + C_Lδe·δe + C_Lq·(q·c̄/(2V))
C_D = C_D0 + k·C_L²  (Oswald efficiency model)
C_m = C_m0 + C_mα·α + C_mδe·δe + C_mq·(q·c̄/(2V))

C_Y = C_Yβ·β + C_Yδa·δa + C_Yδr·δr
C_l = C_lβ·β + C_lδa·δa + C_lδr·δr + C_lp·(p·b/(2V)) + C_lr·(r·b/(2V))
C_n = C_nβ·β + C_nδa·δa + C_nδr·δr + C_np·(p·b/(2V)) + C_nr·(r·b/(2V))
```

### Key Stability Derivatives (Cessna 172 example)

```rust
pub struct FixedWingAero {
    // Geometry
    pub s: f32,       // Wing area (m²) = 16.2
    pub b: f32,       // Wingspan (m)   = 11.0
    pub c_bar: f32,   // MAC (m)        = 1.49

    // Longitudinal
    pub cl0: f32,     // = 0.307
    pub cl_alpha: f32,// = 4.41  rad⁻¹
    pub cl_de: f32,   // = 0.43  rad⁻¹
    pub cl_q: f32,    // = 3.9
    pub cd0: f32,     // = 0.0270
    pub k: f32,       // = 0.0437 (induced drag factor)
    pub cm0: f32,     // = 0.04
    pub cm_alpha: f32,// = -0.613 rad⁻¹ (static stability!)
    pub cm_de: f32,   // = -1.122 rad⁻¹
    pub cm_q: f32,    // = -12.4

    // Lateral-Directional
    pub cy_beta: f32, // = -0.31  rad⁻¹
    pub cl_beta: f32, // = -0.089 rad⁻¹
    pub cn_beta: f32, // = 0.065  rad⁻¹
    pub cl_p: f32,    // = -0.47
    pub cn_r: f32,    // = -0.099 (yaw damping)
}

impl FixedWingAero {
    pub fn compute_forces(&self, state: &AeroState) -> AeroForces {
        let q_dyn = 0.5 * RHO * state.airspeed * state.airspeed;
        let cl = self.cl0 + self.cl_alpha * state.alpha
               + self.cl_de * state.delta_elev
               + self.cl_q * (state.q * self.c_bar / (2.0 * state.airspeed));
        let cd = self.cd0 + self.k * cl * cl;
        let cm = self.cm0 + self.cm_alpha * state.alpha
               + self.cm_de * state.delta_elev
               + self.cm_q * (state.q * self.c_bar / (2.0 * state.airspeed));
        // Wind axes → body axes transformation
        let lift = q_dyn * self.s * cl;
        let drag = q_dyn * self.s * cd;
        AeroForces {
            fx: -drag * state.alpha.cos() + lift * state.alpha.sin(),
            fz: -drag * state.alpha.sin() - lift * state.alpha.cos(),
            my: q_dyn * self.s * self.c_bar * cm,
            // ... lateral
        }
    }
}
```

### Stall Model

```rust
/// Smooth stall onset using sigmoid modification to C_Lα
pub fn stall_modifier(alpha: f32, alpha_stall: f32, sigma: f32) -> f32 {
    let s1 = 1.0 / (1.0 + libm::expf(-sigma * (alpha - alpha_stall)));
    let s2 = 1.0 / (1.0 + libm::expf( sigma * (alpha + alpha_stall)));
    (1.0 - s1 - s2) + s1 * libm::sinf(2.0 * alpha) + s2 * libm::sinf(-2.0 * alpha)
}
```

---

## Multirotor Dynamics

### Quadrotor Forces and Moments

```
F_z = -k_T · (ω₁² + ω₂² + ω₃² + ω₄²)  (thrust, body -z)

L = k_T · d · (-ω₁² + ω₂² + ω₃² - ω₄²)  (roll moment)
M = k_T · d · (-ω₁² - ω₂² + ω₃² + ω₄²)  (pitch moment)
N = k_Q · (-ω₁² + ω₂² - ω₃² + ω₄²)      (yaw moment, reaction torques)

where: k_T = thrust coefficient (N/(rad/s)²)
       k_Q = drag/torque coefficient (Nm/(rad/s)²)
       d   = arm length (m)
       ω_i = rotor speed (rad/s), motors 1-4 in X-frame
```

### General Mixer Matrix

```rust
/// Maps [thrust_cmd, roll_cmd, pitch_cmd, yaw_cmd] → motor ω²
pub struct MotorMixer {
    /// Each column: [thrust_coeff, roll, pitch, yaw] contributions per motor
    matrix: [[f32; 4]; 8],  // up to 8 motors
    n_motors: usize,
    kt: f32,
    kq: f32,
    arm: f32,
}

impl MotorMixer {
    pub fn quadrotor_x() -> Self {
        // FL=1 (CCW), FR=2 (CW), RL=3 (CW), RR=4 (CCW)
        let d = 1.0; // normalized
        Self {
            matrix: [
                // T       R       P       Y
                [1.0,  -d,  d,  1.0, 0.0, 0.0, 0.0, 0.0], // M1 FL
                [1.0,   d,  d, -1.0, 0.0, 0.0, 0.0, 0.0], // M2 FR
                [1.0,   d, -d,  1.0, 0.0, 0.0, 0.0, 0.0], // M3 RL
                [1.0,  -d, -d, -1.0, 0.0, 0.0, 0.0, 0.0], // M4 RR
                [0.0; 8],  // unused
                // ...
            ],
            n_motors: 4, kt: 1.0, kq: 0.05, arm: 0.225,
        }
    }

    pub fn mix(&self, thrust: f32, roll: f32, pitch: f32, yaw: f32)
        -> heapless::Vec<f32, 8>
    {
        let cmd = [thrust, roll, pitch, yaw];
        let mut outputs = heapless::Vec::new();
        for i in 0..self.n_motors {
            let v: f32 = cmd.iter().zip(self.matrix[i].iter())
                .map(|(c, m)| c * m).sum();
            outputs.push(v.clamp(0.0, 1.0)).ok();
        }
        // Desaturation: preserve attitude over thrust
        normalize_motor_outputs(&mut outputs);
        outputs
    }
}
```

### Rotor Inflow Model (Momentum Theory)

```rust
/// Induced velocity for hover: v_i = sqrt(T / (2·ρ·A))
/// Climb: v_i·V_c/(2·v_i0²) + v_i/v_i0 = sqrt(1) (numerical solve)
pub fn induced_velocity(thrust: f32, area: f32, climb_rate: f32) -> f32 {
    const RHO: f32 = 1.225; // kg/m³ at sea level
    let v_i0 = libm::sqrtf(thrust / (2.0 * RHO * area));
    if libm::fabsf(climb_rate) < 0.01 { return v_i0; }
    // Newton-Raphson for climb correction
    let mut vi = v_i0;
    for _ in 0..5 {
        let f = vi * (climb_rate + vi) - v_i0 * v_i0;
        let df = climb_rate + 2.0 * vi;
        vi -= f / df;
    }
    vi
}
```

---

## eVTOL Transition Dynamics

### State Machine Transitions

```
VTOL_HOVER  ──(airspeed > V_trans_start)──▶  TRANSITIONING
TRANSITIONING ──(airspeed > V_cruise)────▶  FW_CRUISE
FW_CRUISE   ──(airspeed < V_trans_start)──▶  TRANSITIONING
TRANSITIONING ──(airspeed < V_hover)─────▶  VTOL_HOVER
```

### Blended Control During Transition

```rust
pub struct TransitionController {
    pub vtol_ctrl: MultiRotorController,
    pub fw_ctrl:   FixedWingController,
    pub blend_pct: f32,   // 0.0 = full VTOL, 1.0 = full FW
}

impl TransitionController {
    pub fn update(&mut self, state: &AircraftState, cmd: &PilotCmd,
                  dt: f32) -> ControlOutput
    {
        let alpha = self.compute_blend_factor(state.airspeed);
        let vtol_out = self.vtol_ctrl.update(state, cmd, dt);
        let fw_out   = self.fw_ctrl.update(state, cmd, dt);
        // Smooth blend
        vtol_out * (1.0 - alpha) + fw_out * alpha
    }

    fn compute_blend_factor(&self, airspeed: f32) -> f32 {
        const V_START: f32 = 8.0;   // m/s — begin transition
        const V_FULL:  f32 = 18.0;  // m/s — full fixed-wing
        ((airspeed - V_START) / (V_FULL - V_START)).clamp(0.0, 1.0)
    }
}
```

---

## Wind Estimation

### Dryden Turbulence Model (MIL-SPEC-1797A)

```rust
pub struct DrydenTurbulence {
    sigma_uvw: [f32; 3],    // turbulence intensities
    l_uvw:     [f32; 3],    // scale lengths
    state:     [f32; 6],    // filter states
}

impl DrydenTurbulence {
    /// Generate turbulence velocity components at current airspeed
    pub fn sample(&mut self, airspeed: f32, altitude: f32, dt: f32)
        -> Vector3<f32>
    {
        // Low-altitude turbulence: scale lengths from MIL-SPEC
        let lu = altitude / libm::powf(0.177 + 0.000823 * altitude, 1.2);
        let lw = altitude;

        // First-order rational spectrum shaping filters
        // H_u(s) = sigma_u * sqrt(2*V/pi*Lu) / (s + V/Lu)
        // Implemented as IIR
        let noise = [white_noise(), white_noise(), white_noise()];
        let tau_u = lu / airspeed;
        let tau_w = lw / airspeed;

        self.state[0] += dt * (-self.state[0] / tau_u + noise[0] / tau_u);
        self.state[1] += dt * (-self.state[1] / tau_u + noise[1] / tau_u);
        self.state[2] += dt * (-self.state[2] / tau_w + noise[2] / tau_w);

        Vector3::new(
            self.sigma_uvw[0] * self.state[0],
            self.sigma_uvw[1] * self.state[1],
            self.sigma_uvw[2] * self.state[2],
        )
    }
}
```

---

## Extended Kalman Filter — State Estimator

### State: [pos(3), vel(3), attitude_quat(4), gyro_bias(3), accel_bias(3)]

```rust
pub struct AhrsEkf {
    pub x: [f32; 16],      // state vector
    pub P: [[f32; 16]; 16], // covariance
    pub Q: [[f32; 16]; 16], // process noise
    pub R_gps: [[f32; 6]; 6],
    pub R_mag: [[f32; 3]; 3],
}

impl AhrsEkf {
    /// Prediction step — 1kHz, uses IMU
    pub fn predict(&mut self, accel: Vector3<f32>, gyro: Vector3<f32>,
                   dt: f32) {
        // Correct for estimated biases
        let accel_corr = accel - self.accel_bias();
        let gyro_corr  = gyro  - self.gyro_bias();

        // Propagate attitude quaternion
        let q = self.attitude();
        let qdot = q * Quaternion::new(0.0, gyro_corr.x,
                                        gyro_corr.y, gyro_corr.z) * 0.5;
        // Update position and velocity
        let accel_ned = q * accel_corr + Vector3::new(0.0, 0.0, GRAVITY);
        // ... Jacobian F, covariance P = F*P*F' + Q
    }

    /// GPS measurement update
    pub fn update_gps(&mut self, gps_pos: Vector3<f32>,
                      gps_vel: Vector3<f32>) {
        // H matrix: observation maps state → [pos, vel]
        // K = P*H' * (H*P*H' + R)⁻¹
        // x = x + K*(z - H*x)
        // P = (I - K*H)*P
    }
}
```