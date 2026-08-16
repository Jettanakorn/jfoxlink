//! Frequency-hopping hop-sequence generators.
//!
//! Two generators live here:
//!
//! * [`FhssEngine`] — the original Galois-LFSR generator. Cheap, deterministic,
//!   and **predictable**: anyone who observes a short run of hops can recover
//!   the state. Acceptable for hobbyist / commercial profiles where the goal is
//!   interference avoidance rather than an adversary.
//! * [`KeyedHopSchedule`] — a cryptographically keyed schedule for the defense
//!   profiles. Each hop epoch of `channel_count` slots is a keyed Fisher–Yates
//!   permutation of the channel set, with the permutation's randomness drawn
//!   from HMAC-SHA256 under a per-session hop key. Without the key the next
//!   channel is unpredictable; with it both peers derive the identical
//!   schedule from a shared slot counter (GPS time / encrypted beacon).
//!
//! INVARIANT: Both peers must agree on the hop key, the channel count and the
//! slot counter. The slot counter is the synchronisation reference; it is
//! *not* transmitted by this module.

use crate::crypto::hkdf::HkdfEngine;
use crate::crypto::hmac::compute_hmac;
use crate::frame::JflError;
use zeroize::{Zeroize, ZeroizeOnDrop};

// ---------------------------------------------------------------------------
// LFSR generator (non-cryptographic)
// ---------------------------------------------------------------------------

/// 100-channel pseudo-random hop sequence generator.
/// INVARIANT: Synchronized via GPS time or encrypted beacon. Never repeats within window.
///
/// SECURITY: This LFSR is deterministic and predictable; it is NOT
/// cryptographically secure. Defense builds must use [`KeyedHopSchedule`]
/// rather than relying on this generator directly.
pub struct FhssEngine {
    seed: u32,
    channel_count: u32,
    index: u32,
}

/// Fallback seed used whenever the LFSR state would otherwise be zero.
/// An all-zero LFSR state is a fixed point that locks the hop sequence to a
/// single channel forever — a trivially jammable failure mode.
const FHSS_RESEED: u32 = 0xACE1_2345;

impl FhssEngine {
    pub fn new(seed: u32, channels: u32) -> Self {
        Self {
            // Never start from the degenerate all-zero state.
            seed: if seed == 0 { FHSS_RESEED } else { seed },
            // Guard against divide-by-zero in `next_channel` (`x % 0` panics).
            channel_count: channels.max(1),
            index: 0,
        }
    }
    pub fn next_channel(&mut self) -> u32 {
        // Galois LFSR hop sequence (deterministic; see security note above).
        let next = (self.seed >> 1) ^ ((self.seed & 1) * 0x8000_0057);
        // Defensive: an LFSR that reaches all-zero is stuck forever. Reseed so
        // the link keeps hopping instead of collapsing onto one frequency.
        self.seed = if next == 0 { FHSS_RESEED } else { next };
        self.index = self.index.wrapping_add(1);
        self.seed % self.channel_count
    }
}

// ---------------------------------------------------------------------------
// Keyed schedule (cryptographic)
// ---------------------------------------------------------------------------

/// Maximum number of hop channels a [`KeyedHopSchedule`] can drive.
///
/// The per-epoch permutation is stored inline (no heap) as a `[u8; 256]`, so
/// channel indices must fit in a byte. Design target is 100 channels.
pub const MAX_HOP_CHANNELS: u32 = 256;

/// HKDF `info` label binding a hop key to this schedule version. Changing the
/// permutation algorithm or keystream layout must bump the version so peers
/// running different code cannot silently derive divergent schedules.
pub const HOP_KEY_INFO: &[u8] = b"JFOXLink-FHSS-hop-key-v1";

/// Domain-separation prefix for the HMAC keystream (distinct from frame HMACs,
/// which are computed over wire bytes starting with `JFL_STX`).
const KEYSTREAM_LABEL: &[u8] = b"JFL-FHSS-KS-v1";

/// 32-byte hop key. Zeroized on drop.
#[derive(Clone, Zeroize, ZeroizeOnDrop)]
pub struct HopKey(pub [u8; 32]);

