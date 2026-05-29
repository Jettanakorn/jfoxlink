# JFOXLink Protocol Reference

## Design Philosophy

JFOXLink extends MAVLink v2 with three non-negotiable properties:

1. **Backward compatibility** — JFOXLink frames are valid MAVLink v2 frames to non-JFL parsers
   (they will discard the unknown INCOMPAT flag, but won't crash)
2. **Zero-trust security** — every frame is authenticated; no plaintext mode in production
3. **Dual-channel native** — the redundancy model is baked into the frame, not bolted on

---

## Frame Structure — Detailed

### MAVLink v2 Base (unchanged)
```
Byte  0:    STX     = 0xFD
Byte  1:    LEN     = payload length (encrypted payload + auth tag)
Byte  2:    INCOMPAT_FLAGS  (JFOXLink sets bits 0x01 + 0x02)
Byte  3:    COMPAT_FLAGS    (JFOXLink sets bit 0x01 for dual-channel)
Byte  4:    SEQ     = frame sequence number (wraps at 255)
Byte  5:    SYS_ID  = originating system
Byte  6:    COMP_ID = originating component
Bytes 7–9:  MSG_ID  (24-bit, little-endian)
```

### JFOXLink Extension Header (immediately after MSG_ID)
```
Byte 10:    JFL_VERSION     = 0x01 (current)
Bytes 11–18: NONCE          = 64-bit counter (little-endian, per-session, per-direction)
Byte 19:    CHANNEL_FLAGS
              bit 0: frame transmitted on Channel A
              bit 1: frame transmitted on Channel B
              bit 2: Channel A health OK
              bit 3: Channel B health OK
              bit 4–7: reserved
```

### Encrypted Payload
```
Bytes 20–N: AES-256-GCM ciphertext of original MAVLink v2 payload
            (variable, same length as plaintext — GCM is a stream cipher)
```

### Authentication Tags
```
Bytes N+1 to N+16:  GCM Auth Tag (128-bit, from AES-256-GCM)
Bytes N+17 to N+48: HMAC-SHA256 (32 bytes)
                    HMAC covers: bytes 0 to N+16 (full frame minus HMAC itself)
```

---

## INCOMPAT_FLAGS Semantics

| Bit | Meaning | MAVLink Standard | JFOXLink |
|---|---|---|---|
| 0x01 | MAVLINK_IFLAG_SIGNED | Frame has signature | Used for HMAC presence |
| 0x02 | Reserved in MAVLink v2 | Must ignore or discard | JFOXLink crypto active |
| 0x04 | Reserved | Must ignore or discard | Reserved for post-quantum |

A legacy MAVLink v2 parser seeing 0x02 set will **drop the frame** (correct per spec).
JFOXLink endpoints know to handle both bits.

---

## Message Priority Classes

```rust
#[repr(u8)]
pub enum MsgPriority {
    Critical    = 0,  // RTH command, failsafe, motor kill — never dropped
    Realtime    = 1,  // IMU, attitude, rate — dropped if channel saturated
    Navigation  = 2,  // Position, waypoint — small drop tolerance
    Telemetry   = 3,  // Battery, GPS status — liberal drop OK
    Background  = 4,  // Param up/download, log streaming — best effort
}
```

Scheduler ensures Critical frames bypass all queuing and encrypt with a pre-allocated nonce block.

---

## Protocol State Machine

```
UNINIT
  │  power_on()
  ▼
KEY_EXCHANGE
  │  ecdh_complete() + keys_derived()
  ▼
CHANNEL_SYNC        ← FHSS hop table synchronized, both channels active
  │  link_quality_ok()
  ▼
OPERATIONAL         ← normal dual-channel encrypted operation
  │  jam_detected() OR single_channel_loss()
  ▼
DEGRADED            ← single-channel fallback, anti-jam max power
  │  both_channels_lost() for > T_failsafe
  ▼
FAILSAFE            ← RTH commanded, minimal beacon only
  │  link_recovered()
  ▼
OPERATIONAL         ← automatic re-entry after authentication
```

---

## Nonce Management

```rust
/// SECURITY: nonce MUST be strictly monotonically increasing.
/// PANIC: never — AtomicU64 fetch_add wraps, but at 2^64 frames
///        that is ~584,942 years at 1000 fps. Not a real concern.
pub struct NonceCounter {
    counter: AtomicU64,
    /// Rolling replay window — last 64 nonces accepted
    window: AtomicU64,  // bitmask
    window_base: AtomicU64,
}

impl NonceCounter {
    pub fn next(&self) -> u64 {
        self.counter.fetch_add(1, Ordering::SeqCst)
    }
    
    pub fn verify(&self, nonce: u64) -> Result<(), NonceError> {
        let base = self.window_base.load(Ordering::SeqCst);
        if nonce < base.saturating_sub(64) {
            return Err(NonceError::TooOld);
        }
        if nonce > base + 128 {
            return Err(NonceError::TooFarAhead);
        }
        // check bitmask for replay
        let bit = nonce.wrapping_sub(base) & 63;
        let mask = 1u64 << bit;
        let prev = self.window.fetch_or(mask, Ordering::SeqCst);
        if prev & mask != 0 {
            return Err(NonceError::Replayed);
        }
        Ok(())
    }
}
```

---

## Versioning and Negotiation

At KEY_EXCHANGE phase, both endpoints exchange capabilities:

```rust
pub struct JflCapabilities {
    pub version: u8,                    // JFL_VERSION supported
    pub crypto_suites: CryptoSuiteMask, // AES-128, AES-256, P-256, P-384
    pub anti_jam_modes: AjModeMask,     // FHSS, DSSS, OFDM
    pub channel_config: ChannelConfig,  // dual, single, SDR
    pub cert_level: CertLevel,          // Commercial, Defense, DO178C
}
```

The lower capability set is used if endpoints differ. Negotiation is itself authenticated.

---

## MAVLink v2 Compatibility Shim

JFOXLink wraps the standard `mavlink` crate:

```rust
use mavlink::common::MavMessage;

pub fn jfl_encode(msg: &MavMessage, session: &Session, ch: ChannelId)
    -> Result<JflFrame, JflError>
{
    let payload = mavlink::write_v2_msg_payload(msg)?;
    let nonce = session.nonce.next();
    let ciphertext = session.aes_gcm.encrypt(nonce, &payload)?;
    Ok(JflFrame::new(msg.message_id(), nonce, ch, ciphertext, &session.hmac_key))
}

pub fn jfl_decode(frame: &JflFrame, session: &Session)
    -> Result<MavMessage, JflError>
{
    frame.verify_hmac(&session.hmac_key)?;
    session.nonce.verify(frame.nonce)?;
    let payload = session.aes_gcm.decrypt(frame.nonce, &frame.ciphertext)?;
    Ok(mavlink::read_v2_msg_payload(frame.msg_id, &payload)?)
}
```

---

## Frame Size Budget

| Component | Size (bytes) |
|---|---|
| MAVLink v2 base header | 10 |
| JFOXLink ext header | 10 (version + nonce + channel_flags) |
| MAVLink payload (max) | 253 |
| GCM auth tag | 16 |
| HMAC-SHA256 | 32 |
| **Total max frame** | **321 bytes** |

Overhead vs plain MAVLink v2: +42 bytes fixed + same payload length (GCM is stream cipher).
At 100Hz telemetry: +42 × 100 = 4.2 KB/s overhead — acceptable on any modern datalink.