# JFOXLink — Project Report

**Date:** 2026-08-16 (updated)
**Scope:** Full workspace — `crates/`, `tools/`, `config/`, docs
**Repo state:** branch `claude/init-fuybn4`, ~3,612 lines of Rust across 4 crates + 3 tools, 49 tests

> This report reflects the codebase **after** the four production-hardening
> tracks (crypto & key management, HAL drivers, executables, CI/fuzzing/tests).
> For the earlier baseline see the git history around commit `bbf7ebc`.

---

## 1. Executive summary

JFOXLink is a **secure UAV command-and-control datalink** — a hardened,
crypto-wrapped derivative of MAVLink v2, targeting commercial and defense
aviation. It is a clean Rust Cargo workspace: a `no_std` protocol core, a
hardware-abstraction layer with real radio drivers, a ground-control station,
a deterministic simulator, and CLI tooling.

**Maturity: real, tested software; not yet hardware-validated or certified.**
The protocol core, cryptography (including real ECDH→HKDF key agreement), radio
drivers, key storage, and all executables are implemented and covered by tests
(0 → 49). What remains out of reach is not *software placeholders* but
*external gates*: validation against physical radios, HSM hardware, and
safety/RF certification.

| Dimension | Status |
|-----------|--------|
| Architecture & module design | 🟢 Strong |
| Protocol core (`jfl-core`) | 🟢 Implemented + tested |
| Crypto wrappers (AES-GCM, HMAC, HKDF) | 🟢 Real (vetted RustCrypto crates) |
| Key agreement (ECDH) & key storage | 🟢 Real ECDH→HKDF; fail-closed store, encrypted-at-rest |
| Hardware abstraction (`jfl-hal`) drivers | 🟢 RFD900 + SX1280 implemented & tested; SDR = host interface |
| GCS / simulator / tool executables | 🟢 Runnable programs (no stubs) |
| Test & verification | 🟢 49 tests; real libFuzzer target; CI pipeline |
| Documentation | 🟢 Refreshed to match code |
| Physical-radio / HSM / certification | 🔴 Out of scope for software (external gates) |

Legend: 🟢 solid · 🟡 partial · 🟠 mixed · 🔴 not addressable in software here

---

## 2. Purpose & domain

An authenticated, encrypted, jam-resilient radio link for UAV telemetry and
command. Design pillars:

- **Confidentiality & integrity** — AES-256-GCM + HMAC-SHA-256, with a real
  ECDH/HKDF session-key path.
- **Redundancy** — two RF channels (A/B) with health-scored arbitration and
  hysteresis-based failover.
- **Anti-jam** — FHSS, DSSS (Barker-11), and a spectral jam detector.
- **Interoperability** — transparent wrapping of MAVLink v2 or native JFOXLink
  payloads behind one crypto envelope.
- **Profiles** — four certification-oriented profiles selecting crypto suite,
  bands, anti-jam strategy, replay window, and key-rotation interval.

See `ARCHITECTURE.md` for the layered diagram and `docs/architecture.html` for a
rendered version.

---

## 3. Architecture

```
jfoxlink/
├── crates/
│   ├── jfl-core   (no_std)  protocol engine: frame, crypto, channel, anti_jam   [~1160 LOC]
│   ├── jfl-hal    (no_std)  radio traits + RFD900/SX1280 drivers + SDR iface     [~720 LOC]
│   ├── jfl-gcs    (std)     ground station: decode stack, key store, tx, CLI     [~890 LOC]
│   └── jfl-sim    (std)     deterministic RF/threat simulator + scenario runner  [~600 LOC]
├── tools/
│   ├── key_provisioner      ECDH provisioning CLI (real)
│   ├── link_analyzer        RF link-quality dashboard (real)
│   └── fuzz (jfl-fuzz)       libFuzzer frame-parser target (real; detached crate)
├── config/                  4 profile TOMLs
├── .github/workflows/ci.yml CI: fmt · clippy · build · test (+ nightly fuzz)
└── docs: README, DEVELOPER, USER_MANUAL, CHANGELOG, LICENSE, CLAUDE,
          PARAMETERS, ARCHITECTURE, docs/architecture.html
```

Dependency flow: everything depends on `jfl-core`. Release profile is
size-optimized (`opt-level="z"`, `lto`, `codegen-units=1`, `panic="abort"`).

---

## 4. Component status

### 4.1 `jfl-core` — protocol engine 🟢

`#![no_std]`, `#![deny(unsafe_code)]`, `#![warn(clippy::all, clippy::pedantic)]`.

