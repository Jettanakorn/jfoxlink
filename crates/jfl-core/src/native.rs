use heapless::Vec;
use crate::frame::{JflError, JFL_CRYPTO_FLAG};

/// Native JFOXLink payload container.
pub struct NativeMessage {
    pub seq: u8,
    pub sysid: u8,
    pub compid: u8,
    pub msgid: [u8; 3],
    pub compat_flags: u8,
    pub channel_flags: u8,
    pub payload: Vec<u8, 512>,
}

/// Native compatibility helpers for building and parsing JFOXLink frames.
pub struct NativeCompat;

impl NativeCompat {
    /// Builds a JFOXLink frame around a native payload.
    pub fn build_jfl_frame(
        payload: &[u8],
        seq: u8,
        sysid: u8,
        compid: u8,
        msgid: [u8; 3],
        nonce: [u8; 12],
        gcm_tag: [u8; 16],
        hmac: [u8; 32],
    ) -> Result<Vec<u8, 600>, JflError> {
        // The length field is a single byte; a longer payload would truncate it
        // and desync the parser's length check. Reject rather than corrupt.
        if payload.len() > u8::MAX as usize { return Err(JflError::LengthMismatch); }
        let mut buf = Vec::new();
        buf.push(0xFD).map_err(|_| JflError::BufferOverflow)?;
        buf.push(payload.len() as u8).map_err(|_| JflError::BufferOverflow)?;
        buf.push(JFL_CRYPTO_FLAG).map_err(|_| JflError::BufferOverflow)?;
        buf.push(0x00).map_err(|_| JflError::BufferOverflow)?; // compat flags
        buf.push(seq).map_err(|_| JflError::BufferOverflow)?;
        buf.push(sysid).map_err(|_| JflError::BufferOverflow)?;
        buf.push(compid).map_err(|_| JflError::BufferOverflow)?;
        buf.extend_from_slice(&msgid).map_err(|_| JflError::BufferOverflow)?;
        buf.push(0x01).map_err(|_| JflError::BufferOverflow)?; // JFL version
        buf.extend_from_slice(&nonce).map_err(|_| JflError::BufferOverflow)?;
        buf.push(0x03).map_err(|_| JflError::BufferOverflow)?; // channel flags
        buf.extend_from_slice(payload).map_err(|_| JflError::BufferOverflow)?;
        buf.extend_from_slice(&gcm_tag).map_err(|_| JflError::BufferOverflow)?;
        buf.extend_from_slice(&hmac).map_err(|_| JflError::BufferOverflow)?;
        Ok(buf)
    }

    /// Parses a raw JFOXLink frame and returns a native payload message.
    pub fn parse_native_message(raw: &[u8]) -> Result<NativeMessage, JflError> {
        let frame = crate::frame::JflFrame::from_bytes(raw)?;
        let mut payload = Vec::new();
        payload.extend_from_slice(frame.encrypted_payload).map_err(|_| JflError::BufferOverflow)?;

        Ok(NativeMessage {
            seq: frame.seq,
            sysid: frame.sysid,
            compid: frame.compid,
            msgid: frame.msgid,
            compat_flags: frame.compat_flags,
            channel_flags: frame.channel_flags,
            payload,
        })
    }
}
