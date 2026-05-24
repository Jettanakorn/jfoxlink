#![no_std]
use heapless::Vec;
use crate::frame::{JflError, JFL_CRYPTO_FLAG, JFL_GCM_TAG_LEN, JFL_HMAC_LEN, JFL_HEADER_LEN};

/// MAVLink v2 compatibility shim.
/// INVARIANT: JFOXLink wraps MAVLink payload; never mutates original MAVLink semantics.
pub struct MavlinkV2Compat;

impl MavlinkV2Compat {
    pub fn validate_mav_header(raw: &[u8]) -> Result<(), JflError> {
        if raw[0] != 0xFD { return Err(JflError::InvalidStx); }
        if (raw[2] & 0x02) == 0 { return Err(JflError::UnsupportedVersion); }
        Ok(())
    }

    /// Constructs a JFOXLink frame around a MAVLink payload.
    pub fn build_jfl_frame(
        payload: &[u8], seq: u8, sysid: u8, compid: u8, msgid: [u8; 3],
        nonce: [u8; 8], gcm_tag: [u8; 16], hmac: [u8; 32]
    ) -> Result<Vec<u8, 512>, JflError> {
        let mut buf = Vec::new();
        buf.push(0xFD).map_err(|_| JflError::BufferOverflow)?;
        buf.push(payload.len() as u8).map_err(|_| JflError::BufferOverflow)?;
        buf.push(JFL_CRYPTO_FLAG).map_err(|_| JflError::BufferOverflow)?;
        buf.push(0x00).map_err(|_| JflError::BufferOverflow)?; // compat
        buf.push(seq).map_err(|_| JflError::BufferOverflow)?;
        buf.push(sysid).map_err(|_| JflError::BufferOverflow)?;
        buf.push(compid).map_err(|_| JflError::BufferOverflow)?;
        buf.extend_from_slice(&msgid);
        buf.push(0x01).map_err(|_| JflError::BufferOverflow)?; // JFL_VERSION
        buf.extend_from_slice(&nonce);
        buf.push(0x03).map_err(|_| JflError::BufferOverflow)?; // CH_FLAGS
        buf.extend_from_slice(payload);
        buf.extend_from_slice(&gcm_tag);
        buf.extend_from_slice(&hmac);
        Ok(buf)
    }
}
