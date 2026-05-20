# JFOXLink Cyber Threat Model

## Threat Model Scope

This threat model covers the JFOXLink datalink stack only — from physical RF layer
through the application message delivery to the GCS or flight controller.
Adjacent systems (GCS application, autopilot firmware, flight controller) have
their own threat models.

**Assets to protect:**
1. Command integrity — no unauthorized flight commands
2. Telemetry confidentiality — position, heading not leaked
3. Link availability — not deniable by jamming
4. Aircraft safety — no commands that could crash the vehicle

---

## Attacker Profile Matrix

| Attacker | Capability | Motivation | Likelihood |
|---|---|---|---|
| **Casual RF interferer** | Unlicensed 2.4 GHz gear | Accidental | HIGH |
| **Script kiddie** | Replay attack tools, SDR | Disruption / theft | MEDIUM |
| **Criminal** | SDR + moderate RF equipment | Drone hijacking, cargo theft | MEDIUM |
| **Nation-state** | Sophisticated EW, follow-jammer | Military/surveillance denial | LOW (defense) |
| **Insider threat** | Access to provisioned hardware | Key exfiltration | LOW |

---

## Attack Surface Map

```
┌─────────────────────────────────────────────────────────────────┐
│  RF ATTACK SURFACE                                              │
│  • Jamming (signal denial)                                      │
│  • Eavesdropping (confidentiality)                              │
│  • Replay attacks                                               │
│  • Frame injection (command spoofing)                           │
│  • Follow-jamming (defeat FHSS)                                 │
│  • GPS spoofing (defeat FHSS sync)                              │
├─────────────────────────────────────────────────────────────────┤
│  PROTOCOL ATTACK SURFACE                                        │
│  • Malformed frame parsing (buffer overflow, panic)             │
│  • Nonce rollback / reset attacks                               │
│  • Key renegotiation downgrade                                  │
│  • Sequence number wrap exploitation                            │
│  • CHANNEL_FLAGS manipulation                                   │
├─────────────────────────────────────────────────────────────────┤
│  CRYPTOGRAPHIC ATTACK SURFACE                                   │
│  • AES-GCM nonce reuse → catastrophic key recovery             │
│  • HMAC timing side-channel                                     │
│  • ECDH small-subgroup attack (twist-security needed)           │
│  • PSK exfiltration from hardware                               │
├─────────────────────────────────────────────────────────────────┤
│  SUPPLY CHAIN ATTACK SURFACE                                    │
│  • Malicious firmware on radio module                           │
│  • Cloned/counterfeit radio hardware                            │
│  • PSK interception during provisioning                         │
└─────────────────────────────────────────────────────────────────┘
```

---

## Attack-by-Attack Analysis

### RF-1: Signal Jamming (Denial of Service)
- **Threat**: Attacker transmits noise to block the datalink
- **Impact**: Loss of command/control → crash or uncontrolled flight
- **Mitigation**:
  - FHSS on Ch A: jammer must cover all 100 channels simultaneously
  - DSSS on Ch B: 10.4 dB processing gain against narrowband
  - Dual-channel: attacker must jam two independent bands
  - Failsafe RTH: aircraft returns home autonomously after T_failsafe
- **Residual risk**: LOW (requires >50W broadband jammer at close range)

### RF-2: Eavesdropping (Confidentiality Breach)
- **Threat**: Passive RF capture of telemetry data
- **Impact**: Position, route, payload information leaked
- **Mitigation**:
  - AES-256-GCM encrypts all payload — ciphertext only on air
  - FHSS and DSSS make capture harder without synchronized receiver
  - Frequency diversity — attacker needs equipment on both bands
- **Residual risk**: NEGLIGIBLE (AES-256 not practically breakable)

### RF-3: Frame Injection (Command Spoofing)
- **Threat**: Attacker injects fake MAVLink commands (e.g., COMMAND_LONG disarm)
- **Impact**: Direct safety impact — crash, loss, theft
- **Mitigation**:
  - HMAC-SHA256 over full frame — requires knowledge of HMAC_KEY
  - AES-256-GCM auth tag — any plaintext manipulation detected
  - PSK-derived keys — unknown to attacker without hardware access
- **Residual risk**: NEGLIGIBLE against outsider; LOW for insider with hardware

### RF-4: Replay Attack
- **Threat**: Capture a valid DISARM or ARM command; replay it later
- **Impact**: Dangerous timing attacks (replay ARM at wrong time)
- **Mitigation**:
  - 64-bit monotonic nonce per session
  - 64-nonce sliding replay window
  - Session bound — replayed frames from previous sessions rejected
- **Residual risk**: NEGLIGIBLE

