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

cargo run -p jfl-gcs -- --config config/defense-full.toml selftest   # GCS: end-to-end handshake+decode self-test
cargo run -p jfl-gcs -- --config config/commercial-high.toml profile # GCS: print a profile (also: `decode`)
cargo run -p jfl-sim -- --scenario defense-full --jam wide           # simulator scenario runner (clap CLI)
cargo run -p key_provisioner -- handshake                            # ECDH provisioning demo
cargo run -p link_analyzer -- --samples 48                           # RF link-quality dashboard
```

The `tools/fuzz` crate is **detached from the workspace** (it's a `#![no_main]` libFuzzer target), so a normal `cargo build`/`test` skips it. Fuzz it with the nightly `cargo-fuzz` toolchain (package `jfl-fuzz`, target `frame_fuzz`):
```bash
cargo +nightly fuzz run frame_fuzz --fuzz-dir tools/fuzz -- -max_len=512
```

CI (`.github/workflows/ci.yml`) gates on `cargo fmt --check`, clippy, build, and test on stable, plus a non-blocking nightly fuzz job.

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
- **`crates/jfl-hal`** — radio traits in `traits.rs`: `RadioTx`, `RadioRx`, `FrequencyHop`, `PowerControl`, `HwStatus`, and a `SerialLine` transport abstraction, all returning a real `HalError` enum (not `()`). Concrete drivers: `rfd900` (900MHz UART, CRC-16 framing + SiK AT commands), `sx1280` (2.4GHz SPI, real datasheet opcodes), `sdr` (host-only `SdrBackend` interface — no bundled `no_std` backend). Add a driver by implementing the traits in a new `src/*.rs` and exporting it from `lib.rs`. (Docs elsewhere name `DatalinkTx`/`DatalinkRx` — those are stale; the real names are above.)
- **`crates/jfl-gcs`** — `decoder.rs` is the full receive stack (parse → HMAC verify → replay check → AES-GCM decrypt → native payload), wired in via `mod` in `main.rs`; `tx.rs` is the encrypt-then-MAC transmit path; `key_store.rs` is **fail-closed** (real ECDH handshake seeded from `OsRng`, AES-256-GCM encrypted-at-rest under a runtime KEK — never returns placeholder keys); `config.rs` loads the profile TOML.
- **`crates/jfl-sim`** — `channel_emulator.rs` (latency/jitter/loss) + `threat_injector.rs` (jam/replay/spoof) for testing failover and anti-jam logic without hardware.

### Frame format (`jfl-core/src/frame.rs`)
The wire layout is MAVLink-v2-derived with a crypto envelope: 24-byte header + payload + 16-byte GCM tag + 32-byte HMAC. Key invariants when touching this file:
- `from_bytes` takes `&'a [u8]` and returns borrowed data — parser returns `Result<_, JflError>` and **must never panic** (a security invariant, enforced by unit tests, a `proptest`, and the `frame_fuzz` libFuzzer target — not just the type).
- Nonce layout: 4-byte session prefix + 8-byte LE monotonic sequence (`crypto/nonce.rs`); RX replay validation is a sliding-window bitmap in `NonceManager`, TX generation is `NonceGenerator`.
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

Current and code-accurate: `CLAUDE.md`, `PROJECT_REPORT.md`, `PARAMETERS.md`, `ARCHITECTURE.md` (+ rendered `docs/architecture.html`).

Stale — predate the current code, trust `Cargo.toml` and source over prose: `README.MD`, `DEVELOPER.md`, `USER_MANUAL.md`. `README.MD` shows Windows/PowerShell paths (`C:\home\project\...`) that don't apply on this Linux checkout; both README and DEVELOPER reference a `REVISION_CONTROL.md` that does not exist and name HAL traits (`DatalinkTx`/`DatalinkRx`) and crypto features (`crypto_aes`/`crypto_ecdh`) that aren't real. Verify commands against the crate you're actually building.
