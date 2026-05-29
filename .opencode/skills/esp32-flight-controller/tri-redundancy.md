# Tri-Redundancy Architecture Reference

## Overview

Tri-Redundancy (Triple Modular Redundancy — TMR) ensures the flight controller continues
operating safely when any single channel fails. The system uses **majority voting** to
detect and isolate faults without interrupting flight operations.

---

## Hardware Architecture

### Physical Layout

```
┌─────────────────────────────────────────────────────────────┐
│                  TRI-REDUNDANT FC BOARD                     │
│                                                             │
│  ┌───────────────┐  ┌───────────────┐  ┌───────────────┐  │
│  │   FCU-A       │  │   FCU-B       │  │   FCU-C       │  │
│  │  ESP32-S3     │  │  ESP32-S3     │  │  ESP32-S3     │  │
│  │  Primary      │  │  Shadow       │  │  Monitor      │  │
│  │               │  │               │  │               │  │
│  │  IMU-A (ICM)  │  │  IMU-B (ICM)  │  │  IMU-C (BMI)  │  │
│  │  GPS-A        │  │  GPS-B        │  │  GPS-B        │  │
│  │  Baro-A       │  │  Baro-B       │  │  Baro-C       │  │
│  └──────┬────────┘  └──────┬────────┘  └──────┬────────┘  │
│         └──────────────────┼────────────────────┘          │
│                       CAN / SPI Voter Bus                   │
│                                                             │
│  ┌─────────────────────────────────────────────────────┐   │
│  │           VOTER / ARBITRATION LOGIC                 │   │
│  │   (Implemented in dedicated FPGA or consensus SW)   │   │
│  └──────────────────────┬──────────────────────────────┘   │
│                         │ Voted Output                      │
│  ┌──────────────────────▼──────────────────────────────┐   │
│  │    ACTUATOR BUS (PWM/DSHOT/CAN — Dual-channel)      │   │
│  └─────────────────────────────────────────────────────┘   │
│                                                             │
│  ┌─────────────────────────────────────────────────────┐   │
│  │    POWER MANAGEMENT                                  │   │
│  │    Supply-A ──┐                                      │   │
│  │    Supply-B ──┼──▶ Ideal Diode OR-ing ──▶ FC Rails  │   │
│  │    Backup ────┘                                      │   │
│  └─────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────┘
```

### Channel Roles

| Channel | Role | Failure Behavior |
|---|---|---|
| FCU-A (Primary) | Computes control outputs, drives actuators | Voted out → FCU-B takes over |
| FCU-B (Shadow) | Runs identical algorithms, ready to takeover | Promoted to primary silently |
| FCU-C (Monitor) | Independent watchdog, cross-checks A+B | Triggers safety if A+B disagree |

---

## Software TMR Voter

### Generic Voter Implementation

```rust
// fc-core/src/redundancy/voter.rs
use core::fmt::Debug;
use libm::fabsf;

pub trait Voteable: Copy + Debug {
    /// Returns true if two values are within acceptable tolerance
    fn agrees_with(&self, other: &Self) -> bool;
    /// Blend two agreeing values (average for sensors)
    fn blend(&self, other: &Self) -> Self;
}

#[derive(Debug)]
pub enum VoteResult<T> {
    Unanimous(T),                        // All 3 agree
    Majority { value: T, fault: ChannelId }, // 2 agree, 1 diverged
    NoConsensus,                         // All 3 disagree → emergency
}

#[derive(Debug, Clone, Copy)]
pub enum ChannelId { A, B, C }

pub struct TmrVoter<T: Voteable> {
    pub fault_mask: u8,           // bit 0=A, 1=B, 2=C
    pub fault_counts: [u16; 3],
    pub fault_threshold: u16,     // consecutive faults before flagging
    _phantom: core::marker::PhantomData<T>,
}

impl<T: Voteable> TmrVoter<T> {
    pub fn new(fault_threshold: u16) -> Self {
        Self { fault_mask: 0, fault_counts: [0; 3],
               fault_threshold, _phantom: Default::default() }
    }

    pub fn vote(&mut self, a: T, b: T, c: T) -> VoteResult<T> {
        let ab = a.agrees_with(&b);
        let bc = b.agrees_with(&c);
        let ac = a.agrees_with(&c);

        match (ab, bc, ac) {
            (true, true, true) => {
                self.fault_counts = [0; 3];
                VoteResult::Unanimous(a.blend(&b).blend(&c))
            }
            (true, false, false) => {  // C is faulty
                self.register_fault(ChannelId::C);
                VoteResult::Majority { value: a.blend(&b), fault: ChannelId::C }
            }
            (false, true, false) => {  // A is faulty
                self.register_fault(ChannelId::A);
                VoteResult::Majority { value: b.blend(&c), fault: ChannelId::A }
            }
            (false, false, true) => {  // B is faulty
                self.register_fault(ChannelId::B);
                VoteResult::Majority { value: a.blend(&c), fault: ChannelId::B }
            }
            _ => VoteResult::NoConsensus,
        }
    }

    fn register_fault(&mut self, ch: ChannelId) {
        let idx = ch as usize;
        self.fault_counts[idx] += 1;
        if self.fault_counts[idx] >= self.fault_threshold {
            self.fault_mask |= 1 << idx;
        }
    }
}
```