| Module | Status | Notes |
|--------|--------|-------|
| `frame.rs` | 🟢 | 24B header + payload + 16B GCM tag + 32B HMAC. Zero-copy parse; rejects plaintext frames. Never-panic invariant covered by unit + property tests. |
| `native.rs` / `mavlink_compat.rs` | 🟢 | Frame builders; reject oversize payloads, propagate capacity errors. |
| `crypto/aes_gcm.rs` | 🟢 | AES-256-GCM in-place detached; key zeroized on drop. |
| `crypto/hmac.rs` | 🟢 | HMAC-SHA-256 compute/verify. |
| `crypto/hkdf.rs` | 🟢 | HKDF-SHA-256 expand; output zeroized. |
| `crypto/ecdh.rs` | 🟢 | **Real** ephemeral ECDH (P-384/P-256) → HKDF `SessionKeys`; caller-injected CSPRNG; rejects malformed peer keys. |
| `crypto/nonce.rs` | 🟢 | `NonceGenerator` (96-bit nonce, exhaustion-safe) + `NonceManager` (RFC-6479 sliding-window bitmap). |
| `channel/{manager,voter,failover}.rs` | 🟢 | Health-scored arbitration, frame voting, hysteresis failover. NaN-safe scoring, saturating timer. |
| `anti_jam/fhss.rs` | 🟡 | LFSR hop generator; hardened against zero-state lockup & divide-by-zero. **Deterministic — not cryptographic** (documented; needs a keyed schedule for defense). |
| `anti_jam/dsss.rs` | 🟡 | Barker-11 spread only (no despread). |
| `anti_jam/detector.rs` | 🟡 | Threshold over a 64-bin FFT buffer; buffer populated by the sim/host, not core; no hysteresis (crude dBm/energy scaling). |

### 4.2 `jfl-hal` — hardware abstraction 🟢

- `traits.rs` — real `HalError` enum + `RadioTx`/`RadioRx`/`FrequencyHop`/`PowerControl`/`HwStatus` + a `SerialLine` transport abstraction.
- `rfd900.rs` 🟢 — RFD900x/SiK driver: CRC-16 framing (send/receive, corruption rejection) + SiK AT commands; unit-tested via in-memory loopback.
- `sx1280.rs` 🟢 — SX1280 SPI driver: real datasheet opcodes + PLL/power encodings; tested against a mock SPI bus.
- `sdr.rs` 🟡 — honest **host-only** `SdrBackend` interface (concrete USRP/HackRF backends need host SDKs, not `no_std`).

### 4.3 `jfl-gcs` — ground control station 🟢

- `decoder.rs` — full receive stack (parse → HMAC → replay → AES-GCM decrypt), wired into the binary, 4 end-to-end tests.
- `key_store.rs` — **fail-closed**; real two-step ECDH handshake (OsRng), AES-256-GCM encrypted-at-rest under a runtime KEK. No placeholder keys.
- `tx.rs` — transmit path (encrypt-then-MAC).
- `config.rs` — profile TOML loader.
- `main.rs` — clap CLI: `profile`, `selftest` (end-to-end, prints PASSED), `decode`.

### 4.4 `jfl-sim` — simulator 🟢

- `channel_emulator.rs` — seeded RF impairments (drop, BER, fading, path loss, latency/jitter).
- `threat_injector.rs` — replay/jam/spoof injection.
- `main.rs` — deterministic scenario runner with delivery/drop/failover/jam stats.

### 4.5 `tools/` 🟢

- `key_provisioner` — `pubkey` / `handshake` over real ECDH.
- `link_analyzer` — RSSI link-quality dashboard (stats, loss estimate, sparkline).
- `fuzz` (`jfl-fuzz`) — real `fuzz_target!` driving `JflFrame::from_bytes`; detached from the workspace, built via `cargo fuzz`.

---

## 5. Wire protocol

MAVLink-v2-derived frame with a crypto envelope:

```
┌───────────────── 24-byte header ─────────────────┐
 STX(0xFD) LEN INCOMPAT COMPAT SEQ SYSID COMPID
 MSGID[3] JFL_VER NONCE[12] CH_FLAGS
├──────────────────────────────────────────────────┤
 encrypted_payload (LEN bytes)
 GCM_TAG[16]
 HMAC[32]
└──────────────────────────────────────────────────┘
```

- `from_bytes` borrows input, returns `Result`, and **never panics** (enforced
  by unit, property, and fuzz tests).
- A frame without the crypto-active bit (`incompat & 0x02`) is rejected.
- Nonce layout: 4-byte session prefix + 8-byte LE monotonic sequence; the full
  12 bytes feed GCM, so tampering breaks the tag.

