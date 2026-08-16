//! Session-key provisioning and storage for the ground station.
//!
//! Security posture:
//! - **Fail-closed**: no method ever returns placeholder/all-zero key material.
//!   If no session has been established, callers get `KeyNotLoaded`.
//! - Session keys are established via real ephemeral ECDH (`jfl-core`) seeded
//!   from the OS CSPRNG (`OsRng`).
//! - At-rest persistence is AES-256-GCM encrypted under a runtime key-encryption
//!   key (KEK) that must come from an OS keychain / HSM — never stored here.
//! - All secret-bearing structs zeroize on drop.

use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;

use heapless::Vec as HVec;
use jfl_core::crypto::aes_gcm::GcmEngine;
use jfl_core::crypto::ecdh::{EcdhSession, EphemeralSecret, MAX_PUBKEY_LEN};
use rand::rngs::OsRng;
use rand::RngCore;
use zeroize::{Zeroize, ZeroizeOnDrop};

/// Domain-separation label bound into the at-rest AES-GCM as AAD.
const KEYSTORE_AAD: &[u8] = b"JFL-KEYSTORE-v1";
/// Plaintext layout on disk: aes(32) || hmac(32) || nonce_prefix(4).
const KEY_MATERIAL_LEN: usize = 32 + 32 + 32 + 4;

#[derive(Debug)]
pub enum KeyStoreError {
    Io(std::io::Error),
    KeyNotLoaded,
    HandshakeIncomplete,
    KeyAgreementFailed,
    DecryptFailed,
    Corrupt,
}

impl From<std::io::Error> for KeyStoreError {
    fn from(e: std::io::Error) -> Self {
        KeyStoreError::Io(e)
    }
}

/// Established symmetric session material. Zeroized on drop.
#[derive(ZeroizeOnDrop)]
pub struct SecureKey {
    pub aes_key: [u8; 32],
    pub hmac_key: [u8; 32],
    /// Keys the FHSS hop schedule (`jfl_core::anti_jam::fhss::KeyedHopSchedule`).
    pub hop_key: [u8; 32],
    /// 4-byte per-session nonce prefix consumed by `NonceGenerator`.
    pub nonce_prefix: [u8; 4],
}

impl SecureKey {
    fn to_bytes(&self) -> [u8; KEY_MATERIAL_LEN] {
        let mut out = [0u8; KEY_MATERIAL_LEN];
        out[0..32].copy_from_slice(&self.aes_key);
        out[32..64].copy_from_slice(&self.hmac_key);
        out[64..96].copy_from_slice(&self.hop_key);
        out[96..100].copy_from_slice(&self.nonce_prefix);
        out
    }

    fn from_bytes(b: &[u8]) -> Result<Self, KeyStoreError> {
        if b.len() != KEY_MATERIAL_LEN {
            return Err(KeyStoreError::Corrupt);
        }
        let mut aes_key = [0u8; 32];
        let mut hmac_key = [0u8; 32];
        let mut hop_key = [0u8; 32];
        let mut nonce_prefix = [0u8; 4];
        aes_key.copy_from_slice(&b[0..32]);
        hmac_key.copy_from_slice(&b[32..64]);
        hop_key.copy_from_slice(&b[64..96]);
        nonce_prefix.copy_from_slice(&b[96..100]);
        Ok(Self {
            aes_key,
            hmac_key,
            hop_key,
            nonce_prefix,
        })
    }
}

/// Thread-safe key store. Holds at most one active session plus any in-flight
/// handshake secret.
pub struct KeyStore {
    session: Mutex<Option<SecureKey>>,
    pending: Mutex<Option<EphemeralSecret>>,
    storage_path: PathBuf,
}

impl KeyStore {
    pub fn new(storage_path: PathBuf) -> Self {
        Self {
            session: Mutex::new(None),
            pending: Mutex::new(None),
            storage_path,
        }
    }

    /// Step 1 of the handshake: generate an ephemeral key pair, retain the
    /// secret, and return our compressed public key to send to the peer.
    pub fn begin_handshake(&self) -> Result<HVec<u8, MAX_PUBKEY_LEN>, KeyStoreError> {
        let (secret, public) = EcdhSession::generate_ephemeral(&mut OsRng)
            .map_err(|_| KeyStoreError::KeyAgreementFailed)?;
        *self
            .pending
            .lock()
            .map_err(|_| KeyStoreError::HandshakeIncomplete)? = Some(secret);
        Ok(public)
    }