### IMU-Specific Voter

```rust
// Tolerance: 0.5°/s for gyro, 0.5m/s² for accel
impl Voteable for ImuData {
    fn agrees_with(&self, other: &Self) -> bool {
        let gyro_ok = (self.gyro - other.gyro).norm() < 0.5_f32.to_radians() * 10.0; // 5°/s
        let accel_ok = (self.accel - other.accel).norm() < 0.5;
        gyro_ok && accel_ok
    }
    fn blend(&self, other: &Self) -> Self {
        ImuData {
            gyro:  (self.gyro  + other.gyro)  * 0.5,
            accel: (self.accel + other.accel) * 0.5,
            temp:  (self.temp  + other.temp)  * 0.5,
            timestamp_us: self.timestamp_us.max(other.timestamp_us),
        }
    }
}
```

---

## Inter-FCU Communication (Redundancy Bus)

### Protocol: CAN FD at 5Mbps

```rust
// Messages exchanged between FCU-A, FCU-B, FCU-C every 1ms
pub struct FcuHeartbeat {
    pub node_id: u8,             // 0=A, 1=B, 2=C
    pub state: FcuState,
    pub attitude_q: Quaternion,  // Estimated attitude
    pub rate_cmd: Vec3,          // Current rate command
    pub control_out: ControlOutput,
    pub health: HealthFlags,
    pub counter: u32,            // Monotonic, detect freeze
}

pub enum FcuState {
    Booting,
    Initializing,
    Active,          // Normal operation
    Degraded,        // Running with warnings
    ShadowMode,      // B/C following A
    TakeoverReady,   // Ready to become primary
    Emergency,       // Initiating safe landing
}
```

### Cross-Check Monitor

```rust
pub struct CrossCheckMonitor {
    history: heapless::Deque<FcuHeartbeat, 10>,
    divergence_threshold: f32,
    timeout_ms: u64,
}

impl CrossCheckMonitor {
    pub fn check_pair(&self, a: &FcuHeartbeat, b: &FcuHeartbeat)
        -> CrossCheckResult
    {
        // 1. Counter continuity check (detect freezes)
        if a.counter == self.last_a_counter {
            return CrossCheckResult::FrozenChannel(ChannelId::A);
        }

        // 2. Attitude divergence check
        let attitude_error = quaternion_error(a.attitude_q, b.attitude_q);
        if attitude_error.angle() > self.divergence_threshold {
            return CrossCheckResult::AttitudeDivergence {
                error_rad: attitude_error.angle()
            };
        }

        // 3. Control output check (catch runaway)
        let ctrl_diff = (a.control_out - b.control_out).max_element();
        if ctrl_diff > 0.3 {
            return CrossCheckResult::ControlDivergence { delta: ctrl_diff };
        }

        CrossCheckResult::Ok
    }
}
```

---

## Fault Isolation and Recovery State Machine

