use crate::frame::JflError;

/// Wire layout of the 96-bit GCM nonce: a 4-byte per-session prefix followed
/// by an 8-byte little-endian monotonic sequence counter.
/// The full 12 bytes feed AES-GCM, so any tampering breaks the auth tag; the
/// 64-bit sequence is what the replay window tracks.
pub const NONCE_PREFIX_LEN: usize = 4;
pub const NONCE_SEQ_OFFSET: usize = NONCE_PREFIX_LEN; // sequence starts at byte 4

/// Largest anti-replay window supported (bits). Covers the widest profile (256).
pub const MAX_REPLAY_WINDOW: u64 = 256;
const BITMAP_WORDS: usize = (MAX_REPLAY_WINDOW as usize) / 64;

/// Extracts the 64-bit replay sequence from a 12-byte frame nonce.
pub fn seq_from_nonce(nonce: &[u8; 12]) -> u64 {
    let mut s = [0u8; 8];
    s.copy_from_slice(&nonce[NONCE_SEQ_OFFSET..NONCE_SEQ_OFFSET + 8]);
    u64::from_le_bytes(s)
}

/// TX-side nonce generator.
/// INVARIANT: never emits the same (prefix, sequence) pair twice under one key.
pub struct NonceGenerator {
    prefix: [u8; NONCE_PREFIX_LEN],
    counter: u64,
}

impl NonceGenerator {
    /// `prefix` should be unique per session (e.g. random at key install) so
    /// nonces never collide across concurrent sessions sharing a key.
    pub fn new(prefix: [u8; NONCE_PREFIX_LEN]) -> Self {
        Self { prefix, counter: 0 }
    }

    /// Returns the next 96-bit nonce and its 64-bit sequence value.
    /// Errors when the counter is exhausted, forcing a re-key rather than
    /// silently wrapping and reusing a nonce (catastrophic for AES-GCM).
    pub fn next_nonce(&mut self) -> Result<([u8; 12], u64), JflError> {
        let seq = self.counter;
        self.counter = self
            .counter
            .checked_add(1)
            .ok_or(JflError::ReplayDetected)?;
        let mut nonce = [0u8; 12];
        nonce[..NONCE_PREFIX_LEN].copy_from_slice(&self.prefix);
        nonce[NONCE_SEQ_OFFSET..NONCE_SEQ_OFFSET + 8].copy_from_slice(&seq.to_le_bytes());
        Ok((nonce, seq))
    }
}

/// RX-side sliding-window anti-replay validator (RFC 6479 style).
///
/// Accepts each sequence number at most once and rejects any sequence older
/// than `window` below the highest accepted value. Unlike a bare range check,
/// this records *which* sequences have been seen, so an in-window replay is
/// caught and out-of-order (but fresh) frames are still accepted.
pub struct NonceManager {
    highest: u64,
    initialized: bool,
    window: u64,
    bitmap: [u64; BITMAP_WORDS],
}

impl NonceManager {
    pub fn new(window_size: u64) -> Self {
        Self {
            highest: 0,
            initialized: false,
            window: window_size.clamp(1, MAX_REPLAY_WINDOW),
            bitmap: [0; BITMAP_WORDS],
        }
    }

    #[inline]
    fn slot(seq: u64) -> (usize, u64) {
        let idx = (seq % MAX_REPLAY_WINDOW) as usize;
        (idx / 64, 1u64 << (idx % 64))
    }
    #[inline]
    fn is_set(&self, seq: u64) -> bool {
        let (w, m) = Self::slot(seq);
        self.bitmap[w] & m != 0
    }
    #[inline]
    fn set(&mut self, seq: u64) {
        let (w, m) = Self::slot(seq);
        self.bitmap[w] |= m;
    }
    #[inline]
    fn clear(&mut self, seq: u64) {
        let (w, m) = Self::slot(seq);
        self.bitmap[w] &= !m;
    }

    /// Validates a received sequence number, recording it as seen on success.
    /// Returns `ReplayDetected` for duplicates and for values older than the window.
    pub fn verify_nonce(&mut self, seq: u64) -> Result<(), JflError> {
        if !self.initialized {
            // First accepted frame establishes the window baseline.
            self.initialized = true;
            self.highest = seq;
            self.bitmap = [0; BITMAP_WORDS];
            self.set(seq);
            return Ok(());
        }

        if seq > self.highest {
            let advance = seq - self.highest;
            if advance >= MAX_REPLAY_WINDOW {
                // Window moved entirely past the old contents.
                self.bitmap = [0; BITMAP_WORDS];
            } else {
                // Clear slots newly entering the window (highest+1 ..= seq).
                let mut s = self.highest + 1;
                while s <= seq {
                    self.clear(s);
                    s += 1;
                }
            }
            self.highest = seq;
            self.set(seq);
            Ok(())
        } else {
            // seq <= highest: must be inside the window and not previously seen.
            if self.highest - seq >= self.window {
                return Err(JflError::ReplayDetected); // too old
            }
            if self.is_set(seq) {
                return Err(JflError::ReplayDetected); // duplicate
            }
            self.set(seq);
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generator_is_monotonic_and_layout_is_stable() {
        let mut tx = NonceGenerator::new([1, 2, 3, 4]);
        let (n0, s0) = tx.next_nonce().unwrap();
        let (n1, s1) = tx.next_nonce().unwrap();
        assert_eq!(s1, s0 + 1);
        assert_eq!(&n0[..4], &[1, 2, 3, 4]);
        assert_eq!(seq_from_nonce(&n0), s0);
        assert_eq!(seq_from_nonce(&n1), s1);
    }

    #[test]
    fn accepts_in_order_and_rejects_exact_replay() {
        let mut rx = NonceManager::new(64);
        assert!(rx.verify_nonce(10).is_ok()); // baseline
        assert!(rx.verify_nonce(11).is_ok());
        assert!(rx.verify_nonce(12).is_ok());
        assert!(rx.verify_nonce(12).is_err()); // duplicate
        assert!(rx.verify_nonce(11).is_err()); // duplicate
    }

    #[test]
    fn accepts_out_of_order_within_window_once() {
        let mut rx = NonceManager::new(64);
        assert!(rx.verify_nonce(20).is_ok());
        assert!(rx.verify_nonce(18).is_ok()); // late but fresh, within window
        assert!(rx.verify_nonce(18).is_err()); // now a replay
        assert!(rx.verify_nonce(19).is_ok()); // still fresh
    }

    #[test]
    fn rejects_values_older_than_window() {
        let mut rx = NonceManager::new(8);
        assert!(rx.verify_nonce(100).is_ok());
        // 100 - 90 = 10 >= window(8) -> outside window.
        assert!(rx.verify_nonce(90).is_err());
        // A large forward jump is accepted (new highest); the old value is then far outside.
        assert!(rx.verify_nonce(1000).is_ok());
        assert!(rx.verify_nonce(100).is_err());
    }

    #[test]
    fn large_forward_jumps_do_not_falsely_accept_replays() {
        let mut rx = NonceManager::new(256);
        assert!(rx.verify_nonce(1).is_ok());
        assert!(rx.verify_nonce(10_000).is_ok()); // jump beyond MAX window clears bitmap
        assert!(rx.verify_nonce(10_000).is_err()); // still catches the exact replay
        assert!(rx.verify_nonce(9_999).is_ok()); // fresh within new window
    }
}