impl HopKey {
    /// Derives a hop key from session key material via HKDF-SHA256 with the
    /// [`HOP_KEY_INFO`] label. `ikm` is typically the ECDH shared secret (or an
    /// already-derived session secret); `salt` binds it to the session. Both
    /// peers must pass identical inputs.
    ///
    /// Note: [`crate::crypto::ecdh::SessionKeys`] already carries a `hop_key`
    /// derived from the same HKDF stream as the AES/HMAC keys — prefer that
    /// when an ECDH session exists. This helper is for integrators that source
    /// session secrets elsewhere (e.g. pre-shared keys / an HSM).
    ///
    /// # Errors
    /// Returns [`JflError::BufferOverflow`] if HKDF expansion fails.
    pub fn derive(ikm: &[u8], salt: &[u8]) -> Result<Self, JflError> {
        let okm = HkdfEngine::expand(salt, ikm, HOP_KEY_INFO, 32)?;
        let mut key = [0u8; 32];
        key.copy_from_slice(&okm[..32]);
        Ok(Self(key))
    }
}

/// Cryptographically keyed FHSS hop schedule.
///
/// Time is divided into hop *slots* (one dwell each). Slots are grouped into
/// *epochs* of `channel_count` slots. Within an epoch every channel is visited
/// exactly once, in an order given by a Fisher–Yates shuffle whose random
/// draws come from `HMAC-SHA256(hop_key, label ‖ epoch ‖ block)`. Consequently:
///
/// * no channel repeats within an epoch (uniform occupancy → no dwell-heavy
///   channels for a follower jammer to camp on);
/// * predicting the next channel without the hop key is equivalent to
///   forging HMAC-SHA256;
/// * both peers regenerate the identical schedule from `(key, slot)` alone,
///   so re-synchronising after a dropout is `sync_to(slot)` — no state replay.
///
/// The permutation for the current epoch is cached, so `next_channel` costs
/// one HMAC per ~4 hops amortised (32 bytes → four 64-bit draws), and
/// `channel_at` is O(1) while the slot stays inside the cached epoch.
pub struct KeyedHopSchedule {
    key: HopKey,
    channel_count: u32,
    /// Epoch whose permutation is currently held in `perm`.
    epoch: u64,
    /// Channel visited at each offset of the cached epoch.
    perm: [u8; MAX_HOP_CHANNELS as usize],
    /// Absolute slot the next `next_channel()` call will return.
    slot: u64,
    /// Whether `perm` reflects `epoch` (false until first use).
    primed: bool,
}

impl KeyedHopSchedule {
    /// Creates a schedule for `channels` hop channels under `key`.
    ///
    /// Errors with [`JflError::BufferOverflow`] if `channels` exceeds
    /// [`MAX_HOP_CHANNELS`]. A zero channel count is clamped to one (a
    /// single-channel "schedule" is degenerate but must not panic).
    ///
    /// # Errors
    /// [`JflError::BufferOverflow`] when `channels > MAX_HOP_CHANNELS`.
    pub fn new(key: HopKey, channels: u32) -> Result<Self, JflError> {
        if channels > MAX_HOP_CHANNELS {
            return Err(JflError::BufferOverflow);
        }
        Ok(Self {
            key,
            channel_count: channels.max(1),
            epoch: 0,
            perm: [0u8; MAX_HOP_CHANNELS as usize],
            slot: 0,
            primed: false,
        })
    }

    #[must_use]
    pub fn channel_count(&self) -> u32 {
        self.channel_count
    }

    /// The slot that the next call to [`next_channel`](Self::next_channel)
    /// will return.
    #[must_use]
    pub fn current_slot(&self) -> u64 {
        self.slot
    }

    /// Re-synchronises the running counter to an absolute slot number (e.g.
    /// derived from GPS time: `t_us / dwell_us`).
    pub fn sync_to(&mut self, slot: u64) {
        self.slot = slot;
    }

    /// Returns the channel for the current slot and advances to the next one.
    pub fn next_channel(&mut self) -> u32 {
        let ch = self.channel_at(self.slot);
        self.slot = self.slot.wrapping_add(1);
        ch
    }

