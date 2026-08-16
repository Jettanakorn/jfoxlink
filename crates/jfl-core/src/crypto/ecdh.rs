//! Ephemeral ECDH key agreement with HKDF session-key derivation.
//!
//! Curve is profile-selected: P-384 (NSA Suite B) under `defense-full`,
//! P-256 otherwise. The caller supplies a cryptographically secure RNG — a
//! hardware RNG via the HAL on embedded targets, or `OsRng` on hosts — so this
//! module has no ambient entropy source and stays `no_std`.

use crate::crypto::hkdf::HkdfEngine;
use crate::frame::JflError;
use heapless::Vec;
use rand_core::CryptoRngCore;
use zeroize::ZeroizeOnDrop;

#[cfg(not(feature = "defense-full"))]
pub use p256::ecdh::EphemeralSecret;
#[cfg(not(feature = "defense-full"))]
use p256::{elliptic_curve::sec1::ToEncodedPoint, PublicKey};
#[cfg(feature = "defense-full")]
pub use p384::ecdh::EphemeralSecret;
#[cfg(feature = "defense-full")]
use p384::{elliptic_curve::sec1::ToEncodedPoint, PublicKey};

/// Upper bound on a compressed SEC1 public key: 33 bytes (P-256) or 49 (P-384).
pub const MAX_PUBKEY_LEN: usize = 49;

/// Symmetric session keys derived from the shared secret. Zeroized on drop.
#[derive(ZeroizeOnDrop)]
pub struct SessionKeys {
    pub aes_key: [u8; 32],
    pub hmac_key: [u8; 32],
}

pub struct EcdhSession;

impl EcdhSession {
    /// Generates an ephemeral secret and its compressed public key bytes to
    /// send to the peer. `rng` MUST be a CSPRNG (HAL hardware RNG / `OsRng`).
    pub fn generate_ephemeral<R: CryptoRngCore>(
        rng: &mut R,
    ) -> Result<(EphemeralSecret, Vec<u8, MAX_PUBKEY_LEN>), JflError> {
        let secret = EphemeralSecret::random(rng);
        let point = secret.public_key().to_encoded_point(true);
        let mut pk = Vec::new();
        pk.extend_from_slice(point.as_bytes())
            .map_err(|_| JflError::BufferOverflow)?;
        Ok((secret, pk))
    }

    /// Completes the exchange: mixes our ephemeral secret with the peer's public
    /// key and derives independent AES and HMAC keys via HKDF-SHA-256.
    ///
    /// `salt` and `info` bind the derivation to a session/context; both peers
    /// must agree on them. A malformed peer key is rejected rather than
    /// producing a weak key.
    pub fn derive_session_keys(
        my_secret: &EphemeralSecret,
        peer_pubkey: &[u8],
        salt: &[u8],
        info: &[u8],
    ) -> Result<SessionKeys, JflError> {
        let peer =
            PublicKey::from_sec1_bytes(peer_pubkey).map_err(|_| JflError::UnsupportedVersion)?;
        let shared = my_secret.diffie_hellman(&peer);
        let ikm = shared.raw_secret_bytes();

        // 64 bytes of OKM split into two independent 32-byte keys.
        let okm = HkdfEngine::expand(salt, ikm.as_slice(), info, 64)?;
        let mut aes_key = [0u8; 32];
        let mut hmac_key = [0u8; 32];
        aes_key.copy_from_slice(&okm[0..32]);
        hmac_key.copy_from_slice(&okm[32..64]);
        Ok(SessionKeys { aes_key, hmac_key })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand_core::{CryptoRng, RngCore};

    /// Deterministic PRNG for reproducible tests only — NOT for production use.
    struct TestRng(u64);
    impl RngCore for TestRng {
        fn next_u32(&mut self) -> u32 {
            self.next_u64() as u32
        }
        fn next_u64(&mut self) -> u64 {
            self.0 = self
                .0
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            self.0
        }
        fn fill_bytes(&mut self, dest: &mut [u8]) {
            for chunk in dest.chunks_mut(8) {
                let r = self.next_u64().to_le_bytes();
                chunk.copy_from_slice(&r[..chunk.len()]);
            }
        }
        fn try_fill_bytes(&mut self, dest: &mut [u8]) -> Result<(), rand_core::Error> {
            self.fill_bytes(dest);
            Ok(())
        }
    }
    impl CryptoRng for TestRng {}

    #[test]
    fn two_party_agreement_yields_identical_keys() {
        let mut rng_a = TestRng(0x1111_1111);
        let mut rng_b = TestRng(0x2222_2222);
        let (sec_a, pub_a) = EcdhSession::generate_ephemeral(&mut rng_a).unwrap();
        let (sec_b, pub_b) = EcdhSession::generate_ephemeral(&mut rng_b).unwrap();

        let ka = EcdhSession::derive_session_keys(&sec_a, &pub_b, b"jfl-salt", b"jfl-v1").unwrap();
        let kb = EcdhSession::derive_session_keys(&sec_b, &pub_a, b"jfl-salt", b"jfl-v1").unwrap();

        // Both sides derive the same keys...
        assert_eq!(ka.aes_key, kb.aes_key);
        assert_eq!(ka.hmac_key, kb.hmac_key);
        // ...and the AES and HMAC keys are independent.
        assert_ne!(ka.aes_key, ka.hmac_key);
        // ...and not all-zero (the old placeholder behavior).
        assert_ne!(ka.aes_key, [0u8; 32]);
    }

    #[test]
    fn different_context_yields_different_keys() {
        let mut rng_a = TestRng(3);
        let mut rng_b = TestRng(4);
        let (sec_a, pub_a) = EcdhSession::generate_ephemeral(&mut rng_a).unwrap();
        let (sec_b, pub_b) = EcdhSession::generate_ephemeral(&mut rng_b).unwrap();

        let k1 = EcdhSession::derive_session_keys(&sec_a, &pub_b, b"salt", b"context-1").unwrap();
        let k2 = EcdhSession::derive_session_keys(&sec_b, &pub_a, b"salt", b"context-2").unwrap();
        // Same shared secret, different HKDF info -> different keys.
        assert_ne!(k1.aes_key, k2.aes_key);
    }

    #[test]
    fn rejects_malformed_peer_pubkey() {
        let mut rng = TestRng(7);
        let (sec, _pk) = EcdhSession::generate_ephemeral(&mut rng).unwrap();
        assert!(EcdhSession::derive_session_keys(&sec, &[0u8; 10], b"s", b"i").is_err());
    }
}
