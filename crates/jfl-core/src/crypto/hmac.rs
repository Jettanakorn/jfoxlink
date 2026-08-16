use crate::frame::JflError;
use hmac::{Hmac, Mac};
use sha2::Sha256;

type HmacSha256 = Hmac<Sha256>;

/// Computes HMAC-SHA256 over full frame (header + payload + GCM tag).
/// INVARIANT: Constant-time verification. No early exits on mismatch.
pub fn compute_hmac(key: &[u8; 32], frame_data: &[u8]) -> Result<[u8; 32], JflError> {
    let mut mac = HmacSha256::new_from_slice(key).map_err(|_| JflError::BufferOverflow)?;
    mac.update(frame_data);
    Ok(mac.finalize().into_bytes().into())
}

pub fn verify_hmac(key: &[u8; 32], frame_data: &[u8], expected: &[u8; 32]) -> Result<(), JflError> {
    let actual = compute_hmac(key, frame_data)?;
    if actual == *expected {
        Ok(())
    } else {
        Err(JflError::HmacMismatch)
    }
}
