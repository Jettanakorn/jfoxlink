# Changelog

All notable changes to JFOXLink are documented in this file. The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [1.0.0] - 2026-05-25

### Added

- **Core Protocol Engine** (`jfl-core`)
  - Zero-panic frame parser and serializer
  - No-std compatible protocol implementation
  - Feature-gated cryptographic modules (AES-GCM, ECDH, HKDF, HMAC)
  - Dual-channel arbitration with health scoring and latency-aware voting
  - Anti-jam resilience with FHSS (100-channel) and DSSS (Barker-11) support
  - Replay detection with configurable sliding window (32–256 frames)
  - MAVLink v2 payload wrapping and compatibility layer

- **Hardware Abstraction Layer** (`jfl-hal`)
  - Embedded radio trait definitions (DatalinkTx, DatalinkRx, FrequencyHop, PowerControl)
  - RFD900x/SiK 900MHz FHSS driver
  - Semtech SX1280 2.4GHz DSSS/OFDM driver
  - USRP/HackRF SDR backend for defense profiles

- **Ground Control Station** (`jfl-gcs`)
  - MAVLink-compatible telemetry decoder
  - Full-stack decryption: PHY → Crypto → Deserialize
  - OS keychain integration (Windows DPAPI, macOS Keychain, Linux Secret Service)
  - Real-time channel health monitoring (RSSI, BER, latency)
  - Dual-channel failover status display

- **Deterministic Simulator** (`jfl-sim`)
  - RF channel impairment emulation (jitter, loss, latency)
  - Adversarial threat injection (jamming, replay, spoofing)
  - Scenario-based validation for all profiles
  - Pre-flight testing without hardware

- **Key Provisioning Tool** (`tools/key_provisioner`)
  - ECDH ephemeral key agreement
  - Session key derivation (HKDF-SHA256/384)
  - Certificate chain validation (Suite B profiles)

- **Link Analysis Dashboard** (`tools/link_analyzer`)
  - Real-time FFT and RSSI trending
  - Bit Error Rate (BER) monitoring
  - Frequency hop sequence visualization
  - Jamming detection and threat assessment

- **Fuzzing Harnesses** (`tools/fuzz`)
  - `cargo-fuzz` targets for frame parser robustness
  - Arbitrary binary input generation for crypto modules

- **Configuration Profiles**
  - `commercial-low`: AES-128-GCM, FHSS, hobbyist use
  - `commercial-high`: AES-256-GCM, FHSS+PowerCtrl, DO-160G target
  - `defense-lite`: AES-256+ECDH-P256, AJ-OFDM, MIL-STD-461G target
  - `defense-full`: Suite B (P-384), Adaptive anti-jam, DO-178C-DAL-B target

- **Documentation**
  - README.md with project overview and quick start
  - DEVELOPER.md with architecture, crate details, and development workflow
  - USER_MANUAL.md with installation, operation, and troubleshooting
  - REVISION_CONTROL.md with Git Flow strategy and contribution guidelines

### Technical Details

- **Language**: Rust (stable toolchain)
- **Build Profile**: opt-level = "z", LTO, codegen-units = 1 for compact release binaries
- **Safety**: `#![deny(unsafe_code)]` in protocol-critical paths, no panics in parsing
- **Dependencies**: Zero-dependency `jfl-core`, minimal HAL and GCS footprint
- **Testing**: Unit tests, integration tests, fuzzing, and deterministic simulation

### Known Limitations

- SDR backend (defense profiles) requires USRP or HackRF hardware
- GUI components (link_analyzer) currently CLI-only; full UI dashboard in planned for v1.1
- Key rotation automatic only; manual rotation not yet exposed via GCS
- Platform support: Windows, macOS, Linux (tested on 64-bit systems)

## Future Roadmap

### v1.1 (Planned)

- Full GUI dashboard for link_analyzer
- Graphical mission planner for GCS
- Extended SDR support (PlutoSDR, LimeSDR)
- Performance optimizations for high-frequency hopping

### v1.2 (Planned)

- Hardware-accelerated AES (AES-NI support)
- Blind spot compensation in frequency-hopping
- Enhanced anti-jam with null-steering arrays
- Real-time log streaming to cloud backend

### v2.0 (Planned)

- Multi-platform redundancy (air/ground/satellite)
- ML-based jamming classification
- Quantum-resistant key exchange (post-quantum cryptography)
- Formal verification of protocol state machines

## Migration Guide

### Updating from earlier development versions

If you have a pre-release version of JFOXLink, please:

1. Backup your configuration files in `config/`
2. Clone fresh: `git clone https://github.com/Jettanakorn/jfoxlink.git`
3. Re-provision all keys using `tools/key_provisioner`
4. Test with simulator before deploying to hardware

## Support

For issues, feature requests, or discussions:

- **GitHub Issues**: https://github.com/Jettanakorn/jfoxlink/issues
- **Discussions**: https://github.com/Jettanakorn/jfoxlink/discussions
- **Security**: Report security vulnerabilities privately to the maintainers

## Contributors

- **Jettanakorn Pengsiri** (JFOX Aircraft Co., Ltd.)

## License

JFOXLink is distributed under the terms specified in LICENSE.md.
