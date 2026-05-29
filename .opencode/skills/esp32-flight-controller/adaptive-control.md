# Adaptive Flight Control Reference

## Algorithm Selection

| Scenario | Algorithm | Stability Proof | Rust Complexity |
|---|---|---|---|
| Parameter uncertainty (Cα changes) | MRAC (Lyapunov) | Yes (global) | Medium |
| Actuator damage, unknown input | L1 Adaptive | Yes (with bounds) | Medium |
| Structural uncertainty, robustness | µ-synthesis + scheduling | Yes (H∞ norm) | High |
| Complete model-free, agile | INDI (Incremental NDI) | Input-output | Low |
| Neural-net disturbance estimation | L1 + NN | Conditional | High |
| Time-varying uncertain systems | Online SysID + update | Indirect | High |

---

## MRAC — Model Reference Adaptive Control

### Architecture

```
Reference Model: ẋ_m = A_m·x_m + B_m·r  (desired dynamics)
Plant:           ẋ   = A·x + B·u         (unknown A, B)
Control Law:     u   = K_x(t)·x + K_r(t)·r
Adaptation:      K̇_x = -Γ_x · e · x'    (MIT/Lyapunov rule)
                 K̇_r = -Γ_r · e · r'
```

### Lyapunov-Stable MRAC (Attitude Rate Controller)

```rust
// fc-core/src/control/mrac.rs

pub struct MracRateController {
    /// Reference model: desired roll/pitch/yaw rate response
    am: Matrix3<f32>,   // A_m = -ωn diag — e.g., -[20, 20, 10] rad/s
    bm: Matrix3<f32>,   // B_m = ωn diag

    /// Adaptive gains (updated online)
    kx: Matrix3<f32>,   // state feedback gain
    kr: Matrix3<f32>,   // command gain

    /// Adaptation rates (tuning parameters)
    gamma_x: f32,       // e.g., 50.0
    gamma_r: f32,       // e.g., 20.0

    /// Reference model state
    xm: Vector3<f32>,

    /// Anti-windup: projection to keep gains bounded
    kx_bounds: (f32, f32),  // (-5.0, 5.0)
    kr_bounds: (f32, f32),  // (0.1, 3.0)
}

impl MracRateController {
    pub fn update(&mut self, rate_cmd: Vector3<f32>,
                  rate_meas: Vector3<f32>, dt: f32) -> Vector3<f32>
    {
        // Reference model output (desired response)
        let xm_dot = self.am * self.xm + self.bm * rate_cmd;
        self.xm += xm_dot * dt;

        // Tracking error
        let e = rate_meas - self.xm;

        // Lyapunov-based adaptation law (gradient descent on V = e'Pe)
        // Assuming P = I (simplified), B positive definite
        let kx_dot = -self.gamma_x * e * rate_meas.transpose();
        let kr_dot = -self.gamma_r * e * rate_cmd.transpose();

        // Update gains with projection (prevent instability)
        self.kx += kx_dot * dt;
        self.kr += kr_dot * dt;
        self.project_gains();

        // Control output
        self.kx * rate_meas + self.kr * rate_cmd
    }

    /// Projection operator: keeps gains in stable region
    fn project_gains(&mut self) {
        for i in 0..3 {
            self.kx[(i,i)] = self.kx[(i,i)]
                .clamp(self.kx_bounds.0, self.kx_bounds.1);
            self.kr[(i,i)] = self.kr[(i,i)]
                .clamp(self.kr_bounds.0, self.kr_bounds.1);
        }
    }
}
```

---

## L1 Adaptive Control

### Theory Summary

L1 distinguishes adaptation from robustness by inserting a **low-pass filter** C(s)
in the adaptive loop. This ensures:
- **Fast adaptation** (high adaptive gain Γ) without sacrificing robustness
- **Bandwidth separation**: adaptation handles slow parameter changes; L1 filter handles noise

