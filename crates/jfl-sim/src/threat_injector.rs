use rand::Rng;
use jfl_core::anti_jam::detector::JamDetector;
use jfl_core::frame::{JFL_STX, JFL_HEADER_LEN, JFL_GCM_TAG_LEN, JFL_HMAC_LEN};

/// Cyber/RF threat injection engine for JFOXLink resilience testing.
/// SIMULATION: Generates attack vectors aligned with SKILL.md threat model.
/// INVARIANT: Does not mutate real hardware; all impacts are virtual & measurable.
#[derive(Debug, Clone, Copy)]
pub enum JamProfile {
    Narrowband { center_bin: usize, power_dbm: f32 },
    Wideband { start_bin: usize, count: usize, power_dbm: f32 },
}

pub struct ThreatInjector {
    pub captured_frames: Vec<Vec<u8>>,
    pub active_jams: Vec<JamProfile>,
    rng: rand::rngs::StdRng,
}

impl ThreatInjector {
    pub fn new(seed: u64) -> Self {
        Self {
            captured_frames: Vec::new(),
            active_jams: Vec::new(),
            rng: rand::rngs::StdRng::seed_from_u64(seed),
        }
    }

    /// Capture a frame for later replay attack injection.
    pub fn capture_frame(&mut self, frame: &[u8]) {
        self.captured_frames.push(frame.to_vec());
    }

    /// Generate replay frame (modifies nonce to bypass naive replay protection).
    pub fn generate_replay(&mut self) -> Option<Vec<u8>> {
        if self.captured_frames.is_empty() { return None; }
        let idx = self.rng.gen_range(0..self.captured_frames.len());
        let mut replay = self.captured_frames[idx].clone();
        
        // ADVANCED: Increment nonce by 1 to test sliding window boundaries
        if replay.len() > JFL_HEADER_LEN + 7 {
            let nonce_start = JFL_HEADER_LEN;
            let nonce_bytes = &mut replay[nonce_start..nonce_start + 8];
            let mut nonce_val = u64::from_le_bytes(nonce_bytes.try_into().unwrap());
            nonce_val += 1;
            nonce_bytes.copy_from_slice(&nonce_val.to_le_bytes());
        }
        Some(replay)
    }

    /// Inject narrowband jamming into FFT energy model.
    pub fn inject_narrowband_jam(&mut self, center_mhz: f32, power_dbm: f32) {
        // Map MHz to 64-bin FFT array (1.5625 MHz/bin)
        let bin = ((center_mhz % 100.0) / 1.5625).floor() as usize % 64;
        self.active_jams.push(JamProfile::Narrowband { center_bin: bin, power_dbm });
    }

    /// Inject wideband jamming into FFT energy model.
    pub fn inject_wideband_jam(&mut self, bandwidth_mhz: f32, power_dbm: f32) {
        let bins = ((bandwidth_mhz / 1.5625).floor() as usize).min(64);
        self.active_jams.push(JamProfile::Wideband { start_bin: 0, count: bins, power_dbm });
    }

    /// Apply jam profiles to a JamDetector instance for threshold testing.
    pub fn apply_to_detector(&self, detector: &mut JamDetector) {
        for jam in &self.active_jams {
            match jam {
                JamProfile::Narrowband { center_bin, power_dbm } => {
                    detector.spectral_energy[*center_bin] = (power_dbm * 10.0) as i16;
                }
                JamProfile::Wideband { start_bin, count, power_dbm } => {
                    for i in *start_bin..(*start_bin + count).min(64) {
                        detector.spectral_energy[i] = (power_dbm * 8.0) as i16;
                    }
                }
            }
        }
    }

    /// Generate a spoofed JFOXLink frame (structurally valid, cryptographically invalid).
    /// INVARIANT: HMAC & GCM tags are zeroed to test auth rejection paths.
    pub fn generate_spoof_frame(&mut self, payload: &[u8], sysid: u8, seq: u8) -> Vec<u8> {
        let mut frame = Vec::with_capacity(JFL_HEADER_LEN + payload.len() + JFL_GCM_TAG_LEN + JFL_HMAC_LEN);
        frame.push(JFL_STX);
        frame.push(payload.len() as u8);
        frame.push(0x01); // INCOMPAT: MAVLINK_IFLAG_SIGNED only (no crypto)
        frame.push(0x00); // COMPAT
        frame.push(seq);
        frame.push(sysid);
        frame.push(0x01); // COMPID
        frame.extend_from_slice(&[0x00; 3]); // MSGID
        frame.push(0x01); // JFL_VERSION
        frame.extend_from_slice(&self.rng.gen::<[u8; 8]>()); // Random nonce
        frame.push(0x03); // CHANNEL_FLAGS
        frame.extend_from_slice(payload);
        frame.extend_from_slice(&[0x00; JFL_GCM_TAG_LEN]); // Fake GCM tag
        frame.extend_from_slice(&[0x00; JFL_HMAC_LEN]);    // Fake HMAC
        frame
    }

    /// Clear all active jam profiles.
    pub fn clear_jams(&mut self) {
        self.active_jams.clear();
    }
}