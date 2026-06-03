# JFOXLink Rust Implementation Guide

## Crate Dependency Tree

```toml
# Cargo.toml (workspace root)
[workspace]
members = [
    "crates/jfl-core",
    "crates/jfl-hal",
    "crates/jfl-gcs",
    "crates/jfl-sim",
]

# crates/jfl-core/Cargo.toml
[package]
name = "jfl-core"
version = "0.1.0"
edition = "2021"

[features]
default = []
std = ["aes-gcm/std", "p256/std", "heapless/defmt-03"]
defense = ["p384"]
post_quantum = ["pqcrypto-kyber"]

[dependencies]
# Cryptography — RustCrypto family
aes-gcm      = { version = "0.10", default-features = false, features = ["aes"] }
p256         = { version = "0.13", default-features = false, features = ["ecdh"] }
p384         = { version = "0.13", default-features = false, features = ["ecdh"], optional = true }
hkdf         = { version = "0.12", default-features = false }
hmac         = { version = "0.12", default-features = false }
sha2         = { version = "0.10", default-features = false }
subtle       = { version = "2.5",  default-features = false }  # constant-time ops
zeroize      = { version = "1.7",  default-features = false, features = ["derive"] }
rand_core    = { version = "0.6",  default-features = false }

# Embedded utilities
heapless     = { version = "0.8",  default-features = false }
embedded-hal = { version = "1.0",  default-features = false }

# MAVLink compatibility
mavlink      = { version = "0.12", default-features = false, features = ["common", "ardupilotmega"] }

# Async (Embassy)
embassy-sync = { version = "0.5",  default-features = false }
embassy-time = { version = "0.3",  default-features = false }

# Logging
defmt        = { version = "0.3",  optional = true }

[dev-dependencies]
arbitrary    = { version = "1.3", features = ["derive"] }
```

---

## Core Module Layout

```
jfl-core/src/
├── lib.rs                   # pub re-exports, feature gating
├── frame.rs                 # JflFrame struct, encode/decode
├── error.rs                 # JflError enum, all error cases
│
├── crypto/
│   ├── mod.rs               # pub use, CryptoSuite enum
│   ├── aes_gcm.rs           # FrameCrypto struct
│   ├── ecdh.rs              # KeyExchange, SessionKeyMaterial
│   ├── hkdf.rs              # derive_session_keys()
│   ├── hmac.rs              # compute_frame_hmac, verify_frame_hmac
│   └── nonce.rs             # NonceCounter, replay window
│
├── channel/
│   ├── mod.rs               # ChannelId, ChannelState
│   ├── health.rs            # ChannelHealth, scoring
│   ├── voter.rs             # FrameVoter
│   ├── manager.rs           # DualChannelManager
│   └── failover.rs          # FailoverStateMachine
│
├── anti_jam/
│   ├── mod.rs               # AjMode, JamStatus
│   ├── fhss.rs              # FhssHopGen, HopSyncMode
│   ├── dsss.rs              # spread_byte, despread
│   ├── power_ctrl.rs        # PowerController
│   └── detector.rs          # JammerDetector, FFT energy
│
├── session.rs               # Session, KeyRotationPolicy
├── mavlink_compat.rs        # jfl_encode, jfl_decode wrappers
└── phases/
    ├── preflight.rs          # PreFlightCheck
    ├── cruise.rs             # CruiseChannelPolicy, BvlosConfig
    └── emergency.rs          # EmergencyTrigger, enter_emergency
```

---

## Session Struct (Central State)

```rust
use zeroize::{Zeroize, ZeroizeOnDrop};

#[derive(ZeroizeOnDrop)]
pub struct Session {
    /// Cryptographic material
    pub keys:           SessionKeyMaterial,
    pub frame_crypto:   FrameCrypto,
    pub nonce:          NonceCounter,
    
    /// Anti-jamming
    pub fhss:           FhssHopGen,
    pub power_ctrl:     PowerController,
    pub jam_detector:   JammerDetector,
    
    /// Channel management  
    pub channels:       DualChannelManager,
    
    /// Session metadata
    pub session_id:     u32,
    pub frame_count:    u64,
    pub start_time:     Instant,
    pub state:          SessionState,
    
    /// Key rotation
    pub rotation_policy: KeyRotationPolicy,
    pub jam_detected:   bool,
}

impl Session {
    pub fn age_secs(&self) -> u64 {
        self.start_time.elapsed().as_secs()
    }
    
    pub fn should_rekey(&self) -> bool {
        self.rotation_policy.should_rotate(self)
    }
    
    /// SECURITY: Called on every frame encode — checks for rotation
    pub fn pre_encode_check(&mut self) -> Result<(), JflError> {
        if self.should_rekey() {
            return Err(JflError::RekeyRequired);
        }
        self.frame_count += 1;
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SessionState {
    Uninit,
    KeyExchange,
    ChannelSync,
    Operational,
    Degraded,
    Failsafe,
    PostFlight,
}
```