    /// Step 2 of the handshake: combine the retained secret with the peer's
    /// public key, derive the session keys, and install them. Both peers must
    /// agree on `salt` and `info`.
    pub fn complete_handshake(
        &self,
        peer_pubkey: &[u8],
        salt: &[u8],
        info: &[u8],
    ) -> Result<(), KeyStoreError> {
        let secret = self
            .pending
            .lock()
            .map_err(|_| KeyStoreError::HandshakeIncomplete)?
            .take()
            .ok_or(KeyStoreError::HandshakeIncomplete)?;

        let keys = EcdhSession::derive_session_keys(&secret, peer_pubkey, salt, info)
            .map_err(|_| KeyStoreError::KeyAgreementFailed)?;

        let mut nonce_prefix = [0u8; 4];
        OsRng.fill_bytes(&mut nonce_prefix);

        let secure = SecureKey {
            aes_key: keys.aes_key,
            hmac_key: keys.hmac_key,
            hop_key: keys.hop_key,
            nonce_prefix,
        };
        self.install(secure)
    }

    fn install(&self, key: SecureKey) -> Result<(), KeyStoreError> {
        let mut slot = self
            .session
            .lock()
            .map_err(|_| KeyStoreError::KeyNotLoaded)?;
        if let Some(ref mut old) = *slot {
            old.aes_key.zeroize();
            old.hmac_key.zeroize();
            old.hop_key.zeroize();
            old.nonce_prefix.zeroize();
        }
        *slot = Some(key);
        Ok(())
    }

    /// Returns a copy of the active session keys for building a decoder.
    /// Fail-closed: errors if no session has been established.
    pub fn load_keys(&self) -> Result<SecureKey, KeyStoreError> {
        let slot = self
            .session
            .lock()
            .map_err(|_| KeyStoreError::KeyNotLoaded)?;
        match *slot {
            Some(ref k) => Ok(SecureKey {
                aes_key: k.aes_key,
                hmac_key: k.hmac_key,
                hop_key: k.hop_key,
                nonce_prefix: k.nonce_prefix,
            }),
            None => Err(KeyStoreError::KeyNotLoaded),
        }
    }

    /// True if an active session is installed.
    pub fn is_provisioned(&self) -> bool {
        self.session.lock().map(|s| s.is_some()).unwrap_or(false)
    }

    /// Zeroize and drop the active session (e.g. on key rotation or shutdown).
    pub fn clear_session(&self) {
        if let Ok(mut slot) = self.session.lock() {
            if let Some(ref mut k) = *slot {
                k.aes_key.zeroize();
                k.hmac_key.zeroize();
                k.hop_key.zeroize();
                k.nonce_prefix.zeroize();
            }
            *slot = None;
        }
    }

    /// Persist the active session to disk, AES-256-GCM encrypted under `kek`.
    /// `kek` must be sourced from an OS keychain / HSM by the caller.
    pub fn persist(&self, kek: &[u8; 32]) -> Result<(), KeyStoreError> {
        let key = self.load_keys()?; // fail-closed if nothing to persist
        let mut nonce = [0u8; 12];
        OsRng.fill_bytes(&mut nonce);

        let mut buf: HVec<u8, 512> = HVec::new();
        buf.extend_from_slice(&key.to_bytes())
            .map_err(|_| KeyStoreError::Corrupt)?;

        let gcm = GcmEngine::new(kek);
        let tag = gcm
            .encrypt(nonce, KEYSTORE_AAD, &mut buf)
            .map_err(|_| KeyStoreError::KeyAgreementFailed)?;

        // File layout: nonce(12) || ciphertext(68) || tag(16).
        let mut out = Vec::with_capacity(12 + buf.len() + 16);
        out.extend_from_slice(&nonce);
        out.extend_from_slice(&buf);
        out.extend_from_slice(&tag);
        fs::write(&self.storage_path, &out)?;
        Ok(())
    }

