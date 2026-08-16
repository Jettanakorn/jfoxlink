/// 100-channel pseudo-random hop sequence generator.
/// INVARIANT: Synchronized via GPS time or encrypted beacon. Never repeats within window.
///
/// SECURITY: This LFSR is deterministic and predictable; it is NOT
/// cryptographically secure. Defense builds must drive the hop schedule from a
/// CSPRNG / keyed sequence rather than relying on this generator directly.
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

#[cfg(test)]
mod tests {
    use super::*;

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
}
