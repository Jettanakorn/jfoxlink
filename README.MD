# JFOXLink 1.0

A workspace for the JFOXLink secure radio protocol suite. This repository contains the core protocol implementation, hardware abstraction layers, ground control station software, simulation tools, and supporting utilities.

Developed by Jettanakorn Pengsiri at JFOX Aircraft Co., Ltd.

## Workspace layout

- `Cargo.toml` — workspace root with `resolver = "2"` and release profile optimized for size + `panic = "abort"`.
- `config/` — example payload and radio profiles for commercial and defense use cases.
- `crates/` — core protocol, hardware abstraction, GCS client, and simulator crates.
- `tools/` — command-line utilities, link visualization, and fuzzing harnesses.

### Crates

- `crates/jfl-core` — `no_std` protocol engine.
  - frame parsing, serialization, redundancy, and feature-gated crypto modules.
- `crates/jfl-hal` — hardware abstraction layer.
  - embedded radio traits, FHSS/DSSS drivers, and SDR backends.
- `crates/jfl-gcs` — ground control station client.
  - MAVLink-compatible decoder plus key storage and provisioning support.
- `crates/jfl-sim` — deterministic protocol simulator.
  - RF channel/emulator and threat injection for testing.

### Tools

- `tools/key_provisioner` — ECDH key agreement and certificate preparation.
- `tools/link_analyzer` — real-time link dashboard and RF health monitor.
- `tools/fuzz` — `cargo-fuzz` targets for frame parser and crypto validation.

## Build and run

### Prerequisites

- Rust toolchain (stable)
- `cargo` installed and available in PATH

### Build everything

```powershell
cd C:\home\project\JFOXLink1.0
cargo test
```

### Run individual packages

- Core unit tests:

```powershell
cargo test -p jfl-core
```

- GCS client:

```powershell
cargo run -p jfl-gcs --
```

- Simulator:

```powershell
cargo run -p jfl-sim --
```

- Link analyzer:

```powershell
cargo run -p link_analyzer --
```

### Fuzz harness

```powershell
cd tools/fuzz
cargo test
```

## Documentation

For detailed information, see:

- **[DEVELOPER.md](DEVELOPER.md)** — Architecture, crate design, development workflow, testing strategy, and contribution guidelines.
- **[USER_MANUAL.md](USER_MANUAL.md)** — Installation, configuration profiles, operational procedures, security practices, and troubleshooting.
- **[REVISION_CONTROL.md](REVISION_CONTROL.md)** — Git Flow branching strategy, commit conventions, and collaboration workflow.

## Quick Start

1. **Clone and build**:
   ```bash
   git clone https://github.com/Jettanakorn/jfoxlink.git
   cd jfoxlink
   cargo build --release
   ```

2. **Run tests**:
   ```bash
   cargo test
   ```

3. **Try the simulator** (no hardware required):
   ```bash
   cargo run -p jfl-sim --release -- --scenario commercial-high
   ```

4. **Launch GCS** (requires transceiver):
   ```bash
   cargo run -p jfl-gcs --release -- --config config/commercial-high.toml
   ```

## Configuration Profiles

Four profiles available; see `config/` directory:

| Profile | Crypto | Use Case | Cert Target |
|---------|--------|----------|------------|
| `commercial-low` | AES-128-GCM | Hobbyist/light commercial | None |
| `commercial-high` | AES-256-GCM | Commercial aircraft | DO-160G |
| `defense-lite` | AES-256+ECDH-P256 | Military applications | MIL-STD-461G |
| `defense-full` | Suite B (P-384) | High-threat environments | DO-178C-DAL-B |

For details, see [USER_MANUAL.md](USER_MANUAL.md#configuration).

## Notes

- `jfl-core` is designed for `no_std` environments with optional crypto features.
- Release builds use `opt-level = "z"`, `lto = true`, and `codegen-units = 1` for compact output.
- The workspace currently compiles successfully under the checked Rust environment.
- Full development guidelines and architecture details are in [DEVELOPER.md](DEVELOPER.md).
- Revision control procedures and contribution workflows are in [REVISION_CONTROL.md](REVISION_CONTROL.md).
