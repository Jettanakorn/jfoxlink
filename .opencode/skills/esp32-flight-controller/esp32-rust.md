# ESP32-S3 Rust HAL Reference

## Toolchain Setup

```toml
# .cargo/config.toml
[target.xtensa-esp32s3-none-elf]
runner = "espflash flash --monitor"

[build]
target = "xtensa-esp32s3-none-elf"

[unstable]
build-std = ["core", "alloc"]
```

```toml
# Cargo.toml dependencies (no_std flight controller)
[dependencies]
esp-hal          = { version = "0.18", features = ["esp32s3", "async"] }
embassy-executor = { version = "0.5", features = ["nightly", "integrated-timers"] }
embassy-time     = { version = "0.3", features = ["tick-hz-1_000_000"] }
embassy-sync     = "0.5"
heapless         = "0.8"
nalgebra         = { version = "0.33", default-features = false, features = ["libm"] }
libm             = "0.2"
defmt            = "0.3"
defmt-rtt        = "0.4"
critical-section = "1.1"
static-cell      = "2.0"
```

---

## Embassy Async Task Architecture

```rust
#![no_std]
#![no_main]
#![feature(type_alias_impl_trait)]

use embassy_executor::Spawner;
use embassy_time::{Duration, Ticker};
use esp_hal::{clock::ClockControl, peripherals::Peripherals, prelude::*,
              timer::TimerGroup};

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let peripherals = Peripherals::take();
    let system = peripherals.SYSTEM.split();
    let clocks = ClockControl::max(system.clock_control).freeze();

    let timg0 = TimerGroup::new(peripherals.TIMG0, &clocks);
    esp_hal_embassy::init(&clocks, timg0.timer0);

    // Spawn real-time tasks at different priorities
    spawner.spawn(imu_task(/* resources */)).unwrap();
    spawner.spawn(attitude_task(/* resources */)).unwrap();
    spawner.spawn(position_task(/* resources */)).unwrap();
    spawner.spawn(telemetry_task(/* resources */)).unwrap();
}

// 8kHz IMU + Rate Controller task
#[embassy_executor::task]
async fn imu_task(mut imu: ImuBundle, mut rate_ctrl: AdaptiveRateController,
                  cmd_rx: ImuRateCommandReceiver) {
    let mut ticker = Ticker::every(Duration::from_micros(125)); // 8kHz
    loop {
        ticker.next().await;
        let reading = imu.read_voted().await;
        let cmd = cmd_rx.try_receive().unwrap_or_default();
        let output = rate_ctrl.update(cmd, reading.gyro, 0.000125);
        ACTUATOR_SIGNAL.signal(output);
    }
}
```

---

## IMU Driver — ICM-42688-P (Triple Instance)

```rust
// fc-hal/src/imu.rs
use esp_hal::spi::{SpiDevice, SpiMode};

pub struct Icm42688 <SPI> {
    spi: SPI,
    config: ImuConfig,
    last_reading: RawImuData,
}

impl<SPI: SpiDevice> Icm42688<SPI> {
    pub const WHOAMI: u8 = 0x47;

    pub async fn init(&mut self) -> Result<(), ImuError> {
        // Verify WHO_AM_I
        let id = self.read_reg(0x75).await?;
        if id != Self::WHOAMI { return Err(ImuError::BadDevice(id)); }

        // Configure: 8kHz ODR, ±2000°/s gyro, ±16g accel, low-noise
        self.write_reg(0x4E, 0x06).await?; // GYRO_CONFIG0: 8kHz, ±2000dps
        self.write_reg(0x50, 0x06).await?; // ACCEL_CONFIG0: 8kHz, ±16g
        self.write_reg(0x4B, 0x02).await?; // PWR_MGMT0: gyro+accel LN mode
        embassy_time::Timer::after_millis(10).await;
        Ok(())
    }

    pub async fn read(&mut self) -> Result<ImuData, ImuError> {
        // Burst read ACCEL + GYRO (14 bytes)
        let buf = self.burst_read(0x1F, 14).await?;
        let raw = RawImuData::from_bytes(&buf);
        Ok(ImuData {
            accel: raw.accel_vec() * 9.81 / 2048.0, // m/s²
            gyro:  raw.gyro_vec()  * (PI / 180.0) / 16.4, // rad/s
            temp:  raw.temp_celsius(),
            timestamp_us: embassy_time::Instant::now().as_micros(),
        })
    }
}

// Triple IMU bundle with voting
pub struct ImuBundle {
    imu_a: Icm42688<SpiDevA>,
    imu_b: Icm42688<SpiDevB>,
    imu_c: Bmi088<SpiDevC>,      // Different manufacturer for diversity
    voter: TmrVoter<ImuData>,
}

impl ImuBundle {
    pub async fn read_voted(&mut self) -> VotedImuData {
        let (a, b, c) = embassy_futures::join::join3(
            self.imu_a.read(), self.imu_b.read(), self.imu_c.read()
        ).await;
        self.voter.vote_imu(a, b, c)
    }
}
```

