# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

JFOXLink is a secure UAV radio-datalink protocol suite (a hardened, crypto-wrapped derivative of MAVLink v2). It is a Rust Cargo workspace: an embedded `no_std` protocol core, a hardware abstraction layer, a ground-control-station client, a simulator, and CLI tools. Built by JFOX Aircraft Co., Ltd.

## Commands

```bash
cargo build --release           # build whole workspace (release profile is opt-level="z", lto, panic=abort)
cargo test                      # run all tests
cargo test -p jfl-core          # test one crate
cargo test -p jfl-core frame::  # run a single module's tests (filter by path substring / test name)
cargo clippy --all-targets --all-features
cargo fmt --all --check

cargo run -p jfl-gcs -- --config config/commercial-high.toml   # GCS client (clap CLI)
cargo run -p jfl-sim --                                         # simulator
cargo build -p key_provisioner -p link_analyzer --release      # tools (note: package names, not paths)
```

Fuzzing needs the nightly `cargo-fuzz` toolchain; the package is named `jfl-fuzz`:
```bash
cd tools/fuzz && cargo fuzz run frame_fuzz -- -max_len=256 -timeout=1
```

## Architecture

Dependency flow (everything depends on `jfl-core`):

```
jfl-core (no_std, protocol + crypto + redundancy + anti-jam)
  ├─ jfl-hal   (no_std, radio/SDR trait abstraction + drivers)
  ├─ jfl-gcs   (std, ground station: full decode stack + key storage)
  └─ jfl-sim   (std, deterministic RF/threat simulator)
tools/{key_provisioner, link_analyzer, fuzz}  → also depend on jfl-core
```

- **`crates/jfl-core`** — the protocol engine. `#![no_std]`, `#![deny(unsafe_code)]`. Module groups: `frame`/`mavlink_compat`/`native` (wire format), `crypto/{aes_gcm,ecdh,hkdf,hmac,nonce}`, `channel/{manager,voter,failover}` (dual-channel arbitration), `anti_jam/{fhss,dsss,detector}`. No heap in core types — uses `heapless::Vec`.
- **`crates/jfl-hal`** — radio traits (`DatalinkTx`, `DatalinkRx`, `FrequencyHop`, `PowerControl`, `HwStatus`) with concrete drivers: `rfd900` (900MHz serial), `sx1280` (2.4GHz Semtech), `sdr` (USRP/HackRF). Add a driver by implementing the traits in a new `src/*.rs` and exporting it from `lib.rs`.
- **`crates/jfl-gcs`** — `decoder.rs` is the full receive stack (PHY → nonce/replay check → AES-GCM decrypt → HMAC verify → native payload decode); `key_store.rs` fronts the OS keychain.
- **`crates/jfl-sim`** — `channel_emulator.rs` (latency/jitter/loss) + `threat_injector.rs` (jam/replay/spoof) for testing failover and anti-jam logic without hardware.

### Frame format (`jfl-core/src/frame.rs`)
The wire layout is MAVLink-v2-derived with a crypto envelope: 24-byte header + payload + 16-byte GCM tag + 32-byte HMAC. Key invariants when touching this file:
- `from_bytes` takes `&'a [u8]` and returns borrowed data — parser returns `Result<_, JflError>` and **must never panic** (this is a security invariant, enforced by convention and fuzzing, not just the type).
- A frame with the crypto-active flag (`incompat_flags & 0x02`) unset is rejected as `UnsupportedVersion` — plaintext frames are intentionally not accepted.

## Feature flags (important — docs are stale here)

`jfl-core` features are **profile-based**, not the `crypto_aes`/`crypto_ecdh` flags mentioned in `DEVELOPER.md`. The real ones (`crates/jfl-core/Cargo.toml`):

```
default = ["defense-full"]        # P-384 / Suite B, pulls in the optional p384 dep
hobbyist | commercial | defense-lite | defense-full
```

The four runtime profiles in `config/*.toml` (`commercial-low`, `commercial-high`, `defense-lite`, `defense-full`) select crypto suite, channel bands, anti-jam strategy, replay-window size, and key-rotation interval. Match config changes to the profile table in `DEVELOPER.md`.

## Conventions

- `jfl-core` is `no_std` with no heap for core types. New core modules must compile under `no_std`, return `JflError` on all error paths (never `unwrap`/`panic`), and carry unit tests in a module-footer `#[cfg(test)]` block.
- Workspace-wide: `#![warn(clippy::all, clippy::pedantic)]`; keep protocol-critical paths panic-free.
- Fuzz targets must not be `#![no_std]` (the libFuzzer runner needs `std`).

## Documentation notes

`README.MD`, `DEVELOPER.md`, and `USER_MANUAL.md` are extensive but predate the current code — trust `Cargo.toml` and source over prose. `README.MD` shows Windows/PowerShell paths (`C:\home\project\...`) that don't apply on this Linux checkout, and both README and DEVELOPER reference a `REVISION_CONTROL.md` that does not exist. Verify commands against the crate you're actually building.
