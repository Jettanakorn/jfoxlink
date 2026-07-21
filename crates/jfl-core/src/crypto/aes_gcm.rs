// aes-gcm 0.10 pins generic-array 0.14, whose GenericArray helpers are
// deprecated in favor of generic-array 1.x. The deprecation is a transitive
// dependency artifact, not a correctness issue; silence it module-wide until
// aes-gcm is bumped.
#![allow(deprecated)]

use crate::frame::JflError;
use aes_gcm::aead::generic_array::GenericArray;
use aes_gcm::{AeadInPlace, Aes256Gcm, KeyInit, Nonce};
use heapless::Vec;
use zeroize::Zeroize;

/// SECURITY: Zeroizes key on drop. Never allocates. In-place encryption.
pub struct GcmEngine {
    key: [u8; 32],
}
// aes-gcm 0.10 pins generic-array 0.14, whose GenericArray helpers are
// deprecated in favor of generic-array 1.x; the deprecation is a transitive
// dependency artifact, not a correctness issue. Silence it here until aes-gcm
// is bumped.
#[allow(deprecated)]
impl GcmEngine {
    pub fn new(key: &[u8; 32]) -> Self {
        Self { key: *key }
    }
    pub fn encrypt(
        &self,
        nonce: [u8; 12],
        aad: &[u8],
        payload: &mut Vec<u8, 512>,
    ) -> Result<[u8; 16], JflError> {
        let cipher = Aes256Gcm::new_from_slice(&self.key).map_err(|_| JflError::BufferOverflow)?;
        let tag = cipher
            .encrypt_in_place_detached(Nonce::from_slice(&nonce), aad, payload.as_mut_slice())
            .map_err(|_| JflError::CryptoTagMismatch)?;
        let mut out = [0u8; 16];
        out.copy_from_slice(tag.as_slice());
        Ok(out)
    }
    pub fn decrypt(
        &self,
        nonce: [u8; 12],
        aad: &[u8],
        payload: &mut Vec<u8, 512>,
        tag: &[u8; 16],
    ) -> Result<(), JflError> {
        let cipher = Aes256Gcm::new_from_slice(&self.key).map_err(|_| JflError::BufferOverflow)?;
        let tag = GenericArray::from_slice(tag);
        cipher
            .decrypt_in_place_detached(Nonce::from_slice(&nonce), aad, payload.as_mut_slice(), tag)
            .map_err(|_| JflError::CryptoTagMismatch)?;
        Ok(())
    }
}
impl Drop for GcmEngine {
    fn drop(&mut self) {
        self.key.zeroize();
    }
}