---

## GPS Driver — u-blox M9N (UBX Protocol)

```rust
// fc-hal/src/gps.rs
pub struct Ublox<UART> {
    uart: UART,
    parser: UbxParser,
    fix: GpsFix,
}

impl<UART: AsyncRead + AsyncWrite> Ublox<UART> {
    pub async fn configure_nav_pvt(&mut self) -> Result<(), GpsError> {
        // Enable UBX-NAV-PVT at 10Hz, disable NMEA
        let cfg_prt = UbxCfgPrt { baud: 460800, proto_in: 0x01, proto_out: 0x01 };
        self.send_ubx(UbxClass::Cfg, 0x00, &cfg_prt.serialize()).await?;

        // Set navigation rate 10Hz
        let cfg_rate = UbxCfgRate { meas_rate_ms: 100, nav_rate: 1 };
        self.send_ubx(UbxClass::Cfg, 0x08, &cfg_rate.serialize()).await?;
        Ok(())
    }

    pub async fn poll(&mut self) -> Option<NavPvt> {
        if let Some(msg) = self.parser.feed(self.uart.read_available().await) {
            match msg {
                UbxMsg::NavPvt(pvt) if pvt.fix_type >= 3 => Some(pvt),
                _ => None,
            }
        } else { None }
    }
}
```

---

## Actuator Output — DSHOT + PWM

```rust
// fc-hal/src/actuator.rs

pub struct DshotOutput {
    rmt: RmtChannel,           // ESP32-S3 RMT peripheral → DSHOT
    last_value: u16,
}

impl DshotOutput {
    /// cmd: 0.0 (idle) to 1.0 (full throttle)
    pub fn set_throttle(&mut self, cmd: f32) {
        let throttle = (cmd.clamp(0.0, 1.0) * 1999.0) as u16 + 48;
        self.transmit_frame(throttle, false);
    }

    fn transmit_frame(&mut self, value: u16, telemetry: bool) {
        // DSHOT600 frame: 11 bit value + 1 telemetry + 4 CRC
        let frame = ((value as u32) << 5) | ((telemetry as u32) << 4);
        let crc = (frame ^ (frame >> 4) ^ (frame >> 8)) & 0x0F;
        let packet = ((frame << 4) | crc) as u16;
        // Encode to RMT symbols and transmit...
        self.rmt.transmit_dshot600(packet);
    }
}

/// Motor mixer for quadrotor X-frame
pub struct QuadMixer {
    motor_dirs: [f32; 4],  // +1 CW, -1 CCW
}

impl QuadMixer {
    pub fn mix(&self, thrust: f32, roll: f32, pitch: f32, yaw: f32)
        -> [f32; 4]
    {
        // Standard X-frame layout: FL, FR, RL, RR
        let m = [
            thrust - roll + pitch + yaw, // FL (CCW)
            thrust + roll + pitch - yaw, // FR (CW)
            thrust + roll - pitch + yaw, // RL (CW)
            thrust - roll - pitch - yaw, // RR (CCW)
        ];
        // Normalize to [0, 1] preserving ratios
        normalize_motor_outputs(m)
    }
}
```

---

## CAN / UAVCAN v1 (TWAI on ESP32)

```rust
// fc-hal/src/comm.rs
use esp_hal::twai::{Twai, TwaiConfig, BaudRate};

pub struct UavcanBus {
    twai: Twai<'static>,
    node_id: u8,
    transfer_id: u8,
}

impl UavcanBus {
    pub fn new(twai: Twai<'static>, node_id: u8) -> Self {
        // 1Mbps for low-latency actuator commands
        Self { twai, node_id, transfer_id: 0 }
    }

    /// Publish ESC setpoint (UAVCAN reg.drone.phy.electricity.Power.0.1)
    pub async fn publish_esc_setpoints(&mut self, values: &[f32]) {
        let payload = EscSetpoint { values: heapless::Vec::from_slice(values).unwrap() };
        let frame = cyphal::build_message_frame(
            self.node_id, self.transfer_id, ESC_SETPOINT_PORT_ID, &payload
        );
        self.transfer_id = self.transfer_id.wrapping_add(1);
        self.twai.transmit(&frame).await.ok();
    }
}
```

