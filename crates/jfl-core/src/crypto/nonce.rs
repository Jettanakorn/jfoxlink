#![no_std]
use core::sync::atomic::{AtomicU64, Ordering};
use crate::frame::JflError;

/// INVARIANT: Monotonically increasing 64-bit counter. Never reuses.
pub struct NonceManager { counter: AtomicU64, window: u64 }
impl NonceManager {
    pub fn new(window_size: u64) -> Self { Self { counter: AtomicU64::new(0), window: window_size } }
    pub fn next_nonce(&self) -> [u8; 8] { self.counter.fetch_add(1, Ordering::Relaxed).to_le_bytes() }
    pub fn verify_nonce(&self, seen: u64) -> Result<(), JflError> {
        let cur = self.counter.load(Ordering::Relaxed);
        if seen > cur || (cur - seen) > self.window { Err(JflError::ReplayDetected) } else { Ok(()) }
    }
}
