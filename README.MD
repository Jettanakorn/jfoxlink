# JFOXLink

A secure UAV radio-datalink protocol suite — a hardened, crypto-wrapped derivative of MAVLink v2 with dual-channel redundancy and anti-jam. This repository contains the protocol core, a hardware-abstraction layer with real radio drivers, ground-control-station software, a deterministic simulator, and supporting CLI tools.

Developed by Jettanakorn Pengsiri at JFOX Aircraft Co., Ltd.

> **Status:** real, tested software (49 tests, CI-gated) — **not** hardware-validated or certified. It is not fieldable until on-air validation and any required certification (DO-178C / MIL-STD) are complete. See [PROJECT_REPORT.md](PROJECT_REPORT.md).

## Workspace layout

```
crates/
  jfl-core   (no_std)  protocol engine: frame, crypto, channel, anti_jam
  jfl-hal    (no_std)  radio traits + RFD900 / SX1280 drivers + SDR interface
  jfl-gcs    (std)     ground station: decode stack, key store, tx, CLI
  jfl-sim    (std)     deterministic RF / threat simulator
tools/
  key_provisioner      ECDH key-provisioning CLI
  link_analyzer        RF link-quality dashboard
  fuzz (jfl-fuzz)      libFuzzer frame-parser target (detached crate)
config/                four runtime profiles (commercial / defense)
```

Everything depends on `jfl-core`. See **[ARCHITECTURE.md](ARCHITECTURE.md)** for the layered design (or the rendered [docs/architecture.html](docs/architecture.html)).

## Build and run

Requires a stable Rust toolchain (`rustup`).

```bash
git clone https://github.com/Jettanakorn/jfoxlink.git
cd jfoxlink

cargo build --release      # build the whole workspace
cargo test                 # run all tests (49)
```

Run the pieces:

```bash
# Ground station — end-to-end secure-link self-test (ECDH → encrypt → decode → replay-reject)
cargo run -p jfl-gcs -- --config config/defense-full.toml selftest
# ...or just print a loaded profile (also: `decode --file <f> --aes-hex <hex> --hmac-hex <hex>`)
cargo run -p jfl-gcs -- --config config/commercial-high.toml profile

# Simulator — deterministic scenario runner
cargo run -p jfl-sim -- --scenario defense-full --frames 1000 --jam wide

# Tools
cargo run -p key_provisioner -- handshake     # two-party ECDH agreement demo
cargo run -p link_analyzer -- --samples 48    # RF link-quality dashboard
```

Binaries land in `target/release/` (`jfl-gcs`, `jfl-sim`, `key_provisioner`, `link_analyzer`).

## Fuzzing

The `tools/fuzz` crate is a `#![no_main]` libFuzzer target detached from the workspace, so `cargo build`/`test` skip it. Fuzz the frame parser with the nightly `cargo-fuzz` toolchain:

```bash
cargo +nightly fuzz run frame_fuzz --fuzz-dir tools/fuzz -- -max_len=512
```

## Configuration profiles

Four profiles in `config/` select crypto suite, RF bands, anti-jam strategy, and (for defense-full) replay window, key rotation, and jam threshold:

| Profile | Crypto | Channel A | Channel B | Anti-jam | Cert target |
|---------|--------|-----------|-----------|----------|-------------|
| `commercial-low`  | AES-128-GCM | 915 MHz FHSS | 2.4 GHz DSSS | FHSS | None |
| `commercial-high` | AES-256-GCM | 900 MHz FHSS | 5.8 GHz OFDM | FHSS + PowerCtrl | DO-160G |
| `defense-lite`    | AES-256-GCM + ECDH-P-256 | 900 MHz FHSS | 1.4 GHz DSSS | AJ-OFDM + FHSS | MIL-STD-461G |
| `defense-full`    | Suite B (AES-256 + ECDH-P-384 + HKDF) | SDR FHSS | SDR DSSS | Adaptive + PowerCtrl + Nulling | DO-178C-DAL-B |

Values are read verbatim from `config/*.toml`; the full parameter reference is in **[PARAMETERS.md](PARAMETERS.md)**.

## Documentation

- **[ARCHITECTURE.md](ARCHITECTURE.md)** — the layered stack as implemented (+ rendered `docs/architecture.html`).
- **[PARAMETERS.md](PARAMETERS.md)** — user (config) and developer (code constant) parameter reference.
- **[PROJECT_REPORT.md](PROJECT_REPORT.md)** — component status, verification, risks, and remaining gaps.
- **[DEVELOPER.md](DEVELOPER.md)** — architecture, crate design, and development workflow.
- **[USER_MANUAL.md](USER_MANUAL.md)** — installation, profiles, operation, and troubleshooting.
- **[CHANGELOG.md](CHANGELOG.md)** · **[LICENSE.md](LICENSE.md)**

## Notes

- `jfl-core` is `no_std` (no heap for core types, `heapless::Vec`), `#![deny(unsafe_code)]`, and its frame parser never panics.
- Release builds use `opt-level = "z"`, `lto = true`, `codegen-units = 1`, `panic = "abort"`.
- Feature flags are **profile-based** (`hobbyist` / `commercial` / `defense-lite` / `defense-full`, default `defense-full`) — not the `crypto_*` flags older prose mentions.