---

## Hardware Watchdog Configuration

```rust
// Dual watchdog: RWDT (RTC) + TGWDT (main)
pub fn configure_watchdogs(timg0: &TimerGroup) {
    // Main watchdog — 50ms timeout (must be fed by rate loop)
    let mut wdt = timg0.wdt;
    wdt.set_timeout(WatchdogStage::Stage0, 50u64.millis());
    wdt.set_action(WatchdogStage::Stage0, WatchdogAction::ResetSystem);
    wdt.enable();

    // RTC watchdog — 500ms timeout (last resort)
    let mut rwdt = Rtc::new(/* rtc peripherals */).rwdt;
    rwdt.set_timeout(500u64.millis());
    rwdt.enable();
}

// Rate loop MUST call this every cycle
#[inline(always)]
pub fn feed_watchdog(timg0: &mut TimerGroup) {
    timg0.wdt.feed();
}
```

---

## ESP32-C6 (RISC-V) Toolchain Setup

The C6 uses the standard RISC-V target — **no Xtensa fork needed**.

```toml
# .cargo/config.toml (ESP32-C6)
[target.riscv32imac-unknown-none-elf]
runner = "espflash flash --monitor"

[build]
target = "riscv32imac-unknown-none-elf"

[unstable]
build-std = ["core", "alloc"]
```

```toml
# Cargo.toml — C6 comms node (JFOXLink + 802.15.4)
[dependencies]
esp-hal          = { version = "0.18", features = ["esp32c6", "async"] }
embassy-executor = { version = "0.5", features = ["nightly", "integrated-timers"] }
embassy-time     = { version = "0.3", features = ["tick-hz-1_000_000"] }
embassy-sync     = "0.5"
heapless         = "0.8"
# NOTE: No nalgebra — avoid f32 heavy math on C6 (no hardware FPU)
defmt            = "0.3"
defmt-rtt        = "0.4"
critical-section = "1.1"
static-cell      = "2.0"
# JFOXLink crypto stack
aes-gcm          = { version = "0.10", default-features = false, features = ["aes"] }
p256             = { version = "0.13", default-features = false, features = ["ecdh"] }
hkdf             = "0.12"
hmac             = "0.12"
sha2             = { version = "0.10", default-features = false }
zeroize          = { version = "1.7", default-features = false }
```

### C6-Specific Peripheral Notes
- **No USB OTG** — uses built-in USB Serial/JTAG controller (CDC-ACM via hardware, no soft USB stack needed)
- **IEEE 802.15.4** radio available for Thread/Zigbee mesh (useful for multi-node UAV swarm mesh)
- **No hardware FPU** — all `f32` ops are soft-float (~8× slower); avoid floating point in hot paths
- **LP (Low-Power) core** — secondary ultra-low-power RISC-V core for sensor polling during deep sleep

### Embassy Task Example (C6 — JFOXLink comms node)
```rust
#![no_std]
#![no_main]

use embassy_executor::Spawner;
use esp_hal::{clock::ClockControl, peripherals::Peripherals, prelude::*};

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let peripherals = Peripherals::take();
    let system = peripherals.SYSTEM.split();
    let clocks = ClockControl::max(system.clock_control).freeze();

    let timg0 = esp_hal::timer::TimerGroup::new(peripherals.TIMG0, &clocks);
    esp_hal_embassy::init(&clocks, timg0.timer0);

    // C6: dedicated JFOXLink radio tasks (no flight control math)
    spawner.spawn(jfoxlink_rx_task(/* radio, inter_mcu_uart */)).unwrap();
    spawner.spawn(jfoxlink_tx_task(/* radio, inter_mcu_uart */)).unwrap();
    spawner.spawn(channel_health_monitor(/* radio_a, radio_b */)).unwrap();
}
```

---

## OTP Manager (eFuse — S3 & C6)

ESP32 eFuse blocks are written once and read-locked post-provisioning.
Use `esp-idf-sys` eFuse API wrapped in safe Rust.

### eFuse Block Layout

