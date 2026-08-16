use crate::frame::JflError;
use heapless::Vec;
use hkdf::Hkdf;
use sha2::Sha256;
use zeroize::Zeroize;

/// Derives AES + HMAC (+ FHSS hop) keys from an ECDH shared secret.
/// Up to 96 bytes of OKM (three independent 32-byte keys).
pub struct HkdfEngine;
impl HkdfEngine {
    pub fn expand(
        salt: &[u8],
        ikm: &[u8],
        info: &[u8],
        len: usize,
    ) -> Result<Vec<u8, 96>, JflError> {
        if len > 96 {
            return Err(JflError::BufferOverflow);
        }
        let hk = Hkdf::<Sha256>::new(Some(salt), ikm);
        let mut okm = [0u8; 96];
        hk.expand(info, &mut okm[..len])
            .map_err(|_| JflError::BufferOverflow)?;
        let mut out = Vec::new();
        out.extend_from_slice(&okm[..len])
            .map_err(|_| JflError::BufferOverflow)?;
        okm.zeroize();
        Ok(out)
    }
}
