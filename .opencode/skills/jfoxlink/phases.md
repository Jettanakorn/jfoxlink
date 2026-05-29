# JFOXLink Operational Phases Reference

## Phase Overview

```
PRE-FLIGHT ──▶ KEY_EXCHANGE ──▶ CHANNEL_SYNC ──▶ TAKEOFF ──▶ CRUISE
                                                               │
                                                    EMERGENCY ◀┤ (any phase)
                                                               │
                                               POST-FLIGHT ◀──┘
```

---

## Phase 1: Pre-Flight

**Duration**: 0–10 minutes before engines armed  
**Safety requirement**: No command authority until crypto established  

### Pre-Flight Checklist (automated)

```rust
pub struct PreFlightCheck {
    psk_loaded: bool,
    ch_a_radio_ok: bool,
    ch_b_radio_ok: bool,
    gps_lock: bool,         // for FHSS sync
    fhss_synced: bool,
    ecdh_complete: bool,
    session_keys_valid: bool,
    nonce_counter_reset: bool,
    jam_check_clear: bool,
}

impl PreFlightCheck {
    /// Returns Ok only if ALL checks pass
    pub fn all_green(&self) -> Result<(), PreFlightError> {
        if !self.psk_loaded       { return Err(PreFlightError::NoPsk); }
        if !self.ch_a_radio_ok && !self.ch_b_radio_ok {
            return Err(PreFlightError::BothRadiosFailed);
        }
        // Single-channel degraded start is allowed with GCS ack
        if !self.ecdh_complete    { return Err(PreFlightError::NoCrypto); }
        if !self.session_keys_valid { return Err(PreFlightError::KeyDeriveFailed); }
        if !self.nonce_counter_reset { return Err(PreFlightError::NonceNotReset); }
        Ok(())
    }
}
```

### Key Exchange Timing Budget (Pre-Flight)

| Step | Max Duration | Action on Timeout |
|---|---|---|
| HELLO handshake | 5 seconds | Retry × 3, then abort |
| ECDH key offer | 3 seconds | Retry × 3, then abort |
| Session keys derived | 100 ms | Fatal error |
| FHSS hop sync | 10 seconds (GPS) / 30 seconds (beacon) | Warn operator |
| Radio self-test | 5 seconds per radio | Flag degraded |

### Pre-Flight Message Flow
```
GCS                                UAV
 │──── PARAM_REQUEST_LIST ──────────▶│  (verify connectivity)
 │◀─── PARAM_VALUE (link config) ────│
 │──── COMMAND_LONG(MAV_CMD_COMPONENT_ARM_DISARM=0) ──▶│  (confirm disarmed)
 │◀─── COMMAND_ACK ─────────────────│
 │     ... link quality check ...
 │◀─── LINK_NODE_STATUS (both CH)───│  (health report)
 │──── SESSION_START_CONFIRM ────────▶│  (finalize crypto)
 ■ Ready                             ■ Ready
```

---

## Phase 2: Takeoff

**Duration**: Arm → 100m AGL or stable hover  
**Link requirement**: < 20ms command latency, < 1% frame loss  
**Security**: Full GCM + HMAC active, nonce advancing  

### Message Priorities During Takeoff

```
Critical  (must not miss): ARM, DISARM, MANUAL_CONTROL, SET_MODE
Realtime  (< 20ms): ATTITUDE, ATTITUDE_QUATERNION, HIGHRES_IMU
Navigation (< 100ms): GLOBAL_POSITION_INT, LOCAL_POSITION_NED
Telemetry (< 500ms): SYS_STATUS, BATTERY_STATUS, GPS_RAW_INT
```

### Takeoff Link Monitoring

```rust
pub struct TakeoffMonitor {
    pub frame_loss_threshold: f32,    // abort if > 5%
    pub latency_threshold_ms: u32,    // warn if > 50ms, abort if > 200ms
    pub min_rssi_dbm: i16,           // warn if < -90 dBm
    pub consecutive_loss_abort: u8,   // abort after N consecutive losses
}

impl TakeoffMonitor {
    pub fn check(&self, health: &ChannelHealth) -> TakeoffDecision {
        if health.frame_loss_rate > self.frame_loss_threshold
        || health.latency_ms > self.latency_threshold_ms as f32 {
            TakeoffDecision::HoldAndInvestigate
        } else {
            TakeoffDecision::Continue
        }
    }
}
```

---

## Phase 3: Cruise / BVLOS Mission

**Duration**: Mission-dependent, hours for BVLOS  
**Link requirement**: Dual-channel active, < 5% tolerable frame loss on non-critical msgs  
**Security**: Full suite; key rotation after 1 hour or 2^32 frames  

### Cruise Channel Management

Both channels active simultaneously. Health score updated every 500ms:

```rust
pub struct CruiseChannelPolicy {
    pub ch_a_min_score:  u8,    // default: 30 — failover to Ch B if below
    pub ch_b_min_score:  u8,    // default: 20 — degrade to single if below
    pub rekey_interval:  u64,   // default: 3600 seconds
    pub jam_rekey:       bool,  // default: true
}
```

