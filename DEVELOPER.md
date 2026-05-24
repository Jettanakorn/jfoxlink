# JFOXLink 1.0 — Developer Manual

## Overview

JFOXLink is a high-assurance secure radio protocol suite designed for mission-critical applications in commercial and defense domains. This manual covers architecture, crate design, development workflows, and contribution guidelines.

## Architecture

### Core Principles

1. **No-panic guarantee**: `jfl-core` uses `#![deny(unsafe_code)]` and all parsing returns `Result<_, JflError>`.
2. **Profile-driven configuration**: Four pre-configured profiles (commercial-low/high, defense-lite/full) match certification targets.
3. **Dual-channel redundancy**: Automatic failover with health scoring and latency-aware voting.
4. **Modular crypto**: Feature-gated AES-GCM, ECDH, HKDF, HMAC modules supporting Suite B (P-384).
5. **Anti-jam resilience**: FHSS (100-channel), DSSS (Barker-11), and adaptive detection.
6. **MAVLink interop**: Transparent wrapping of MAVLink v2 payloads with cryptographic envelope.

### Crate Dependency Graph

```
┌─────────────────────────────────────────────────────────────┐
│ jfl-core (no_std, crypto, protocol, redundancy)             │
└────────────────────────────────────────────────────────────┘
    ↑
    ├─ jfl-hal (no_std, hw abstraction)
    │   ├─ rfd900 (900MHz FHSS driver)
    │   ├─ sx1280 (2.4GHz DSSS/AJ-OFDM driver)
    │   └─ sdr (USRP/HackRF backend for defense)
    │
    ├─ jfl-gcs (std, ground control)
    │   ├─ decoder (PHY → Crypto → MAVLink)
    │   └─ key_store (keychain/HSM interface)
    │
    └─ jfl-sim (std, simulator)
        ├─ channel_emulator (RF impairments)
        └─ threat_injector (jam/spoof simulation)
```

## Crate Details

### `crates/jfl-core`

**Purpose**: Core protocol engine, zero-dependency, suitable for embedded platforms.

**Key modules**:
- `frame.rs` — JFOXLink frame parser/serializer with zero-copy design
- `crypto/{aes_gcm, ecdh, hkdf, hmac, nonce}.rs` — Cryptographic operations
- `channel/{manager, voter, failover}.rs` — Dual-channel arbitration
- `anti_jam/{fhss, dsss, detector}.rs` — Frequency hopping and jamming detection
- `mavlink_compat.rs` — MAVLink v2 payload wrapping

**Features**:
- `default`: All crypto modules enabled
- `crypto_aes`: AES-GCM support (enabled by default)
- `crypto_ecdh`: ECDH key exchange (enabled by default)

**Build**:
```bash
cargo build -p jfl-core --release
```

**Tests**:
```bash
cargo test -p jfl-core
```

### `crates/jfl-hal`

**Purpose**: Hardware abstraction layer for radio transceivers and SDRs.

**Traits**:
- `DatalinkTx` / `DatalinkRx` — transmit/receive operations
- `FrequencyHop` — frequency-hopping control
- `PowerControl` — transmit power adjustment
- `HwStatus` — health monitoring (RSSI, BER, temperature)

**Drivers**:
- `rfd900.rs` — RFD900x and SiK firmware via serial UART
- `sx1280.rs` — Semtech SX1280 LoRa/FHSS transceiver
- `sdr.rs` — USRP/HackRF via gr-osmosdr (defense profile only)

**Build**:
```bash
cargo build -p jfl-hal --release
```

### `crates/jfl-gcs`

**Purpose**: Ground control station decoder and key provisioning client.

**Modules**:
- `decoder.rs` — Full stack: PHY → nonce validation → AES-GCM decrypt → HMAC verify → MAVLink deserialize
- `key_store.rs` — OS keychain integration (Windows DPAPI, macOS Keychain, Linux Secret Service)
- `main.rs` — CLI/TUI interface for mission upload and live telemetry

**Build**:
```bash
cargo build -p jfl-gcs --release
```

**Run**:
```bash
cargo run -p jfl-gcs -- --config config/commercial-high.toml
```

### `crates/jfl-sim`

**Purpose**: Deterministic protocol simulator for testing and validation.

**Capabilities**:
- Simulates dual-channel RF link with configurable latency, jitter, and frame loss
- Injects jamming signals (narrowband, wideband, chirp)
- Simulates replay attacks and spoofing attempts
- Validates anti-jam and failover logic under adversarial conditions

**Build**:
```bash
cargo build -p jfl-sim --release
```

**Run**:
```bash
cargo run -p jfl-sim -- --scenario defense-full --jam-power -80 --frame-loss 0.05
```

### `tools/fuzz`

**Purpose**: Fuzzing harnesses for protocol robustness.

**Targets**:
- `frame_fuzz` — Frame parser with arbitrary binary input

**Run**:
```bash
cd tools/fuzz
cargo fuzz run frame_fuzz --
```

### `tools/key_provisioner`

**Purpose**: Pre-flight key agreement and certificate signing.

**Usage**: ECDH ephemeral key exchange, mutual authentication, session key derivation.

**Build**:
```bash
cargo build -p key_provisioner --release
```

### `tools/link_analyzer`

**Purpose**: Real-time FFT, RSSI, BER, and hop-sequence dashboard.

**Build**:
```bash
cargo build -p link_analyzer --release
```

## Profiles

Each profile pre-configures crypto, channels, and anti-jam strategy:

