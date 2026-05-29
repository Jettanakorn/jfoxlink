# JFOXLink Dual-Redundancy Reference

## Design Rationale

Dual-redundancy is not load-balancing — it is **simultaneous independent operation**
on two physically and spectrally separated channels, with a deterministic voter choosing
the authoritative frame. This eliminates the single point of failure at the RF layer
without requiring triple-channel overhead.

```
GCS ──┬── Channel A (900 MHz FHSS)  ──┐
      │                               ├── VOTER ── Accepted Frame
      └── Channel B (2.4 GHz DSSS)  ──┘

UAV ──┬── Channel A (900 MHz FHSS)  ──┐
      │                               ├── VOTER ── Accepted Frame
      └── Channel B (2.4 GHz DSSS)  ──┘
```

Both channels transmit **identical encrypted frames** simultaneously. The voter selects
the best frame based on health metrics. If one channel is jammed, the other carries on.

---

## Channel Specifications

### Channel A — Primary (900 MHz FHSS)
| Parameter | Value |
|---|---|
| Frequency band | 902–928 MHz (ISM) or 869 MHz (EU) |
| Modulation | GFSK, 2-FSK |
| Channel count | 100 pseudo-random channels |
| Hop period | 50 ms (configurable 10–200 ms) |
| Bandwidth per channel | 250 kHz |
| Data rate | 57.6 kbps (default), 128 kbps (high-rate mode) |
| TX power | 6–30 dBm adaptive |
| Range | ~40 km @ 30 dBm (line-of-sight, no interference) |
| Hardware reference | RFD900x, SiK V3 |

### Channel B — Secondary (2.4 GHz DSSS)
| Parameter | Value |
|---|---|
| Frequency band | 2.400–2.4835 GHz |
| Modulation | DSSS-OQPSK (IEEE 802.15.4-inspired) |
| Spreading factor | 11-chip Barker code |
| Processing gain | 10.4 dB vs narrowband jammer |
| Channel count | 79 (1 MHz spacing) |
| Hop period | 20 ms |
| Data rate | 250 kbps effective after spreading |
| TX power | 0–20 dBm adaptive |
| Range | ~10 km @ 20 dBm (line-of-sight) |
| Hardware reference | Semtech SX1280, Microhard pDDL2450 |

**Spectral independence**: 900 MHz and 2.4 GHz are >1 GHz apart — a jammer targeting
one band will not affect the other without broadband power that is itself detectable.

---

## Channel Health Scoring

Each channel is continuously scored 0–100:

```rust
pub struct ChannelHealth {
    pub rssi_dbm: i16,        // received signal strength
    pub snr_db: f32,          // signal-to-noise ratio
    pub ber: f32,             // bit error rate (rolling 100-frame window)
    pub frame_loss_rate: f32, // lost frames / total expected
    pub jam_score: f32,       // 0.0 (clean) to 1.0 (definitely jammed)
    pub latency_ms: f32,      // round-trip or one-way timestamp delta
    pub age_ms: u32,          // ms since last valid frame
}

impl ChannelHealth {
    pub fn score(&self) -> u8 {
        // Weighted composite — lower is worse
        let rssi_score   = ((self.rssi_dbm.max(-120) + 120) as f32 / 90.0).min(1.0) * 30.0;
        let snr_score    = (self.snr_db / 30.0).clamp(0.0, 1.0) * 25.0;
        let ber_score    = (1.0 - self.ber * 10.0).clamp(0.0, 1.0) * 20.0;
        let jam_score    = (1.0 - self.jam_score) * 15.0;
        let latency_score = (1.0 - (self.latency_ms / 500.0)).clamp(0.0, 1.0) * 10.0;
        
        (rssi_score + snr_score + ber_score + jam_score + latency_score) as u8
    }
    
    pub fn is_failed(&self) -> bool {
        self.age_ms > 2000 || self.score() < 10
    }
}
```

---

## Frame Voter Logic