```
u(t) = C(s) · [K_g·r(t) - σ̂(t)] + u_bl(t)

where: σ̂ = adaptive estimate of matched uncertainty
       C(s) = low-pass filter (design parameter)
       K_g = DC gain compensator
       u_bl = baseline (nom.) controller
```

```rust
pub struct L1AdaptiveController {
    pub bandwidth: f32,         // Low-pass filter BW (rad/s), e.g., 50 Hz → 314 rad/s
    pub gamma: f32,             // Adaptive gain, e.g., 1000.0
    pub x_pred: Vector3<f32>,   // Predicted state from reference model
    pub sigma_hat: Vector3<f32>,// Matched uncertainty estimate
    pub lp_state: Vector3<f32>, // Low-pass filter state
    pub nominal: Box<dyn NominalController>,
}

impl L1AdaptiveController {
    pub fn update(&mut self, x: Vector3<f32>, cmd: Vector3<f32>,
                  dt: f32) -> Vector3<f32>
    {
        // 1. State predictor (reference model with adaptive correction)
        let pred_error = self.x_pred - x;
        let sigma_dot = -self.gamma * pred_error;
        self.sigma_hat += sigma_dot * dt;

        // 2. Update predictor
        let u_nom = self.nominal.compute(x, cmd, dt);
        let x_pred_dot = self.reference_model(self.x_pred)
                       + u_nom + self.sigma_hat;
        self.x_pred += x_pred_dot * dt;

        // 3. L1 output: filter adaptive signal
        // C(s) = ω_c / (s + ω_c)  → IIR first-order
        let wc = self.bandwidth;
        let lp_dot = -wc * self.lp_state + wc * self.sigma_hat;
        self.lp_state += lp_dot * dt;

        // 4. Total control: nominal + filtered adaptive compensation
        u_nom - self.lp_state
    }

    fn reference_model(&self, x: Vector3<f32>) -> Vector3<f32> {
        // A_m * x — simple decay toward zero rate
        x * (-20.0) // ωn = 20 rad/s bandwidth
    }
}
```

---

## INDI — Incremental Nonlinear Dynamic Inversion

### Concept
INDI computes control increments needed to achieve desired angular accelerations,
without requiring a full aerodynamic model. Relies on **sensor differentiation**.

```
Δu = G⁻¹ · (ν - ω̇_meas)

where: ν   = desired angular acceleration (from outer loop)
       ω̇   = measured/estimated angular acceleration (IMU differentiation)
       G   = control effectiveness matrix (partial ∂f/∂u)
```

```rust
pub struct IndiRateController {
    pub effectiveness: Matrix3<f32>, // G: maps actuator Δ → angular accel
    pub inv_effectiveness: Matrix3<f32>, // G⁻¹ (precomputed or online)
    pub accel_filter: [LowPassFilter; 3], // smooth ω̇ estimate
    pub prev_omega: Vector3<f32>,
    pub sync_filter: [LowPassFilter; 3], // synchronize u with ω̇ measurement
}

impl IndiRateController {
    pub fn update(&mut self, rate_cmd: Vector3<f32>,
                  rate_meas: Vector3<f32>, u_prev: Vector3<f32>,
                  dt: f32) -> Vector3<f32>
    {
        // Estimate angular acceleration from IMU
        let omega_dot_raw = (rate_meas - self.prev_omega) / dt;
        let omega_dot = Vector3::new(
            self.accel_filter[0].update(omega_dot_raw.x, dt),
            self.accel_filter[1].update(omega_dot_raw.y, dt),
            self.accel_filter[2].update(omega_dot_raw.z, dt),
        );
        self.prev_omega = rate_meas;

        // Virtual input: desired angular acceleration
        const KP: f32 = 30.0;
        let nu = (rate_cmd - rate_meas) * KP;

        // INDI increment
        let u_filt = Vector3::new(
            self.sync_filter[0].update(u_prev.x, dt),
            self.sync_filter[1].update(u_prev.y, dt),
            self.sync_filter[2].update(u_prev.z, dt),
        );
        let delta_u = self.inv_effectiveness * (nu - omega_dot);
        (u_filt + delta_u).clamp_element(-1.0, 1.0)
    }
}
```