### RF-5: GPS Spoofing → FHSS Desynchronization
- **Threat**: Fake GPS signal disrupts GPS-disciplined hop timing
- **Impact**: Hop desync → temporary link loss (1–3 hop periods)
- **Mitigation**:
  - Link-disciplined fallback mode (beacon on Ch B)
  - IMU-based GPS anomaly detection (sudden position jump)
  - Hop-key can be re-synced over authenticated Ch B beacon
- **Residual risk**: LOW (3–150ms link interruption; failsafe handles it)

### PROTO-1: Malformed Frame Parsing
- **Threat**: Malformed frame triggers buffer overflow or panic in parser
- **Impact**: Firmware crash → loss of vehicle
- **Mitigation**:
  - All parsers return `Result<_, JflError>` — no panics
  - Fuzz testing with `cargo-fuzz` + `libfuzzer`
  - Bounds-checked slice indexing (Rust default)
  - Frame length validated before any field access
- **Residual risk**: VERY LOW (Rust memory safety + fuzzing)

### PROTO-2: Cryptographic Downgrade
- **Threat**: MITM strips INCOMPAT flags to force plaintext mode
- **Impact**: Exposed plaintext — eavesdropping and injection
- **Mitigation**:
  - JFOXLink **never operates** without crypto if negotiated at session start
  - INCOMPAT=0 frames rejected entirely post-session-establishment
  - Capability negotiation is itself HMAC-authenticated with PSK
- **Residual risk**: NEGLIGIBLE

### CRYPTO-1: AES-GCM Nonce Reuse
- **Threat**: Two frames encrypted with same (key, nonce) → key recovery
- **Impact**: Full session key recovery → all past frames decrypted
- **Mitigation**:
  - `AtomicU64` monotonic counter — system-wide invariant
  - Independent per-direction counters (no collision possible)
  - Session reset = new ECDH keys → new nonce space
- **Residual risk**: NEGLIGIBLE (architectural guarantee, not just code)

### CRYPTO-2: Timing Side-Channel on HMAC Verify
- **Threat**: Measure HMAC comparison time → recover key bits
- **Impact**: HMAC key recovery → frame forgery
- **Mitigation**:
  - `subtle::ConstantTimeEq` for all HMAC comparisons
  - Rust `hmac` crate uses constant-time internally
- **Residual risk**: NEGLIGIBLE on embedded MCU without precision timing

### SUPPLY-1: Malicious Radio Firmware
- **Threat**: Compromised radio module firmware exfiltrates keys
- **Impact**: Session key compromise → full link compromise
- **Mitigation**:
  - Attestation: verify radio firmware hash at startup
  - Keys derived in MCU, not in radio — radio sees only ciphertext
  - Use modules with secure boot (Semtech SX1280 + custom firmware)
- **Residual risk**: LOW (depends on hardware selection)

---

## Risk Register Summary

| Attack | Likelihood | Impact | Inherent Risk | Residual Risk |
|---|---|---|---|---|
| Jamming | HIGH | HIGH | HIGH | LOW |
| Eavesdropping | MEDIUM | MEDIUM | MEDIUM | NEGLIGIBLE |
| Frame injection | LOW | CRITICAL | HIGH | NEGLIGIBLE |
| Replay | LOW | HIGH | MEDIUM | NEGLIGIBLE |
| GPS spoof | LOW | LOW | LOW | LOW |
| Frame parser crash | LOW | HIGH | MEDIUM | VERY LOW |
| Crypto downgrade | LOW | CRITICAL | HIGH | NEGLIGIBLE |
| Nonce reuse | VERY LOW | CRITICAL | HIGH | NEGLIGIBLE |
| Radio firmware | VERY LOW | HIGH | MEDIUM | LOW |

**Overall residual risk**: LOW — acceptable for commercial and most defense applications.
For DAL-A certified systems, additionally require DO-178C Level B software processes.

---

## Penetration Testing Checklist

Prior to first flight of a new JFOXLink deployment:

- [ ] Replay captured ARM command → confirm rejection
- [ ] Inject frame with wrong HMAC → confirm dropped silently
- [ ] Send frame with nonce = 0 (replay of session start) → confirm rejected
- [ ] Flip CHANNEL_FLAGS bit in transit → confirm HMAC catches it
- [ ] Send 10,000 malformed frames (fuzz harness output) → confirm no panic
- [ ] Simulate GPS PPS loss for 30 seconds → confirm beacon fallback
- [ ] Jam Ch A with spot jammer → confirm failover to Ch B < 500ms
- [ ] Attempt INCOMPAT=0 frame post-session-start → confirm rejected
- [ ] Zeroize check: power-cycle after session → confirm keys not in RAM/flash