#![no_std]
use core::result::Result;
use heapless::Vec;

/// SECURITY: Never panic. All parsing returns Result<_, JflError>.
/// INVARIANT: Layout exactly matches SKILL.md protocol diagram.
#[derive(Debug, PartialEq, Eq)]
pub enum JflError {
    InvalidStx,
    LengthMismatch,
    CryptoTagMismatch,
    HmacMismatch,
    ReplayDetected,
    BufferOverflow,
    UnsupportedVersion,
}

pub const JFL_STX: u8 = 0xFD;
pub const JFL_HEADER_LEN: usize = 19;
pub const JFL_GCM_TAG_LEN: usize = 16;
pub const JFL_HMAC_LEN: usize = 32;
/// Bit 0x01 = MAVLINK_IFLAG_SIGNED, Bit 0x02 = JFOX_CRYPTO_ACTIVE
pub const JFL_CRYPTO_FLAG: u8 = 0x03;

pub struct JflFrame<'a> {
    pub stx: u8,
    pub len: u8,
    pub incompat_flags: u8,
    pub compat_flags: u8,
    pub seq: u8,
    pub sysid: u8,
    pub compid: u8,
    pub msgid: [u8; 3],
    pub jfl_version: u8,
    pub nonce: [u8; 8],
    pub channel_flags: u8,
    pub encrypted_payload: &'a [u8],
    pub gcm_tag: &'a [u8; JFL_GCM_TAG_LEN],
    pub hmac: &'a [u8; JFL_HMAC_LEN],
}

impl<'a> JflFrame<'a> {
    /// Parses raw wire bytes into structured frame.
    /// PANIC: never
    pub fn from_bytes(raw: &'a [u8]) -> Result<Self, JflError> {
        if raw.len() < JFL_HEADER_LEN + JFL_GCM_TAG_LEN + JFL_HMAC_LEN {
            return Err(JflError::LengthMismatch);
        }
        if raw[0] != JFL_STX { return Err(JflError::InvalidStx); }

        let payload_len = raw[1] as usize;
        let expected = JFL_HEADER_LEN + payload_len + JFL_GCM_TAG_LEN + JFL_HMAC_LEN;
        if raw.len() != expected { return Err(JflError::LengthMismatch); }
        
        /// SECURITY: Verify crypto active flag before processing
        if (raw[2] & 0x02) == 0 { return Err(JflError::UnsupportedVersion); }

        Ok(JflFrame {
            stx: raw[0], len: raw[1], incompat_flags: raw[2], compat_flags: raw[3],
            seq: raw[4], sysid: raw[5], compid: raw[6],
            msgid: [raw[7], raw[8], raw[9]], jfl_version: raw[10],
            nonce: raw[11..19].try_into().map_err(|_| JflError::LengthMismatch)?,
            channel_flags: raw[19],
            encrypted_payload: &raw[JFL_HEADER_LEN..JFL_HEADER_LEN + payload_len],
            gcm_tag: raw[JFL_HEADER_LEN + payload_len..][..JFL_GCM_TAG_LEN].try_into().map_err(|_| JflError::LengthMismatch)?,
            hmac: raw[raw.len() - JFL_HMAC_LEN..].try_into().map_err(|_| JflError::LengthMismatch)?,
        })
    }

    /// Serializes frame to mutable buffer.
    /// PANIC: never
    pub fn to_bytes<'b>(&self, buf: &'b mut Vec<u8, 512>) -> Result<&'b [u8], JflError> {
        if buf.capacity() < JFL_HEADER_LEN + self.encrypted_payload.len() + JFL_GCM_TAG_LEN + JFL_HMAC_LEN {
            return Err(JflError::BufferOverflow);
        }
        buf.clear();
        buf.push(self.stx); buf.push(self.len);
        buf.push(self.incompat_flags); buf.push(self.compat_flags);
        buf.push(self.seq); buf.push(self.sysid); buf.push(self.compid);
        buf.extend_from_slice(&self.msgid);
        buf.push(self.jfl_version); buf.extend_from_slice(&self.nonce);
        buf.push(self.channel_flags);
        buf.extend_from_slice(self.encrypted_payload);
        buf.extend_from_slice(self.gcm_tag.as_slice());
        buf.extend_from_slice(self.hmac.as_slice());
        Ok(buf.as_slice())
    }
}