```
BLOCK0  [256 bit] — WR/RD protection bits, secure boot key digest (ROM-managed)
BLOCK1  [256 bit] — Flash encryption key (AES-XTS-256, auto-burned by bootloader)
BLOCK2  [256 bit] — Device UUID [127:0] + JFOX serial [255:128]
BLOCK3  [256 bit] — JFOXLink identity pre-shared key (256-bit)
BLOCK4  [256 bit] — Customer key slot A (reserved)
BLOCK5  [256 bit] — Customer key slot B (reserved)
BLOCK8  [256 bit] — Manufacturing: HW revision, cal date, test pass flags
BLOCK9  [256 bit] — Anti-rollback firmware version counter
```

### Rust OTP API

```rust
// fc-hal/src/otp.rs
use esp_idf_sys::{esp_efuse_read_field_blob, esp_efuse_write_field_blob,
                  esp_efuse_batch_write_begin, esp_efuse_batch_write_commit};
use zeroize::Zeroizing;

pub struct OtpManager;

impl OtpManager {
    /// Read 128-bit device UUID burned at manufacture (BLOCK2[127:0])
    pub fn device_uuid() -> [u8; 16] {
        let mut uuid = [0u8; 16];
        unsafe {
            esp_efuse_read_field_blob(
                ESP_EFUSE_OPTIONAL_UNIQUE_ID.as_ptr(), uuid.as_mut_ptr() as _, 128
            );
        }
        uuid
    }

    /// Read 256-bit JFOXLink identity key from BLOCK3
    /// Returns Zeroizing wrapper — key wiped from stack on drop
    pub fn jfoxlink_identity_key() -> Zeroizing<[u8; 32]> {
        let mut key = Zeroizing::new([0u8; 32]);
        unsafe {
            esp_efuse_read_field_blob(
                JFOX_EFUSE_JFOXLINK_KEY.as_ptr(), key.as_mut_ptr() as _, 256
            );
        }
        key
    }

    /// Burn manufacturing metadata — ONE-TIME, irreversible
    /// Requires physical burn authorization token (hardware enable pin)
    pub fn burn_manufacturing_data(data: &MfgData, _auth: BurnAuth)
        -> Result<(), OtpError>
    {
        unsafe { esp_efuse_batch_write_begin(); }
        // Write HW revision, serial, test pass
        // ... field writes ...
        let ret = unsafe { esp_efuse_batch_write_commit() };
        if ret != 0 { return Err(OtpError::BurnFailed(ret)); }
        Ok(())
    }

    /// Check secure boot V2 is enabled
    pub fn secure_boot_enabled() -> bool {
        let mut val: u8 = 0;
        unsafe {
            esp_efuse_read_field_blob(
                ESP_EFUSE_SECURE_BOOT_EN.as_ptr(), &mut val as *mut u8 as _, 1
            );
        }
        val != 0
    }
}

#[derive(Debug)]
pub enum OtpError {
    BurnFailed(i32),
    AlreadyBurned,
    AuthRequired,
}

/// Physical burn authorization — prevents accidental eFuse writes
/// Only constructible by asserting dedicated BURN_EN GPIO at boot
pub struct BurnAuth(());
impl BurnAuth {
    pub fn from_gpio_assertion(gpio_asserted: bool) -> Option<Self> {
        if gpio_asserted { Some(BurnAuth(())) } else { None }
    }
}
```

### Manufacturing Provisioning Workflow

```
Factory provisioning station (tools/otp_provisioner CLI):

1. Flash base firmware to blank ESP32
2. Generate device UUID (random 128-bit)
3. Derive JFOXLink identity key from master key + UUID (HKDF)
4. Assert BURN_EN GPIO on test fixture
5. Run: otp_provisioner --port /dev/ttyUSB0 \
         --uuid <uuid> --jfoxlink-key <key> \
         --hw-rev 1.2 --serial JFOX-FC-00123
6. Verify eFuse readback matches
7. Enable secure boot + flash encryption (burns BLOCK0 protection bits)
8. Unit is now locked — identity immutable
```

---

## USB Firmware Flash Manager (DFU + CDC-ACM)

### Hardware Setup

**ESP32-S3** has full USB OTG — connect D+/D− directly to USB port.  
**ESP32-C6** has USB Serial/JTAG controller only — exposes CDC-ACM automatically; no OTG/DFU.

For S3, strapping pin `GPIO0` selects mode at boot:
- `GPIO0 = LOW` → Download/DFU mode (ROM USB DFU)
- `GPIO0 = HIGH` → Normal boot (application firmware)

