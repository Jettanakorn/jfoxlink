#![no_std]
#[cfg(feature = "defense-full")] use p384::ecdh::EphemeralSecret;
#[cfg(not(feature = "defense-full"))] use p256::ecdh::EphemeralSecret;
use crate::frame::JflError;

/// Ephemeral key exchange generator.
/// SECURITY: Keys are generated per-session. Zeroized on drop.
pub struct EcdhSession;
impl EcdhSession {
    #[cfg(feature = "std")]
    pub fn generate_ephemeral() -> (EphemeralSecret, [u8; 49]) {
        let secret = EphemeralSecret::random(&mut rand::thread_rng());
        let pk = secret.public_key().to_encoded_point(true);
        (secret, *pk.as_bytes())
    }
    // Embedded CSPRNG hook omitted; use HAL RNG in production.
    pub fn derive_session_keys(_my_sec: &EphemeralSecret, _peer_pk: &[u8], _info: &[u8]) -> Result<([u8;32],[u8;32]), JflError> {
        Ok(([0u8;32], [0u8;32])) // Placeholder for HKDF integration
    }
}
