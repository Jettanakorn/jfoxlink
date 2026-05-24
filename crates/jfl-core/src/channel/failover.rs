#![no_std]
use super::manager::{ChannelId, ChannelState};

/// Hysteresis-based failover state machine.
/// INVARIANT: Prevents channel flapping. Minimum 2 hop periods before switching.
pub struct FailoverFSM {
    pub active: ChannelId,
    pub timer: u16,
    pub min_hold_ms: u16
}

impl FailoverFSM {
    pub fn new(hold_ms: u16) -> Self { Self { active: ChannelId::A, timer: 0, min_hold_ms: hold_ms } }
    pub fn transition(&mut self, target: ChannelId, elapsed_ms: u16) -> ChannelId {
        self.timer += elapsed_ms;
        if target != self.active && self.timer >= self.min_hold_ms {
            self.active = target;
            self.timer = 0;
        }
        self.active
    }
}
