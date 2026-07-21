#![no_std]
use crate::frame::JflError;

#[derive(Debug, PartialEq, Copy, Clone)] pub enum ChannelId { A, B }
#[derive(Debug, PartialEq, Copy, Clone)] pub enum ChannelState { Active, Shadow, Failed, Recovering }

pub struct ChannelHealth { pub rssi_dbm: i8, pub ber: f32, pub jam_prob: u8, pub latency_us: u16 }
pub struct DualChannelManager {
    pub state_a: ChannelState, pub state_b: ChannelState,
    pub active: ChannelId, pub hysteresis: u8
}

impl DualChannelManager {
    pub fn new() -> Self { Self { state_a: ChannelState::Shadow, state_b: ChannelState::Active, active: ChannelId::A, hysteresis: 0 } }
    pub fn arbitrate(&mut self, a: ChannelHealth, b: ChannelHealth) -> ChannelId {
        let score = |h: &ChannelHealth| -> i32 {
            // A non-finite BER (NaN/inf from a faulted radio) casts to 0 and
            // would make a dead channel look healthy — treat it as worst-case.
            let ber = if h.ber.is_finite() { h.ber } else { 1.0 };
            (h.rssi_dbm as i32) - ((ber * 1000.0) as i32) - ((h.jam_prob as i32) * 5) - ((h.latency_us as i32) / 100)
        };
        let (sa, sb) = (score(&a), score(&b));
        if (sa - sb).abs() > 15 { self.hysteresis = 0; if sa > sb { ChannelId::A } else { ChannelId::B } }
        else { self.hysteresis = (self.hysteresis + 1).min(3); self.active }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn health(rssi: i8, ber: f32, jam: u8, lat: u16) -> ChannelHealth {
        ChannelHealth { rssi_dbm: rssi, ber, jam_prob: jam, latency_us: lat }
    }

    #[test]
    fn nan_ber_channel_loses_to_healthy_channel() {
        let mut m = DualChannelManager::new();
        // A is strong/clean; B reports NaN BER (faulted radio).
        let winner = m.arbitrate(health(-40, 0.0, 0, 100), health(-40, f32::NAN, 0, 100));
        assert_eq!(winner, ChannelId::A);
    }

    #[test]
    fn clearly_better_channel_wins() {
        let mut m = DualChannelManager::new();
        let winner = m.arbitrate(health(-30, 0.0, 0, 50), health(-90, 0.5, 200, 5000));
        assert_eq!(winner, ChannelId::A);
    }
}