---

## Frame Encode/Decode Pipeline

```rust
/// Complete encode pipeline: MAVLink message → JFOXLink encrypted frame bytes
pub fn encode_message(
    msg:     &MavMessage,
    session: &mut Session,
    ch:      ChannelId,
) -> Result<heapless::Vec<u8, 321>, JflError> {
    // 0. Pre-encode safety check (rotation, state)
    session.pre_encode_check()?;
    
    // 1. Serialize MAVLink payload
    let mut payload = heapless::Vec::<u8, 253>::new();
    mavlink_serialize(msg, &mut payload)?;
    
    // 2. Build JFOXLink extension header
    let nonce = session.nonce.next();
    let header = build_jfl_header(msg, nonce, ch, &session.channels)?;
    
    // 3. Encrypt payload (AES-256-GCM), header as AAD
    let gcm_tag = session.frame_crypto.encrypt_payload(nonce, &header, &mut payload)?;
    
    // 4. Assemble frame (header + ciphertext + GCM tag)
    let mut frame_bytes = heapless::Vec::<u8, 289>::new();
    frame_bytes.extend_from_slice(&header).map_err(|_| JflError::BufferFull)?;
    frame_bytes.extend_from_slice(&payload).map_err(|_| JflError::BufferFull)?;
    frame_bytes.extend_from_slice(&gcm_tag).map_err(|_| JflError::BufferFull)?;
    
    // 5. Compute HMAC-SHA256 over everything so far
    let hmac = compute_frame_hmac(&session.keys.hmac_key, &frame_bytes);
    
    let mut output = heapless::Vec::<u8, 321>::new();
    output.extend_from_slice(&frame_bytes).map_err(|_| JflError::BufferFull)?;
    output.extend_from_slice(&hmac).map_err(|_| JflError::BufferFull)?;
    
    Ok(output)
}

/// Complete decode pipeline: raw bytes → MAVLink message
pub fn decode_frame(
    bytes:   &[u8],
    session: &mut Session,
    ch:      ChannelId,
) -> Result<MavMessage, JflError> {
    // 0. Minimum length check
    if bytes.len() < 42 + 10 { // min MAVLink + JFL header + tags
        return Err(JflError::FrameTooShort);
    }
    
    // 1. Verify HMAC first (authenticate before decrypt — critical!)
    let hmac_offset = bytes.len() - 32;
    verify_frame_hmac(
        &session.keys.hmac_key,
        &bytes[..hmac_offset],
        bytes[hmac_offset..].try_into().map_err(|_| JflError::InvalidHmac)?,
    )?;
    
    // 2. Parse JFL header, extract nonce and channel flags
    let header: [u8; 20] = bytes[0..20].try_into().map_err(|_| JflError::ParseError)?;
    let nonce = u64::from_le_bytes(bytes[11..19].try_into().unwrap());
    
    // 3. Verify nonce (replay protection)
    session.nonce.verify(nonce)?;
    
    // 4. Decrypt payload (GCM verify + decrypt)
    let gcm_tag_offset = hmac_offset - 16;
    let gcm_tag: [u8; 16] = bytes[gcm_tag_offset..hmac_offset]
        .try_into().map_err(|_| JflError::ParseError)?;
    let mut payload = heapless::Vec::<u8, 253>::new();
    payload.extend_from_slice(&bytes[20..gcm_tag_offset])
        .map_err(|_| JflError::BufferFull)?;
    session.frame_crypto.decrypt_payload(nonce, &header, &mut payload, &gcm_tag)?;
    
    // 5. Update channel health
    session.channels.update_rx_health(ch);
    
    // 6. Deserialize MAVLink message
    let msg_id = u32::from_le_bytes([bytes[7], bytes[8], bytes[9], 0]);
    mavlink_deserialize(msg_id, &payload)
}
```

