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
pub const JFL_HEADER_LEN: usize = 24;
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
    pub nonce: [u8; 12],
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
        if raw[0] != JFL_STX {
            return Err(JflError::InvalidStx);
        }

        let payload_len = raw[1] as usize;
        let expected = JFL_HEADER_LEN + payload_len + JFL_GCM_TAG_LEN + JFL_HMAC_LEN;
        if raw.len() != expected {
            return Err(JflError::LengthMismatch);
        }

        // SECURITY: Verify crypto active flag before processing
        if (raw[2] & 0x02) == 0 {
            return Err(JflError::UnsupportedVersion);
        }

        Ok(JflFrame {
            stx: raw[0],
            len: raw[1],
            incompat_flags: raw[2],
            compat_flags: raw[3],
            seq: raw[4],
            sysid: raw[5],
            compid: raw[6],
            msgid: [raw[7], raw[8], raw[9]],
            jfl_version: raw[10],
            nonce: raw[11..23]
                .try_into()
                .map_err(|_| JflError::LengthMismatch)?,
            channel_flags: raw[23],
            encrypted_payload: &raw[JFL_HEADER_LEN..JFL_HEADER_LEN + payload_len],
            gcm_tag: raw[JFL_HEADER_LEN + payload_len..][..JFL_GCM_TAG_LEN]
                .try_into()
                .map_err(|_| JflError::LengthMismatch)?,
            hmac: raw[raw.len() - JFL_HMAC_LEN..]
                .try_into()
                .map_err(|_| JflError::LengthMismatch)?,
        })
    }

    /// Serializes frame to mutable buffer.
    /// PANIC: never
    pub fn to_bytes<'b>(&self, buf: &'b mut Vec<u8, 512>) -> Result<&'b [u8], JflError> {
        if buf.capacity()
            < JFL_HEADER_LEN + self.encrypted_payload.len() + JFL_GCM_TAG_LEN + JFL_HMAC_LEN
        {
            return Err(JflError::BufferOverflow);
        }
        buf.clear();
        // Propagate capacity failures instead of silently dropping bytes: a
        // truncated frame would carry a length field that disagrees with its
        // contents and fail HMAC/parse at the far end (or worse, misparse).
        // (push/extend_from_slice have different Err types, so map each inline.)
        buf.push(self.stx).map_err(|_| JflError::BufferOverflow)?;
        buf.push(self.len).map_err(|_| JflError::BufferOverflow)?;
        buf.push(self.incompat_flags)
            .map_err(|_| JflError::BufferOverflow)?;
        buf.push(self.compat_flags)
            .map_err(|_| JflError::BufferOverflow)?;
        buf.push(self.seq).map_err(|_| JflError::BufferOverflow)?;
        buf.push(self.sysid).map_err(|_| JflError::BufferOverflow)?;
        buf.push(self.compid)
            .map_err(|_| JflError::BufferOverflow)?;
        buf.extend_from_slice(&self.msgid)
            .map_err(|_| JflError::BufferOverflow)?;
        buf.push(self.jfl_version)
            .map_err(|_| JflError::BufferOverflow)?;
        buf.extend_from_slice(&self.nonce)
            .map_err(|_| JflError::BufferOverflow)?;
        buf.push(self.channel_flags)
            .map_err(|_| JflError::BufferOverflow)?;
        buf.extend_from_slice(self.encrypted_payload)
            .map_err(|_| JflError::BufferOverflow)?;
        buf.extend_from_slice(self.gcm_tag.as_slice())
            .map_err(|_| JflError::BufferOverflow)?;
        buf.extend_from_slice(self.hmac.as_slice())
            .map_err(|_| JflError::BufferOverflow)?;
        Ok(buf.as_slice())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Builds a minimal well-formed frame (empty payload) as raw bytes.
    fn make_frame(payload: &[u8]) -> Vec<u8, 512> {
        let mut v: Vec<u8, 512> = Vec::new();
        v.push(JFL_STX).unwrap();
        v.push(payload.len() as u8).unwrap();
        v.push(0x02).unwrap(); // incompat: crypto active
        v.push(0x00).unwrap(); // compat
        for _ in 0..7 {
            v.push(0).unwrap();
        } // seq,sysid,compid,msgid[3],jfl_version
        for _ in 0..12 {
            v.push(0).unwrap();
        } // nonce
        v.push(0).unwrap(); // channel_flags  -> header is 24 bytes total
        v.extend_from_slice(payload).unwrap();
        v.extend_from_slice(&[0u8; JFL_GCM_TAG_LEN]).unwrap();
        v.extend_from_slice(&[0u8; JFL_HMAC_LEN]).unwrap();
        v
    }

    #[test]
    fn parses_well_formed_frame() {
        let raw = make_frame(&[1, 2, 3, 4]);
        let f = JflFrame::from_bytes(&raw).expect("should parse");
        assert_eq!(f.stx, JFL_STX);
        assert_eq!(f.encrypted_payload, &[1, 2, 3, 4]);
    }

    // JflFrame has no Debug/PartialEq (it borrows), so match on the error arm.
    #[test]
    fn rejects_crypto_inactive_frame() {
        let mut raw = make_frame(&[]);
        raw[2] = 0x00; // clear crypto-active bit
        assert!(matches!(
            JflFrame::from_bytes(&raw),
            Err(JflError::UnsupportedVersion)
        ));
    }

    #[test]
    fn rejects_bad_stx() {
        let mut raw = make_frame(&[]);
        raw[0] = 0x00;
        assert!(matches!(
            JflFrame::from_bytes(&raw),
            Err(JflError::InvalidStx)
        ));
    }

    #[test]
    fn rejects_length_mismatch() {
        let mut raw = make_frame(&[9, 9, 9]);
        raw[1] = 100; // claim 100-byte payload the buffer doesn't have
        assert!(matches!(
            JflFrame::from_bytes(&raw),
            Err(JflError::LengthMismatch)
        ));
    }

    /// The crate's foremost invariant: `from_bytes` must never panic, for ANY input.
    #[test]
    fn never_panics_on_arbitrary_or_truncated_input() {
        // Every truncation of a valid frame.
        let valid = make_frame(&[7; 16]);
        for len in 0..=valid.len() {
            let _ = JflFrame::from_bytes(&valid[..len]);
        }
        // A deterministic spread of adversarial byte patterns at many lengths.
        for len in 0..300usize {
            let mut buf: Vec<u8, 512> = Vec::new();
            for i in 0..len {
                let _ = buf.push(((i * 31 + len * 7) ^ 0xA5) as u8);
            }
            let _ = JflFrame::from_bytes(&buf); // must return, never panic
        }
    }

    #[test]
    fn roundtrip_serialize_parse() {
        let raw = make_frame(&[5, 6, 7]);
        let f = JflFrame::from_bytes(&raw).unwrap();
        let mut out: Vec<u8, 512> = Vec::new();
        let bytes = f.to_bytes(&mut out).unwrap();
        assert_eq!(bytes, &raw[..]);
    }
}

