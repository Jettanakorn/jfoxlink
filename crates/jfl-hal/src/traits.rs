//! Radio hardware-abstraction traits and the shared error type.
//!
//! Drivers implement the subset of these that their hardware supports. Errors
//! are a real enum (not `()`) so a failed transmit or status read carries a
//! diagnosable cause into the channel-management layer.

/// Faults a radio driver can report.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HalError {
    /// Operation did not complete within the expected time.
    Timeout,
    /// Underlying bus (UART/SPI) transport error.
    Bus,
    /// Frame integrity check (CRC) failed on receive.
    Crc,
    /// Framing/sync error: bad start byte or length.
    Framing,
    /// Supplied buffer too small or payload too large.
    Overflow,
    /// Requested parameter out of the hardware's supported range.
    InvalidParam,
    /// Hardware busy / not ready (retry).
    NotReady,
    /// Feature not supported by this driver.
    Unsupported,
}

/// Blocking byte-stream transport (e.g. a UART) used by serial radio drivers.
/// Abstracting the transport keeps drivers testable without real hardware.
pub trait SerialLine {
    fn write_all(&mut self, data: &[u8]) -> Result<(), HalError>;
    /// Reads up to `buf.len()` bytes; returns the count read (may be 0).
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, HalError>;
}

/// Transmit a framed payload.
pub trait RadioTx {
    fn send_frame(&mut self, payload: &[u8]) -> Result<(), HalError>;
}

/// Receive a framed payload into `buf`; returns the payload length.
pub trait RadioRx {
    fn read_frame(&mut self, buf: &mut [u8]) -> Result<usize, HalError>;
}

/// Frequency-hopping control.
pub trait FrequencyHop {
    /// Select a logical hop channel index.
    fn set_channel(&mut self, channel: u32) -> Result<(), HalError>;
    /// Tune to an absolute frequency in kHz.
    fn set_frequency_khz(&mut self, freq_khz: u32) -> Result<(), HalError>;
}

/// Transmit power control.
pub trait PowerControl {
    fn set_power_dbm(&mut self, dbm: i8) -> Result<(), HalError>;
}

/// Health / link-quality readout feeding the dual-channel voter.
pub trait HwStatus {
    fn rssi_dbm(&mut self) -> Result<i8, HalError>;
    fn is_healthy(&mut self) -> bool;
}
