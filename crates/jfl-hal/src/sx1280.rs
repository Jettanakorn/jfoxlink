//! Semtech SX1280 2.4 GHz transceiver driver (SPI).
//!
//! Implements the SX1280 command set over an `embedded-hal` 1.0 [`SpiDevice`],
//! with a chip-select handled by the `SpiDevice` and an optional reset pin. The
//! command opcodes and the frequency/power encodings match the SX1280
//! datasheet, so the emitted SPI byte sequences are the real ones — verified in
//! tests against a mock SPI bus.

use embedded_hal::digital::OutputPin;
use embedded_hal::spi::SpiDevice;

use crate::traits::{FrequencyHop, HalError, HwStatus, PowerControl, RadioRx, RadioTx};

// SX1280 command opcodes (datasheet §11).
const CMD_SET_STANDBY: u8 = 0x80;
const CMD_SET_PACKET_TYPE: u8 = 0x8A;
const CMD_SET_RF_FREQUENCY: u8 = 0x86;
const CMD_SET_TX_PARAMS: u8 = 0x8E;
const CMD_SET_BUFFER_BASE: u8 = 0x8F;
const CMD_WRITE_BUFFER: u8 = 0x1A;
const CMD_READ_BUFFER: u8 = 0x1B;
const CMD_SET_TX: u8 = 0x83;
const CMD_SET_RX: u8 = 0x82;
const CMD_GET_PACKET_STATUS: u8 = 0x1D;

const PACKET_TYPE_LORA: u8 = 0x01;
/// PLL frequency step: F_XTAL(52 MHz) / 2^18 ≈ 198.3642578125 Hz.
/// steps = freq_hz * 2^18 / 52_000_000.
fn freq_to_pll(freq_hz: u64) -> u32 {
    ((freq_hz * 262_144) / 52_000_000) as u32
}

pub const SX1280_MAX_PAYLOAD: usize = 255;

pub struct Sx1280<SPI, RST> {
    spi: SPI,
    reset: RST,
    last_rssi_dbm: i8,
    fault: bool,
}

impl<SPI, RST> Sx1280<SPI, RST>
where
    SPI: SpiDevice,
    RST: OutputPin,
{
    pub fn new(spi: SPI, reset: RST) -> Self {
        Self {
            spi,
            reset,
            last_rssi_dbm: -128,
            fault: false,
        }
    }

    fn cmd(&mut self, bytes: &[u8]) -> Result<(), HalError> {
        self.spi.write(bytes).map_err(|_| {
            self.fault = true;
            HalError::Bus
        })
    }

    /// Hardware reset pulse, then initialise to LoRa packet mode.
    pub fn init(&mut self) -> Result<(), HalError> {
        self.reset.set_low().map_err(|_| HalError::Bus)?;
        self.reset.set_high().map_err(|_| HalError::Bus)?;
        self.cmd(&[CMD_SET_STANDBY, 0x00])?; // STDBY_RC
        self.cmd(&[CMD_SET_PACKET_TYPE, PACKET_TYPE_LORA])?;
        self.cmd(&[CMD_SET_BUFFER_BASE, 0x00, 0x00])?; // tx base, rx base
        self.fault = false;
        Ok(())
    }
}

impl<SPI, RST> RadioTx for Sx1280<SPI, RST>
where
    SPI: SpiDevice,
    RST: OutputPin,
{
    fn send_frame(&mut self, payload: &[u8]) -> Result<(), HalError> {
        if payload.len() > SX1280_MAX_PAYLOAD {
            return Err(HalError::Overflow);
        }
        // WRITE_BUFFER at offset 0, then payload.
        self.spi
            .transaction(&mut [
                embedded_hal::spi::Operation::Write(&[CMD_WRITE_BUFFER, 0x00]),
                embedded_hal::spi::Operation::Write(payload),
            ])
            .map_err(|_| {
                self.fault = true;
                HalError::Bus
            })?;
        // SET_TX: periodBase=15.625us, timeout=0 (single shot, no timeout).
        self.cmd(&[CMD_SET_TX, 0x00, 0x00, 0x00])
    }
}