---

## Error Handling

```rust
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum JflError {
    // Frame structural errors
    FrameTooShort,
    FrameTooLong,
    ParseError,
    BufferFull,
    
    // Cryptographic errors
    HmacMismatch,        // authentication failed — drop silently (don't respond)
    GcmAuthFailed,       // authenticated encryption tag invalid
    EncryptFailed,       // shouldn't happen with correct key size
    RekeyRequired,       // rotation policy triggered
    
    // Nonce/Replay errors
    NonceReplayed,       // exact replay detected
    NonceTooOld,         // outside replay window
    NonceTooFarAhead,    // suspicious — possible attack
    
    // Session errors
    SessionNotEstablished,
    KeyExchangeFailed,
    KeyDerivationFailed,
    
    // Channel errors
    BothChannelsFailed,
    ChannelHealthLow(ChannelId),
    
    // MAVLink compatibility errors
    MavlinkSerializeError,
    MavlinkDeserializeError,
    UnknownMessageId(u32),
}

// SECURITY: HMAC and GCM failures must log identically (no oracle)
impl JflError {
    pub fn is_auth_failure(&self) -> bool {
        matches!(self, JflError::HmacMismatch | JflError::GcmAuthFailed)
    }
    
    pub fn should_log_silently(&self) -> bool {
        // Don't reveal auth failure reason to potential attacker
        self.is_auth_failure() || matches!(self, JflError::NonceReplayed)
    }
}
```

---

## Testing Strategy

### Unit Tests (jfl-core)
```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn roundtrip_encrypt_decrypt() {
        let session = Session::test_session();
        let msg = MavMessage::Heartbeat(Heartbeat::default());
        let encoded = encode_message(&msg, &mut session.clone(), ChannelId::A).unwrap();
        let decoded = decode_frame(&encoded, &mut session.clone(), ChannelId::A).unwrap();
        assert_eq!(msg, decoded);
    }
    
    #[test]
    fn replay_rejected() {
        let mut session = Session::test_session();
        let msg = MavMessage::Heartbeat(Heartbeat::default());
        let frame = encode_message(&msg, &mut session, ChannelId::A).unwrap();
        // First decode OK
        decode_frame(&frame, &mut session.clone(), ChannelId::A).unwrap();
        // Second decode with same nonce → rejected
        assert_eq!(
            decode_frame(&frame, &mut session, ChannelId::A),
            Err(JflError::NonceReplayed)
        );
    }
    
    #[test]
    fn hmac_tamper_detected() {
        let mut session = Session::test_session();
        let msg = MavMessage::Heartbeat(Heartbeat::default());
        let mut frame = encode_message(&msg, &mut session, ChannelId::A).unwrap();
        frame[5] ^= 0xFF;  // flip SYS_ID byte
        assert_eq!(
            decode_frame(&frame, &mut session, ChannelId::A),
            Err(JflError::HmacMismatch)
        );
    }
}
```

### Fuzz Targets (jfl-sim)
```rust
// fuzz/fuzz_targets/decode_frame.rs
#![no_main]
use libfuzzer_sys::fuzz_target;
use jfl_core::{decode_frame, Session};

fuzz_target!(|data: &[u8]| {
    let mut session = Session::test_session();
    let _ = decode_frame(data, &mut session, jfl_core::ChannelId::A);
    // Must not panic regardless of input
});
```

Run with: `cargo +nightly fuzz run decode_frame -- -max_total_time=3600`

---

## Hardware Bring-Up Checklist

```
[ ] RFD900x SPI link verified at 10 MHz
[ ] SX1280 SPI link verified at 8 MHz
[ ] Antenna installed with ≥λ/2 separation between Ch A and Ch B
[ ] RF shielding on JFL HAL PCB (reduces cross-coupling to <-60 dBc)
[ ] Hardware RNG entropy source verified (TRNG or ATECC608B)
[ ] PSK programmed to eFuse and eFuse write-locked
[ ] Firmware signed + secure boot enabled
[ ] Attestation check passes at startup (radio firmware hash)
[ ] Pre-flight check all-green in bench test
[ ] Frame roundtrip test at 100 Hz for 10 minutes (0 crypto errors)
[ ] Failover test: disable Ch A antenna → confirm Ch B takes over <500ms
[ ] Jam test: narrowband interference on Ch A → confirm Ch B active
```