```rust
pub struct FrameVoter {
    pub active: ChannelId,
    pub ch_a_health: ChannelHealth,
    pub ch_b_health: ChannelHealth,
    /// Hysteresis: switchover requires SCORE_DELTA advantage AND min_hold_ms
    pub score_delta_threshold: u8,  // default: 15 points
    pub min_hold_ms: u32,           // default: 500 ms
    last_switch_time: Instant,
}

impl FrameVoter {
    /// Called when a frame arrives on a channel — returns whether to accept it.
    pub fn arbitrate(&mut self, ch: ChannelId, frame: &JflFrame) -> VoteDecision {
        // Always accept if it's the active channel
        if ch == self.active {
            return VoteDecision::Accept;
        }
        
        // Accept from shadow channel only if active channel has failed
        let (active_health, shadow_health) = match self.active {
            ChannelId::A => (&self.ch_a_health, &self.ch_b_health),
            ChannelId::B => (&self.ch_b_health, &self.ch_a_health),
        };
        
        if active_health.is_failed() {
            self.do_failover(ch);
            return VoteDecision::AcceptWithFailover;
        }
        
        // Proactive switch if shadow is significantly better AND hold time elapsed
        let score_diff = shadow_health.score().saturating_sub(active_health.score());
        let held_long_enough = self.last_switch_time.elapsed_ms() > self.min_hold_ms;
        
        if score_diff > self.score_delta_threshold && held_long_enough {
            self.do_failover(ch);
            return VoteDecision::AcceptWithFailover;
        }
        
        VoteDecision::Discard  // shadow frame, not switching yet
    }
    
    fn do_failover(&mut self, new_active: ChannelId) {
        self.active = new_active;
        self.last_switch_time = Instant::now();
        // emit CHANNEL_CHANGE telemetry on the newly active channel
    }
}

pub enum VoteDecision {
    Accept,
    AcceptWithFailover,
    Discard,
}
```

---

## Failover State Machine

```
DUAL_ACTIVE
  │  ch_a_health.is_failed() AND active == A
  ▼
SINGLE_B_ACTIVE ──────────────────────────────┐
  │  ch_a recovers (score > 40 for 2s)        │ ch_b also fails
  ▼                                           ▼
DUAL_ACTIVE                              BOTH_FAILED
                                              │  > T_failsafe (default 5s)
                                              ▼
                                         FAILSAFE_RTH
```

**T_failsafe** is mission-configurable:
- Takeoff phase: 2 seconds
- Cruise phase: 5 seconds
- BVLOS: 10 seconds (with onboard autonomous return)

---

## Simultaneous Receive Architecture

Both channels receive independently on separate hardware SPI/UART paths. The firmware
never time-division-multiplexes on a single radio — that would halve reliability:

```rust
// Embassy async tasks — run concurrently
#[embassy_executor::task]
async fn channel_a_rx(mut radio: RfdRadio, tx: Sender<JflFrame>) {
    loop {
        let frame = radio.receive().await;
        tx.send(TaggedFrame { ch: ChannelId::A, frame }).await;
    }
}

#[embassy_executor::task]
async fn channel_b_rx(mut radio: SxRadio, tx: Sender<JflFrame>) {
    loop {
        let frame = radio.receive().await;
        tx.send(TaggedFrame { ch: ChannelId::B, frame }).await;
    }
}

#[embassy_executor::task]
async fn link_manager(mut rx: Receiver<TaggedFrame>, voter: &mut FrameVoter) {
    loop {
        let tagged = rx.receive().await;
        match voter.arbitrate(tagged.ch, &tagged.frame) {
            VoteDecision::Accept | VoteDecision::AcceptWithFailover => {
                process_frame(tagged.frame).await;
            }
            VoteDecision::Discard => {}
        }
    }
}
```

---

## Transmit Diversity

On the TX side, both channels transmit every frame simultaneously:

```rust
pub async fn transmit(frame: JflFrame, ch_a: &mut ChannelA, ch_b: &mut ChannelB) {
    // Clone frame and send on both channels concurrently
    // Channel flags already set inside JflFrame by caller
    let (frame_a, frame_b) = frame.clone_for_channels();
    tokio::join!(
        ch_a.send(frame_a),
        ch_b.send(frame_b),
    );
}
```

This doubles RF power draw but ensures the receiver always gets at least one copy.
For power-constrained UAVs, configure Ch B to transmit every-other-frame (50% TX diversity
mode) — still provides substantial link redundancy.

---

## DO-160G / Certification Alignment

| Requirement | Implementation |
|---|---|
| EMC Category M (DO-160G §21) | Separate RF chassis, ferrite on coax, shielded connectors |
| Conducted susceptibility | Isolated power supply per radio module |
| Radiated susceptibility | Cavity-shielded PCB, SMA feedthrough filters |
| Fail-safe on power loss | Hardware watchdog per radio; auto-RTH on power-loss detect |
| Independence of channels | Separate MCU SPI buses, separate antennas (≥λ/2 spacing) |