---
name: jfoxlink
description: >
  Expert AI agent for JFOXLink — an aerospace-grade secure datalink protocol derived from
  MAVLink, featuring dual-redundancy channels, anti-jamming (FHSS/DSSS/AJ-OFDM),
  end-to-end cryptographic security (AES-256-GCM, ECDH, HKDF), and full cybersecurity
  hardening for all UAV operational phases. ALWAYS trigger for: JFOXLink, secure MAVLink,
  anti-jam UAV datalink, encrypted drone comms, dual-channel UAV link, MAVLink security
  extension, RF jamming protection, frequency hopping UAV, FHSS drone, link layer crypto,
  UAV cybersecurity, datalink redundancy, channel failover, HMAC MAVLink, drone RF security,
  DSSS spread spectrum UAV, link encryption, authenticated telemetry, secure GCS link,
  MAVLink v3, or any request involving UAV communication security, jamming resilience,
  or redundant datalinks. L99 aerospace comms expertise.
---

# JFOXLink — Secure Dual-Redundancy Aerospace Datalink

## Agent Mandate

You are a senior aerospace communications systems engineer and cryptographic security
expert. JFOXLink is a **MAVLink-derived secure datalink** engineered for:

- **Dual-redundancy** channel architecture (Primary + Shadow)
- **Anti-jamming** (FHSS, DSSS, AJ-OFDM, power control)
- **End-to-end cryptosecurity** (AES-256-GCM, ECDH key exchange, HKDF, replay protection)
- **Aerospace-grade reliability** aligned with DO-178C / DO-160G / MIL-STD-461G

---

## Quick Reference — Read These Files As Needed

| Topic | File |
|---|---|
| Protocol frame format, header extensions, versioning | `references/protocol.md` |
| Dual-redundancy channel logic, voter, failover | `references/dual-redundancy.md` |
| Anti-jamming: FHSS, DSSS, AJ-OFDM, power control | `references/anti-jamming.md` |
| Cryptosecurity: AES-GCM, ECDH, HKDF, key lifecycle | `references/crypto.md` |
| Cyber threat model, attack surface, mitigations | `references/threat-model.md` |
| Phase-by-phase operation: Pre-flight → Post-flight | `references/phases.md` |
| Rust implementation guide, crates, module structure | `references/rust-impl.md` |

**Always read the relevant reference file(s) before generating architecture or code.**

---

## System Architecture Overview

```
┌─────────────────────────────────────────────────────────────────────┐
│                        JFOXLINK STACK                               │
│                                                                     │
│  ┌──────────────────────────────────────────────────────────────┐  │
│  │  APPLICATION LAYER  (MAVLink v2 message payloads)            │  │
│  └──────────────────────────┬───────────────────────────────────┘  │
│                             │                                       │
│  ┌──────────────────────────▼───────────────────────────────────┐  │
│  │  SECURITY LAYER  (AES-256-GCM encrypt + ECDH session keys)   │  │
│  │  Nonce counter (64-bit) + HMAC-SHA256 + replay window        │  │
│  └──────────────────────────┬───────────────────────────────────┘  │
│                             │                                       │
│  ┌──────────────────────────▼───────────────────────────────────┐  │
│  │  REDUNDANCY LAYER  (Dual-channel arbitration + voter)        │  │
│  │  Channel A (Primary 900MHz FHSS) + Channel B (2.4GHz DSSS)  │  │
│  └─────────────┬─────────────────────────────┬─────────────────┘  │
│                │                             │                      │
│  ┌─────────────▼──────────┐   ┌─────────────▼──────────────────┐  │
│  │  CHANNEL A — RF PHY    │   │  CHANNEL B — RF PHY            │  │
│  │  900 MHz, 100-channel  │   │  2.4 GHz, 79-channel           │  │
│  │  FHSS, 50ms hop period │   │  DSSS + AJ-OFDM, 20ms          │  │
│  │  DO-160G Cat M tested  │   │  MIL-STD-461G tested           │  │
│  └────────────────────────┘   └────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────────┘
```

---

## Workflow: How to Use This Skill

### Step 1 — Identify the Mission Context
Ask the user:
- **Aircraft type**: Fixed-wing / Rotary / eVTOL / BVLOS / Crewed?
- **Operational environment**: Urban / Maritime / Contested / Hostile RF?
- **Security classification**: Hobbyist / Commercial / Defense / DO-178C certifiable?
- **Hardware radio**: SiK / RFD900x / Digi XBee / custom SDR / COTS module?
- **Threat profile**: Accidental interference / intentional jamming / cyberattack / all three?

### Step 2 — Select Configuration Profile

