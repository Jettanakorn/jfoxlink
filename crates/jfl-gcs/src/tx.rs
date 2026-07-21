//! Transmit side: assemble a valid encrypted JFOXLink wire frame.
//!
//! Mirrors what the decoder expects — AES-256-GCM over the payload (header as
//! AAD), then HMAC-SHA-256 over header+ciphertext+tag — so a GCS can both send
//! commands and self-test its own receive path.

use heapless::Vec as HVec;
use jfl_core::crypto::aes_gcm::GcmEngine;
use jfl_core::crypto::hmac::compute_hmac;
use jfl_core::crypto::nonce::NonceGenerator;
use jfl_core::frame::{JFL_HEADER_LEN, JFL_STX};

pub struct FrameTx {
    gcm: GcmEngine,
    hmac_key: [u8; 32],
    gen: NonceGenerator,
}

#[derive(Debug, PartialEq, Eq)]
pub enum TxError {
    PayloadTooLarge,
    NonceExhausted,
    Crypto,
}

impl FrameTx {
    pub fn new(aes: &[u8; 32], hmac: &[u8; 32], nonce_prefix: [u8; 4]) -> Self {
        Self {
            gcm: GcmEngine::new(aes),
            hmac_key: *hmac,
            gen: NonceGenerator::new(nonce_prefix),
        }
    }

    /// Encode one frame. `payload` must be <= 255 bytes (single-byte length field).
    pub fn encode(
        &mut self,
        seq: u8,
        sysid: u8,
        compid: u8,
        msgid: [u8; 3],
        payload: &[u8],
    ) -> Result<Vec<u8>, TxError> {
        if payload.len() > u8::MAX as usize {
            return Err(TxError::PayloadTooLarge);
        }
        let (nonce, _seq) = self.gen.next().map_err(|_| TxError::NonceExhausted)?;

        let mut header = [0u8; JFL_HEADER_LEN];
        header[0] = JFL_STX;
        header[1] = payload.len() as u8;
        header[2] = 0x02; // crypto active
        header[4] = seq;
        header[5] = sysid;
        header[6] = compid;
        header[7..10].copy_from_slice(&msgid);
        header[10] = 0x01; // jfl version
        header[11..23].copy_from_slice(&nonce);
        header[23] = 0x03; // channel flags

        let mut buf: HVec<u8, 512> = HVec::new();
        buf.extend_from_slice(payload).map_err(|_| TxError::PayloadTooLarge)?;
        let tag = self
            .gcm
            .encrypt(nonce, &header, &mut buf)
            .map_err(|_| TxError::Crypto)?;

        let mut raw = Vec::with_capacity(JFL_HEADER_LEN + buf.len() + 16 + 32);
        raw.extend_from_slice(&header);
        raw.extend_from_slice(&buf);
        raw.extend_from_slice(&tag);
        let mac = compute_hmac(&self.hmac_key, &raw).map_err(|_| TxError::Crypto)?;
        raw.extend_from_slice(&mac);
        Ok(raw)
    }
}
