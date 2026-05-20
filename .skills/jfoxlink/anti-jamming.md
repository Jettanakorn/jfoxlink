# JFOXLink Anti-Jamming Reference

## Threat Taxonomy

### Jamming Types by Technique
| Type | Description | Mitigation |
|---|---|---|
| **Spot jammer** | Narrowband, fixed freq | FHSS — hop away within 50ms |
| **Swept jammer** | Narrowband, sweeping | DSSS processing gain + FHSS |
| **Barrage jammer** | Broadband, full band | Power control + dual-band |
| **Partial-band** | Covers fraction of band | FHSS hop density management |
| **Smart/follow jammer** | Tracks hop sequence | Cryptographic FHSS sync |
| **Repeater jammer** | Captures & replays | Nonce replay protection |
| **Deceptive jammer** | Injects false frames | HMAC authentication |
| **GPS denial** | Disrupts positioning | INS dead-reckoning + link timing |

---

## Anti-Jamming Layer 1: FHSS (Channel A)

### Hop Sequence Generation

The hop sequence MUST be cryptographically unpredictable to prevent a smart jammer
from tracking it. Use AES-128-CTR as a PRNG:

```rust
pub struct FhssHopGen {
    aes_ctr: Aes128Ctr,       // keyed from HKDF-derived hop_key
    channel_map: [u8; 100],   // available channels (excludes regional restrictions)
    current_idx: u32,
    hop_period_ms: u32,       // default: 50ms
}

impl FhssHopGen {
    /// Generate next channel index — deterministic from shared hop_key
    pub fn next_channel(&mut self) -> u8 {
        let mut buf = [0u8; 1];
        loop {
            self.aes_ctr.apply_keystream(&mut [0u8; 1]);  // advance CTR
            let idx = buf[0] as usize % 100;
            if self.channel_map[idx] != 0xFF {  // not masked out
                return self.channel_map[idx];
            }
        }
    }
    
    /// Resync after desync — both sides agree on absolute frame count
    pub fn resync(&mut self, frame_count: u64) {
        // Reconstruct CTR state from frame_count × channels_per_second
        self.aes_ctr.seek(frame_count * (1000 / self.hop_period_ms as u64));
    }
}
```

### Hop Synchronization

Two synchronization methods — ranked by resilience:

1. **GPS-disciplined**: Both GCS and UAV lock hop timing to GPS PPS signal.
   - Resilience: Survives 100ms GPS outage via crystal holdover
   - Attack surface: GPS spoofing (mitigated by cross-check with IMU)

2. **Link-disciplined**: UAV broadcasts encrypted timing beacon on Channel B.
   - Resilience: Independent of GPS
   - Overhead: 1 beacon frame per hop period

```rust
pub enum HopSyncMode {
    GpsDisciplined { pps_pin: GpioPin },
    LinkDisciplined { beacon_interval_ms: u32 },
    HybridFallback,   // GPS primary, Link fallback
}
```

---

## Anti-Jamming Layer 2: DSSS (Channel B)

### Barker Code Spreading

The 11-chip Barker code `[+1,+1,+1,−1,−1,−1,+1,−1,−1,+1,−1]` provides:
- **Processing gain**: 10 × log10(11) ≈ **10.4 dB** against narrowband jammers
- **Autocorrelation**: Near-zero sidelobes → resistant to multipath
- **Low cross-correlation**: Multiple UAVs can share band with different chip phases

```rust
const BARKER_11: [i8; 11] = [1, 1, 1, -1, -1, -1, 1, -1, -1, 1, -1];

pub fn spread_byte(byte: u8, output: &mut [i8; 88]) {
    for bit_idx in 0..8 {
        let bit = ((byte >> bit_idx) & 1) as i8 * 2 - 1;  // +1 or -1
        for chip in 0..11 {
            output[bit_idx * 11 + chip] = bit * BARKER_11[chip];
        }
    }
}

pub fn despread(chips: &[i8; 88]) -> u8 {
    let mut byte = 0u8;
    for bit_idx in 0..8 {
        let corr: i32 = chips[bit_idx*11..(bit_idx+1)*11]
            .iter().zip(BARKER_11.iter())
            .map(|(&c, &b)| c as i32 * b as i32)
            .sum();
        if corr > 0 { byte |= 1 << bit_idx; }
    }
    byte
}
```

---

## Anti-Jamming Layer 3: Adaptive TX Power Control

Minimum necessary power to maintain link quality — reduces detectability and battery drain:

