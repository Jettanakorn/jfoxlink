#![no_std]
//! RFD900x / SiK 900 MHz FHSS radio driver.
//!
//! The RFD900x presents a transparent serial (UART) interface. This driver adds
//! a length-prefixed, CRC-16 (CCITT/XModem) framing on top so a receiver can
//! find frame boundaries and reject corruption, and issues SiK "AT" commands for
//! frequency and power control. It is generic over any [`SerialLine`], which
//! makes it fully unit-testable against an in-memory loopback (no hardware).

use crate::traits::{FrequencyHop, HalError, HwStatus, PowerControl, RadioRx, RadioTx, SerialLine};

/// Start-of-frame marker for the serial framing layer.
const SOF: u8 = 0xA7;
/// Max payload the framing carries (fits a full JFOXLink frame + margin).
pub const RFD900_MAX_PAYLOAD: usize = 255;

/// CRC-16/XMODEM over `data`.
fn crc16(data: &[u8]) -> u16 {
    let mut crc: u16 = 0xFFFF;
    for &b in data {
        crc ^= (b as u16) << 8;
        for _ in 0..8 {
            crc = if crc & 0x8000 != 0 { (crc << 1) ^ 0x1021 } else { crc << 1 };
        }
    }
    crc
}

pub struct Rfd900<S: SerialLine> {
    serial: S,
    channel: u32,
    last_rssi_dbm: i8,
    fault: bool,
}

impl<S: SerialLine> Rfd900<S> {
    pub fn new(serial: S) -> Self {
        Self { serial, channel: 0, last_rssi_dbm: -128, fault: false }
    }

    /// Consume the driver and return the underlying transport.
    pub fn release(self) -> S {
        self.serial
    }

    /// Enter AT command mode and send a single SiK AT command line.
    fn at_command(&mut self, cmd: &[u8]) -> Result<(), HalError> {
        // SiK requires a 1s guard, "+++", 1s guard before AT mode. We emit the
        // sequence; timing is the integrator's responsibility at the HAL clock.
        self.serial.write_all(b"+++")?;
        self.serial.write_all(cmd)?;
        self.serial.write_all(b"\r\n")?;
        Ok(())
    }
}

impl<S: SerialLine> RadioTx for Rfd900<S> {
    fn send_frame(&mut self, payload: &[u8]) -> Result<(), HalError> {
        if payload.len() > RFD900_MAX_PAYLOAD {
            return Err(HalError::Overflow);
        }
        let crc = crc16(payload);
        // Frame: SOF | LEN | payload | CRC_hi | CRC_lo
        self.serial.write_all(&[SOF, payload.len() as u8])?;
        self.serial.write_all(payload)?;
        self.serial.write_all(&[(crc >> 8) as u8, crc as u8])?;
        Ok(())
    }
}

impl<S: SerialLine> RadioRx for Rfd900<S> {
    fn read_frame(&mut self, buf: &mut [u8]) -> Result<usize, HalError> {
        // Hunt for SOF.
        let mut b = [0u8; 1];
        loop {
            let n = self.serial.read(&mut b)?;
            if n == 0 {
                return Err(HalError::Timeout);
            }
            if b[0] == SOF {
                break;
            }
        }
        // Length.
        let mut len_b = [0u8; 1];
        if self.serial.read(&mut len_b)? != 1 {
            return Err(HalError::Framing);
        }
        let len = len_b[0] as usize;
        if len > buf.len() {
            return Err(HalError::Overflow);
        }
        // Payload (read until filled).
        let mut got = 0;
        while got < len {
            let n = self.serial.read(&mut buf[got..len])?;
            if n == 0 {
                return Err(HalError::Timeout);
            }
            got += n;
        }
        // CRC.
        let mut crc_b = [0u8; 2];
        let mut c = 0;
        while c < 2 {
            let n = self.serial.read(&mut crc_b[c..])?;
            if n == 0 {
                return Err(HalError::Timeout);
            }
            c += n;
        }
        let expected = ((crc_b[0] as u16) << 8) | crc_b[1] as u16;
        if crc16(&buf[..len]) != expected {
            self.fault = true;
            return Err(HalError::Crc);
        }
        Ok(len)
    }
}

impl<S: SerialLine> FrequencyHop for Rfd900<S> {
    fn set_channel(&mut self, channel: u32) -> Result<(), HalError> {
        // SiK register S10 selects the hop channel within the configured band.
        self.channel = channel;
        let mut cmd = [0u8; 16];
        let s = fmt_u32(b"ATS10=", channel, &mut cmd)?;
        self.at_command(s)
    }