| Profile | Crypto | Ch A | Ch B | Anti-jam | Target | Replay Window | Key Rotation |
|---------|--------|------|------|----------|--------|---------------|--------------|
| `commercial-low` | AES-128-GCM | 900MHz-FHSS | 2.4GHz-OFDM | FHSS | none | 32 | none |
| `commercial-high` | AES-256-GCM | 900MHz-FHSS | 5.8GHz-OFDM | FHSS+PowerCtrl | DO-160G | 64 | 7200s |
| `defense-lite` | AES-256+ECDH-P256 | SDR-FHSS | SDR-DSSS | AJ-OFDM | MIL-STD-461G | 128 | 3600s |
| `defense-full` | Suite-B AES-256+ECDH-P384+HKDF | SDR-FHSS | SDR-DSSS | Adaptive+Nulling | DO-178C-DAL-B | 256 | 1800s |

## Development Workflow

### Setting up your environment

```bash
# Install Rust (if not present)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env

# Clone and navigate
git clone https://github.com/Jettanakorn/jfoxlink.git
cd jfoxlink

# Verify build
cargo test -q
```

### Building and testing

```bash
# Full workspace
cargo build --release
cargo test

# Specific crate
cargo build -p jfl-core --release
cargo test -p jfl-core

# With feature flags
cargo build -p jfl-core --features crypto_ecdh --release
```

### Code style and linting

The workspace enforces:
- `#![deny(unsafe_code)]` in `jfl-core`
- `#![warn(clippy::all, clippy::pedantic)]` across all crates
- No panics in protocol-critical paths

Run clippy:
```bash
cargo clippy --all-targets --all-features
```

Format code:
```bash
cargo fmt --all
```

### Adding a new module to `jfl-core`

1. Create `crates/jfl-core/src/new_module.rs`
2. Add `pub mod new_module;` to `crates/jfl-core/src/lib.rs`
3. Implement `#![no_std]` compatibility (no heap allocation for core types)
4. Add unit tests in module footer
5. Ensure all error paths return `JflError` (never unwrap)
6. Run:
   ```bash
   cargo test -p jfl-core
   cargo clippy -p jfl-core --all-targets
   ```

### Adding a new hardware driver to `jfl-hal`

1. Create `crates/jfl-hal/src/new_driver.rs`
2. Implement `DatalinkTx`, `DatalinkRx`, and optional traits
3. Add `pub mod new_driver;` to `crates/jfl-hal/src/lib.rs`
4. Add integration tests in `crates/jfl-hal/tests/`
5. Document the serial protocol or register map in comments
6. Test:
   ```bash
   cargo test -p jfl-hal
   ```

### Submitting changes

1. Create a feature branch: `git checkout -b feature/my-feature`
2. Commit atomically: `git commit -m "brief: detailed description"`
3. Run full test suite: `cargo test --all`
4. Push: `git push origin feature/my-feature`
5. Open a pull request on GitHub
6. Ensure CI passes and code review is complete before merging

## Testing Strategy

### Unit tests

Located in each module, run with:
```bash
cargo test --lib
```

### Integration tests

Scenario-based tests in `tests/` directories, run with:
```bash
cargo test --test '*'
```

### Fuzzing

Continuous fuzzing with LLVM libFuzzer:
```bash
cd tools/fuzz
cargo fuzz run frame_fuzz -- -max_len=256 -timeout=1
```

### Simulation

Deterministic replay of adversarial scenarios:
```bash
cargo run -p jfl-sim --release -- \
  --scenario defense-full \
  --jam-power -85 \
  --frame-loss 0.10 \
  --channel-delay-ms 50
```

## Debugging

### Enable logging

Set environment variable:
```bash
export RUST_LOG=debug
cargo run -p jfl-gcs
```

### Inspect frame parsing

Add temporary debug output to `frame.rs`:
```rust
eprintln!("Parsed frame: stx={:02x}, len={}, seq={}", frame.stx, frame.len, frame.seq);
```

### Use GDB or LLDB

```bash
cargo build -p jfl-gcs
rust-lldb target/debug/jfl-gcs -- --config config/commercial-high.toml
```

## Release checklist

Before tagging a release:

- [ ] All tests pass: `cargo test --all`
- [ ] No clippy warnings: `cargo clippy --all-targets --all-features`
- [ ] Format is clean: `cargo fmt --all --check`
- [ ] README and manuals are up to date
- [ ] Version bumped in all `Cargo.toml` files
- [ ] CHANGELOG.md updated
- [ ] GitHub tag created: `git tag -a v1.0.0 -m "Release v1.0.0"`
- [ ] Push tags: `git push origin --tags`

## Common issues

| Issue | Cause | Fix |
|-------|-------|-----|
| `hkdf` feature error | Invalid feature flag | Use only `default` or omit features |
| `heapless::Vec::try_push` missing | Old version of heapless | Update `Cargo.lock`: `cargo update` |
| AES tag mismatch | Type mismatch with `GenericArray` | Use `GenericArray::from(*array_slice)` |
| Lifetime in `frame.rs` | Parser returns borrowed data | Signature is `fn from_bytes(raw: &'a [u8])` |
| `no_std` error in fuzz target | Fuzz runner needs `std` | Remove `#![no_std]` from fuzz target |
| UI dependency failure on Windows | `winapi` feature | Remove `egui`/`eframe` from non-GUI tools |

## Contributing

We welcome contributions! Please:
1. Fork the repository
2. Create a feature branch
3. Ensure all tests pass and code style is correct
4. Submit a pull request with a clear description
5. Respond to review feedback promptly

## License

See LICENSE file in the repository root.

## Contact

Developed by Jettanakorn Pengsiri at JFOX Aircraft Co., Ltd.

For questions or issues, please open a GitHub issue or contact the development team.
