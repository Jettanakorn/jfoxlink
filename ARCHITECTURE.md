# JFOXLink — Architecture (as implemented)

JFOXLink is a **secure RF command-and-control datalink** (a crypto-wrapped,
anti-jam derivative of MAVLink v2), not a transport-agnostic pub/sub bus. This
document reflects the code as built: an explicit security envelope, dual-channel
redundancy, an anti-jam layer, and RF transports (not CAN/UDP).

## 1. Layered layout

```
┌────────────────────────────────────────────────────────────────────┐
│                         Application Layer                           │
│              (Flight Telemetry · Servo / C2 Commands)               │
└────────────────────────────────────────────────────────────────────┘
                     │  Native Rust payload
                     ▼
┌────────────────────────────────────────────────────────────────────┐
│                  Payload / Compatibility Layer                      │
│   native.rs (NativeMessage)   ·   mavlink_compat.rs (MAVLink v2)    │
└────────────────────────────────────────────────────────────────────┘
                     │  plaintext payload bytes
                     ▼
┌────────────────────────────────────────────────────────────────────┐
│              SECURITY ENVELOPE  (jfl-core::crypto, frame)           │
│  ┌──────────────────────────────────────────────────────────────┐  │
│  │ Frame codec   frame.rs   24B header + payload + GCM tag(16) +  │  │
│  │               HMAC(32);  zero-copy from_bytes / to_bytes      │  │
│  ├──────────────────────────────────────────────────────────────┤  │
│  │ Confidentiality   AES-256-GCM (aes_gcm)   header = AAD         │  │
│  │ Integrity / auth  HMAC-SHA-256 (hmac)                          │  │
│  │ Key agreement     ECDH P-256/P-384 → HKDF (ecdh, hkdf)         │  │
│  │ Anti-replay       sliding-window nonce (nonce.rs)             │  │
│  └──────────────────────────────────────────────────────────────┘  │
└────────────────────────────────────────────────────────────────────┘
                     │  authenticated JflFrame  (Unified Packet)
                     ▼
┌────────────────────────────────────────────────────────────────────┐
│           REDUNDANCY / ARBITRATION  (jfl-core::channel)             │
│   manager (health scoring)  ·  voter (best frame)  ·  failover FSM  │
└────────────────────────────────────────────────────────────────────┘
             │  active channel select                 │
   ┌─────────┘                                        └─────────┐
   ▼                                                            ▼
┌───────────────────────────────┐        ┌───────────────────────────────┐
│          CHANNEL A            │        │          CHANNEL B            │
│  Anti-jam: FHSS (anti_jam)    │        │  Anti-jam: DSSS / OFDM        │
│  HAL: RadioTx/Rx, FreqHop,    │        │  HAL: RadioTx/Rx, FreqHop,    │
│       PowerControl, HwStatus  │        │       PowerControl, HwStatus  │
│  Driver: RFD900 (900 MHz UART)│        │  Driver: SX1280 (2.4 GHz SPI) │
└───────────────────────────────┘        └───────────────────────────────┘
   │                                                            │
   ▼                                                            ▼
  ~~ RF link A (FHSS) ~~                          ~~ RF link B (DSSS/OFDM) ~~

        (In defense profiles an SDR (USRP/HackRF) can back either
         channel, adding adaptive hopping + spectral nulling.)
```

Compared with a Cyphal/DDS-style model: the **Unified Packet** here is the
`JflFrame`; the transport fan-out is **RF Channel A / Channel B** (not CAN/UDP);
and the **security + anti-jam layers are first-class**, which is the entire
purpose of the protocol.

## 2. Receive pipeline (`jfl-gcs::decoder`)

The full-stack decoder validates before it decrypts:

```
raw bytes ─► frame::from_bytes ─► HMAC-SHA256 verify ─► replay-window check
          (zero-copy parse)      (header+payload+tag)    (nonce sequence)
          │                                                     │
          └──────────────────────────────► AES-256-GCM decrypt ─┘
                                            (header = AAD)
                                                   │
                                                   ▼
                                          NativeMessage (payload)
```

Any step failing returns a typed error (`FrameParse`, `HmacMismatch`,
`ReplayDetected`, `CryptoFailure`) — the frame is dropped, never trusted.
Transmit (`jfl-gcs::tx`) runs the inverse: encrypt-then-MAC.

## 3. Layer → module → crate map

| Layer | Modules | Crate | std? |
|-------|---------|-------|------|
| Payload / compatibility | `native`, `mavlink_compat` | `jfl-core` | no_std |
| Frame codec | `frame` | `jfl-core` | no_std |
| Crypto envelope | `crypto/{aes_gcm,hmac,ecdh,hkdf,nonce}` | `jfl-core` | no_std |
| Redundancy / arbitration | `channel/{manager,voter,failover}` | `jfl-core` | no_std |
| Anti-jam | `anti_jam/{fhss,dsss,detector}` | `jfl-core` | no_std |
| HAL / PHY traits | `traits` (RadioTx/Rx, FrequencyHop, PowerControl, HwStatus, SerialLine) | `jfl-hal` | no_std |
| Radio drivers | `rfd900` (UART), `sx1280` (SPI), `sdr` (host interface) | `jfl-hal` | no_std |
| Ground station | `decoder`, `key_store`, `tx`, `config` | `jfl-gcs` | std |
| Simulator | `channel_emulator`, `threat_injector` | `jfl-sim` | std |
| Tooling | `key_provisioner`, `link_analyzer`, `fuzz` | `tools/*` | std |

## 4. Transports (vs. the Cyphal CAN/UDP model)

JFOXLink transports are **RF radios**, selected per channel by the runtime
profile (`config/*.toml`):

| Profile | Channel A | Channel B |
|---------|-----------|-----------|
| commercial-low | 915 MHz FHSS | 2.4 GHz DSSS |
| commercial-high | 900 MHz FHSS | 5.8 GHz OFDM |
| defense-lite | 900 MHz FHSS | 1.4 GHz DSSS |
| defense-full | SDR FHSS | SDR DSSS |

A CAN and/or UDP transport could be added later behind the existing HAL trait
surface, but they are **not** part of the current design — the routing decision
here is RF channel arbitration, not subject/node routing.

## 5. What this design deliberately is not

- **No pub/sub Subject IDs or dynamic Node Allocation.** Addressing is MAVLink-
  style `sysid`/`compid`; there is no Cyphal-style session manager.
- **No transport-agnostic routing trait over CAN/UDP.** Transports are RF, and
  "routing" means choosing the healthier of two redundant RF channels.
- **Security and anti-jam are not optional add-ons** — they are the core of the
  stack and sit in the main data path, not off to the side.
