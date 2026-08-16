use super::manager::ChannelId;

/// Hysteresis-based failover state machine.
/// INVARIANT: Prevents channel flapping. Minimum 2 hop periods before switching.
pub struct FailoverFSM {
    pub active: ChannelId,
    pub timer: u16,
    pub min_hold_ms: u16,
}

impl FailoverFSM {
    pub fn new(hold_ms: u16) -> Self {
        Self {
            active: ChannelId::A,
            timer: 0,
            min_hold_ms: hold_ms,
        }
    }
    pub fn transition(&mut self, target: ChannelId, elapsed_ms: u16) -> ChannelId {
        // Saturating: an unchecked `+=` overflows u16 once the accumulated
        // hold time exceeds ~65s, which (in release) wraps to a small value and
        // resets the anti-flap guard — exactly the flapping this FSM prevents.
        self.timer = self.timer.saturating_add(elapsed_ms);
        if target != self.active && self.timer >= self.min_hold_ms {
            self.active = target;
            self.timer = 0;
        }
        self.active
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn holds_active_channel_until_min_hold_elapses() {
        let mut fsm = FailoverFSM::new(100);
        assert_eq!(fsm.transition(ChannelId::B, 50), ChannelId::A); // too soon
        assert_eq!(fsm.transition(ChannelId::B, 60), ChannelId::B); // 110ms >= 100
    }

    #[test]
    fn timer_does_not_overflow_on_sustained_hold() {
        let mut fsm = FailoverFSM::new(u16::MAX);
        // Accumulate far past u16::MAX with no switch; must saturate, not panic/wrap.
        for _ in 0..10_000 {
            assert_eq!(fsm.transition(ChannelId::B, u16::MAX), ChannelId::B);
        }
    }
}