```rust
pub struct PowerController {
    pub current_dbm: i8,          // 6..=30 dBm
    pub target_rssi_dbm: i16,     // -75 dBm (tunable)
    pub step_db: i8,              // 2 dB steps
    pub update_interval_ms: u32,  // 500ms
}

impl PowerController {
    pub fn update(&mut self, measured_rssi: i16) -> i8 {
        let error = self.target_rssi_dbm - measured_rssi;
        if error > 5 {
            self.current_dbm = (self.current_dbm + self.step_db).min(30);
        } else if error < -5 {
            self.current_dbm = (self.current_dbm - self.step_db).max(6);
        }
        self.current_dbm
    }
    
    /// Emergency mode: max power immediately
    pub fn emergency_max(&mut self) {
        self.current_dbm = 30;
    }
}
```

---

## Anti-Jamming Layer 4: Jammer Detection

Real-time FFT-based spectral energy monitor runs as a background task:

```rust
pub struct JammerDetector {
    fft_size: usize,          // 512 points
    noise_floor_dbm: f32,     // calibrated at startup
    energy_threshold_db: f32, // default: +15 dB over noise floor
    detection_count: u32,     // consecutive detections before alert
    pub jam_score: f32,       // 0.0–1.0
}

impl JammerDetector {
    pub fn update(&mut self, spectrum: &[f32]) -> JamStatus {
        let peak = spectrum.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
        let excess = peak - self.noise_floor_dbm;
        
        if excess > self.energy_threshold_db {
            self.detection_count += 1;
            self.jam_score = (self.detection_count as f32 / 5.0).min(1.0);
        } else {
            self.detection_count = self.detection_count.saturating_sub(1);
            self.jam_score *= 0.95;  // exponential decay
        }
        
        match self.jam_score {
            s if s > 0.8 => JamStatus::Confirmed,
            s if s > 0.4 => JamStatus::Suspected,
            _             => JamStatus::Clear,
        }
    }
}

pub enum JamStatus { Clear, Suspected, Confirmed }
```

### Actions on Jam Detection
| Status | Action |
|---|---|
| `Suspected` | Increase TX power 6 dB; log event |
| `Confirmed` on Ch A | Switch voter to Ch B; trigger emergency FHSS re-key |
| `Confirmed` on both | Failsafe RTH; reduce TX to beacon-only |
| RSSI > -30 dBm (very close) | Suspect drone intercept; alert GCS operator |

---

## Emergency Frequency Hopping Re-Key

If a smart jammer appears to track the hop sequence (follow-jammer signature),
trigger an in-flight re-key:

```rust
pub async fn emergency_rekey(
    session: &mut Session,
    beacon_ch: &mut ChannelB,
) -> Result<(), JflError> {
    // 1. Generate new hop key from current session key + counter
    let new_hop_key = session.hkdf.expand(b"emergency_hop_rekey", 16)?;
    
    // 2. Encrypt new key with current AES-GCM session
    let encrypted_key = session.aes_gcm.encrypt(session.nonce.next(), &new_hop_key)?;
    
    // 3. Broadcast on Ch B (assumed cleaner during Ch A jamming)
    let rekey_frame = JflFrame::rekey(encrypted_key);
    beacon_ch.send(rekey_frame).await?;
    
    // 4. Apply new hop key after 3 hop periods (time for receiver to get it)
    Timer::after(Duration::from_millis(3 * 50)).await;
    session.fhss.update_key(new_hop_key);
    
    Ok(())
}
```

---

## Anti-Jam Performance Budget

| Threat | Jammer Power | Expected Link Survival |
|---|---|---|
| Spot jammer (1 channel) | Any | 100% — FHSS hops away in ≤50ms |
| Swept jammer (100 kHz/ms) | <1W | >99% — DSSS + FHSS combination |
| Barrage 900 MHz | <10W @ 1km | Degraded; Ch B takes over |
| Barrage 900 MHz | >10W @ 1km | Ch B active; RTH if Ch B also hit |
| Barrage both bands | >50W @ 1km | Failsafe RTH on battery backup |
| Follow-jammer (GPS-sync'd hop) | Any | Re-key within 150ms |

---

## MIL-STD-461G Alignment

| Test | Requirement | JFOXLink Compliance |
|---|---|---|
| RS103 (radiated susceptibility) | Survive 200 V/m | Shielded enclosure + LC filters on antenna |
| CS114 (conducted susceptibility) | Up to 400 MHz | Isolated supply per radio + bulk cap |
| RE102 (radiated emissions) | Limits per category | FHSS average power spread vs narrowband |
| CE102 (conducted emissions) | Power line limits | Common-mode choke on all supply rails |