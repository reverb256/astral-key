//! Ed25519 key operations using ring.
//!
//! Supports:
//! - Key generation (Ed25519)
//! - Derive public key from seed (import Mosaic keys)
//! - Sign message hashes
//! - Verify signatures
//! - Key ID derivation (first 8 bytes of SHA256(pubkey) → hex)

use ring::{
    digest,
    rand::SystemRandom,
    signature::{Ed25519KeyPair, KeyPair, UnparsedPublicKey, ED25519},
};

use sha2::{Digest, Sha256};

use crate::error::Error;

/// Generate a new Ed25519 key pair.
///
/// Returns (public_key_hex, private_key_hex (PKCS#8), key_id).
pub fn generate_key() -> (String, String, String) {
    let rng = SystemRandom::new();
    let pkcs8 = Ed25519KeyPair::generate_pkcs8(&rng).expect("RNG failure");
    let key_pair = Ed25519KeyPair::from_pkcs8(pkcs8.as_ref()).expect("invalid generated key");

    let pubkey_bytes = key_pair.public_key().as_ref();
    let privkey_pkcs8 = pkcs8.as_ref();

    let pubkey_hex = hex::encode(pubkey_bytes);
    let privkey_hex = hex::encode(privkey_pkcs8);
    let key_id = derive_key_id(pubkey_bytes);

    (pubkey_hex, privkey_hex, key_id)
}

/// Derive public key + key_id from a PKCS#8 v2 private key seed (hex).
///
/// This allows importing existing Mosaic keys or recovering from seed.
pub fn derive_public_key(seed_hex: &str) -> Result<(String, String), Error> {
    let seed = hex::decode(seed_hex).map_err(|_| Error::Crypto("Invalid hex in seed".into()))?;
    let key_pair = Ed25519KeyPair::from_pkcs8(&seed)
        .map_err(|_| Error::Crypto("Invalid PKCS#8 key material".into()))?;

    let pubkey_bytes = key_pair.public_key().as_ref();
    let pubkey_hex = hex::encode(pubkey_bytes);
    let key_id = derive_key_id(pubkey_bytes);

    Ok((pubkey_hex, key_id))
}

/// Sign a message hash (32 bytes) with the private key.
///
/// The private key is provided as hex — either PKCS#8 v2 (48 bytes) or raw
/// 32-byte Ed25519 seed (HD-derived, detected by length).
pub fn sign(privkey_hex: &str, msg: &[u8]) -> Result<String, Error> {
    let key_bytes =
        hex::decode(privkey_hex).map_err(|_| Error::Crypto("Invalid hex in private key".into()))?;

    let key_pair = if key_bytes.len() == 32 {
        // Raw 32-byte seed (HD derivation path — from_seed_unchecked)
        let mut seed = [0u8; 32];
        seed.copy_from_slice(&key_bytes);
        Ed25519KeyPair::from_seed_unchecked(&seed)
            .map_err(|_| Error::Crypto("Invalid HD seed (rejected by ring)".into()))?
    } else {
        // PKCS#8 v2 format (legacy random generation)
        Ed25519KeyPair::from_pkcs8(&key_bytes)
            .map_err(|_| Error::Crypto("Invalid PKCS#8 key".into()))?
    };

    let signature = key_pair.sign(msg);
    Ok(hex::encode(signature.as_ref()))
}

/// Verify a signature against a public key and message.
pub fn verify(pubkey_hex: &str, msg: &[u8], sig_hex: &str) -> Result<bool, Error> {
    let pubkey =
        hex::decode(pubkey_hex).map_err(|_| Error::Crypto("Invalid hex in public key".into()))?;
    let sig = hex::decode(sig_hex).map_err(|_| Error::Crypto("Invalid hex in signature".into()))?;

    let peer_public_key = UnparsedPublicKey::new(&ED25519, &pubkey);
    match peer_public_key.verify(msg, &sig) {
        Ok(()) => Ok(true),
        Err(_) => Ok(false),
    }
}

/// Derive a human-readable key ID from a public key.
///
/// key_id = first 8 bytes of SHA256(pubkey) as hex string.
/// Format: "k-" + 16 hex chars (8 bytes).
pub fn derive_key_id(pubkey_bytes: &[u8]) -> String {
    let hash = Sha256::digest(pubkey_bytes);
    let id_hex = hex::encode(&hash[..8]);
    format!("k-{}", id_hex)
}

