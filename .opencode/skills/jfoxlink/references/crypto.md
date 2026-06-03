# JFOXLink Cryptographic Security Reference

## Cryptographic Suite Selection

| Profile | Key Exchange | Encryption | Authentication | Post-Quantum |
|---|---|---|---|---|
| COMMERCIAL-LOW | ECDH P-256 | AES-128-GCM | HMAC-SHA256 | No |
| COMMERCIAL-HIGH | ECDH P-256 + pre-shared | AES-256-GCM | HMAC-SHA256 | No |
| DEFENSE-LITE | ECDH P-384 | AES-256-GCM | HMAC-SHA384 | Optional CRYSTALS |
| DEFENSE-FULL | ECDH P-384 + CRYSTALS-Kyber768 | AES-256-GCM | HMAC-SHA384 | Kyber hybrid |

All suites are aligned with **NIST SP 800-175B** ("Guide for Using Cryptographic Standards in
Federal Government") and **NSA Commercial National Security Algorithm Suite 2.0**.

---

## Key Hierarchy

```
Pre-Shared Identity Key (PSK)
    │  loaded at manufacturing / provisioning
    │  stored in HSM or eFuse OTP
    ▼
ECDH Session Key Material  ←── ephemeral P-256 keypairs per session
    │
    ├── HKDF-SHA256(ikm=ecdh_shared_secret, salt=PSK, info="jfl-v1-session")
    │
    ├── AES_KEY  (32 bytes)  ← for AES-256-GCM payload encryption
    ├── HMAC_KEY (32 bytes)  ← for HMAC-SHA256 frame authentication
    ├── HOP_KEY  (16 bytes)  ← for FHSS AES-128-CTR hop sequence
    └── BEACON_KEY (16 bytes) ← for encrypted timing beacons
```

Keys are **directional**: GCS→UAV and UAV→GCS use separate derived keys
(different HKDF `info` strings), preventing reflection attacks.

---

## Key Exchange Protocol (Session Establishment)

### Phase 1: Challenge-Response Authentication

```
GCS                                          UAV
 │                                            │
 │──── HELLO {GCS_ID, timestamp, rand_a} ────▶│
 │                                            │
 │◀─── HELLO_ACK {UAV_ID, rand_b, signed_b} ──│
 │     (signed_b = HMAC(PSK, rand_a || rand_b || UAV_ID))
 │                                            │
 │ Verify signed_b                            │
 │ Generate ephemeral P-256 keypair (gcs_priv, gcs_pub)
 │                                            │
 │──── KEY_OFFER {gcs_pub_ecdh, signed_offer}─▶│
 │     (signed_offer = HMAC(PSK, gcs_pub || rand_a || rand_b))
 │                                            │
 │◀─── KEY_CONFIRM {uav_pub_ecdh, signed_confirm}
 │                                            │
 │ Compute shared_secret = ECDH(gcs_priv, uav_pub_ecdh)
 │ Derive session keys via HKDF               │
 │                                            │
 │──── SESSION_START {nonce=0, encrypted_ack}─▶│
 │     (proves GCS derived keys correctly)    │
 │                                            │
 ■ OPERATIONAL                                ■ OPERATIONAL
```

### Rust Implementation

```rust
use p256::{ecdh::EphemeralSecret, PublicKey};
use hkdf::Hkdf;
use sha2::Sha256;
use hmac::{Hmac, Mac};
use zeroize::Zeroize;

pub struct SessionKeyMaterial {
    pub aes_key:    [u8; 32],
    pub hmac_key:   [u8; 32],
    pub hop_key:    [u8; 16],
    pub beacon_key: [u8; 16],
}

impl Drop for SessionKeyMaterial {
    fn drop(&mut self) {
        self.aes_key.zeroize();
        self.hmac_key.zeroize();
        self.hop_key.zeroize();
        self.beacon_key.zeroize();
    }
}

pub fn derive_session_keys(
    ecdh_shared: &[u8; 32],
    psk:         &[u8; 32],
    direction:   Direction,
) -> SessionKeyMaterial {
    let info = match direction {
        Direction::GcsToUav => b"jfl-v1-gcs-to-uav" as &[u8],
        Direction::UavToGcs => b"jfl-v1-uav-to-gcs",
    };
    
    let hk = Hkdf::<Sha256>::new(Some(psk), ecdh_shared);
    let mut okm = [0u8; 96];  // 32+32+16+16
    hk.expand(info, &mut okm).expect("HKDF expand");
    
    let mut keys = SessionKeyMaterial::zeroed();
    keys.aes_key.copy_from_slice(&okm[0..32]);
    keys.hmac_key.copy_from_slice(&okm[32..64]);
    keys.hop_key.copy_from_slice(&okm[64..80]);
    keys.beacon_key.copy_from_slice(&okm[80..96]);
    okm.zeroize();
    keys
}
```

---

## Frame Encryption (AES-256-GCM)

```rust
use aes_gcm::{Aes256Gcm, KeyInit, AeadInPlace, Nonce};

pub struct FrameCrypto {
    cipher: Aes256Gcm,
}

impl FrameCrypto {
    pub fn new(key: &[u8; 32]) -> Self {
        Self { cipher: Aes256Gcm::new(key.into()) }
    }
    
    /// Encrypt MAVLink payload in-place.
    /// AAD = JFOXLink header (bytes 0–19) — authenticated but not encrypted.
    pub fn encrypt_payload(
        &self,
        nonce_counter: u64,
        header: &[u8; 20],  // AAD
        payload: &mut Vec<u8>,
    ) -> Result<[u8; 16], CryptoError> {
        let mut nonce_bytes = [0u8; 12];
        nonce_bytes[..8].copy_from_slice(&nonce_counter.to_le_bytes());
        // bytes 8–11 are zero (system_id could go here for multi-system)
        
        let nonce = Nonce::from(nonce_bytes);
        let tag = self.cipher
            .encrypt_in_place_detached(&nonce, header, payload)
            .map_err(|_| CryptoError::EncryptFailed)?;
        
        Ok(tag.into())
    }
    
    pub fn decrypt_payload(
        &self,
        nonce_counter: u64,
        header: &[u8; 20],
        payload: &mut Vec<u8>,
        tag: &[u8; 16],
    ) -> Result<(), CryptoError> {
        let mut nonce_bytes = [0u8; 12];
        nonce_bytes[..8].copy_from_slice(&nonce_counter.to_le_bytes());
        let nonce = Nonce::from(nonce_bytes);
        
        self.cipher
            .decrypt_in_place_detached(&nonce, header, payload, tag.into())
            .map_err(|_| CryptoError::AuthFailed)
    }
}
```

**Security property**: GCM authentication tag covers BOTH the payload and the AAD
(the JFOXLink header). Any tampering with the header (SYS_ID, MSG_ID, SEQ, CHANNEL_FLAGS)
is detected immediately before the payload is processed.

---

## Frame Authentication (HMAC-SHA256)

HMAC covers the full frame including the GCM tag — provides defense-in-depth:

```rust
use hmac::{Hmac, Mac};
use sha2::Sha256;

type HmacSha256 = Hmac<Sha256>;

pub fn compute_frame_hmac(key: &[u8; 32], frame_without_hmac: &[u8]) -> [u8; 32] {
    let mut mac = HmacSha256::new_from_slice(key)
        .expect("HMAC key length valid");
    mac.update(frame_without_hmac);
    mac.finalize().into_bytes().into()
}

pub fn verify_frame_hmac(
    key: &[u8; 32],
    frame_without_hmac: &[u8],
    expected_hmac: &[u8; 32],
) -> Result<(), CryptoError> {
    let mut mac = HmacSha256::new_from_slice(key)
        .expect("HMAC key length valid");
    mac.update(frame_without_hmac);
    mac.verify_slice(expected_hmac)
        .map_err(|_| CryptoError::HmacMismatch)
}
```

---

## Key Lifecycle

### Session Key Rotation Policy

| Trigger | Action |
|---|---|
| Session age > 1 hour | Re-run ECDH key exchange |
| Frame count > 2^32 | Force re-key (nonce space 50% consumed) |
| Jam detected + re-key flag | Emergency re-key (new hop + new session keys) |
| GPS time jump > 10 seconds | Force re-sync and re-key |
| Post-flight | Zeroize all session keys immediately |

```rust
pub struct KeyRotationPolicy {
    pub max_session_age_secs: u64,    // default: 3600
    pub max_frames_per_session: u64,  // default: 2^32 (~4 billion)
    pub rotate_on_jam: bool,          // default: true
}

impl KeyRotationPolicy {
    pub fn should_rotate(&self, session: &Session) -> bool {
        session.age_secs() > self.max_session_age_secs
        || session.frame_count > self.max_frames_per_session
        || (self.rotate_on_jam && session.jam_detected)
    }
}
```

### Pre-Shared Key Provisioning (Manufacturing)
```
1. Generate 256-bit PSK using hardware RNG (MCU TRNG or external ATECC608)
2. Store in MCU eFuse / one-time-programmable region (non-extractable after lock)
3. Store GCS copy in PKCS#11 HSM or encrypted key store (TPM-backed)
4. Destroy plaintext PSK immediately after programming
5. Record key fingerprint (SHA-256 of PSK) in aircraft logbook
```

---

## Post-Quantum Considerations (DEFENSE-FULL)

Kyber-768 (CRYSTALS-Kyber) is layered over ECDH as a hybrid:

```
shared_secret = ECDH_shared XOR Kyber_shared_secret
```

This ensures that breaking either primitive (but not both) does not compromise the session.
CRYSTALS-Kyber-768 is NIST PQC Round 3 winner — provides 178-bit classical equivalent
security. Add `pqcrypto-kyber` crate for Rust implementation.

---

## Security Invariants (Must Never Violate)

1. **No nonce reuse**: `AtomicU64` monotonic counter, never reset within a session
2. **No key reuse**: New ECDH ephemeral keys for every session
3. **Authenticate before decrypt**: Verify HMAC before calling `decrypt_payload`
4. **Zeroize on drop**: All key material implements `Zeroize` + `ZeroizeOnDrop`
5. **No panic in crypto**: All crypto functions return `Result<_, CryptoError>`
6. **Constant-time comparison**: Use `subtle::ConstantTimeEq` for HMAC comparison
7. **No crypto in INCOMPAT=0 mode**: If negotiation fails, abort — no fallback to plaintext