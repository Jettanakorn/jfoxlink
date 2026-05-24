use core::convert::TryInto;
use heapless::Vec;
use jfl_core::frame::{JflError, JflFrame, JFL_HEADER_LEN};
use jfl_core::crypto::aes_gcm::GcmEngine;
use jfl_core::crypto::hmac::verify_hmac;
use jfl_core::crypto::nonce::NonceManager;
use jfl_core::native::NativeMessage;
use zeroize::Zeroize;

/// GCS-side decoding errors
#[derive(Debug, PartialEq, Eq)]
pub enum GcsDecodeError {
    FrameParse(JflError),
    HmacMismatch,
    ReplayDetected,
    CryptoFailure,
    BufferTooSmall,
}

/// Full-stack decoder: PHY → Frame Parse → HMAC → Nonce → AES-GCM → Native payload
/// INVARIANT: Zero-allocation path for crypto.
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

    /// Decodes a raw wire frame into a verified native JFOXLink message.
    /// PANIC: never
    pub fn decode_frame(&self, raw: &[u8]) -> Result<NativeMessage, GcsDecodeError> {
        // 1. Parse JFOXLink frame (zero-panic, strict layout validation)
        let frame = JflFrame::from_bytes(raw).map_err(GcsDecodeError::FrameParse)?;

        // 2. Verify HMAC-SHA256 over entire frame (header + payload + GCM tag)
        verify_hmac(&self.hmac_key, raw, frame.hmac)
            .map_err(|_| GcsDecodeError::HmacMismatch)?;

        // 3. Replay protection: validate 64-bit nonce against sliding window
        let nonce_val = u64::from_le_bytes(frame.nonce[0..8].try_into().map_err(|_| GcsDecodeError::CryptoFailure)?);
        self.nonce_manager
            .verify_nonce(nonce_val)
            .map_err(|_| GcsDecodeError::ReplayDetected)?;

        // 4. Decrypt AES-256-GCM payload
        let aad = &raw[..JFL_HEADER_LEN];
        let mut plaintext_buf = Vec::new();
        plaintext_buf
            .extend_from_slice(frame.encrypted_payload)
            .map_err(|_| GcsDecodeError::BufferTooSmall)?;

        self.gcm_engine
            .decrypt(frame.nonce, aad, &mut plaintext_buf, frame.gcm_tag)
            .map_err(|_| GcsDecodeError::CryptoFailure)?;

        Ok(NativeMessage {
            seq: frame.seq,
            sysid: frame.sysid,
            compid: frame.compid,
            msgid: frame.msgid,
            compat_flags: frame.compat_flags,
            channel_flags: frame.channel_flags,
            payload: plaintext_buf,
        })
    }
}

impl Drop for GcsDecoder {
    fn drop(&mut self) {
        self.hmac_key.zeroize();
        // GcmEngine zeroizes internally via its own Drop impl
    }
}