    /// Returns the channel used in absolute slot `slot`. Random access —
    /// does not disturb the running counter.
    pub fn channel_at(&mut self, slot: u64) -> u32 {
        let n = u64::from(self.channel_count);
        let epoch = slot / n;
        // `offset < channel_count <= 256`, so the narrowing is lossless.
        #[allow(clippy::cast_possible_truncation)]
        let offset = (slot % n) as usize;
        self.ensure_epoch(epoch);
        u32::from(self.perm[offset])
    }

    /// Makes `perm` hold the permutation for `epoch`.
    fn ensure_epoch(&mut self, epoch: u64) {
        if self.primed && self.epoch == epoch {
            return;
        }
        Self::fill_permutation(&self.key, self.channel_count, epoch, &mut self.perm);
        // Boundary rule (channel_count >= 3): avoid a "double dwell" — the
        // same channel at the last slot of epoch e-1 and the first slot of
        // epoch e — by swapping offsets 0 and 1 when they'd collide. Swapping
        // 0/1 never touches the *last* offset when n >= 3, so "last channel of
        // epoch e-1" is simply the last element of its raw permutation: O(n),
        // no recursion, and identical whether reached sequentially or by
        // random access. With <= 2 channels there is no schedule secrecy to
        // protect anyway, so the raw permutation is used as-is.
        if epoch > 0 && self.channel_count >= 3 {
            let prev_last = self.raw_last_channel(epoch - 1);
            if self.perm[0] == prev_last {
                self.perm.swap(0, 1);
            }
        }
        self.epoch = epoch;
        self.primed = true;
    }

    /// Last element of the raw (pre-boundary-rule) permutation of `epoch`.
    fn raw_last_channel(&self, epoch: u64) -> u8 {
        let mut scratch = [0u8; MAX_HOP_CHANNELS as usize];
        Self::fill_permutation(&self.key, self.channel_count, epoch, &mut scratch);
        let last = scratch[(self.channel_count - 1) as usize];
        scratch.zeroize();
        last
    }

    /// Keyed Fisher–Yates: `perm[..n]` becomes a permutation of `0..n` that
    /// depends only on `(key, epoch)`.
    fn fill_permutation(key: &HopKey, n: u32, epoch: u64, perm: &mut [u8; 256]) {
        for (i, p) in perm.iter_mut().enumerate().take(n as usize) {
            // `i < n <= 256`; values 0..=255 fit in u8.
            #[allow(clippy::cast_possible_truncation)]
            {
                *p = i as u8;
            }
        }
        let mut ks = Keystream::new(key, epoch);
        // Standard inside-out order: for j = n-1 down to 1, swap(j, r) with r
        // uniform in 0..=j. Drawing 64 bits and reducing mod (j+1) <= 256 has
        // bias < 2^-56 — negligible for schedule secrecy — and, unlike
        // rejection sampling, is loop-free (bounded time, no liveness hazard).
        let mut j = n as usize;
        while j > 1 {
            j -= 1;
            let bound = (j as u64) + 1;
            #[allow(clippy::cast_possible_truncation)]
            let r = (ks.next_u64() % bound) as usize;
            perm.swap(j, r);
        }
    }
}

/// HMAC-SHA256 counter-mode keystream: block `b` of epoch `e` is
/// `HMAC(key, KEYSTREAM_LABEL ‖ e_le64 ‖ b_le32)`, consumed as four u64 draws.
struct Keystream<'k> {
    key: &'k HopKey,
    epoch: u64,
    block: u32,
    buf: [u8; 32],
    used: usize,
}

impl<'k> Keystream<'k> {
    fn new(key: &'k HopKey, epoch: u64) -> Self {
        Self {
            key,
            epoch,
            block: 0,
            buf: [0u8; 32],
            used: 32, // force a refill on first draw
        }
    }

    fn refill(&mut self) {
        let mut msg = [0u8; KEYSTREAM_LABEL.len() + 8 + 4];
        msg[..KEYSTREAM_LABEL.len()].copy_from_slice(KEYSTREAM_LABEL);
        msg[KEYSTREAM_LABEL.len()..KEYSTREAM_LABEL.len() + 8]
            .copy_from_slice(&self.epoch.to_le_bytes());
        msg[KEYSTREAM_LABEL.len() + 8..].copy_from_slice(&self.block.to_le_bytes());
        // `compute_hmac` only fails if the key length is rejected, which cannot
        // happen for a fixed 32-byte key; fall back to a zero block rather than
        // panicking (both peers would fall back identically).
        self.buf = compute_hmac(&self.key.0, &msg).unwrap_or([0u8; 32]);
        self.block = self.block.wrapping_add(1);
        self.used = 0;
    }