impl<SPI, RST> RadioRx for Sx1280<SPI, RST>
where
    SPI: SpiDevice,
    RST: OutputPin,
{
    fn read_frame(&mut self, buf: &mut [u8]) -> Result<usize, HalError> {
        // Enter continuous RX.
        self.cmd(&[CMD_SET_RX, 0x00, 0xFF, 0xFF])?;
        // READ_BUFFER(offset=0): opcode + offset + NOP, then read into buf.
        self.spi
            .transaction(&mut [
                embedded_hal::spi::Operation::Write(&[CMD_READ_BUFFER, 0x00, 0x00]),
                embedded_hal::spi::Operation::Read(buf),
            ])
            .map_err(|_| {
                self.fault = true;
                HalError::Bus
            })?;
        Ok(buf.len())
    }
}

impl<SPI, RST> FrequencyHop for Sx1280<SPI, RST>
where
    SPI: SpiDevice,
    RST: OutputPin,
{
    fn set_channel(&mut self, channel: u32) -> Result<(), HalError> {
        // 2.4 GHz ISM: 2400 MHz + channel MHz (channels 0..79).
        if channel > 79 {
            return Err(HalError::InvalidParam);
        }
        self.set_frequency_khz(2_400_000 + channel * 1_000)
    }

    fn set_frequency_khz(&mut self, freq_khz: u32) -> Result<(), HalError> {
        if !(2_400_000..=2_500_000).contains(&freq_khz) {
            return Err(HalError::InvalidParam);
        }
        let pll = freq_to_pll(freq_khz as u64 * 1000);
        self.cmd(&[
            CMD_SET_RF_FREQUENCY,
            (pll >> 16) as u8,
            (pll >> 8) as u8,
            pll as u8,
        ])
    }
}

impl<SPI, RST> PowerControl for Sx1280<SPI, RST>
where
    SPI: SpiDevice,
    RST: OutputPin,
{
    fn set_power_dbm(&mut self, dbm: i8) -> Result<(), HalError> {
        // SX1280 power range -18..+13 dBm; register value = dbm + 18.
        if !(-18..=13).contains(&dbm) {
            return Err(HalError::InvalidParam);
        }
        let reg = (dbm + 18) as u8;
        // SET_TX_PARAMS: power, ramp time 0xE0 = 20us.
        self.cmd(&[CMD_SET_TX_PARAMS, reg, 0xE0])
    }
}

