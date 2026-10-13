//! PSK-bound ephemeral ECDH handshake: authenticated per-peer sessions.
//!
//! `ecdh` alone agrees keys with *whoever answers*: an unauthenticated
//! exchange that a man in the middle can complete with both ends. This module
//! binds the exchange to a pre-shared key (PSK) the operator provisions for
//! each peer pair (`key_provisioner psk`):
//!
//! - **Authentication:** every handshake message carries an HMAC under a key
//!   derived from the PSK, and the session keys are derived with a PSK-bound
//!   HKDF salt. A peer without the PSK can neither produce an acceptable
//!   message nor derive the session keys.
//! - **Forward secrecy:** the session keys come from fresh ephemeral ECDH, so
//!   recording traffic and later learning the PSK does not decrypt it.
//! - **Key confirmation:** the reply's MAC covers the whole transcript (both
//!   messages), so the initiator knows the responder saw exactly its hello.
//!
//! Two messages, initiator (receiver) first:
//!
//! ```text
//! HELLO  I -> R : magic "JFH1" | ver | kind=1 | id_I | nonce_I[16] | pk_len | pk_I | mac_I
//! REPLY  R -> I : magic "JFH1" | ver | kind=2 | id_R | nonce_R[16] | pk_len | pk_R | mac_R
//!
//! k_auth = HKDF-SHA256(salt = "jfl-hs-v1 auth", ikm = PSK, info = "auth")
//! mac_I  = HMAC(k_auth, HELLO without mac)
//! mac_R  = HMAC(k_auth, HELLO || REPLY without mac)
//! salt   = HMAC(k_auth, "jfl-hs-v1 salt" || HELLO || REPLY without mac)
//! keys   = ecdh::derive_session_keys(ephemeral, peer pk, salt, "jfl-psk-ecdhe-v1")
//! ```
//!
//! Both directions share the session keys, so the two roles MUST use
//! different nonce prefixes ([`INITIATOR_NONCE_PREFIX`],
//! [`RESPONDER_NONCE_PREFIX`]): AES-GCM never tolerates a repeated nonce
//! under one key.
//!
//! SECURITY: never panics; every parse returns `Result`. Secrets zeroize on
//! drop. MAC checks are constant time.

use crate::crypto::ecdh::{EcdhSession, EphemeralSecret, SessionKeys, MAX_PUBKEY_LEN};
use crate::crypto::hkdf::HkdfEngine;
use crate::crypto::hmac::{compute_hmac, verify_hmac};
use crate::frame::JflError;
use heapless::Vec;
use rand_core::CryptoRngCore;
use zeroize::{Zeroize, ZeroizeOnDrop};

/// Handshake message magic, "JFH1".
pub const HS_MAGIC: [u8; 4] = *b"JFH1";
/// Handshake protocol version.
pub const HS_VERSION: u8 = 1;
/// Message kind: initiator's hello.
pub const KIND_HELLO: u8 = 1;
/// Message kind: responder's reply.
pub const KIND_REPLY: u8 = 2;
/// Nonce bytes in each handshake message.
pub const HS_NONCE_LEN: usize = 16;
/// Pre-shared key length.
pub const PSK_LEN: usize = 32;
/// Fixed part before the public key: magic, version, kind, id, nonce, pk_len.
const FIXED: usize = 4 + 1 + 1 + 4 + HS_NONCE_LEN + 1;
/// Largest handshake message.
pub const MAX_HS_LEN: usize = FIXED + MAX_PUBKEY_LEN + 32;

/// Nonce prefix for frames the initiator sends.
pub const INITIATOR_NONCE_PREFIX: [u8; 4] = *b"JFLI";
/// Nonce prefix for frames the responder sends.
pub const RESPONDER_NONCE_PREFIX: [u8; 4] = *b"JFLR";

const AUTH_SALT: &[u8] = b"jfl-hs-v1 auth";
const SESSION_SALT_LABEL: &[u8] = b"jfl-hs-v1 salt";
const SESSION_INFO: &[u8] = b"jfl-psk-ecdhe-v1";

/// A pre-shared key, as provisioned by the operator. Zeroized on drop.
#[derive(Zeroize, ZeroizeOnDrop)]
pub struct Psk([u8; PSK_LEN]);

impl Psk {
    /// Wrap raw key bytes.
    #[must_use]
    pub fn new(bytes: [u8; PSK_LEN]) -> Self {
        Self(bytes)
    }

