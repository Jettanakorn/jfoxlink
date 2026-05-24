#![no_std]
use aes_gcm::{Aes256Gcm, AeadInPlace, KeyInit, Nonce};
use aes_gcm::aead::generic_array::GenericArray;
use heapless::Vec;
use zeroize::Zeroize;
use crate::frame::JflError;

/// SECURITY: Zeroizes key on drop. Never allocates. In-place encryption.
pub struct GcmEngine { key: [u8; 32] }
impl GcmEngine {
    pub fn new(key: &[u8; 32]) -> Self { Self { key: *key } }
    pub fn encrypt(&self, nonce: [u8; 12], aad: &[u8], payload: &mut Vec<u8, 512>) -> Result<[u8; 16], JflError> {
        let cipher = Aes256Gcm::new_from_slice(&self.key).map_err(|_| JflError::BufferOverflow)?;
        let tag = cipher.encrypt_in_place_detached(Nonce::from_slice(&nonce), aad, payload.as_mut_slice())
            .map_err(|_| JflError::CryptoTagMismatch)?;
        let mut out = [0u8; 16];
        out.copy_from_slice(tag.as_slice());
        Ok(out)
    }
    pub fn decrypt(&self, nonce: [u8; 12], aad: &[u8], payload: &mut Vec<u8, 512>, tag: &[u8; 16]) -> Result<(), JflError> {
        let cipher = Aes256Gcm::new_from_slice(&self.key).map_err(|_| JflError::BufferOverflow)?;
        let tag = GenericArray::from_slice(tag);
        cipher.decrypt_in_place_detached(Nonce::from_slice(&nonce), aad, payload.as_mut_slice(), tag)
            .map_err(|_| JflError::CryptoTagMismatch)?;
        Ok(())
    }
}
impl Drop for GcmEngine { fn drop(&mut self) { self.key.zeroize(); } }
