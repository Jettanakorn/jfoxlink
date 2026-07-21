# JFOXLink — Project Report

**Date:** 2026-07-21
**Scope:** Full workspace exploration — `crates/`, `tools/`, `config/`, docs
**Repo state:** branch `claude/init-fuybn4`, ~1,478 lines of Rust across 4 crates + 3 tools

---

## 1. Executive summary

JFOXLink is a **secure UAV command-and-control datalink** — a hardened,
crypto-wrapped derivative of MAVLink v2, targeting commercial and defense
aviation. It is organized as a clean Rust Cargo workspace with a `no_std`
protocol core, a hardware-abstraction layer, a ground-control station, a
simulator, and CLI tooling.

**Maturity: early-stage, mixed.** The design and module decomposition are
sound and professionally structured, and the protocol core (`jfl-core`) carries
real, now-tested logic for framing, AES-GCM/HMAC, replay protection, and
dual-channel arbitration. However, a significant fraction of the surrounding
system is **scaffolding**: all radio drivers, the key-provisioning and
link-analyzer tools, the executable entrypoints, the fuzz target, and the
ECDH/HKDF key-derivation and key-storage material are **placeholders or stubs**.

This session hardened the core (fixed panic/lockup paths), **rebuilt replay
protection** into a real sliding-window validator, and **activated the GCS
receive path** (which had never been compiled). Test coverage went from
**0 → 24 tests**, all green.

| Dimension | Status |
|-----------|--------|
| Architecture & module design | 🟢 Strong |
| Protocol core (`jfl-core`) | 🟢 Substantially implemented, now tested |
| Cryptographic wrappers (AES-GCM, HMAC, HKDF) | 🟢 Real (use vetted RustCrypto crates) |
| Key agreement (ECDH) & key storage | 🔴 Placeholder (returns all-zero keys) |
| Hardware abstraction (`jfl-hal`) drivers | 🔴 Comment-only placeholders |
| GCS / simulator / tool executables | 🟠 Libraries real; `main()` entrypoints are stubs |
| Test & verification | 🟡 Core covered (24 tests); fuzz target is a stub; no CI |
| Documentation | 🟡 Extensive but partly stale / ahead of code |

Legend: 🟢 solid · 🟡 partial · 🟠 mixed · 🔴 not implemented

---

## 2. Purpose & domain

JFOXLink provides an authenticated, encrypted, jam-resilient radio link for
UAV telemetry and command. Design pillars (from source and docs):

- **Confidentiality & integrity** — AES-256-GCM encryption + HMAC-SHA-256, with
  an ECDH/HKDF session-key path for defense profiles.
- **Redundancy** — two RF channels (A/B) with health-scored arbitration and
  hysteresis-based failover.
- **Anti-jam** — frequency hopping (FHSS), direct-sequence spread spectrum
  (DSSS/Barker-11), and a spectral jam detector.
- **Interoperability** — transparent wrapping of MAVLink v2 payloads or a
  native JFOXLink payload, behind one crypto envelope.
- **Profiles** — four certification-oriented profiles (`commercial-low/high`,
  `defense-lite/full`) selecting crypto suite, bands, anti-jam strategy,
  replay-window size, and key-rotation interval.

---

## 3. Architecture

### Workspace layout

```
jfoxlink/
├── crates/
│   ├── jfl-core   (no_std)  protocol engine: frame, crypto, channel, anti_jam   [808 LOC]
│   ├── jfl-hal    (no_std)  radio trait abstraction + driver placeholders        [26 LOC]
│   ├── jfl-gcs    (std)     ground station: full decode stack + key store        [304 LOC]
│   └── jfl-sim    (std)     deterministic RF/threat simulator                     [321 LOC]
├── tools/
│   ├── key_provisioner      ECDH provisioning CLI (stub)
│   ├── link_analyzer        RF health dashboard (stub)
│   └── fuzz (jfl-fuzz)       cargo-fuzz frame parser target (stub)
├── config/                  4 profile TOMLs
└── docs: README, DEVELOPER, USER_MANUAL, CHANGELOG, LICENSE, CLAUDE, PARAMETERS
```

### Dependency flow

```
jfl-core (no_std, no heap for core types — heapless::Vec)
  ├─ jfl-hal   → radio traits + drivers
  ├─ jfl-gcs   → receive stack + key storage
  └─ jfl-sim   → channel emulator + threat injector
tools/* → depend on jfl-core
```

Release profile is size-optimized: `opt-level="z"`, `lto=true`,
`codegen-units=1`, `panic="abort"`.

---

## 4. Component status

### 4.1 `jfl-core` — protocol engine 🟢 (with one 🔴 gap)

`#![no_std]`, `#![deny(unsafe_code)]`, `#![warn(clippy::all, clippy::pedantic)]`.
The most complete crate.

