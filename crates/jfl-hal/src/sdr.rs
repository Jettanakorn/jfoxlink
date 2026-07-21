//! SDR (USRP / HackRF) backend interface for defense profiles.
//!
//! Concrete SDR backends drive host SDKs (UHD, SoapySDR, libhackrf) and cannot
//! be `no_std`; they live in a separate host-side crate. What lives here is the
//! **interface** the rest of JFOXLink programs against — tuning, wideband
//! capture, and adaptive-nulling control — so the anti-jam layer can target SDR
//! hardware without depending on a specific vendor SDK.
//!
//! There is intentionally no concrete USRP/HackRF driver bundled in this
//! `no_std` crate; a host crate implements [`SdrBackend`].

use crate::traits::{FrequencyHop, HalError, HwStatus, PowerControl, RadioRx, RadioTx};

/// A block of complex baseband samples (I/Q interleaved as i16 pairs), borrowed
/// from the backend's capture buffer.
pub struct IqBlock<'a> {
    pub center_khz: u32,
    pub sample_rate_hz: u32,
    /// Interleaved I,Q,I,Q… samples.
    pub samples: &'a [i16],
}

/// Adaptive anti-jam controls exposed by wideband SDR hardware, beyond the
/// baseline radio traits. A backend implements this in addition to
/// [`RadioTx`]/[`RadioRx`]/[`FrequencyHop`]/[`PowerControl`]/[`HwStatus`].
pub trait SdrBackend: RadioTx + RadioRx + FrequencyHop + PowerControl + HwStatus {
    /// Capture a wideband block for the spectral jam detector.
    fn capture(&mut self, out: &mut [i16]) -> Result<IqBlock<'_>, HalError>;

    /// Steer a spectral null toward `center_khz` with the given bandwidth to
    /// suppress a detected jammer. Returns `Unsupported` if the backend/antenna
    /// array cannot null.
    fn steer_null(&mut self, center_khz: u32, bandwidth_khz: u32) -> Result<(), HalError>;

    /// Clear any active nulls and return to omnidirectional reception.
    fn clear_nulls(&mut self) -> Result<(), HalError>;
}