    /// Parse the key file format: 64 hex digits, surrounding whitespace and
    /// `#` comment lines ignored.
    ///
    /// # Errors
    ///
    /// [`JflError::LengthMismatch`] unless exactly 32 bytes of hex are present.
    pub fn from_hex(text: &str) -> Result<Self, JflError> {
        let mut out = [0u8; PSK_LEN];
        let mut n = 0usize;
        let mut hi: Option<u8> = None;
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            for c in line.bytes() {
                let v = match c {
                    b'0'..=b'9' => c - b'0',
                    b'a'..=b'f' => c - b'a' + 10,
                    b'A'..=b'F' => c - b'A' + 10,
                    _ => {
                        out.zeroize();
                        return Err(JflError::LengthMismatch);
                    }
                };
                match hi.take() {
                    None => hi = Some(v),
                    Some(h) => {
                        if n == PSK_LEN {
                            out.zeroize();
                            return Err(JflError::LengthMismatch);
                        }
                        out[n] = (h << 4) | v;
                        n += 1;
                    }
                }
            }
        }
        if n != PSK_LEN || hi.is_some() {
            out.zeroize();
            return Err(JflError::LengthMismatch);
        }
        Ok(Self(out))
    }

    fn auth_key(&self) -> Result<[u8; 32], JflError> {
        let okm = HkdfEngine::expand(AUTH_SALT, &self.0, b"auth", 32)?;
        let mut k = [0u8; 32];
        k.copy_from_slice(&okm);
        Ok(k)
    }
}

/// A parsed handshake message (borrowed view, MAC already checked).
struct Parsed<'a> {
    id: u32,
    pk: &'a [u8],
}

fn build(
    kind: u8,
    id: u32,
    nonce: &[u8; HS_NONCE_LEN],
    pk: &[u8],
) -> Result<Vec<u8, MAX_HS_LEN>, JflError> {
    let mut m: Vec<u8, MAX_HS_LEN> = Vec::new();
    let pk_len = u8::try_from(pk.len()).map_err(|_| JflError::BufferOverflow)?;
    m.extend_from_slice(&HS_MAGIC)
        .map_err(|_| JflError::BufferOverflow)?;
    m.push(HS_VERSION).map_err(|_| JflError::BufferOverflow)?;
    m.push(kind).map_err(|_| JflError::BufferOverflow)?;
    m.extend_from_slice(&id.to_le_bytes())
        .map_err(|_| JflError::BufferOverflow)?;
    m.extend_from_slice(nonce)
        .map_err(|_| JflError::BufferOverflow)?;
    m.push(pk_len).map_err(|_| JflError::BufferOverflow)?;
    m.extend_from_slice(pk)
        .map_err(|_| JflError::BufferOverflow)?;
    Ok(m)
}

/// Check shape and kind; return the body (without MAC) and the MAC.
fn split(msg: &[u8], kind: u8) -> Result<(&[u8], &[u8; 32]), JflError> {
    if msg.len() < FIXED + 32 || msg.len() > MAX_HS_LEN {
        return Err(JflError::LengthMismatch);
    }
    if msg[0..4] != HS_MAGIC {
        return Err(JflError::InvalidStx);
    }
    if msg[4] != HS_VERSION || msg[5] != kind {
        return Err(JflError::UnsupportedVersion);
    }
    let pk_len = msg[FIXED - 1] as usize;
    if msg.len() != FIXED + pk_len + 32 {
        return Err(JflError::LengthMismatch);
    }
    let (body, mac) = msg.split_at(msg.len() - 32);
    let mac: &[u8; 32] = mac.try_into().map_err(|_| JflError::LengthMismatch)?;
    Ok((body, mac))
}

fn parsed(body: &[u8]) -> Result<Parsed<'_>, JflError> {
    let id = u32::from_le_bytes(
        body[6..10]
            .try_into()
            .map_err(|_| JflError::LengthMismatch)?,
    );
    Ok(Parsed {
        id,
        pk: &body[FIXED..],
    })
}

/// HMAC over the concatenation of several parts, without allocating.
fn mac_over(key: &[u8; 32], parts: &[&[u8]]) -> Result<[u8; 32], JflError> {
    use hmac::{Hmac, Mac};
    use sha2::Sha256;
    let mut m = <Hmac<Sha256> as Mac>::new_from_slice(key).map_err(|_| JflError::BufferOverflow)?;
    for p in parts {
        m.update(p);
    }
    Ok(m.finalize().into_bytes().into())
}

/// Constant-time equality of two MACs.
fn ct_eq(a: &[u8; 32], b: &[u8; 32]) -> bool {
    let mut d = 0u8;
    for (x, y) in a.iter().zip(b) {
        d |= x ^ y;
    }
    d == 0
}

