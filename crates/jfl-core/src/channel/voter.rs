use super::manager::ChannelHealth;
use crate::frame::JflFrame;

/// Selects the highest-integrity frame from dual channels.
/// INVARIANT: Deterministic selection based on weighted health metrics.
pub struct FrameVoter;
impl FrameVoter {
    pub fn vote<'a>(
        frame_a: Option<&'a JflFrame<'a>>,
        health_a: ChannelHealth,
        frame_b: Option<&'a JflFrame<'a>>,
        health_b: ChannelHealth,
    ) -> Option<&'a JflFrame<'a>> {
        match (frame_a, frame_b) {
            (Some(a), None) => Some(a),
            (None, Some(b)) => Some(b),
            (Some(a), Some(_)) => {
                // Non-finite BER (NaN/inf) casts to 0 and would flatter a faulted
                // channel; treat it as worst-case so a healthy channel wins.
                let ber = |x: f32| if x.is_finite() { x } else { 1.0 };
                let score_a = (health_a.rssi_dbm as i32) - ((ber(health_a.ber) * 1000.0) as i32);
                let score_b = (health_b.rssi_dbm as i32) - ((ber(health_b.ber) * 1000.0) as i32);
                if score_a > score_b {
                    Some(a)
                } else {
                    frame_b
                }
            }
            _ => None,
        }
    }
}