| Module | Status | Notes |
|--------|--------|-------|
| `frame.rs` | 🟢 | 24-byte header + payload + 16-byte GCM tag + 32-byte HMAC. Zero-copy borrowed parse; rejects plaintext (crypto-flag) frames. Now panic-tested. |
| `native.rs` / `mavlink_compat.rs` | 🟢 | Frame builders for native and MAVLink payloads. Now reject oversize payloads and propagate capacity errors. |
| `crypto/aes_gcm.rs` | 🟢 | AES-256-GCM in-place detached (RustCrypto `aes-gcm`); key zeroized on drop. |
| `crypto/hmac.rs` | 🟢 | HMAC-SHA-256 compute/verify. |
| `crypto/hkdf.rs` | 🟢 | HKDF-SHA-256 expand; output zeroized. |
| `crypto/nonce.rs` | 🟢 | **Rebuilt this session** — RFC-6479-style sliding-window replay validator + separate 96-bit nonce generator. |
| `crypto/ecdh.rs` | 🔴 | `derive_session_keys` returns `([0;32],[0;32])`; real keygen gated behind an unused `cfg(std)`. **Placeholder.** |
| `channel/{manager,voter,failover}.rs` | 🟢 | Health-scored arbitration + frame voting + hysteresis failover. NaN-safe, overflow-safe after this session's fixes. |
| `anti_jam/fhss.rs` | 🟡 | LFSR hop generator; hardened against zero-state lockup & divide-by-zero. Not cryptographic (documented). |
| `anti_jam/dsss.rs` | 🟡 | Barker-11 spread only (no despread). |
| `anti_jam/detector.rs` | 🟡 | Threshold check over a 64-bin FFT buffer; buffer not populated by core; no hysteresis. |

### 4.2 `jfl-hal` — hardware abstraction 🔴

- `traits.rs` defines **`RadioTx` / `RadioRx`** with `Result<_, ()>` returns.
- `rfd900.rs`, `sx1280.rs`, `sdr.rs` are **comment-only placeholders** — no
  driver logic exists. (Total crate: 26 lines.)
- Note: docs describe richer traits (`DatalinkTx`, `FrequencyHop`,
  `PowerControl`, `HwStatus`) that are **not** in the code.

### 4.3 `jfl-gcs` — ground control station 🟠

- `decoder.rs` 🟢 — full receive stack (parse → HMAC → replay → AES-GCM
  decrypt → native payload). **Activated and fixed this session** (it was
  previously dead code, never compiled). Now covered by 4 end-to-end tests.
- `key_store.rs` 🟠 — thread-safe cache + zeroization structure is real, but
  `load_keys`/`rotate_keys` return **all-zero placeholder key material**.
- `main.rs` 🔴 — prints a banner; no CLI/telemetry loop (`clap` unused).

### 4.4 `jfl-sim` — simulator 🟠 (libraries real)

- `channel_emulator.rs` 🟢 — seeded RNG RF impairments: drop, BER bit-flips,
  Rayleigh-ish fading, log-distance path loss, latency/jitter, state transitions.
- `threat_injector.rs` 🟢 — replay capture/mutation, narrow/wideband jam
  injection into the detector, spoof-frame generation. Has 2 tests.
- `main.rs` 🔴 — banner only; no scenario runner (despite documented CLI flags).

### 4.5 `tools/` 🔴

- `key_provisioner` — banner-only stub.
- `link_analyzer` — banner-only stub.
- `fuzz` (`jfl-fuzz`) — declares an `Arbitrary` struct but the target `main()`
  is an **empty placeholder**; it does not actually drive `JflFrame::from_bytes`.

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

Key invariants:
- Parser (`from_bytes`) borrows input, returns `Result`, and **must never
  panic** (security invariant — now enforced by a dedicated test).
- A frame without the crypto-active bit (`incompat & 0x02`) is rejected —
  plaintext frames are intentionally unsupported.
- Nonce layout (as of this session): 4-byte session prefix + 8-byte LE
  monotonic sequence; the full 12 bytes feed GCM, so tampering breaks the tag.

---

## 6. Security architecture

**Encrypt-then-MAC** on send; on receive: parse → HMAC verify (over header +
ciphertext + GCM tag) → replay check → GCM decrypt. This ordering is correct
(authentication before decryption).

Strengths:
- Uses vetted RustCrypto primitives, not hand-rolled crypto.
- Key material zeroized on drop across engines/stores.
- `#![deny(unsafe_code)]` in the core; size- and panic-hardened release profile.
- Real sliding-window anti-replay (post-rebuild).

Open security gaps (see §8):
- ECDH/HKDF **session-key derivation returns zeros**; key store returns zeros —
  a real build must source keys from ECDH or an HSM before operational use.
- FHSS sequence is a non-cryptographic LFSR (predictable) — must be replaced
  with a keyed/CSPRNG schedule for defense builds.
- No CSPRNG wired for embedded (`no_std`) key generation.

---

## 7. Code quality & verification