#[cfg(test)]
mod proptests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        /// The never-panic invariant, fuzzed: any byte string returns Ok/Err.
        #[test]
        fn from_bytes_never_panics(data in proptest::collection::vec(any::<u8>(), 0..600)) {
            let _ = JflFrame::from_bytes(&data);
        }

        /// Any well-formed frame parses and exposes its fields faithfully.
        #[test]
        fn well_formed_frames_parse_faithfully(
            payload in proptest::collection::vec(any::<u8>(), 0..=255usize),
            seq in any::<u8>(),
            sysid in any::<u8>(),
            compid in any::<u8>(),
        ) {
            let mut raw: Vec<u8, 512> = Vec::new();
            raw.push(JFL_STX).unwrap();
            raw.push(payload.len() as u8).unwrap();
            raw.push(0x02).unwrap(); // crypto active
            raw.push(0x00).unwrap();
            raw.push(seq).unwrap();
            raw.push(sysid).unwrap();
            raw.push(compid).unwrap();
            raw.extend_from_slice(&[1, 0, 0]).unwrap(); // msgid
            raw.push(0x01).unwrap(); // version
            raw.extend_from_slice(&[0xAB; 12]).unwrap(); // nonce
            raw.push(0x03).unwrap(); // channel flags
            raw.extend_from_slice(&payload).unwrap();
            raw.extend_from_slice(&[0u8; JFL_GCM_TAG_LEN]).unwrap();
            raw.extend_from_slice(&[0u8; JFL_HMAC_LEN]).unwrap();

            let f = JflFrame::from_bytes(&raw).expect("well-formed frame must parse");
            prop_assert_eq!(f.seq, seq);
            prop_assert_eq!(f.sysid, sysid);
            prop_assert_eq!(f.compid, compid);
            prop_assert_eq!(f.encrypted_payload.len(), payload.len());
            prop_assert_eq!(f.encrypted_payload, &payload[..]);
        }
    }
}
