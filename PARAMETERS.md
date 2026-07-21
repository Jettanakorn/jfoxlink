# JFOXLink — Parameter Reference

This document lists every configurable parameter in JFOXLink, split into two
audiences:

- **[User Parameters](#user-parameters)** — the keys you set in a `config/*.toml`
  profile to operate a link. No code changes required.
- **[Developer Parameters](#developer-parameters)** — the constants, struct
  fields, and constructor arguments in `jfl-core` that govern protocol behavior.
  Changing these means editing and rebuilding the crate.

Values below are taken from the actual `config/` files and `crates/jfl-core`
source. Where the four shipped profiles disagree, each column shows that
profile's own value; `—` means the key is not present in that profile's file
and the code default applies.

---

## User Parameters

These live under the `[profile]` table of a `config/*.toml` file and are loaded
by `jfl-gcs` / `jfl-sim` at startup (`--config <file>`).

### Schema

| Key | Type | Unit | Required | Description |
|-----|------|------|----------|-------------|
| `name` | string | — | yes | Human-readable profile label (upper-case by convention). |
| `crypto_suite` | string (enum) | — | yes | Cipher + key-exchange + KDF suite. See values below. |
| `channel_a` | string | — | yes | Primary channel: RF band + spreading technique. |
| `channel_b` | string | — | yes | Secondary (redundant) channel: RF band + spreading technique. |
| `anti_jam` | string | — | yes | Anti-jam strategy stack applied to the link. |
| `cert_target` | string | — | no | Certification target this profile is tuned for (`None` if uncertified). |
| `replay_window` | integer | frames | no | Size of the sliding nonce window for replay rejection. Larger = more memory, more tolerance for reordering. |
| `key_rotation_s` | integer | seconds | no | Session-key rotation interval. Shorter = higher forward secrecy, more re-keying overhead. |
| `jam_threshold_dbm` | integer | dBm | no | Spectral-energy level above which the jamming detector trips and triggers anti-jam response. |

### Values per shipped profile

Taken verbatim from `config/*.toml`:

| Parameter | `commercial-low` | `commercial-high` | `defense-lite` | `defense-full` |
|-----------|------------------|-------------------|----------------|----------------|
| `name` | `COMMERCIAL-LOW` | `COMMERCIAL-HIGH` | `DEFENSE-LITE` | `DEFENSE-FULL` |
| `crypto_suite` | `AES-128-GCM` | `AES-256-GCM` | `AES-256-GCM+ECDH-P-256` | `SuiteB-AES256-ECDH-P384-HKDF` |
| `channel_a` | `915MHz-FHSS` | `900MHz-FHSS` | `900MHz-FHSS` | `SDR-FHSS` |
| `channel_b` | `2.4GHz-DSSS` | `5.8GHz-OFDM` | `1.4GHz-DSSS` | `SDR-DSSS` |
| `anti_jam` | `FHSS` | `FHSS+PowerCtrl` | `AJ-OFDM+FHSS` | `Adaptive+PowerCtrl+Nulling` |
| `cert_target` | `None` | `DO-160G` | `MIL-STD-461G` | `DO-178C-DAL-B` |
| `replay_window` | — | — | — | `64` |
| `key_rotation_s` | — | — | — | `3600` |
| `jam_threshold_dbm` | — | — | — | `-85` |

> **Note:** Only `config/defense-full.toml` currently defines the three numeric
> tunables (`replay_window`, `key_rotation_s`, `jam_threshold_dbm`). The other
> profiles omit them and fall back to code defaults. Recommended per-profile
> values (from `USER_MANUAL.md`) are: replay window 32 / 64 / 128 / 256 and key
> rotation none / 7200 / 3600 / 1800 seconds, in ascending threat order — add
> them to the respective TOML file if you need those settings enforced.

### Accepted `crypto_suite` values

| Value | Cipher | Key exchange | KDF | Notes |
|-------|--------|--------------|-----|-------|
| `AES-128-GCM` | AES-128-GCM | pre-shared | — | Lowest overhead; no forward secrecy. |
| `AES-256-GCM` | AES-256-GCM | pre-shared | — | Commercial-grade confidentiality. |
| `AES-256-GCM+ECDH-P-256` | AES-256-GCM | ECDH P-256 | HKDF | Ephemeral session keys. |
| `SuiteB-AES256-ECDH-P384-HKDF` | AES-256-GCM | ECDH P-384 | HKDF-SHA-384 | NSA Suite B; requires the `defense-full` build feature (pulls in `p384`). |

---

## Developer Parameters

These are compile-time constants and runtime tunables defined in
`crates/jfl-core`. They are not read from config — change the source and rebuild.

### Frame layout constants — `frame.rs`

| Constant | Type | Value | Purpose |
|----------|------|-------|---------|
| `JFL_STX` | `u8` | `0xFD` | Start-of-frame marker (MAVLink v2 STX). |
| `JFL_HEADER_LEN` | `usize` | `24` | Fixed header length in bytes. |
| `JFL_GCM_TAG_LEN` | `usize` | `16` | AES-GCM authentication tag length. |
| `JFL_HMAC_LEN` | `usize` | `32` | Trailing HMAC length (SHA-256). |
| `JFL_CRYPTO_FLAG` | `u8` | `0x03` | Bitmask: `0x01` = MAVLINK_IFLAG_SIGNED, `0x02` = JFOX_CRYPTO_ACTIVE. `from_bytes` rejects any frame with the `0x02` bit clear. |

Minimum valid frame size is `JFL_HEADER_LEN + JFL_GCM_TAG_LEN + JFL_HMAC_LEN`
(= 72 bytes) plus payload.

### Anti-jam — `anti_jam/`

| Parameter | Where | Type | Default / Value | Purpose |
|-----------|-------|------|-----------------|---------|
| `seed` | `FhssEngine::new` | `u32` | caller-supplied | LFSR seed for the hop sequence. **Replace the LFSR with a CSPRNG for defense builds** (noted in source). |
| `channel_count` | `FhssEngine::new` (`channels`) | `u32` | caller-supplied (design target: 100) | Number of hop channels; `next_channel()` returns `hop % channel_count`. |
| `BARKER_11` | `dsss.rs` | `[i8; 11]` | `[1,1,1,-1,-1,-1,1,-1,-1,1,-1]` | 11-chip Barker spreading code; ~10.4 dB processing gain. |
| `threshold_dbm` | `JamDetector` | `i8` | caller-supplied (profile `jam_threshold_dbm`, e.g. `-85`) | Spectral energy level that trips `evaluate()`. |
| `spectral_energy` | `JamDetector` | `[i16; 64]` | runtime | 64-bin FFT energy buffer scanned by the detector. |

### Dual-channel arbitration — `channel/`

| Parameter | Where | Type | Default / Value | Purpose |
|-----------|-------|------|-----------------|---------|
| `min_hold_ms` | `FailoverFSM::new` (`hold_ms`) | `u16` | caller-supplied | Minimum dwell before a channel switch is allowed; prevents flapping (design: ≥ 2 hop periods). |
| `hysteresis` | `DualChannelManager` | `u8` | starts `0`, capped at `3` | Counts consecutive inconclusive arbitrations; holds the active channel until a decisive score gap. |
| arbitration switch gap | `DualChannelManager::arbitrate` | `i32` | `15` | A channel switches only when the score difference `|sa - sb|` exceeds this. |
| health score weights | `DualChannelManager::arbitrate` | — | `rssi - ber*1000 - jam_prob*5 - latency_us/100` | Relative weighting of the four health metrics in channel scoring. |
| `rssi_dbm` | `ChannelHealth` | `i8` | runtime | Received signal strength, dBm. |
| `ber` | `ChannelHealth` | `f32` | runtime | Bit error rate (fraction). |
| `jam_prob` | `ChannelHealth` | `u8` | runtime (0–255) | Jamming-probability estimate. |
| `latency_us` | `ChannelHealth` | `u16` | runtime | One-way latency, microseconds. |

### Replay / nonce — `crypto/nonce.rs`

| Parameter | Where | Type | Default / Value | Purpose |
|-----------|-------|------|-----------------|---------|
| `window` | `NonceManager::new` (`window_size`) | `u64` | caller-supplied (profile `replay_window`) | Sliding acceptance window. `verify_nonce` rejects any nonce that is in the future or older than `current - window`. |
| `counter` | `NonceManager` | `AtomicU64` | starts `0` | Monotonic nonce counter; never reused. |

### Build features — `crates/jfl-core/Cargo.toml`

| Feature | Default | Effect |
|---------|---------|--------|
| `defense-full` | **on** | Full Suite B; enables the optional `p384` dependency (ECDH P-384). |
| `defense-lite` | off | Defense profile without P-384. |
| `commercial` | off | Commercial profile feature gate. |
| `hobbyist` | off | Minimal / lowest-overhead feature gate. |

> The `default` feature is `defense-full`. Build a lighter configuration with
> `cargo build -p jfl-core --no-default-features --features <profile>`.

---

*Grounded in `config/*.toml` and `crates/jfl-core` source as of this revision.
If you change a default in code, update the corresponding row here.*