```rust
pub struct RedundancyManager {
    state: RedundancyState,
    voter: TmrVoter<ImuData>,
    monitor: CrossCheckMonitor,
    fault_log: heapless::Vec<FaultEvent, 32>,
}

#[derive(Clone, Copy, PartialEq)]
pub enum RedundancyState {
    FullTriplex,                    // All 3 healthy
    Degraded { faulty: ChannelId }, // 2-of-3 operational
    Simplex   { active: ChannelId }, // 1 remaining (last resort)
    FailSafe,                       // Initiate RTL/land
}

impl RedundancyManager {
    pub fn tick(&mut self, a: &FcuHeartbeat, b: &FcuHeartbeat,
                c: &FcuHeartbeat) -> RedundancyAction
    {
        match self.state {
            RedundancyState::FullTriplex => {
                // Check all pairs
                let ab = self.monitor.check_pair(a, b);
                let bc = self.monitor.check_pair(b, c);
                let ac = self.monitor.check_pair(a, c);
                if let Some(faulty) = isolate_fault(ab, bc, ac) {
                    self.transition_to_degraded(faulty);
                    return RedundancyAction::IsolateFaultAndContinue(faulty);
                }
                RedundancyAction::Normal
            }
            RedundancyState::Degraded { faulty } => {
                // Only 2 channels remain — any disagreement = fail-safe
                let result = self.monitor.check_pair(
                    &self.get_channel(faulty.next_a()),
                    &self.get_channel(faulty.next_b()),
                );
                if result != CrossCheckResult::Ok {
                    self.state = RedundancyState::FailSafe;
                    return RedundancyAction::InitiateFailSafe;
                }
                RedundancyAction::Normal
            }
            RedundancyState::FailSafe => RedundancyAction::EmergencyLand,
            _ => RedundancyAction::EmergencyLand,
        }
    }
}
```

---

## Sensor-Level Redundancy

### Triple IMU Configuration

| IMU | Position | Interface | Manufacturer |
|---|---|---|---|
| ICM-42688-P | FC-A dedicated | SPI0 @10MHz | TDK InvenSense |
| ICM-42688-P | FC-B dedicated | SPI1 @10MHz | TDK InvenSense |
| BMI088 | FC-C dedicated | SPI2 @10MHz | Bosch (diverse!) |

**Diversity rationale**: Using two different manufacturers prevents correlated failures
from common-cause events (vibration resonance, EMI, temperature drift).

### GPS Redundancy

```rust
pub struct DualGps {
    primary:   Ublox<Uart0>,   // u-blox M9N — main
    secondary: Ublox<Uart1>,   // u-blox M9N or MTK — backup
    health: [GpsHealth; 2],
}

impl DualGps {
    pub fn best_fix(&self) -> Option<&NavPvt> {
        match (&self.health[0], &self.health[1]) {
            (GpsHealth::GoodFix(a), GpsHealth::GoodFix(b)) => {
                // Use the one with more satellites and lower PDOP
                if a.satellites >= b.satellites { Some(a) } else { Some(b) }
            }
            (GpsHealth::GoodFix(a), _) => Some(a),
            (_, GpsHealth::GoodFix(b)) => Some(b),
            _ => None,
        }
    }
}
```

---

## Actuator Redundancy

### Dual-Channel PWM + CAN

For safety-critical actuators (throttle, flight control surfaces):

```rust
pub struct RedundantActuator {
    primary_pwm:   PwmChannel,    // Direct PWM to servo/ESC
    backup_can:    UavcanBus,     // UAVCAN v1 path (independent bus)
    mode: ActuatorMode,
}

pub enum ActuatorMode {
    DualActive,     // Both paths active, primary takes precedence
    PrimaryOnly,    // Normal operation
    BackupOnly,     // Primary failed, using CAN
    Inhibited,      // Safety lockout
}
```

---

## Power Redundancy

```rust
pub struct PowerMonitor {
    main_voltage:   AdcChannel,   // Main battery: 3S-6S LiPo/Li-ion
    backup_voltage: AdcChannel,   // Backup: supercapacitor or small LiPo
    current_sense:  AdcChannel,   // INA228 or ACS758
    state: PowerState,
}

pub enum PowerState {
    Normal    { main_v: f32, current_a: f32 },
    LowBattery { main_v: f32, remaining_pct: f32 },
    CriticalBattery,   // < 20% → force RTL
    MainFailed,        // Running on backup
    BothFailed,        // Immediate emergency descent
}
```