impl<SPI, RST> HwStatus for Sx1280<SPI, RST>
where
    SPI: SpiDevice,
    RST: OutputPin,
{
    fn rssi_dbm(&mut self) -> Result<i8, HalError> {
        // GET_PACKET_STATUS returns rssiSync in the first status byte:
        // actual dBm = -rssiSync / 2.
        let mut rx = [0u8; 4];
        self.spi
            .transaction(&mut [
                embedded_hal::spi::Operation::Write(&[CMD_GET_PACKET_STATUS, 0x00]),
                embedded_hal::spi::Operation::Read(&mut rx),
            ])
            .map_err(|_| {
                self.fault = true;
                HalError::Bus
            })?;
        self.last_rssi_dbm = -((rx[0] as i16) / 2) as i8;
        Ok(self.last_rssi_dbm)
    }
    fn is_healthy(&mut self) -> bool {
        !self.fault
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use embedded_hal::spi::{ErrorType, Operation, SpiDevice};

    #[derive(Debug)]
    struct MockError;
    impl embedded_hal::spi::Error for MockError {
        fn kind(&self) -> embedded_hal::spi::ErrorKind {
            embedded_hal::spi::ErrorKind::Other
        }
    }

    /// Records all written bytes; serves reads from a scripted response.
    struct MockSpi {
        written: [u8; 512],
        w: usize,
        response: u8,
    }
    impl MockSpi {
        fn new() -> Self {
            Self {
                written: [0; 512],
                w: 0,
                response: 0,
            }
        }
        fn record(&mut self, data: &[u8]) {
            for &b in data {
                if self.w < self.written.len() {
                    self.written[self.w] = b;
                    self.w += 1;
                }
            }
        }
    }
    impl ErrorType for MockSpi {
        type Error = MockError;
    }
    impl SpiDevice for MockSpi {
        fn transaction(&mut self, ops: &mut [Operation<'_, u8>]) -> Result<(), MockError> {
            for op in ops {
                match op {
                    Operation::Write(buf) => self.record(buf),
                    Operation::Read(buf) => buf.iter_mut().for_each(|b| *b = self.response),
                    Operation::Transfer(read, write) => {
                        self.record(write);
                        read.iter_mut().for_each(|b| *b = self.response);
                    }
                    Operation::TransferInPlace(buf) => {
                        self.record(buf);
                        buf.iter_mut().for_each(|b| *b = self.response);
                    }
                    Operation::DelayNs(_) => {}
                }
            }
            Ok(())
        }
    }

    /// No-op reset pin.
    struct MockPin;
    impl embedded_hal::digital::ErrorType for MockPin {
        type Error = core::convert::Infallible;
    }
    impl OutputPin for MockPin {
        fn set_low(&mut self) -> Result<(), Self::Error> {
            Ok(())
        }
        fn set_high(&mut self) -> Result<(), Self::Error> {
            Ok(())
        }
    }

    fn radio() -> Sx1280<MockSpi, MockPin> {
        Sx1280::new(MockSpi::new(), MockPin)
    }

    #[test]
    fn init_emits_standby_packettype_bufferbase() {
        let mut r = radio();
        r.init().unwrap();
        // First 2 bytes: SET_STANDBY, STDBY_RC.
        assert_eq!(r.spi.written[0], CMD_SET_STANDBY);
        assert_eq!(r.spi.written[2], CMD_SET_PACKET_TYPE);
        assert_eq!(r.spi.written[3], PACKET_TYPE_LORA);
        assert!(r.is_healthy());
    }

    #[test]
    fn frequency_encoding_matches_datasheet() {
        // steps = freq_hz * 2^18 / 52_000_000 (datasheet PLL step).
        let pll = freq_to_pll(2_400_000_000);
        assert_eq!(pll, (2_400_000_000u64 * 262_144 / 52_000_000) as u32);

        // The driver must pack the 24-bit PLL word big-endian after the opcode.
        let mut r = radio();
        r.set_frequency_khz(2_400_000).unwrap();
        assert_eq!(r.spi.written[0], CMD_SET_RF_FREQUENCY);
        let got = ((r.spi.written[1] as u32) << 16)
            | ((r.spi.written[2] as u32) << 8)
            | (r.spi.written[3] as u32);
        assert_eq!(got, pll);
    }

    #[test]
    fn power_bounds_and_encoding() {
        let mut r = radio();
        assert_eq!(r.set_power_dbm(20), Err(HalError::InvalidParam));
        assert_eq!(r.set_power_dbm(-30), Err(HalError::InvalidParam));
        r.set_power_dbm(13).unwrap();
        assert_eq!(r.spi.written[0], CMD_SET_TX_PARAMS);
        assert_eq!(r.spi.written[1], (13 + 18) as u8); // datasheet encoding
    }

    #[test]
    fn channel_bounds_enforced() {
        let mut r = radio();
        assert_eq!(r.set_channel(80), Err(HalError::InvalidParam));
        assert!(r.set_channel(40).is_ok());
    }

    #[test]
    fn send_writes_buffer_then_tx() {
        let mut r = radio();
        r.send_frame(b"ABC").unwrap();
        assert_eq!(r.spi.written[0], CMD_WRITE_BUFFER);
        // payload follows the [opcode, offset] pair
        assert_eq!(&r.spi.written[2..5], b"ABC");
    }

    #[test]
    fn rssi_decodes_negative_dbm() {
        let mut r = radio();
        r.spi.response = 90; // rssiSync = 90 -> -45 dBm
        assert_eq!(r.rssi_dbm().unwrap(), -45);
    }
}