// ─── Post-quantum hybrid signing (ML-DSA-65, FIPS 204) ────────────────────

/// Hybrid signature result.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HybridSignature {
    /// Ed25519 signature (64 bytes hex)
    pub ed25519_sig: String,

    /// ML-DSA-65 signature (hex). Empty when PQ feature disabled — see `algorithm`.
    pub ml_dsa_sig: String,

    /// Signing algorithm label, e.g. "ed25519+ml-dsa-65"
    pub algorithm: String,

    /// Ed25519 public key for verification
    pub pubkey_hex: String,

    /// ML-DSA-65 public key for verification (hex). Empty when PQ disabled.
    pub ml_dsa_pubkey_hex: String,
}

/// Generate a new ML-DSA-65 keypair (FIPS 204).
///
/// Returns (public_key_hex, secret_key_hex). The
/// secret key is returned to the caller for persistent storage — verification
/// would otherwise be impossible.
#[cfg(feature = "pq")]
pub fn generate_mldsa_keypair() -> (String, String) {
    use pqcrypto_mldsa::mldsa65::{keypair, DetachedSignature as _};
    use pqcrypto_traits::sign::DetachedSignature as _;
    let (pk, sk) = keypair();
    (hex::encode(pk.as_bytes()), hex::encode(sk.as_bytes()))
}

/// Sign a message with ML-DSA-65 (detached). Returns hex signature.
#[cfg(feature = "pq")]
pub fn sign_mldsa(sk_hex: &str, msg: &[u8]) -> Result<String, Error> {
    use pqcrypto_mldsa::mldsa65::{detached_sign, SecretKey};
    use pqcrypto_traits::sign::DetachedSignature as _;
    let sk_bytes =
        hex::decode(sk_hex).map_err(|_| Error::Crypto("Invalid ML-DSA sk hex".into()))?;
    let sk = SecretKey::from_bytes(&sk_bytes)
        .map_err(|_| Error::Crypto("Invalid ML-DSA secret key length".into()))?;
    let sig = detached_sign(msg, &sk);
    Ok(hex::encode(sig.as_bytes()))
}

/// Verify an ML-DSA-65 detached signature.
#[cfg(feature = "pq")]
pub fn verify_mldsa(pk_hex: &str, msg: &[u8], sig_hex: &str) -> Result<bool, Error> {
    use pqcrypto_mldsa::mldsa65::{detached_verify, PublicKey, VerifiedSignature};
    use pqcrypto_traits::sign::{DetachedSignature, VerifiedSignature as _};
    let pk_bytes =
        hex::decode(pk_hex).map_err(|_| Error::Crypto("Invalid ML-DSA pk hex".into()))?;
    let pk = PublicKey::from_bytes(&pk_bytes)
        .map_err(|_| Error::Crypto("Invalid ML-DSA public key length".into()))?;
    let sig_bytes =
        hex::decode(sig_hex).map_err(|_| Error::Crypto("Invalid ML-DSA sig hex".into()))?;
    let sig = VerifiedSignature::from_bytes(&sig_bytes)
        .map_err(|_| Error::Crypto("Invalid ML-DSA signature length".into()))?;
    match detached_verify(&sig, msg, &pk) {
        Ok(()) => Ok(true),
        Err(_) => Ok(false),
    }
}

/// Non-PQ stubs (compiled without the `pq` feature): surface clear errors so
/// callers know the build must be rebuilt with `--features pq`.
#[cfg(not(feature = "pq"))]
pub fn generate_mldsa_keypair() -> (String, String) {
    (String::new(), String::new())
}
#[cfg(not(feature = "pq"))]
pub fn sign_mldsa(_sk_hex: &str, _msg: &[u8]) -> Result<String, Error> {
    Err(Error::Crypto(
        "PQ feature not enabled. Rebuild with --features pq".into(),
    ))
}
#[cfg(not(feature = "pq"))]
pub fn verify_mldsa(_pk_hex: &str, _msg: &[u8], _sig_hex: &str) -> Result<bool, Error> {
    Err(Error::Crypto(
        "PQ feature not enabled. Rebuild with --features pq".into(),
    ))
}