---

## Online System Identification

### Recursive Least Squares (RLS) — Parameter Estimation

```rust
/// Estimates [Cm_alpha, Cm_q, Cm_delta_e] online
pub struct RlsParamEstimator {
    theta: Vector3<f32>,        // parameter estimates
    P: Matrix3<f32>,            // covariance matrix
    lambda: f32,                // forgetting factor (0.97–0.999)
}

impl RlsParamEstimator {
    pub fn update(&mut self, phi: Vector3<f32>, z: f32) {
        // Kalman gain
        let pf = self.P * phi;
        let s = phi.dot(&pf) + self.lambda;
        let k = pf / s;

        // Innovation
        let z_hat = self.theta.dot(&phi);
        let innov = z - z_hat;

        // Update
        self.theta += k * innov;
        self.P = (self.P - k * pf.transpose()) / self.lambda;
    }
}
```

---

## Gain Scheduling

### Mach/Altitude Scheduled Gains (Fixed-Wing)

```rust
pub struct GainScheduler {
    /// 2D lookup table: [mach_idx][alt_idx] → gains
    table: [[PidGains; 8]; 8],
    mach_breakpoints: [f32; 8],  // [0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8]
    alt_breakpoints: [f32; 8],   // [0, 500, 1000, 2000, 4000, 6000, 8000, 10000] m
}

impl GainScheduler {
    pub fn interpolate(&self, mach: f32, alt: f32) -> PidGains {
        let (mi, mf) = find_interval(&self.mach_breakpoints, mach);
        let (ai, af) = find_interval(&self.alt_breakpoints, alt);

        // Bilinear interpolation
        let g00 = self.table[mi][ai];
        let g10 = self.table[mi+1][ai];
        let g01 = self.table[mi][ai+1];
        let g11 = self.table[mi+1][ai+1];

        let gm0 = lerp(g00, g10, mf);
        let gm1 = lerp(g01, g11, mf);
        lerp(gm0, gm1, af)
    }
}
```

---

## Neural Network Augmentation (Research)

### L1 + NN Disturbance Estimator

```rust
/// Shallow NN estimates aerodynamic uncertainty online
/// Input: [alpha, beta, p, q, r, V, mach] → output: [ΔL, ΔM, ΔN]
pub struct NnDisturbanceEstimator {
    w1: [[f32; 7]; 16],   // Hidden layer weights (16 neurons, 7 inputs)
    w2: [[f32; 16]; 3],   // Output layer weights
    /// Online weight update via e-modification
    lambda_mod: f32,
    learning_rate: f32,
}

impl NnDisturbanceEstimator {
    pub fn forward(&self, x: &[f32; 7]) -> Vector3<f32> {
        // Hidden layer: tanh activation
        let mut h = [0.0f32; 16];
        for (i, row) in self.w1.iter().enumerate() {
            h[i] = libm::tanhf(row.iter().zip(x).map(|(w,xi)| w*xi).sum::<f32>());
        }
        // Output layer: linear
        let mut y = [0.0f32; 3];
        for (j, row) in self.w2.iter().enumerate() {
            y[j] = row.iter().zip(h.iter()).map(|(w,hi)| w*hi).sum::<f32>();
        }
        Vector3::new(y[0], y[1], y[2])
    }

    pub fn update_weights(&mut self, error: Vector3<f32>, x: &[f32; 7]) {
        // e-modification: W_dot = Γ·(e·∂σ/∂W - λ·W)
        // Prevents weight drift during off-manifold adaptation
        // ... backpropagation with e-modification
    }
}
```