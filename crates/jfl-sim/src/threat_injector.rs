use rand::{Rng, SeedableRng};
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
        frame.extend_from_slice(&self.rng.gen::<[u8; 12]>()); // Random nonce
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

#[cfg(test)]
mod tests {
    use super::*;
    use jfl_core::anti_jam::detector::JamDetector;
    use jfl_core::crypto::nonce::NonceManager;
    use jfl_core::frame::{JflError, JflFrame, JFL_CRYPTO_FLAG, JFL_GCM_TAG_LEN, JFL_HEADER_LEN, JFL_HMAC_LEN, JFL_STX};

    fn build_valid_crypto_frame(payload: &[u8]) -> Vec<u8> {
        let mut frame = Vec::with_capacity(JFL_HEADER_LEN + payload.len() + JFL_GCM_TAG_LEN + JFL_HMAC_LEN);
        frame.push(JFL_STX);
        frame.push(payload.len() as u8);
        frame.push(JFL_CRYPTO_FLAG);
        frame.push(0x00);
        frame.push(0x10);
        frame.push(0x20);
        frame.push(0x30);
        frame.extend_from_slice(&[0x01, 0x00, 0x00]);
        frame.push(0x01);
        frame.extend_from_slice(&[0xAA; 12]);
        frame.push(0x03);
        frame.extend_from_slice(payload);
        frame.extend_from_slice(&[0xBB; JFL_GCM_TAG_LEN]);
        frame.extend_from_slice(&[0xCC; JFL_HMAC_LEN]);
        frame
    }

    #[test]
    fn full_attack_protection_test() {
        let mut injector = ThreatInjector::new(0xC0FFEE);
        let payload = b"HELLO";
        let raw_frame = build_valid_crypto_frame(payload);

        let parsed_frame = JflFrame::from_bytes(&raw_frame).expect("valid frame should parse");
        assert_eq!(parsed_frame.len, payload.len() as u8);
        assert_eq!(parsed_frame.encrypted_payload, payload);

        injector.capture_frame(&raw_frame);
        let replay_frame = injector.generate_replay().expect("replay frame generated");
        assert_eq!(replay_frame.len(), raw_frame.len());
        assert_ne!(replay_frame, raw_frame, "replay must modify the captured frame");

        let original_nonce = u64::from_le_bytes(raw_frame[JFL_HEADER_LEN..JFL_HEADER_LEN + 8].try_into().unwrap());
        let replay_nonce = u64::from_le_bytes(replay_frame[JFL_HEADER_LEN..JFL_HEADER_LEN + 8].try_into().unwrap());
        assert_eq!(replay_nonce, original_nonce + 1);

        let mut detector = JamDetector { threshold_dbm: -10, spectral_energy: [i16::MIN; 64] };
        assert!(!detector.evaluate(), "no jam should be detected initially");

        injector.inject_narrowband_jam(12.5, 5.0);
        injector.apply_to_detector(&mut detector);
        assert!(detector.evaluate(), "narrowband jam should exceed threshold");

        injector.clear_jams();
        assert!(injector.active_jams.is_empty(), "jam profiles should be cleared");

        detector.spectral_energy = [i16::MIN; 64];
        injector.inject_wideband_jam(25.0, 5.0);
        injector.apply_to_detector(&mut detector);
        assert!(detector.evaluate(), "wideband jam should exceed threshold");

        let spoof_frame = injector.generate_spoof_frame(b"ATTACK", 0xEF, 0x99);
        assert_eq!(spoof_frame[0], JFL_STX);
        assert_eq!(spoof_frame[1], 6);
        assert_eq!(spoof_frame.len(), JFL_HEADER_LEN + 6 + JFL_GCM_TAG_LEN + JFL_HMAC_LEN);

        let spoof_result = JflFrame::from_bytes(&spoof_frame);
        assert!(matches!(spoof_result, Err(JflError::UnsupportedVersion)), "spoofed frame must be rejected by parser");
    }

    #[test]
    fn replay_window_nonce_manager_test() {
        let nonce_manager = NonceManager::new(2);

        let first = nonce_manager.next_nonce();
        let second = nonce_manager.next_nonce();
        let third = nonce_manager.next_nonce();

        let first_val = u64::from_le_bytes(first);
        let second_val = u64::from_le_bytes(second);
        let third_val = u64::from_le_bytes(third);

        assert_eq!(second_val, first_val + 1);
        assert_eq!(third_val, second_val + 1);

        // Current counter is 3, window is 2: value 1 is still inside the replay window.
        assert!(nonce_manager.verify_nonce(second_val).is_ok());
        assert!(nonce_manager.verify_nonce(third_val).is_ok());

        // Value 0 falls outside the sliding window and should be rejected.
        assert_eq!(nonce_manager.verify_nonce(first_val).unwrap_err(), JflError::ReplayDetected);

        // Future values must also be rejected.
        assert_eq!(nonce_manager.verify_nonce(third_val + 10).unwrap_err(), JflError::ReplayDetected);
    }
}