### BVLOS-Specific Adaptations

For Beyond-Visual-Line-of-Sight operations:
- Enable onboard autonomous RTH mission with no-link threshold
- Log all frames to onboard NVMe for post-flight audit
- Use 900 MHz (Ch A) as primary — better propagation over terrain
- Set T_failsafe = 10 seconds (operator may not see aircraft)

```rust
pub struct BvlosConfig {
    pub no_link_rth_threshold_secs: u32,   // default: 10
    pub autonomous_mission_fallback: bool,  // continue mission without link
    pub geofence_enforce_offline: bool,     // enforce geofence even with no link
    pub encrypted_log_on_flash: bool,       // log with AES-256-GCM to SD/NVMe
}
```

---

## Phase 4: Emergency

**Entry conditions**: Jam confirmed on both channels, hardware failure, link loss  
**Priority**: Aircraft safety above all else  

### Emergency State Machine

```rust
pub enum EmergencyTrigger {
    LinkLossTimeout,          // T_failsafe exceeded with no valid frame
    JamConfirmedBothChannels,
    HardwareFailure(FaultType),
    CryptoFailure,            // repeated HMAC/GCM failures
    OperatorTriggered,
}

pub fn enter_emergency(trigger: EmergencyTrigger, fc: &mut FlightController) {
    // 1. Command RTH immediately (onboard autopilot, no link needed)
    fc.command_rth();
    
    // 2. Reduce TX to emergency beacon only (save power, reduce jam target)
    fc.link.set_beacon_only_mode();
    
    // 3. Raise TX power to maximum on surviving channel
    fc.link.set_max_tx_power();
    
    // 4. Broadcast STATUSTEXT MAYDAY on all available channels
    fc.link.broadcast_mayday(trigger);
    
    // 5. Log emergency event to onboard flash
    fc.logger.log_emergency(trigger, fc.state.clone());
}
```

### Emergency Beacon Frame

Minimal 40-byte authenticated beacon (no encryption — pilot needs to know aircraft is alive):

```
STX | LEN=8 | INCOMPAT=0x01 | COMPAT=0 | SEQ | SYS_ID | COMP_ID | MSG_ID(HEARTBEAT)
| JFL_VERSION | NONCE | CHANNEL=0xFF | HEARTBEAT_PAYLOAD(8B) | HMAC-SHA256(32B)
```

Beacon repeats every 1 second on both channels at maximum power.

### Recovery from Emergency

```rust
pub fn recover_from_emergency(link: &mut DualChannelManager) -> bool {
    // Accept only authenticated frames from known GCS after emergency
    // Require explicit RESUME command (COMMAND_LONG 520) with correct HMAC
    // Do NOT auto-resume on link restoration alone
    link.state == LinkState::Emergency && link.resume_command_received
}
```

---

## Phase 5: Post-Flight

**Duration**: Landing → motors stopped → key zeroization  

### Post-Flight Procedures

```rust
pub async fn post_flight_shutdown(session: &mut Session, logger: &mut FlightLogger) {
    // 1. Download flight log over encrypted link
    logger.upload_to_gcs().await;
    
    // 2. Rotate PSK fingerprint (signal GCS for next-session provisioning)
    session.increment_session_counter();
    
    // 3. Zeroize ALL session keys
    session.keys.zeroize();
    session.nonce.reset_and_zeroize();
    
    // 4. Upload jamming and anomaly report to GCS
    session.jam_events.upload().await;
    
    // 5. Final HEARTBEAT before shutdown
    send_heartbeat(MavState::Standby, MavMode::PreflightDisarmed).await;
}
```

### Post-Flight Audit Log (Encrypted on Flash)

Each flight generates an encrypted audit log:

| Record | Content | Encrypted |
|---|---|---|
| KEY_EXCHANGE_RECORD | Timestamps, ECDH fingerprints | Yes |
| CHANNEL_EVENTS | Failover events, health scores | Yes |
| JAM_EVENTS | Detection times, bands, durations | Yes |
| CRYPTO_EVENTS | Re-keys, session rotations | Yes |
| FRAME_STATS | Frame counts, loss rates per phase | Yes |
| EMERGENCY_LOG | Trigger, state snapshot, recovery | Yes |

Encryption: AES-256-GCM with a **long-term audit key** (different from session keys).
The audit key is stored in the GCS HSM — not on the aircraft.

---

## Timing Budget Summary

| Phase | Max Command Latency | Frame Loss Budget | Failsafe Timeout |
|---|---|---|---|
| Pre-Flight | 5 seconds | 0% (must complete) | N/A |
| Takeoff | 20 ms | < 1% | 2 seconds |
| Cruise (VLOS) | 100 ms | < 5% non-critical | 5 seconds |
| Cruise (BVLOS) | 500 ms | < 10% non-critical | 10 seconds |
| Emergency | N/A (beacon only) | N/A | N/A |
| Post-Flight | 10 seconds | 5% (log upload) | N/A |