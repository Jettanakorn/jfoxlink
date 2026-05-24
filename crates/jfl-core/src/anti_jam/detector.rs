#![no_std]
/// FFT-based spectral energy monitor.
/// INVARIANT: Triggers emergency hop sequence when threshold breached.
pub struct JamDetector { pub threshold_dbm: i8, pub spectral_energy: [i16; 64] }
impl JamDetector {
    pub fn evaluate(&self) -> bool {
        self.spectral_energy.iter().any(|&b| b as i16 > self.threshold_dbm as i16)
    }
}