- **Tests: 24, all passing** (was 0 at session start): `jfl-core` 18, `jfl-gcs`
  4, `jfl-sim` 2. Coverage now includes the never-panic parser invariant,
  FHSS no-lockup/in-range, failover no-overflow, NaN-safe arbitration, the
  replay validator, and an end-to-end encrypt→decode→replay→tamper path.
- **Build:** whole workspace compiles clean; no clippy errors on core/gcs.
- **Warnings:** `jfl-core` still emits pedantic-lint and dead-code warnings
  (unused placeholder items). Not blocking.
- **No CI configured** — the fuzz target and `cargo fmt/clippy` gates are not
  automated. The fuzz harness itself is a stub.
- **~12 placeholder/TODO markers** remain across the tree.

---

## 8. Findings from this session (weakness analysis)

Ranked; all high-severity robustness items were **fixed and pushed**.

**Fixed 🟢**
1. FHSS LFSR **zero-state lockup** (collapse to one channel) + **divide-by-zero**
   panic when `channel_count == 0`.
2. Failover **`u16` timer overflow** that reset the anti-flap guard.
3. **Silent frame truncation** — serializers ignored capacity errors; `len as u8`
   truncated oversize payloads.
4. **`validate_mav_header` panic** on short input (unchecked indexing).
5. **NaN BER** made a dead channel score as healthy in arbitration/voting.
6. **Replay protection non-functional** — old range-only check rejected all RX
   traffic and let in-window replays pass; **rebuilt** as a sliding-window bitmap.
7. **GCS receive path was dead code** (never compiled); **wired in**, which
   surfaced and fixed a **wrong HMAC verification range** and a wrong nonce offset.

**Open 🔴 (need product/design decisions)**
- Placeholder crypto returning **all-zero keys** (ECDH derive, key store).
- HAL drivers and tool executables are unimplemented.
- HAL trait errors are `Result<_, ()>` (no fault diagnostics).
- Jam detector unit/semantics (raw FFT magnitude vs dBm threshold; no hysteresis).
- Fuzz target is a stub; no CI.

---

## 9. Documentation status

Extensive (`README`, `DEVELOPER`, `USER_MANUAL`, `CHANGELOG`) plus the
`CLAUDE.md` and `PARAMETERS.md` added this session. Known drift to be aware of:

- `DEVELOPER.md` lists `crypto_aes`/`crypto_ecdh` features — actual features are
  profile-based (`hobbyist`/`commercial`/`defense-lite`/`defense-full`,
  default `defense-full`).
- Docs reference HAL traits (`DatalinkTx`, etc.) and a `REVISION_CONTROL.md`
  that don't exist; README shows Windows/PowerShell paths.
- `PARAMETERS.md`'s nonce section predates this session's nonce rewrite and
  needs a refresh.

---

## 10. Risk assessment

| Risk | Severity | Note |
|------|----------|------|
| All-zero placeholder keys used in a real build | **Critical** | Total loss of confidentiality/integrity if the stub path is ever hit operationally. Guard or implement before any field use. |
| No hardware drivers | High | System cannot transmit/receive on real radios yet. |
| Predictable FHSS sequence | High (defense) | Jammer can follow the hop pattern. |
| No CI / stub fuzzing | Medium | Regressions and parser panics can slip in; the "never panic" invariant is only spot-checked. |
| Documentation drift | Low | Misleads integrators; source is authoritative. |

---

## 11. Recommendations (suggested order)

1. **Fail-closed on placeholder keys** — make ECDH/HKDF/key-store return an
   error (or `debug_assert`) rather than zero keys, so a real build cannot
   silently run unauthenticated.
2. **Implement session-key agreement** — wire a real CSPRNG (HAL RNG for
   `no_std`), complete ECDH→HKDF derivation, and back the key store with an OS
   keychain/HSM.
3. **Implement at least one HAL driver end-to-end** (e.g. RFD900 over UART) to
   prove the trait surface, and give the traits a real error enum.
4. **Stand up CI** — `cargo test/clippy/fmt` + a real `frame_fuzz` body on every
   push; the parser's no-panic invariant deserves continuous fuzzing.
5. **Replace the FHSS LFSR** with a keyed/CSPRNG hop schedule for defense builds.
6. **Flesh out the executables** — GCS telemetry loop, simulator scenario
   runner, provisioner CLI — and reconcile the docs with the actual API.

---

## 12. Bottom line

JFOXLink is a **well-architected early-stage prototype** with a genuinely solid,
now-tested protocol core, wrapped in a professional workspace and thorough (if
partly aspirational) documentation. The gap between the documented product and
the implemented system is concentrated in **hardware I/O, key management, and
executables** — all currently placeholders. Closing the key-management gap
(recommendation 1–2) is the highest-value next step, because it is the one
placeholder whose silent failure mode is a security compromise rather than a
missing feature.

---

*Report generated from a full read of the workspace source, config, and git
history. Figures (LOC, test counts) measured from the tree at report time.*