fn session(
    secret: &EphemeralSecret,
    peer_pk: &[u8],
    k_auth: &[u8; 32],
    hello: &[u8],
    reply_body: &[u8],
) -> Result<SessionKeys, JflError> {
    let mut salt = mac_over(k_auth, &[SESSION_SALT_LABEL, hello, reply_body])?;
    let keys = EcdhSession::derive_session_keys(secret, peer_pk, &salt, SESSION_INFO);
    salt.zeroize();
    keys
}

/// The initiator's half-open handshake: holds its ephemeral secret until the
/// reply arrives.
pub struct Initiator {
    secret: EphemeralSecret,
    hello: Vec<u8, MAX_HS_LEN>,
}

impl Initiator {
    /// Start a handshake as `id`. Returns the state to keep and the HELLO to
    /// send. `rng` MUST be a CSPRNG.
    ///
    /// # Errors
    ///
    /// Only on an internal buffer or key-derivation failure.
    pub fn start<R: CryptoRngCore>(
        psk: &Psk,
        id: u32,
        rng: &mut R,
    ) -> Result<(Self, Vec<u8, MAX_HS_LEN>), JflError> {
        let (secret, pk) = EcdhSession::generate_ephemeral(rng)?;
        let mut nonce = [0u8; HS_NONCE_LEN];
        rng.fill_bytes(&mut nonce);
        let mut hello = build(KIND_HELLO, id, &nonce, &pk)?;
        let mut k_auth = psk.auth_key()?;
        let mac = compute_hmac(&k_auth, &hello)?;
        k_auth.zeroize();
        hello
            .extend_from_slice(&mac)
            .map_err(|_| JflError::BufferOverflow)?;
        Ok((
            Self {
                secret,
                hello: hello.clone(),
            },
            hello,
        ))
    }

    /// Finish with the responder's REPLY. Returns the responder's id and the
    /// session keys.
    ///
    /// # Errors
    ///
    /// [`JflError::HmacMismatch`] if the reply was not made with the same PSK
    /// over this exact hello; shape errors for a malformed reply.
    pub fn finish(self, psk: &Psk, reply: &[u8]) -> Result<(u32, SessionKeys), JflError> {
        let (body, mac) = split(reply, KIND_REPLY)?;
        let mut k_auth = psk.auth_key()?;
        let expect = mac_over(&k_auth, &[&self.hello, body])?;
        if !ct_eq(&expect, mac) {
            k_auth.zeroize();
            return Err(JflError::HmacMismatch);
        }
        let p = parsed(body)?;
        let keys = session(&self.secret, p.pk, &k_auth, &self.hello, body);
        k_auth.zeroize();
        Ok((p.id, keys?))
    }
}

