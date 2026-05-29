# JFOXLink ↔ ESP32 Flight Controller Integration

This file covers how the **JFOXLink secure datalink** integrates into the JFOX ESP32
flight controller architecture. For the full JFOXLink protocol spec, crypto, and
anti-jamming details, refer to the **jfoxlink skill** (`/mnt/skills/user/jfoxlink/`).

---

## Physical Architecture

```
┌──────────────────────────────┐     UART/SPI       ┌──────────────────────────────┐
│      ESP32-S3 (FCU)          │◄──────────────────►│    ESP32-C6 (Comms Node)     │
│                              │   COBS-framed       │                              │
│  fc-core (control laws)      │   921600 baud       │  fc-jfoxlink                 │
│  fc-mavlink (msg bus)        │                     │  ├── adapter.rs              │
│  fc-hal (sensors/actuators)  │                     │  ├── session.rs (ECDH)       │
│                              │                     │  └── radio_hal.rs            │
└──────────────────────────────┘                     │                              │
                                                     │  jfl-core (no_std)           │
                                                     │  ├── AES-256-GCM             │
                                                     │  ├── FHSS hop sequencer      │
                                                     │  ├── DSSS spreading          │
                                                     │  └── Dual-channel voter      │
                                                     │                              │
                                                     │  Radio Hardware              │
                                                     │  ├── RFD900x  (Ch A 900MHz)  │
                                                     │  └── SX1280   (Ch B 2.4GHz)  │
                                                     └──────────────────────────────┘
```

**Why a dedicated C6 comms node?**
- Isolates RF/crypto complexity from flight-critical S3 loops
- C6 soft-float penalty irrelevant for JFOXLink (mostly integer/crypto ops)
- Fault isolation: comms failure cannot corrupt rate controller state
- C6 can be power-cycled independently for link reset without affecting FCU

---

## Inter-MCU Link Protocol

Frames exchanged over UART between S3 and C6 use **COBS encoding** (Consistent Overhead
Byte Stuffing) for framing — zero-copy, no heap, deterministic.

```rust
// Shared type (fc-mavlink + fc-jfoxlink crates)
#[repr(C)]
pub struct InterMcuFrame {
    pub frame_type: InterMcuFrameType,
    pub seq: u16,
    pub payload_len: u16,
    pub payload: [u8; 280],   // max MAVLink v2 payload
    pub crc16: u16,
}

pub enum InterMcuFrameType {
    MavlinkToGcs   = 0x01,  // S3 → C6: telemetry to encrypt + send
    MavlinkFromGcs = 0x02,  // C6 → S3: decrypted command from GCS
    LinkStatus     = 0x03,  // C6 → S3: channel health, RSSI, jam detect
    SessionRekey   = 0x04,  // C6 → S3: notify new JFOXLink session established
    Heartbeat      = 0x05,  // bidirectional: watchdog
}
```

---

## Adapter Crate (`fc-jfoxlink`)

```rust
// fc-jfoxlink/src/adapter.rs

use jfl_core::{JflSession, JflFrame, ChannelId};
use crate::radio_hal::{RadioA, RadioB};

pub struct JfoxLinkAdapter<UART, RA, RB> {
    inter_mcu: CobsUart<UART>,    // link to S3
    session:   JflSession,        // jfl-core session (crypto + channels)
    radio_a:   RA,                // RFD900x (FHSS, 900 MHz)
    radio_b:   RB,                // SX1280  (DSSS, 2.4 GHz)
    link_stats: LinkStats,
}

impl<UART, RA, RB> JfoxLinkAdapter<UART, RA, RB>
where
    UART: AsyncRead + AsyncWrite,
    RA: RadioTx + RadioRx,
    RB: RadioTx + RadioRx,
{
    /// S3 → C6 → GCS: encrypt and transmit telemetry
    pub async fn forward_to_gcs(&mut self, raw: &[u8]) -> Result<(), LinkError> {
        let jfl_frame = self.session.encrypt_frame(raw)?;
        // Transmit on both channels simultaneously (dual redundancy)
        embassy_futures::join::join(
            self.radio_a.transmit(jfl_frame.channel_a_bytes()),
            self.radio_b.transmit(jfl_frame.channel_b_bytes()),
        ).await;
        Ok(())
    }

    /// GCS → C6 → S3: receive, verify, decrypt, forward
    pub async fn receive_from_gcs(&mut self) -> Result<Option<&[u8]>, LinkError> {
        // Poll both channels; voter selects best frame
        let frame_a = self.radio_a.receive_nonblocking();
        let frame_b = self.radio_b.receive_nonblocking();
        let best = self.session.voter().arbitrate(frame_a, frame_b)?;
        if let Some(frame) = best {
            let plaintext = self.session.decrypt_frame(&frame)?;
            self.inter_mcu.send_to_s3(InterMcuFrameType::MavlinkFromGcs,
                                      plaintext).await?;
            Ok(Some(plaintext))
        } else {
            Ok(None)
        }
    }
}
```

---

## Session Lifecycle

