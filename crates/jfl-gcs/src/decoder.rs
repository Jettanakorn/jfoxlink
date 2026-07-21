use heapless::Vec;
use jfl_core::frame::{JflError, JflFrame, JFL_HEADER_LEN, JFL_HMAC_LEN};
use jfl_core::crypto::aes_gcm::GcmEngine;
use jfl_core::crypto::hmac::verify_hmac;
use jfl_core::crypto::nonce::{seq_from_nonce, NonceManager};
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
    ///
    /// Takes `&mut self`: the anti-replay window is stateful and must record
    /// each accepted sequence, so decoding is not a read-only operation.
    pub fn decode_frame(&mut self, raw: &[u8]) -> Result<NativeMessage, GcsDecodeError> {
        // 1. Parse JFOXLink frame (zero-panic, strict layout validation)
        let frame = JflFrame::from_bytes(raw).map_err(GcsDecodeError::FrameParse)?;

        // 2. Verify HMAC-SHA256 over header + payload + GCM tag.
        //    The HMAC covers everything EXCEPT its own trailing bytes; feeding
        //    the whole frame (including the HMAC) would never match the sender.
        let signed = &raw[..raw.len() - JFL_HMAC_LEN];
        verify_hmac(&self.hmac_key, signed, frame.hmac)
            .map_err(|_| GcsDecodeError::HmacMismatch)?;

        // 3. Replay protection: validate the 64-bit sequence against the
        //    sliding window (sequence lives in nonce bytes 4..12).
        let nonce_val = seq_from_nonce(&frame.nonce);
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

#[cfg(test)]
mod tests {
    use super::*;
    use jfl_core::crypto::hmac::compute_hmac;
    use jfl_core::crypto::nonce::NonceGenerator;
    use jfl_core::frame::JFL_STX;

    /// Encrypts a payload and assembles a fully valid wire frame the same way a
    /// conforming transmitter would, so the decoder path is exercised end to end.
    fn build_encrypted_frame(aes: &[u8; 32], hmac_key: &[u8; 32], plaintext: &[u8]) -> std::vec::Vec<u8> {
        let mut gen = NonceGenerator::new([0xAA, 0xBB, 0xCC, 0xDD]);
        let (nonce, _seq) = gen.next().unwrap();

        // 24-byte header (AAD for GCM).
        let mut header = [0u8; JFL_HEADER_LEN];
        header[0] = JFL_STX;
        header[1] = plaintext.len() as u8;
        header[2] = 0x02; // crypto active
        header[4] = 0x01; // seq
        header[5] = 0x10; // sysid
        header[6] = 0x20; // compid
        header[7] = 0x01; // msgid[0]
        header[10] = 0x01; // jfl version
        header[11..23].copy_from_slice(&nonce);
        header[23] = 0x03; // channel flags

        // Encrypt payload in place, authenticating the header as AAD.
        let mut buf: Vec<u8, 512> = Vec::new();
        buf.extend_from_slice(plaintext).unwrap();
        let gcm = GcmEngine::new(aes);
        let tag = gcm.encrypt(nonce, &header, &mut buf).unwrap();

        // Assemble header + ciphertext + tag, then HMAC over all of that.
        let mut raw = std::vec::Vec::new();
        raw.extend_from_slice(&header);
        raw.extend_from_slice(&buf);
        raw.extend_from_slice(&tag);
        let mac = compute_hmac(hmac_key, &raw).unwrap();
        raw.extend_from_slice(&mac);
        raw
    }

    #[test]
    fn decodes_valid_frame_roundtrip() {
        let aes = [7u8; 32];
        let hmac_key = [9u8; 32];
        let raw = build_encrypted_frame(&aes, &hmac_key, b"CMD-ARM");

        let mut dec = GcsDecoder::new(&aes, &hmac_key, 64);
        let msg = dec.decode_frame(&raw).expect("valid frame should decode");
        assert_eq!(msg.payload.as_slice(), b"CMD-ARM");
        assert_eq!(msg.sysid, 0x10);
        assert_eq!(msg.compid, 0x20);
    }

    #[test]
    fn rejects_replayed_frame() {
        let aes = [7u8; 32];
        let hmac_key = [9u8; 32];
        let raw = build_encrypted_frame(&aes, &hmac_key, b"CMD-ARM");

        let mut dec = GcsDecoder::new(&aes, &hmac_key, 64);
        assert!(dec.decode_frame(&raw).is_ok());
        // Exact same frame again: caught by the sliding-window replay check.
        assert!(matches!(dec.decode_frame(&raw), Err(GcsDecodeError::ReplayDetected)));
    }

    #[test]
    fn rejects_tampered_ciphertext() {
        let aes = [7u8; 32];
        let hmac_key = [9u8; 32];
        let mut raw = build_encrypted_frame(&aes, &hmac_key, b"CMD-ARM");
        raw[JFL_HEADER_LEN] ^= 0xFF; // flip a ciphertext byte

        let mut dec = GcsDecoder::new(&aes, &hmac_key, 64);
        // HMAC covers the ciphertext, so tampering is caught before decryption.
        assert!(matches!(dec.decode_frame(&raw), Err(GcsDecodeError::HmacMismatch)));
    }

    #[test]
    fn rejects_wrong_hmac_key() {
        let aes = [7u8; 32];
        let raw = build_encrypted_frame(&aes, &[9u8; 32], b"CMD-ARM");

        let mut dec = GcsDecoder::new(&aes, &[0u8; 32], 64); // wrong HMAC key
        assert!(matches!(dec.decode_frame(&raw), Err(GcsDecodeError::HmacMismatch)));
    }
}