    fn set_frequency_khz(&mut self, freq_khz: u32) -> Result<(), HalError> {
        // RFD900 900 MHz ISM band (min-freq register S8, in kHz).
        if !(902_000..=928_000).contains(&freq_khz) {
            return Err(HalError::InvalidParam);
        }
        let mut cmd = [0u8; 20];
        let s = fmt_u32(b"ATS8=", freq_khz, &mut cmd)?;
        self.at_command(s)
    }
}

impl<S: SerialLine> PowerControl for Rfd900<S> {
    fn set_power_dbm(&mut self, dbm: i8) -> Result<(), HalError> {
        // RFD900x supports 1..30 dBm via register S4 (TXPOWER).
        if !(1..=30).contains(&dbm) {
            return Err(HalError::InvalidParam);
        }
        let mut cmd = [0u8; 12];
        let s = fmt_u32(b"ATS4=", dbm as u32, &mut cmd)?;
        self.at_command(s)
    }
}

impl<S: SerialLine> HwStatus for Rfd900<S> {
    fn rssi_dbm(&mut self) -> Result<i8, HalError> {
        Ok(self.last_rssi_dbm)
    }
    fn is_healthy(&mut self) -> bool {
        !self.fault
    }
}

/// Append a decimal `value` to `prefix` in `out`, returning the filled slice.
fn fmt_u32<'a>(prefix: &[u8], value: u32, out: &'a mut [u8]) -> Result<&'a [u8], HalError> {
    if prefix.len() >= out.len() {
        return Err(HalError::Overflow);
    }
    out[..prefix.len()].copy_from_slice(prefix);
    let start = prefix.len();
    let mut idx = start;
    let mut v = value;
    loop {
        if idx >= out.len() {
            return Err(HalError::Overflow);
        }
        out[idx] = b'0' + (v % 10) as u8;
        idx += 1;
        v /= 10;
        if v == 0 {
            break;
        }
    }
    out[start..idx].reverse();
    Ok(&out[..idx])
}

#[cfg(test)]
mod tests {
    use super::*;

    /// In-memory loopback: bytes written to TX appear on RX.
    struct Loopback {
        buf: [u8; 512],
        w: usize,
        r: usize,
    }
    impl Loopback {
        fn new() -> Self {
            Self { buf: [0; 512], w: 0, r: 0 }
        }
    }
    impl SerialLine for Loopback {
        fn write_all(&mut self, data: &[u8]) -> Result<(), HalError> {
            for &b in data {
                if self.w >= self.buf.len() {
                    return Err(HalError::Overflow);
                }
                self.buf[self.w] = b;
                self.w += 1;
            }
            Ok(())
        }
        fn read(&mut self, out: &mut [u8]) -> Result<usize, HalError> {
            let mut n = 0;
            while n < out.len() && self.r < self.w {
                out[n] = self.buf[self.r];
                self.r += 1;
                n += 1;
            }
            Ok(n)
        }
    }

    #[test]
    fn frame_roundtrip_through_loopback() {
        let mut radio = Rfd900::new(Loopback::new());
        let payload = b"JFOXLINK-FRAME-\x01\x02\x03";
        radio.send_frame(payload).unwrap();

        let mut rx = [0u8; 64];
        let n = radio.read_frame(&mut rx).unwrap();
        assert_eq!(&rx[..n], payload);
    }

    #[test]
    fn corrupted_payload_fails_crc() {
        let mut radio = Rfd900::new(Loopback::new());
        radio.send_frame(b"HELLO").unwrap();
        // Flip a payload byte in the shared buffer (index: SOF, LEN, then payload).
        radio.serial.buf[2] ^= 0xFF;

        let mut rx = [0u8; 16];
        assert_eq!(radio.read_frame(&mut rx), Err(HalError::Crc));
        assert!(!radio.is_healthy());
    }

    #[test]
    fn oversize_payload_rejected() {
        let mut radio = Rfd900::new(Loopback::new());
        let big = [0u8; RFD900_MAX_PAYLOAD + 1];
        assert_eq!(radio.send_frame(&big), Err(HalError::Overflow));
    }

    #[test]
    fn power_and_frequency_bounds_enforced() {
        let mut radio = Rfd900::new(Loopback::new());
        assert_eq!(radio.set_power_dbm(0), Err(HalError::InvalidParam));
        assert_eq!(radio.set_power_dbm(31), Err(HalError::InvalidParam));
        assert!(radio.set_power_dbm(20).is_ok());
        assert_eq!(radio.set_frequency_khz(800_000), Err(HalError::InvalidParam));
        assert!(radio.set_frequency_khz(915_000).is_ok());
    }

    #[test]
    fn at_command_formats_decimal() {
        let mut out = [0u8; 16];
        let s = fmt_u32(b"ATS4=", 20, &mut out).unwrap();
        assert_eq!(s, b"ATS4=20");
        let s = fmt_u32(b"ATS8=", 915_000, &mut out).unwrap();
        assert_eq!(s, b"ATS8=915000");
    }
}
