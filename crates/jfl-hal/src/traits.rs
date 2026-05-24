#![no_std]
use embedded_hal::spi::SpiDevice;
use embedded_hal::digital::OutputPin;

pub trait RadioTx {
    fn send_frame(&mut self, payload: &[u8]) -> Result<(), ()>;
    fn set_frequency(&mut self, freq_mhz: u32) -> Result<(), ()>;
    fn set_power(&mut self, dbm: u8) -> Result<(), ()>;
}
pub trait RadioRx {
    fn read_frame(&mut self, buf: &mut [u8]) -> Result<usize, ()>;
    fn get_rssi(&self) -> Result<i8, ()>;
}
