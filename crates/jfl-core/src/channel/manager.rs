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
            (h.rssi_dbm as i32) - ((h.ber * 1000.0) as i32) - ((h.jam_prob as i32) * 5) - ((h.latency_us as i32) / 100) 
        };
        let (sa, sb) = (score(&a), score(&b));
        if (sa - sb).abs() > 15 { self.hysteresis = 0; if sa > sb { ChannelId::A } else { ChannelId::B } }
        else { self.hysteresis = (self.hysteresis + 1).min(3); self.active }
    }
}