### Partition Table for OTA

```
# partitions.csv
# Name,   Type, SubType,  Offset,   Size,     Flags
nvs,      data, nvs,      0x9000,   0x6000,
otadata,  data, ota,      0xf000,   0x2000,
ota_0,    app,  ota_0,    0x20000,  0x1E0000,
ota_1,    app,  ota_1,    0x200000, 0x1E0000,
storage,  data, spiffs,   0x3E0000, 0x20000,
```

### USB DFU State Machine (Rust)

```rust
// fc-hal/src/usb_dfu.rs
use embassy_usb::{Builder, UsbDevice};
use embassy_usb::class::cdc_acm::CdcAcmClass;

pub enum DfuState {
    Idle,
    AuthChallenge { nonce: [u8; 32] },
    AuthVerified,
    Downloading { offset: u32, sha: Sha256 },
    Verifying,
    Verified { image_size: u32 },
    ManifestSync,
    Error(DfuError),
}

pub struct UsbDfuManager {
    state: DfuState,
    ota: OtaFlashWriter,
    expected_sig: Option<Ed25519Signature>,
}

impl UsbDfuManager {
    /// Called per USB DFU_DNLOAD request (4KB block)
    pub fn on_download_block(&mut self, block_num: u16, data: &[u8])
        -> Result<(), DfuError>
    {
        match &mut self.state {
            DfuState::AuthVerified | DfuState::Downloading { .. } => {
                let DfuState::Downloading { offset, sha } =
                    core::mem::replace(&mut self.state, DfuState::Idle)
                    else { unreachable!() };

                sha.update(data);
                self.ota.write(offset, data)?;
                self.state = DfuState::Downloading {
                    offset: offset + data.len() as u32,
                    sha,
                };
                Ok(())
            }
            _ => Err(DfuError::BadState),
        }
    }

    /// Called when host sends zero-length DFU_DNLOAD (manifest phase)
    pub fn on_manifest(&mut self) -> Result<(), DfuError> {
        let DfuState::Downloading { sha, image_size: _ } =
            core::mem::replace(&mut self.state, DfuState::Idle)
            else { return Err(DfuError::BadState); };

        let digest = sha.finalize();

        // Verify Ed25519 signature over SHA-256 digest
        if let Some(sig) = &self.expected_sig {
            JFOX_RELEASE_PUBKEY.verify(&digest, sig)
                .map_err(|_| DfuError::SignatureInvalid)?;
        }

        self.ota.set_boot_partition(OtaSlot::Ota1)?;
        self.state = DfuState::ManifestSync;
        Ok(())
    }
}

#[derive(Debug)]
pub enum DfuError {
    FlashWrite(esp_idf_sys::EspError),
    SignatureInvalid,
    AuthFailed,
    BadState,
    Rollback,   // firmware version < anti-rollback counter
}
```

### Host-Side CLI (tools/fw_flasher)

```bash
# Factory / field firmware update
fw_flasher \
  --port /dev/ttyUSB0 \
  --image fc-firmware-v1.3.0.signed.bin \
  --signing-key jfox-release-pub.pem \
  --mode dfu          # or --mode cdc-acm

# Output:
# [1/6] Connected to JFOX-FC-00123 (UUID: a1b2c3d4...)
# [2/6] Authenticated via ECDH (device identity from OTP)
# [3/6] Verified Ed25519 signature on firmware image
# [4/6] Erasing OTA_1 partition (1920 KB)...
# [5/6] Flashing: ████████████████████ 100% (1.3 MB @ 420 KB/s)
# [6/6] Verifying SHA-256... OK
# Rebooting device → OTA_1 active
# Anti-rollback counter: 3 → 4 (burned to eFuse BLOCK9)
```

### Rollback Protection Logic

```rust
// In bootloader / fc-core startup
fn check_rollback_protection(hdr: &FirmwareHeader) -> Result<(), BootError> {
    let otp_min_version = OtpManager::read_antirollback_counter();
    if hdr.min_version < otp_min_version {
        // Refuse to boot — downgrade attack attempt
        return Err(BootError::RollbackViolation {
            image: hdr.min_version,
            minimum: otp_min_version,
        });
    }
    // If new version > counter, burn new counter value (irreversible)
    if hdr.min_version > otp_min_version {
        OtpManager::increment_antirollback_counter(hdr.min_version)?;
    }
    Ok(())
}
```