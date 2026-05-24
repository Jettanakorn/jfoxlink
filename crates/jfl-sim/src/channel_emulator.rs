use rand::Rng;
use std::collections::VecDeque;
use jfl_core::channel::manager::{ChannelId, ChannelState, ChannelHealth};

/// Dual-channel RF impairment model for deterministic & stochastic simulation.
/// INVARIANT: All impairments are reversible for testing; no side-effects outside emulator state.
/// SIMULATION: Uses seeded RNG for reproducible test vectors.
#[derive(Debug)]
pub enum EmulatorError {
    FrameDrop,
    CorruptionLimitReached,
    InvalidRssiRange,
}

pub struct ChannelEmulator {
    pub id: ChannelId,
    pub base_rssi_dbm: i8,
    pub current_rssi_dbm: i8,
    pub ber: f32,
    pub latency_ms: f32,
    pub jitter_ms: f32,
    pub drop_probability: f32,
    pub fading_coeff: f32,
    pub state: ChannelState,
    pub packet_history: VecDeque<Vec<u8>>,
    rng: rand::rngs::StdRng,
}

impl ChannelEmulator {
    /// Create a seeded channel emulator for reproducible testing.
    pub fn new(id: ChannelId, base_rssi: i8, seed: u64) -> Self {
        Self {
            id,
            base_rssi_dbm: base_rssi,
            current_rssi_dbm: base_rssi,
            ber: 0.0,
            latency_ms: 10.0,
            jitter_ms: 2.0,
            drop_probability: 0.0,
            fading_coeff: 1.0,
            state: ChannelState::Active,
            packet_history: VecDeque::with_capacity(128),
            rng: rand::rngs::StdRng::seed_from_u64(seed),
        }
    }

    /// Apply RF impairments to a raw frame. Returns `Ok(None)` on simulated drop.
    pub fn apply_impairments(&mut self, frame: &[u8]) -> Result<Option<Vec<u8>>, EmulatorError> {
        // 1. Packet drop simulation
        if self.rng.gen::<f32>() < self.drop_probability {
            return Ok(None);
        }

        let mut out = frame.to_vec();
        
        // 2. BER simulation: probabilistic bit flips
        if self.ber > 0.0 {
            for byte in out.iter_mut() {
                if self.rng.gen::<f32>() < self.ber {
                    *byte ^= 1 << self.rng.gen_range(0..8);
                }
            }
        }

        // 3. Fading & RSSI variation (Rayleigh placeholder)
        self.current_rssi_dbm = (self.base_rssi_dbm as f32 * self.fading_coeff +
                                 self.rng.gen_range(-5.0..5.0)) as i8;

        // 4. Latency + Jitter accumulation
        let jitter = self.rng.gen_range(-self.jitter_ms..self.jitter_ms);
        self.latency_ms = (self.latency_ms + jitter).max(1.0);

        // Cache for replay/threat injection
        self.packet_history.push_back(frame.to_vec());
        if self.packet_history.len() > 128 { self.packet_history.pop_front(); }

        Ok(Some(out))
    }

    /// Update environmental conditions using log-distance path loss + weather loss.
    /// INVARIANT: RSSI clamped to [-95, -10] dBm (realistic RF receiver range).
    pub fn update_environment(&mut self, distance_m: f32, weather_loss_db: f32) {
        let n = if self.id == ChannelId::A { 2.2 } else { 2.8 }; // FHSS vs DSSS path loss exponent
        let pl = 10.0 * n * distance_m.max(1.0).log10() + weather_loss_db;
        self.base_rssi_dbm = (-50.0 - pl as i8).clamp(-95, -10);
        
        // Random fade events
        self.fading_coeff = if self.rng.gen_bool(0.05) { 0.6 } else { 1.0 };
    }

    /// Return current health metrics for the dual-channel voter.
    pub fn health(&self) -> ChannelHealth {
        ChannelHealth {
            rssi_dbm: self.current_rssi_dbm,
            ber: self.ber,
            jam_prob: 0, // Updated by ThreatInjector
            latency_us: (self.latency_ms * 1000.0) as u16,
        }
    }

    /// Simulate channel state transitions based on health thresholds.
    pub fn update_state(&mut self) {
        self.state = if self.current_rssi_dbm < -90 || self.ber > 0.1 {
            ChannelState::Failed
        } else if self.current_rssi_dbm < -80 {
            ChannelState::Recovering
        } else {
            ChannelState::Active
        };
    }
}