```
Power-on:
  C6 reads JFOXLink identity key from OTP (BLOCK3, 256-bit)
  C6 generates ephemeral ECDH keypair (P-256)
  C6 broadcasts pre-auth beacon on both channels

GCS connects:
  ECDH handshake → shared secret
  HKDF-SHA256 → session AES key + HMAC key
  FHSS hop sequence negotiated (GPS-disciplined timing)
  C6 notifies S3 via SessionRekey frame

In-flight:
  Dual-channel voter runs continuously
  Nonce counter increments per frame (AtomicU64, never reuses)
  Replay window: 64-nonce sliding window

Post-flight:
  Session keys zeroized (Zeroizing<[u8;32]> drops)
  New session required for next flight
  ULOG download via JFOXLink POST_FLIGHT profile (full crypto, disabled AJ)
```

---

## Session Rust Implementation

```rust
// fc-jfoxlink/src/session.rs

use jfl_core::crypto::{EcdhKeyPair, HkdfDeriver, AesGcmCipher};
use zeroize::Zeroizing;

pub struct JflSessionManager {
    identity_key: Zeroizing<[u8; 32]>,     // from OTP BLOCK3
    ephemeral_kp: Option<EcdhKeyPair>,
    session_aes:  Option<Zeroizing<[u8; 32]>>,
    session_hmac: Option<Zeroizing<[u8; 32]>>,
    nonce:        core::sync::atomic::AtomicU64,
}

impl JflSessionManager {
    pub fn new_from_otp() -> Self {
        Self {
            identity_key: OtpManager::jfoxlink_identity_key(),
            ephemeral_kp: None,
            session_aes: None,
            session_hmac: None,
            nonce: core::sync::atomic::AtomicU64::new(1),
        }
    }

    pub fn begin_handshake(&mut self) -> EcdhPublicKey {
        let kp = EcdhKeyPair::generate();
        let pubkey = kp.public_key();
        self.ephemeral_kp = Some(kp);
        pubkey
    }

    pub fn complete_handshake(&mut self, peer_pub: &EcdhPublicKey)
        -> Result<(), SessionError>
    {
        let kp = self.ephemeral_kp.take().ok_or(SessionError::NoHandshake)?;
        let shared = kp.diffie_hellman(peer_pub);

        // HKDF: derive AES key and HMAC key
        let deriver = HkdfDeriver::new(shared.raw_bytes(), &self.identity_key);
        self.session_aes  = Some(deriver.expand(b"jfoxlink-aes-key",  32));
        self.session_hmac = Some(deriver.expand(b"jfoxlink-hmac-key", 32));
        Ok(())
    }

    pub fn next_nonce(&self) -> u64 {
        // Monotonic — AtomicU64 never decrements, never reuses
        self.nonce.fetch_add(1, core::sync::atomic::Ordering::Relaxed)
    }
}
```

---

## Radio HAL (`radio_hal.rs`)

```rust
// fc-jfoxlink/src/radio_hal.rs

/// Channel A: RFD900x (900 MHz FHSS) via UART
pub struct Rfd900xRadio<UART> {
    uart: UART,
    hop_seq: FhssHopSequencer,
    current_freq_khz: u32,
}

impl<UART: AsyncRead + AsyncWrite> RadioTx for Rfd900xRadio<UART> {
    async fn transmit(&mut self, frame: &[u8]) -> Result<(), RadioError> {
        // Apply current hop frequency, send AT command + payload
        let freq = self.hop_seq.current_frequency_khz();
        self.uart.write_all(&build_rfd_tx_cmd(freq, frame)).await?;
        Ok(())
    }
}

/// Channel B: Semtech SX1280 (2.4 GHz DSSS) via SPI
pub struct Sx1280Radio<SPI> {
    spi: SPI,
    spreading_factor: u8,
}

impl<SPI: SpiDevice> RadioTx for Sx1280Radio<SPI> {
    async fn transmit(&mut self, frame: &[u8]) -> Result<(), RadioError> {
        // Set DSSS spreading, write payload buffer, trigger TX
        self.write_reg(SX1280_REG_SF, self.spreading_factor).await?;
        self.write_fifo(frame).await?;
        self.set_tx_mode().await?;
        Ok(())
    }
}
```

---

## JFOXLink Integration Checklist

Before using this integration in production:

- [ ] OTP BLOCK3 provisioned with JFOXLink identity key at manufacture
- [ ] Secure boot enabled on ESP32-C6 (verify: `OtpManager::secure_boot_enabled()`)
- [ ] Inter-MCU UART CRC validation enabled (detect wire faults)
- [ ] C6 watchdog fed by S3 heartbeat frame (link health monitor)
- [ ] Nonce counter persisted to NVS on graceful shutdown (prevent reuse across resets)
- [ ] Session keys zeroized on power-down / failsafe trigger
- [ ] FHSS hop sequence synchronized via GPS PPS signal (±1µs accuracy)
- [ ] Jammer detection FFT threshold tuned for operating RF environment
- [ ] Dual-channel failover tested with deliberate Channel A jamming