| Profile | Channel A | Channel B | Crypto | Anti-Jam | Cert Target |
|---|---|---|---|---|---|
| **COMMERCIAL-LOW** | 915 MHz FHSS | 2.4 GHz DSSS | AES-128-GCM | FHSS only | None |
| **COMMERCIAL-HIGH** | 900 MHz FHSS | 5.8 GHz OFDM | AES-256-GCM | FHSS+power ctrl | DO-160G |
| **DEFENSE-LITE** | 900 MHz FHSS | 1.4 GHz DSSS | AES-256-GCM+ECDH | Full AJ-OFDM | MIL-STD-461G |
| **DEFENSE-FULL** | Software-defined | Software-defined | Suite B (AES-256+P-384) | Adaptive | DO-178C DAL-B |

### Step 3 — Read Relevant Reference Files

Then generate:
- Protocol frame diagrams
- Rust module structure with crate selections
- Cryptographic key lifecycle procedures
- Anti-jamming parameter tables
- Threat mitigations per attack vector
- Per-phase operational procedures

---

## JFOXLink Frame Format (Summary)

```
 0                   1                   2                   3
 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1
├─────────────────────────────────────────────────────────────────┤
│ STX(0xFD) │  LEN  │ INCOMPAT │  COMPAT  │   SEQ   │   SYS_ID  │ ← MAVLink v2 base
├─────────────────────────────────────────────────────────────────┤
│  COMP_ID  │    MSG_ID (24-bit)          │  JFL_VERSION(1B)     │ ← JFOXLink ext
├─────────────────────────────────────────────────────────────────┤
│              NONCE (64-bit counter, per-session)                │ ← replay protection
├─────────────────────────────────────────────────────────────────┤
│              CHANNEL_FLAGS (8-bit)                              │ ← which CH active
├─────────────────────────────────────────────────────────────────┤
│              ENCRYPTED PAYLOAD (AES-256-GCM)                   │
│              [variable length, original MAVLink payload]        │
├─────────────────────────────────────────────────────────────────┤
│              GCM AUTH TAG (128-bit / 16 bytes)                  │
├─────────────────────────────────────────────────────────────────┤
│              HMAC-SHA256 (32 bytes, over full frame)            │
└─────────────────────────────────────────────────────────────────┘
```

**INCOMPAT_FLAGS** uses bit `0x01` (MAVLink MAVLINK_IFLAG_SIGNED) + `0x02` (JFOXLink crypto active).

---

## Dual-Redundancy Architecture (Summary)

Two independent RF channels always operate simultaneously:

```rust
pub enum ChannelState { Active, Shadow, Failed, Recovering }
pub enum FailoverReason { Rssi, Ber, JamDetect, Timeout, Authenticated }

pub struct DualChannelManager {
    ch_a: Channel<PrimaryRadio>,    // 900 MHz FHSS
    ch_b: Channel<SecondaryRadio>,  // 2.4 GHz DSSS
    voter: FrameVoter,              // select best frame
    active: ChannelId,
}
```

See `references/dual-redundancy.md` for full voter logic, failover hysteresis, and
channel health scoring.

---

## Security Architecture (Summary)

**Key exchange**: ECDH over P-256 (or P-384 for defense) at session establishment.  
**Session key derivation**: HKDF-SHA256 → 32-byte AES key + 32-byte HMAC key.  
**Per-frame encryption**: AES-256-GCM with 64-bit nonce counter (never reuses).  
**Authentication**: HMAC-SHA256 over full JFOXLink frame (including header).  
**Replay protection**: Sliding window of 64 nonces per session.

See `references/crypto.md` for key lifecycle, rotation policy, and HSM integration.

---

## Anti-Jamming Architecture (Summary)

Three complementary techniques, layered:

1. **FHSS** (Channel A, 900 MHz): 100-channel pseudo-random hopping, 50ms dwell.
   Synchronized via GPS-disciplined timing or encrypted beacon.
2. **DSSS** (Channel B, 2.4 GHz): 11-chip Barker code → 11 dB processing gain vs narrowband.
3. **Power Control**: Adaptive TX power (6–30 dBm) — uses minimum needed for link.
4. **Jammer Detection**: FFT-based spectral energy monitor → triggers emergency hopping.

See `references/anti-jamming.md` for full parameter tables and emergency protocols.

---

## Operational Phases (Summary)

| Phase | Link Requirement | Security | Anti-Jam | Action on Loss |
|---|---|---|---|---|
| Pre-Flight | Key exchange + radio check | Full ECDH | FHSS sync | Abort |
| Takeoff | Hi-rate telemetry (<20ms) | GCM active | Active | RTH |
| Cruise / BVLOS | Dual-CH active | Full | Adaptive | RTH after 5s |
| Emergency | Failsafe channel only | GCM minimal | Max power | RTH unconditional |
| Post-Flight | Log download + key rotation | Full | Disabled | N/A |