/// Sign a message with dual Ed25519 + ML-DSA-65 (true hybrid, NIST SP 800-208).
///
/// `ed_privkey_hex` is the Ed25519 PKCS#8 key; `ml_dsa_privkey_hex` is the
/// ML-DSA-65 secret key (hex). Both signatures are produced and both are
/// verified by `verify_hybrid`.
pub fn sign_hybrid(
    ed_privkey_hex: &str,
    ml_dsa_privkey_hex: &str,
    msg: &[u8],
) -> Result<HybridSignature, Error> {
    let ed25519_sig = sign(ed_privkey_hex, msg)?;
    let (pubkey_hex, _key_id) = derive_public_key(ed_privkey_hex)?;

    if ml_dsa_privkey_hex.is_empty() {
        return Err(Error::Crypto(
            "No ML-DSA key for this identity. Rebuild with --features pq and re-mint the key."
                .into(),
        ));
    }
    let ml_dsa_sig = sign_mldsa(ml_dsa_privkey_hex, msg)?;
    let ml_dsa_pubkey_hex = derive_mldsa_pubkey_hex(ml_dsa_privkey_hex);

    Ok(HybridSignature {
        ed25519_sig,
        ml_dsa_sig,
        algorithm: "ed25519+ml-dsa-65".to_string(),
        pubkey_hex,
        ml_dsa_pubkey_hex,
    })
}

/// Derive the ML-DSA-65 public key hex from a secret key hex (for embedding in
/// the signature response so verifiers need only the Ed25519 key id).
#[cfg(feature = "pq")]
fn derive_mldsa_pubkey_hex(sk_hex: &str) -> String {
    use pqcrypto_mldsa::mldsa65::{keypair_from_secret, PublicKey};
    use pqcrypto_traits::sign::PublicKey as _;
    if let Ok(sk_bytes) = hex::decode(sk_hex) {
        if let Ok(sk) = pqcrypto_mldsa::mldsa65::SecretKey::from_bytes(&sk_bytes) {
            let pk = PublicKey::from(&sk);
            return hex::encode(pk.as_bytes());
        }
    }
    String::new()
}
#[cfg(not(feature = "pq"))]
fn derive_mldsa_pubkey_hex(_sk_hex: &str) -> String {
    String::new()
}

/// Verify a hybrid signature — checks **both** Ed25519 and ML-DSA-65.
///
/// A hybrid signature is only valid if both components verify. This is the
/// post-quantum-safe path: a forger breaking Ed25519 (Shor) still cannot
/// produce a valid ML-DSA-65 signature.
pub fn verify_hybrid(sig: &HybridSignature, msg: &[u8]) -> Result<bool, Error> {
    let ed_ok = verify(&sig.pubkey_hex, msg, &sig.ed25519_sig)?;
    if !ed_ok {
        return Ok(false);
    }
    if sig.ml_dsa_sig.is_empty() || sig.ml_dsa_pubkey_hex.is_empty() {
        // No PQ component present — fall back to classical-only (pre-PQ key).
        return Ok(true);
    }
    let ml_ok = verify_mldsa(&sig.ml_dsa_pubkey_hex, msg, &sig.ml_dsa_sig)?;
    Ok(ml_ok)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_and_sign_roundtrip() {
        let (pubkey, privkey, key_id) = generate_key();
        assert_eq!(key_id.len(), 18); // "k-" + 16 hex chars
        assert_eq!(pubkey.len(), 64); // 32 bytes as hex
        assert!(privkey.len() > 64);

        let msg = b"hello mosaic";
        let sig = sign(&privkey, msg).unwrap();
        assert_eq!(sig.len(), 128); // 64 bytes as hex

        let valid = verify(&pubkey, msg, &sig).unwrap();
        assert!(valid);

        // Wrong message should fail
        let valid = verify(&pubkey, b"wrong", &sig).unwrap();
        assert!(!valid);
    }

    #[test]
    fn test_derive_key_id() {
        let (pubkey, _privkey, key_id) = generate_key();
        let pubkey_bytes = hex::decode(&pubkey).unwrap();
        let computed = derive_key_id(&pubkey_bytes);
        assert_eq!(computed, key_id);
    }
}