---

## 6. Security architecture

**Encrypt-then-MAC** on send; on receive: parse → HMAC verify (header +
ciphertext + GCM tag) → replay check → GCM decrypt — authentication before
decryption, fail-closed at every step.

Strengths:
- Vetted RustCrypto primitives; no hand-rolled crypto.
- Real ephemeral ECDH→HKDF session keys; **fail-closed** key store (no all-zero
  keys); secrets zeroized on drop.
- Sliding-window anti-replay with a seen-set bitmap.
- `#![deny(unsafe_code)]` in the core; never-panic parser.

Residual security notes:
- **FHSS hop sequence is a deterministic LFSR** (predictable) — replace with a
  keyed/CSPRNG schedule for defense builds.
- **At-rest KEK** must be supplied from an OS keychain / HSM by the integrator;
  the store provides the encrypted-blob mechanism, not the hardware root.
- **Jam detector** semantics are coarse (energy/dBm scaling, no hysteresis).

---

## 7. Code quality & verification

- **Tests: 49, all passing** — `jfl-core` 23 (incl. property tests), `jfl-gcs` 9,
  `jfl-hal` 11, `jfl-sim` 6. Coverage spans the never-panic parser, ECDH
  agreement, sliding-window replay, NaN-safe arbitration, failover overflow,
  FHSS lockup, driver framing/opcodes, key-store handshake + persistence, and an
  end-to-end encrypt→decode→replay→tamper path.
- **CI** (`.github/workflows/ci.yml`): fmt-check, clippy, build, test on stable;
  a non-blocking nightly `cargo-fuzz` job.
- **Fuzzing:** real libFuzzer target driving the parser (was a stub).
- **Build:** whole workspace builds clean; `cargo fmt --all --check` clean.
- Clippy `pedantic`/`nursery` remain advisory (the crates opt into `warn`, not
  `deny`); some future-API methods are exercised only by tests.

---

## 8. Remaining gaps (post-hardening)

The Critical/High software items from the original report are resolved. What
remains is either an external gate or a documented residual:

| Item | Kind | Note |
|------|------|------|
| Physical-radio validation (RFD900/SX1280/SDR in the loop) | External | Drivers are unit-tested against mocks; not run on hardware here. |
| HSM hardware & key ceremony | External | Store consumes a runtime KEK; hardware root is the integrator's. |
| DO-178C / MIL-STD certification, formal verification, RF/regulatory testing | External | Cannot be produced in software. |
| Cryptographic FHSS schedule | Residual (software) | LFSR hardened but still deterministic. |
| Jam-detector calibration/hysteresis | Residual (software) | Coarse energy model. |
| SDR concrete backend | Residual (host) | Interface defined; backend is host-side. |

---

## 9. Risk assessment

| Risk | Severity | Note |
|------|----------|------|
| Deployed without hardware/RF validation | **High** | Software is tested but unproven on real radios; do not fly on this basis. |
| No certification evidence | **High** (for regulated use) | DO-178C/MIL-STD not started. |
| Predictable FHSS sequence | Medium (defense) | Replace with a keyed schedule before high-threat use. |
| KEK sourcing left to integrator | Medium | A weak/persisted KEK undermines at-rest protection. |
| Documentation drift | Low | Docs refreshed this session; keep in sync on change. |

---

## 10. Recommendations (suggested order)

1. **Hardware bring-up** — run RFD900 and SX1280 drivers against real modules;
   validate framing, timing, and RSSI on-air.
2. **Keyed FHSS schedule** — derive the hop sequence from session key material
   (CSPRNG/keyed) for defense profiles.
3. **KEK integration** — wire the key store to a real OS keychain / PKCS#11 HSM.
4. **Jam-detector calibration** — define units and add hysteresis/dwell.
5. **SDR backend** — implement a host-side `SdrBackend` (SoapySDR/UHD).
6. **Certification track** — if targeting regulated use, begin DO-178C / MIL-STD
   evidence and an independent security audit.

---

## 11. Bottom line

JFOXLink is now a **real, tested software implementation** of a secure RF
datalink — not a scaffold. The protocol core, cryptography (with genuine ECDH
key agreement), radio drivers, key storage, executables, fuzzing, and CI are all
in place and green. The remaining work is dominated by **hardware validation and
certification** — external gates that no amount of code can substitute for — plus
a few documented software residuals (keyed FHSS, jam-detector calibration, a
concrete SDR backend). It should not be flown or fielded until the hardware and
certification gates are met.

---

*Figures (LOC, test counts) measured from the tree at report time.*