    /// Load and install a session previously persisted with `persist`.
    pub fn load_encrypted(&self, kek: &[u8; 32]) -> Result<(), KeyStoreError> {
        let raw = fs::read(&self.storage_path)?;
        if raw.len() != 12 + KEY_MATERIAL_LEN + 16 {
            return Err(KeyStoreError::Corrupt);
        }
        let mut nonce = [0u8; 12];
        nonce.copy_from_slice(&raw[0..12]);
        let ct = &raw[12..12 + KEY_MATERIAL_LEN];
        let mut tag = [0u8; 16];
        tag.copy_from_slice(&raw[12 + KEY_MATERIAL_LEN..]);

        let mut buf: HVec<u8, 512> = HVec::new();
        buf.extend_from_slice(ct)
            .map_err(|_| KeyStoreError::Corrupt)?;

        let gcm = GcmEngine::new(kek);
        gcm.decrypt(nonce, KEYSTORE_AAD, &mut buf, &tag)
            .map_err(|_| KeyStoreError::DecryptFailed)?;

        let key = SecureKey::from_bytes(&buf)?;
        self.install(key)
    }
}

impl Drop for KeyStore {
    fn drop(&mut self) {
        self.clear_session();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_path(tag: &str) -> PathBuf {
        let mut p = std::env::temp_dir();
        p.push(format!("jfl-keystore-test-{tag}.bin"));
        let _ = fs::remove_file(&p);
        p
    }

    #[test]
    fn fresh_store_is_fail_closed() {
        let ks = KeyStore::new(temp_path("failclosed"));
        assert!(!ks.is_provisioned());
        assert!(matches!(ks.load_keys(), Err(KeyStoreError::KeyNotLoaded)));
    }

    #[test]
    fn two_stores_agree_on_session_keys() {
        let a = KeyStore::new(temp_path("a"));
        let b = KeyStore::new(temp_path("b"));

        let pub_a = a.begin_handshake().unwrap();
        let pub_b = b.begin_handshake().unwrap();

        a.complete_handshake(&pub_b, b"salt", b"jfl-session")
            .unwrap();
        b.complete_handshake(&pub_a, b"salt", b"jfl-session")
            .unwrap();

        let ka = a.load_keys().unwrap();
        let kb = b.load_keys().unwrap();
        assert_eq!(ka.aes_key, kb.aes_key);
        assert_eq!(ka.hmac_key, kb.hmac_key);
        assert_eq!(ka.hop_key, kb.hop_key);
        assert_ne!(ka.aes_key, [0u8; 32]); // never the old placeholder
        assert_ne!(ka.hop_key, [0u8; 32]);
    }

    #[test]
    fn complete_without_begin_fails() {
        let ks = KeyStore::new(temp_path("nobegin"));
        assert!(matches!(
            ks.complete_handshake(&[0u8; 49], b"s", b"i"),
            Err(KeyStoreError::HandshakeIncomplete)
        ));
    }

    #[test]
    fn persist_and_reload_roundtrip() {
        let path = temp_path("persist");
        let kek = [0x42u8; 32];

        let a = KeyStore::new(path.clone());
        let b = KeyStore::new(path.clone());
        let pub_a = a.begin_handshake().unwrap();
        let pub_b = b.begin_handshake().unwrap();
        a.complete_handshake(&pub_b, b"salt", b"sess").unwrap();
        let original = a.load_keys().unwrap();

        a.persist(&kek).unwrap();

        // A fresh store loads the same material back.
        let reloaded = KeyStore::new(path.clone());
        reloaded.load_encrypted(&kek).unwrap();
        let got = reloaded.load_keys().unwrap();
        assert_eq!(got.aes_key, original.aes_key);
        assert_eq!(got.hmac_key, original.hmac_key);
        assert_eq!(got.hop_key, original.hop_key);
        assert_eq!(got.nonce_prefix, original.nonce_prefix);

        // Wrong KEK must fail (authenticated decryption).
        let wrong = KeyStore::new(path.clone());
        assert!(matches!(
            wrong.load_encrypted(&[0u8; 32]),
            Err(KeyStoreError::DecryptFailed)
        ));

        let _ = fs::remove_file(&path);
    }
}
