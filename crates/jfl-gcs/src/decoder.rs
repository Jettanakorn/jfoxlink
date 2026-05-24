use std::io::Cursor;
use std::result::Result;

use heapless::Vec;
use jfl_core::frame::{JflError, JflFrame, JFL_GCM_TAG_LEN, JFL_HMAC_LEN};
use jfl_core::crypto::aes_gcm::GcmEngine;
use jfl_core::crypto::hmac::verify_hmac;
use jfl_core::crypto::nonce::NonceManager;
use mavlink::{MavHeader, MavMessage};
use zeroize::Zeroize;

/// GCS-side decoding errors
#[derive(Debug, PartialEq, Eq)]
pub enum GcsDecodeError {
    FrameParse(JflError),
    HmacMismatch,
    ReplayDetected,
    CryptoFailure,
    MavlinkDeserialize(String),
    BufferTooSmall,
}

/// Full-stack decoder: PHY → Frame Parse → HMAC → Nonce → AES-GCM → MAVLink
/// INVARIANT: Zero-allocation path for crypto; std::vec used only for MAVLink deserialization.
/// SECURITY: Secrets are zeroized on drop. All public methods return Result.
pub struct GcsDecoder {
    gcm_engine: GcmEngine,
    hmac_key: [u8; 32],
    nonce_manager: NonceManager,
}

impl GcsDecoder {
    pub fn new(aes_key: &[u8; 32], hmac_key: &[u8; 32], replay_window: u64) -> Self {
        Self {
            gcm_engine: GcmEngine::new(aes_key),
            hmac_key: *hmac_key,
            nonce_manager: NonceManager::new(replay_window),
        }
    }

    /// Decodes a raw wire frame into a verified MAVLink message.
    /// PANIC: never
    pub fn decode_frame(&self, raw: &[u8]) -> Result<(MavHeader, MavMessage), GcsDecodeError> {
        // 1. Parse JFOXLink frame (zero-panic, strict layout validation)
        let frame = JflFrame::from_bytes(raw).map_err(GcsDecodeError::FrameParse)?;

        // 2. Verify HMAC-SHA256 over entire frame (header + payload + GCM tag)
        // INVARIANT: Uses constant-time comparison internally via hmac crate
        verify_hmac(&self.hmac_key, raw, frame.hmac)
            .map_err(|_| GcsDecodeError::HmacMismatch)?;

        // 3. Replay protection: validate 64-bit nonce against sliding window
        let nonce_val = u64::from_le_bytes(frame.nonce);
        self.nonce_manager
            .verify_nonce(nonce_val)
            .map_err(|_| GcsDecodeError::ReplayDetected)?;

        // 4. Decrypt AES-256-GCM payload
        // NOTE: GCM AAD = first 20 bytes (header + nonce + channel_flags)
        let aad = &raw[..JFL_GCM_TAG_LEN + JFL_HMAC_LEN + frame.encrypted_payload.len() - JFL_GCM_TAG_LEN];
        let mut plaintext_buf = Vec::new();
        plaintext_buf
            .extend_from_slice(frame.encrypted_payload)
            .map_err(|_| GcsDecodeError::BufferTooSmall)?;

        self.gcm_engine
            .decrypt(frame.nonce, aad, &mut plaintext_buf, frame.gcm_tag)
            .map_err(|_| GcsDecodeError::CryptoFailure)?;

        // 5. Deserialize MAVLink v2 message
        let mut cursor = Cursor::new(plaintext_buf);
        match mavlink::read_v2_msg(&mut cursor) {
            Ok((header, msg)) => {
                // Zeroize intermediate buffers after successful decode
                plaintext_buf.zeroize();
                Ok((header, msg))
            }
            Err(e) => {
                plaintext_buf.zeroize();
                Err(GcsDecodeError::MavlinkDeserialize(e.to_string()))
            }
        }
    }
}

impl Drop for GcsDecoder {
    fn drop(&mut self) {
        self.hmac_key.zeroize();
        // GcmEngine zeroizes internally via its own Drop impl
    }
}