# JFOXLink — User Manual

## Overview

JFOXLink is a production-grade secure radio communication protocol suite designed for mission-critical applications. It provides:

- **Military-grade encryption** (AES-256-GCM, Suite B with P-384 ECDH)
- **Automatic failover** between dual redundant radio channels
- **Anti-jam resilience** with adaptive frequency hopping and spread-spectrum
- **Optional MAVLink-compatible payload support** for autopilot integration
- **Real-time monitoring** via ground control station and link analyzer
- **Deterministic simulation** for pre-flight validation

This manual guides operators and system integrators through installation, configuration, and operational use.

## System Requirements

### Hardware

- **Host computer**: Windows, macOS, or Linux
- **Radio transceivers**: RFD900x (900MHz FHSS) or Semtech SX1280 (2.4GHz)
- **Optional SDR**: USRP or HackRF (defense profiles only)
- **USB/Serial interface**: For transceiver connection

### Software

- **Rust toolchain** (stable): For building from source
- **Java 11+**: Optional, for GUI tools
- **Python 3.8+**: Optional, for analysis scripts

## Installation

### Prerequisites

1. Install Rust from https://rustup.rs/

   ```bash
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   source $HOME/.cargo/env  # Linux/macOS
   # or on Windows, run installer
   ```

2. Clone the JFOXLink repository

   ```bash
   git clone https://github.com/Jettanakorn/jfoxlink.git
   cd jfoxlink
   ```

### Building

Build the complete workspace:

```bash
cargo build --release
```

Build specific tools:

```bash
# Ground Control Station (GCS)
cargo build -p jfl-gcs --release

# Link Analyzer dashboard
cargo build -p link_analyzer --release

# Simulator (for testing)
cargo build -p jfl-sim --release
```

Binary locations:
- Windows: `target\release\*.exe`
- Linux/macOS: `target/release/*`

## Configuration

JFOXLink provides four pre-configured profiles. Choose based on your use case:

### Commercial-Low

For hobbyist and light commercial use.

```toml
# config/commercial-low.toml
[profile]
name = "COMMERCIAL-LOW"
crypto_suite = "AES-128-GCM"
channel_a = "900MHz-FHSS"
channel_b = "2.4GHz-OFDM"
anti_jam = "FHSS"
```

**Use when**:
- Non-critical data links
- Low-cost commercial applications
- No encryption requirements

### Commercial-High

For commercial aircraft and precision applications (DO-160G certified).

```toml
# config/commercial-high.toml
[profile]
name = "COMMERCIAL-HIGH"
crypto_suite = "AES-256-GCM"
channel_a = "900MHz-FHSS"
channel_b = "5.8GHz-OFDM"
anti_jam = "FHSS+PowerCtrl"
cert_target = "DO-160G"
key_rotation_s = 7200
```

**Use when**:
- Commercial airborne platforms
- DO-160G certification required
- High reliability needed

### Defense-Lite

For military applications with moderate anti-jam requirements (MIL-STD-461G).

```toml
# config/defense-lite.toml
[profile]
name = "DEFENSE-LITE"
crypto_suite = "AES-256+ECDH-P256-HKDF"
channel_a = "SDR-FHSS"
channel_b = "SDR-DSSS"
anti_jam = "AJ-OFDM"
cert_target = "MIL-STD-461G"
replay_window = 128
key_rotation_s = 3600
jam_threshold_dbm = -85
```

**Use when**:
- Military or sensitive applications
- MIL-STD-461G compliance
- Moderate RF threat environment

### Defense-Full

For high-threat environments with DO-178C DAL-B certification.

```toml
# config/defense-full.toml
[profile]
name = "DEFENSE-FULL"
crypto_suite = "SuiteB-AES256-ECDH-P384-HKDF"
channel_a = "SDR-FHSS"
channel_b = "SDR-DSSS"
anti_jam = "Adaptive+PowerCtrl+Nulling"
cert_target = "DO-178C-DAL-B"
replay_window = 256
key_rotation_s = 1800
jam_threshold_dbm = -85
```

**Use when**:
- High-threat RF environment
- DO-178C DAL-B requirements
- Critical military or aerospace missions

## Basic Operation

### Starting the Ground Control Station (GCS)

The GCS decodes incoming radio frames, validates encryption, and displays live telemetry.