/// Answer a HELLO as `id`. Returns the initiator's id, the REPLY to send and
/// the session keys. `rng` MUST be a CSPRNG.
///
/// # Errors
///
/// [`JflError::HmacMismatch`] if the hello was not made with this PSK; shape
/// errors for a malformed hello. Nothing is sent back for a rejected hello.
pub fn respond<R: CryptoRngCore>(
    psk: &Psk,
    id: u32,
    hello: &[u8],
    rng: &mut R,
) -> Result<(u32, Vec<u8, MAX_HS_LEN>, SessionKeys), JflError> {
    let (body, mac) = split(hello, KIND_HELLO)?;
    let mut k_auth = psk.auth_key()?;
    if verify_hmac(&k_auth, body, mac).is_err() {
        k_auth.zeroize();
        return Err(JflError::HmacMismatch);
    }
    let peer = parsed(body)?;
    let (secret, pk) = EcdhSession::generate_ephemeral(rng)?;
    let mut nonce = [0u8; HS_NONCE_LEN];
    rng.fill_bytes(&mut nonce);
    let mut reply = build(KIND_REPLY, id, &nonce, &pk)?;
    let mac = mac_over(&k_auth, &[hello, &reply])?;
    let keys = session(&secret, peer.pk, &k_auth, hello, &reply);
    k_auth.zeroize();
    let keys = keys?;
    reply
        .extend_from_slice(&mac)
        .map_err(|_| JflError::BufferOverflow)?;
    Ok((peer.id, reply, keys))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand_core::{CryptoRng, RngCore};

    /// Deterministic PRNG for reproducible tests only - NOT for production.
    struct TestRng(u64);
    impl RngCore for TestRng {
        fn next_u32(&mut self) -> u32 {
            self.next_u64() as u32
        }
        fn next_u64(&mut self) -> u64 {
            self.0 = self
                .0
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            self.0
        }
        fn fill_bytes(&mut self, dest: &mut [u8]) {
            for chunk in dest.chunks_mut(8) {
                let r = self.next_u64().to_le_bytes();
                chunk.copy_from_slice(&r[..chunk.len()]);
            }
        }
        fn try_fill_bytes(&mut self, dest: &mut [u8]) -> Result<(), rand_core::Error> {
            self.fill_bytes(dest);
            Ok(())
        }
    }
    impl CryptoRng for TestRng {}

    fn psk(b: u8) -> Psk {
        Psk::new([b; PSK_LEN])
    }

    #[test]
    fn both_ends_derive_the_same_keys_and_learn_each_others_id() {
        let (mut ri, mut rr) = (TestRng(1), TestRng(2));
        let (init, hello) = Initiator::start(&psk(7), 0x11, &mut ri).unwrap();
        let (id_i, reply, kr) = respond(&psk(7), 0x22, &hello, &mut rr).unwrap();
        let (id_r, ki) = init.finish(&psk(7), &reply).unwrap();
        assert_eq!((id_i, id_r), (0x11, 0x22));
        assert_eq!(ki.aes_key, kr.aes_key);
        assert_eq!(ki.hmac_key, kr.hmac_key);
        assert_ne!(ki.aes_key, [0u8; 32]);
        assert_ne!(ki.aes_key, ki.hmac_key);
    }

    #[test]
    fn a_peer_without_the_psk_is_refused_both_ways() {
        let (mut ri, mut rr) = (TestRng(3), TestRng(4));
        let (init, hello) = Initiator::start(&psk(7), 1, &mut ri).unwrap();
        // A responder with another key will not answer the hello.
        assert_eq!(
            respond(&psk(8), 2, &hello, &mut rr).err(),
            Some(JflError::HmacMismatch)
        );
        // An impostor responder's reply is refused by the initiator.
        let (_, imp_hello) = Initiator::start(&psk(8), 9, &mut TestRng(5)).unwrap();
        let (_, imp_reply, _) = respond(&psk(8), 2, &imp_hello, &mut rr).unwrap();
        assert_eq!(
            init.finish(&psk(7), &imp_reply).err(),
            Some(JflError::HmacMismatch)
        );
    }

    #[test]
    fn a_reply_is_bound_to_its_own_hello() {
        let (mut ri, mut rr) = (TestRng(6), TestRng(7));
        let (init_a, _hello_a) = Initiator::start(&psk(7), 1, &mut ri).unwrap();
        let (_init_b, hello_b) = Initiator::start(&psk(7), 1, &mut ri).unwrap();
        let (_, reply_b, _) = respond(&psk(7), 2, &hello_b, &mut rr).unwrap();
        // A genuine reply, but to a different hello: refused.
        assert_eq!(
            init_a.finish(&psk(7), &reply_b).err(),
            Some(JflError::HmacMismatch)
        );
    }

    #[test]
    fn tampering_with_any_byte_is_refused() {
        let (mut ri, mut rr) = (TestRng(8), TestRng(9));
        let (_, hello) = Initiator::start(&psk(7), 1, &mut ri).unwrap();
        for i in 0..hello.len() {
            let mut bad = hello.clone();
            bad[i] ^= 0x01;
            assert!(respond(&psk(7), 2, &bad, &mut rr).is_err(), "byte {i}");
        }
    }

    #[test]
    fn malformed_input_never_panics() {
        let mut rr = TestRng(10);
        for len in 0..(MAX_HS_LEN + 8) {
            let mut junk: Vec<u8, { MAX_HS_LEN + 8 }> = Vec::new();
            for i in 0..len {
                let _ = junk.push((i * 37 + len) as u8);
            }
            let _ = respond(&psk(7), 2, &junk, &mut rr);
            let (init, _) = Initiator::start(&psk(7), 1, &mut rr).unwrap();
            let _ = init.finish(&psk(7), &junk);
        }
    }

    #[test]
    fn the_key_file_format_parses_and_rejects_bad_files() {
        // 32, 31 and 33 bytes of hex, and one with a non-hex pair.
        const OK: &str =
            "# jfl psk\nabababababababababababababababababababababababababababababababab\n";
        const SHORT: &str = "ababababababababababababababababababababababababababababababab";
        const LONG: &str = "ababababababababababababababababababababababababababababababababab";
        const BAD: &str = "zzababababababababababababababababababababababababababababababab";
        assert_eq!((SHORT.len(), LONG.len(), BAD.len()), (62, 66, 64));
        let k = Psk::from_hex(OK).unwrap();
        assert_eq!(k.0, [0xab; PSK_LEN]);
        assert!(Psk::from_hex(SHORT).is_err());
        assert!(Psk::from_hex(LONG).is_err());
        assert!(Psk::from_hex(BAD).is_err());
        assert!(Psk::from_hex("").is_err());
    }

    #[test]
    fn the_role_nonce_prefixes_differ() {
        assert_ne!(INITIATOR_NONCE_PREFIX, RESPONDER_NONCE_PREFIX);
    }
}
