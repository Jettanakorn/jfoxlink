/// 11-chip Barker code spreading/despreading.
/// INVARIANT: Provides ~11dB processing gain against narrowband interference.
const BARKER_11: [i8; 11] = [1, 1, 1, -1, -1, -1, 1, -1, -1, 1, -1];
pub struct DssSEngine;
impl DssSEngine {
    pub fn spread_bit(bit: bool) -> [i8; 11] {
        let mut out = [0i8; 11];
        let polarity = if bit { 1 } else { -1 };
        for i in 0..11 {
            out[i] = BARKER_11[i] * polarity;
        }
        out
    }
}