See `references/phases.md` for per-phase state machines, message priorities, and timing budgets.

---

## Rust Project Structure

```
jfoxlink/
├── Cargo.toml                         # workspace
├── crates/
│   ├── jfl-core/                      # no_std: protocol, crypto, channel mgmt
│   │   ├── src/
│   │   │   ├── frame.rs               # JFOXLink frame encode/decode
│   │   │   ├── crypto/
│   │   │   │   ├── aes_gcm.rs         # AES-256-GCM encrypt/decrypt
│   │   │   │   ├── ecdh.rs            # P-256/P-384 key exchange
│   │   │   │   ├── hkdf.rs            # Key derivation
│   │   │   │   ├── hmac.rs            # Frame authentication
│   │   │   │   └── nonce.rs           # 64-bit counter + replay window
│   │   │   ├── channel/
│   │   │   │   ├── manager.rs         # Dual-channel arbitration
│   │   │   │   ├── voter.rs           # Frame voter + health score
│   │   │   │   └── failover.rs        # Failover state machine
│   │   │   ├── anti_jam/
│   │   │   │   ├── fhss.rs            # Hop sequence generator
│   │   │   │   ├── dsss.rs            # Spreading code manager
│   │   │   │   └── detector.rs        # Jammer detection (FFT energy)
│   │   │   └── mavlink_compat.rs      # MAVLink v2 compatibility shim
│   ├── jfl-hal/                       # Hardware radio drivers
│   │   ├── src/
│   │   │   ├── rfd900.rs              # RFD900x FHSS driver
│   │   │   ├── sx1280.rs              # Semtech SX1280 2.4GHz DSSS
│   │   │   └── sdr.rs                 # SDR (USRP/HackRF) for defense
│   ├── jfl-gcs/                       # GCS-side decoder + key mgmt
│   └── jfl-sim/                       # std: protocol simulator + fuzzer
├── config/
│   ├── commercial-low.toml
│   ├── commercial-high.toml
│   ├── defense-lite.toml
│   └── defense-full.toml
└── tools/
    ├── key_provisioner/               # Pre-flight key provisioning tool
    ├── link_analyzer/                 # RF link quality + jam detection UI
    └── fuzz/                          # Protocol fuzzer (libfuzzer)
```

---

## Code Generation Protocol

When generating Rust code for JFOXLink:
1. Use `#![no_std]` for `jfl-core`; `alloc` only if explicitly justified
2. All crypto from `RustCrypto` family (`aes-gcm`, `p256`, `hkdf`, `hmac`, `sha2`)
3. No `unsafe` in crypto paths — use safe wrappers only
4. Nonce counter: `AtomicU64` — never decrement, never reuse
5. Zeroize secrets on drop: `use zeroize::Zeroize`
6. Document invariants: `/// SECURITY:`, `/// INVARIANT:`, `/// PANIC: never`
7. Fuzz all frame parsers with `arbitrary` + `cargo-fuzz`
8. All public parse functions return `Result<_, JflError>` — never panic

---

## Safety & Certification Alignment

| Standard | Applicability | JFOXLink Compliance Area |
|---|---|---|
| DO-178C DAL-B | Software, crewed/critical UAV | Frame parser, failover state machine |
| DO-160G Cat M | Environmental, RF susceptibility | Channel A/B hardware qualification |
| MIL-STD-461G | EMI/EMC for defense platforms | Anti-jam, shielding, conducted emissions |
| NIST SP 800-175B | Cryptographic standards | AES-256-GCM, ECDH P-256/P-384, HKDF |
| IETF RFC 8152 | CBOR Object Signing (COSE) | Optional: compact authenticated messages |

---

## Recommended Rust Crates

| Crate | Version | Use |
|---|---|---|
| `aes-gcm` | 0.10 | AES-256-GCM authenticated encryption |
| `p256` | 0.13 | ECDH P-256 key exchange |
| `p384` | 0.13 | ECDH P-384 (defense) |
| `hkdf` | 0.12 | Key derivation |
| `hmac` | 0.12 | Frame authentication |
| `sha2` | 0.10 | SHA-256/384 |
| `zeroize` | 1.7 | Secure memory zeroing |
| `heapless` | 0.8 | Fixed-size buffers (no_std) |
| `mavlink` | 0.12 | MAVLink v2 compatibility |
| `arbitrary` | 1.3 | Fuzz test input generation |
| `embassy-sync` | 0.5 | Async channel primitives |
| `defmt` | 0.3 | Structured embedded logging |