    fn next_u64(&mut self) -> u64 {
        if self.used + 8 > self.buf.len() {
            self.refill();
        }
        let mut b = [0u8; 8];
        b.copy_from_slice(&self.buf[self.used..self.used + 8]);
        self.used += 8;
        u64::from_le_bytes(b)
    }
}

impl Drop for KeyedHopSchedule {
    fn drop(&mut self) {
        // `key` zeroizes itself; wipe the cached permutation too so a dumped
        // stack/heap doesn't leak the current epoch's schedule.
        self.perm.zeroize();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- LFSR ------------------------------------------------------------

    #[test]
    fn zero_seed_does_not_lock_to_single_channel() {
        let mut e = FhssEngine::new(0, 100);
        let mut seen = [false; 100];
        for _ in 0..5_000 {
            seen[e.next_channel() as usize] = true;
        }
        let distinct = seen.iter().filter(|&&s| s).count();
        // A locked LFSR would visit exactly one channel; require broad coverage.
        assert!(
            distinct > 50,
            "hop sequence collapsed: only {distinct} channels"
        );
    }

    #[test]
    fn zero_channel_count_does_not_panic() {
        let mut e = FhssEngine::new(0x1234, 0);
        // channel_count is clamped to 1, so this must not divide by zero.
        assert_eq!(e.next_channel(), 0);
    }

    #[test]
    fn channels_stay_in_range() {
        let mut e = FhssEngine::new(0xDEAD_BEEF, 37);
        for _ in 0..10_000 {
            assert!(e.next_channel() < 37);
        }
    }

    // ---- Keyed schedule ----------------------------------------------------

    fn key(b: u8) -> HopKey {
        HopKey([b; 32])
    }

    #[test]
    fn keyed_epoch_is_a_permutation_of_all_channels() {
        let mut s = KeyedHopSchedule::new(key(0x11), 100).unwrap();
        for epoch in 0..20u64 {
            let mut seen = [false; 100];
            for off in 0..100u64 {
                let ch = s.channel_at(epoch * 100 + off) as usize;
                assert!(ch < 100);
                assert!(!seen[ch], "channel {ch} repeated within epoch {epoch}");
                seen[ch] = true;
            }
            assert!(seen.iter().all(|&v| v));
        }
    }

    #[test]
    fn keyed_peers_with_same_key_agree_and_resync() {
        let mut a = KeyedHopSchedule::new(key(0x42), 100).unwrap();
        let mut b = KeyedHopSchedule::new(key(0x42), 100).unwrap();
        for _ in 0..1_000 {
            assert_eq!(a.next_channel(), b.next_channel());
        }
        // B drops out and re-syncs from a GPS-derived slot number.
        for _ in 0..333 {
            a.next_channel();
        }
        b.sync_to(a.current_slot());
        for _ in 0..500 {
            assert_eq!(a.next_channel(), b.next_channel());
        }
        // Random access agrees with the sequential path.
        assert_eq!(a.channel_at(123_456), b.channel_at(123_456));
    }

    #[test]
    fn keyed_different_keys_give_different_schedules() {
        let mut a = KeyedHopSchedule::new(key(0x01), 100).unwrap();
        let mut b = KeyedHopSchedule::new(key(0x02), 100).unwrap();
        let mut agree = 0;
        for _ in 0..1_000 {
            if a.next_channel() == b.next_channel() {
                agree += 1;
            }
        }
        // Independent uniform sequences over 100 channels agree ~1% of the
        // time; a shared/weak schedule would agree ~100%.
        assert!(agree < 50, "schedules too correlated: {agree}/1000");
    }

    #[test]
    fn keyed_epochs_differ_from_each_other() {
        // A schedule that reused one permutation every epoch would be
        // periodic with period = channel_count — trivially learnable.
        let mut s = KeyedHopSchedule::new(key(0x77), 64).unwrap();
        let e0: heapless::Vec<u32, 64> = (0..64).map(|i| s.channel_at(i)).collect();
        let e1: heapless::Vec<u32, 64> = (64..128).map(|i| s.channel_at(i)).collect();
        assert_ne!(e0, e1);
    }

    #[test]
    fn keyed_no_double_dwell_across_epoch_boundary() {
        for k in 0..64u8 {
            let mut s = KeyedHopSchedule::new(key(k), 7).unwrap();
            for epoch in 1..200u64 {
                let last = s.channel_at(epoch * 7 - 1);
                let first = s.channel_at(epoch * 7);
                assert_ne!(last, first, "double dwell at epoch {epoch}, key {k}");
            }
        }
        // Three channels is the smallest count the rule applies to.
        let mut s = KeyedHopSchedule::new(key(0x99), 3).unwrap();
        let mut prev = s.next_channel();
        for _ in 0..3_000 {
            let cur = s.next_channel();
            assert_ne!(cur, prev);
            prev = cur;
        }
        // Two channels: rule disabled, but must still be a valid permutation.
        let mut two = KeyedHopSchedule::new(key(0x99), 2).unwrap();
        for _ in 0..100 {
            let a = two.next_channel();
            let b = two.next_channel();
            assert!(a < 2 && b < 2 && a != b);
        }
    }

    #[test]
    fn keyed_boundary_rule_is_consistent_between_random_and_sequential_access() {
        // Sequential caching and random access must yield the identical
        // schedule (a divergence would desynchronise peers).
        let mut seq = KeyedHopSchedule::new(key(0x5A), 5).unwrap();
        let mut rnd = KeyedHopSchedule::new(key(0x5A), 5).unwrap();
        let mut sequential = heapless::Vec::<u32, 400>::new();
        for _ in 0..400 {
            sequential.push(seq.next_channel()).unwrap();
        }
        // Visit slots in a scrambled order.
        for i in (0..400u64).rev().step_by(3).chain(0..400) {
            assert_eq!(rnd.channel_at(i), sequential[i as usize], "slot {i}");
        }
    }

    #[test]
    fn keyed_uniform_coverage_over_many_epochs() {
        let mut s = KeyedHopSchedule::new(key(0xC3), 100).unwrap();
        let mut hist = [0u32; 100];
        for _ in 0..10_000 {
            hist[s.next_channel() as usize] += 1;
        }
        // Permutation per epoch → every channel hit exactly once per 100 hops.
        assert!(hist.iter().all(|&h| h == 100));
    }

    #[test]
    fn keyed_rejects_too_many_channels_and_clamps_zero() {
        assert!(matches!(
            KeyedHopSchedule::new(key(0), MAX_HOP_CHANNELS + 1),
            Err(JflError::BufferOverflow)
        ));
        let mut one = KeyedHopSchedule::new(key(0), 0).unwrap();
        assert_eq!(one.channel_count(), 1);
        assert_eq!(one.next_channel(), 0);
        assert_eq!(one.next_channel(), 0);
        let mut max = KeyedHopSchedule::new(key(0), MAX_HOP_CHANNELS).unwrap();
        for _ in 0..1_000 {
            assert!(max.next_channel() < MAX_HOP_CHANNELS);
        }
    }

    #[test]
    fn keyed_slot_counter_wraps_without_panic() {
        let mut s = KeyedHopSchedule::new(key(0xEE), 100).unwrap();
        s.sync_to(u64::MAX - 1);
        let _ = s.next_channel();
        let _ = s.next_channel();
        let _ = s.next_channel(); // wrapped to slot 0/1
        assert_eq!(s.current_slot(), 1);
    }

    #[test]
    fn hop_key_derivation_is_deterministic_and_context_bound() {
        let a = HopKey::derive(b"shared-secret", b"session-1").unwrap();
        let b = HopKey::derive(b"shared-secret", b"session-1").unwrap();
        let c = HopKey::derive(b"shared-secret", b"session-2").unwrap();
        assert_eq!(a.0, b.0);
        assert_ne!(a.0, c.0);
        assert_ne!(a.0, [0u8; 32]);
    }
}