```bash
# Using the default commercial-high profile
cargo run -p jfl-gcs --release -- --config config/commercial-high.toml

# Or run the pre-built binary
./target/release/jfl-gcs --config config/commercial-high.toml
```

**Key features**:
- Real-time telemetry display for decrypted native JFOXLink payloads
- Automatic key provisioning UI
- Channel health monitoring (RSSI, BER, latency)
- Dual-channel failover status

### Real-Time Link Monitoring (Link Analyzer)

Monitor RF health metrics in real time.

```bash
cargo run -p link_analyzer --release
```

**Dashboard displays**:
- Frequency hop pattern (100-channel sequence)
- RSSI (Received Signal Strength Indicator) per channel
- Bit Error Rate (BER) trending
- Channel latency and jitter
- Jamming detection alerts

### Pre-Flight Validation with Simulator

Test your configuration before deploying to hardware.

```bash
# Simulate defense-full profile under jamming
cargo run -p jfl-sim --release -- \
  --scenario defense-full \
  --jam-type narrowband \
  --jam-power -80 \
  --frame-loss 0.05 \
  --duration-sec 60
```

**Simulator scenarios**:
- `commercial-low`, `commercial-high`, `defense-lite`, `defense-full`
- Jamming types: `narrowband`, `wideband`, `chirp`, `none`
- Frame loss: 0.0 – 1.0
- Channel delay: configurable milliseconds

## Operational Procedures

### Pre-Flight Checklist

1. **Hardware verification**
   ```bash
   # Check transceiver connection (RFD900x or SX1280)
   lsof -i @/dev/ttyUSB0  # Linux/macOS
   # or Device Manager → COM ports (Windows)
   ```

2. **Key provisioning**
   - Exchange ephemeral ECDH keys with remote platform
   - Verify certificate chain (Suite B only)
   - Confirm shared session key derivation

3. **Link health check**
   - Launch Link Analyzer
   - Confirm both channels reporting > -80 dBm RSSI
   - Verify BER < 1e-6 on each channel
   - Confirm failover logic responds to simulated channel loss

4. **Scenario simulation**
   ```bash
   cargo run -p jfl-sim --release -- \
     --scenario <your-profile> \
     --jam-power -75 \
     --duration-sec 300
   ```
   - Verify anti-jam holds lock during jamming
   - Confirm replay window blocks repeated frames
   - Validate key rotation on schedule

5. **GCS connectivity**
   - Launch GCS with active radio link
   - Confirm heartbeat messages arriving
   - Verify telemetry decryption (no crypto errors)

### In-Flight Monitoring

Once airborne:

1. **Continuous monitoring**
   - GCS dashboard displays live native JFOXLink telemetry
   - Link Analyzer shows real-time RSSI, BER, latency
   - System auto-logs all transmitted/received frames

2. **Failover events**
   - System automatically switches to secondary channel if primary fails
   - GCS displays channel switch alert
   - No user intervention required (transparent to autopilot)

3. **Anti-jam response**
   - On detection of jamming:
     - FHSS: Channels hop faster
     - DSSS: Spreading code spreads wider
     - Adaptive: Power increased (if within limits)
   - GCS displays jam alert with threat assessment

### Emergency procedures

| Condition | Action | GCS Display |
|-----------|--------|-------------|
| Primary channel loss | System switches to secondary (0–100ms) | "Failover to Channel B" |
| Crypto tag mismatch | Frame rejected, no action | "Crypto error" (red) |
| Replay detected | Frame silently dropped | "Replay blocked" (log only) |
| Both channels failed | Loss of telemetry, System enters hold mode | "Link Lost" (red flash) |
| Jamming detected | FHSS/DSSS adapts, power increases | "Jam Alert: -85 dBm" |

## Security Practices

### Key Management

1. **Generation**: Use `tools/key_provisioner` for ECDH key exchange
   ```bash
   cargo run -p key_provisioner --release
   ```

2. **Storage**: Keys stored in OS keychain (Windows DPAPI, macOS Keychain, Linux Secret Service)

3. **Rotation**: Automatic on schedule
   - Commercial-high: every 2 hours
   - Defense-lite: every 1 hour
   - Defense-full: every 30 minutes

### Replay Protection

Each profile maintains a replay window:
- Sliding window of N previous nonce values
- Any repeated nonce silently rejected
- Window size increases with threat level
- Commercial-low: 32-frame window
- Defense-full: 256-frame window

### Jamming Immunity

Three-layer defense:

1. **FHSS** (Frequency Hopping Spread Spectrum)
   - 100-channel pseudo-random sequence
   - Dwell time: 10 ms per frequency
   - Regenerated every key rotation

2. **DSSS** (Direct Sequence Spread Spectrum) — defense profiles only
   - Barker-11 spreading code
   - Process gain: 10.4 dB
   - Secondary channel diversity

3. **Adaptive anti-jam** — defense-full only
   - Detects jamming energy
   - Increases transmit power (up to regulatory limits)
   - Activates null-steering on secondary channel

## Troubleshooting

### "Connection refused" or "Serial port not found"

**Cause**: Transceiver not connected or wrong port.

**Fix**:
```bash
# Linux/macOS
lsof -i | grep ttyUSB
# or
ls -la /dev/tty*

# Windows Device Manager
# Check COM port assignment
```

### "Crypto tag mismatch"

**Cause**: Encryption key mismatch or corrupted frame.

**Fix**:
1. Re-provision keys with `key_provisioner`
2. Verify both ends using same profile (e.g., both `commercial-high`)
3. Check for EMI (electrical noise) causing bit errors

### "Replay detected" (repeated messages)

**Cause**: Duplicate frame received or nonce window exhausted.

**Fix**:
1. Verify transmitter is not re-sending duplicate packets
2. Increase replay window in config (larger = higher memory usage)
3. Trigger key rotation: reduce `key_rotation_s`

### "Link Lost" or low RSSI

**Cause**: Distance, obstruction, interference, or transceiver failure.

**Fix**:
1. Move closer to base station
2. Check for metal obstacles or tall buildings
3. Verify antenna connections
4. Switch to defense profile for stronger anti-jam
5. Check GCS log for link quality before loss

### "Jamming detected" continuously

**Cause**: Environmental RF interference (radar, WiFi, mobile).

**Fix**:
1. Move to different location
2. Reduce transmit power (if safety allows)
3. Switch to defense-full profile (adaptive anti-jam)
4. Use SDR backend for dynamic channel selection

### Build fails with "missing `xyz` crate"

**Cause**: Incomplete clone or dependency issue.

**Fix**:
```bash
cargo clean
cargo update
cargo build --release
```

## Advanced Topics

### Custom configurations

Modify `config/` files to suit your platform:

```toml
[profile]
name = "CUSTOM"
crypto_suite = "AES-256-GCM"
channel_a = "900MHz-FHSS"
channel_b = "5.8GHz-OFDM"
anti_jam = "FHSS+PowerCtrl"
cert_target = "CUSTOM"
replay_window = 64
key_rotation_s = 1800
jam_threshold_dbm = -85
```

Then pass to GCS:
```bash
cargo run -p jfl-gcs -- --config config/custom.toml
```

### Hardware integration

To integrate custom radio hardware:

1. Create new driver in `crates/jfl-hal/src/my_radio.rs`
2. Implement `DatalinkTx`, `DatalinkRx`, `FrequencyHop` traits
3. Register in `crates/jfl-hal/src/lib.rs`
4. Link in GCS or jfl-sim

Example:
```rust
pub struct MyRadio { /* ... */ }

impl DatalinkTx for MyRadio {
    fn send(&mut self, frame: &[u8]) -> Result<(), HalError> {
        // Transmit to your hardware
    }
}

impl DatalinkRx for MyRadio {
    fn recv(&mut self, buf: &mut [u8]) -> Result<usize, HalError> {
        // Receive from your hardware
    }
}
```

### Python analysis scripts

JFOXLink logs raw frame data. Analyze with Python:

```python
import struct
import json

# Parse log file
with open("jfoxlink.log", "rb") as f:
    while True:
        frame = f.read(256)
        if not frame:
            break
        stx, length, flags, seq, sysid, compid = struct.unpack("<BBBBBB", frame[:6])
        if stx == 0xFD:
            print(f"Seq: {seq}, SysID: {sysid}, CompID: {compid}")
```

## Support and Resources

- **GitHub**: https://github.com/Jettanakorn/jfoxlink
- **Issues**: Report bugs at https://github.com/Jettanakorn/jfoxlink/issues
- **Discussions**: Join the community at https://github.com/Jettanakorn/jfoxlink/discussions

## License

JFOXLink is distributed under the license specified in the LICENSE file.

## Acknowledgments

Developed by Jettanakorn Pengsiri at JFOX Aircraft Co., Ltd.

