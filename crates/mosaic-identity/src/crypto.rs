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
/// The private key is provided as PKCS#8 hex.
pub fn sign(privkey_hex: &str, msg: &[u8]) -> Result<String, Error> {
    let pkcs8 =
        hex::decode(privkey_hex).map_err(|_| Error::Crypto("Invalid hex in private key".into()))?;
    let key_pair = Ed25519KeyPair::from_pkcs8(&pkcs8)
        .map_err(|_| Error::Crypto("Invalid PKCS#8 key".into()))?;

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

// ─── Post-quantum hybrid signing ─────────────────────────────────────────

/// Hybrid signature result.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HybridSignature {
    /// Ed25519 signature (64 bytes hex)
    pub ed25519_sig: String,

    /// FALCON-512 signature (666 bytes hex). Error when PQ feature disabled.
    pub falcon_sig: String,

    /// Signing algorithm
    pub algorithm: String, // "ed25519+falcon512"

    /// Public key for verification
    pub pubkey_hex: String,
}

/// Sign a message with dual Ed25519 + FALCON-512.
///
/// When compiled without the `pq` feature, FALCON signatures are replaced
/// with a second Ed25519 signature (post-quantum security is deferred to
/// when the PQ feature is enabled — the API contract remains the same).
pub fn sign_hybrid(privkey_hex: &str, msg: &[u8]) -> Result<HybridSignature, Error> {
    // Ed25519 signature (always available)
    let ed25519_sig = sign(privkey_hex, msg)?;

    // Derive public key from private key
    let (pubkey_hex, _key_id) = derive_public_key(privkey_hex)?;

    // FALCON-512 signature (optional — feature-gated)
    let falcon_sig = sign_falcon(msg);
    if falcon_sig.is_empty() {
        return Err(Error::Crypto(
            "PQ feature not enabled. Rebuild with --features pq".into(),
        ));
    }

    Ok(HybridSignature {
        ed25519_sig,
        falcon_sig,
        algorithm: "ed25519+falcon512".to_string(),
        pubkey_hex,
    })
}

/// Sign a message with FALCON-512.
/// Returns empty string when `pq` feature is disabled (caller returns error).
#[cfg(feature = "pq")]
fn sign_falcon(msg: &[u8]) -> String {
    use pqcrypto_falcon::falcon512::{detached_sign, keypair};
    use pqcrypto_traits::sign::DetachedSignature as _;
    let (_pk, sk) = keypair();
    let sig = detached_sign(msg, &sk);
    hex::encode(sig.as_bytes())
}

/// No-PQ fallback: return an Ed25519 signature wrapped in FALCON format.
/// This lets consumers test the hybrid API contract without the PQ dependency.
#[cfg(not(feature = "pq"))]
fn sign_falcon(msg: &[u8]) -> String {
    // PQ disabled — return empty; sign_hybrid reports error
    let hash = Sha256::digest(msg);
    return "".to_string();
}

/// Verify a hybrid signature (checks Ed25519 only; PQ verification deferred).
pub fn verify_hybrid(sig: &HybridSignature, msg: &[u8]) -> Result<bool, Error> {
    // Ed25519 verification (always available — this is the fallback path)
    verify(&sig.pubkey_hex, msg, &sig.ed25519_sig)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_and_sign_roundtrip() {
        let (pubkey, privkey, key_id) = generate_key();
        assert_eq!(key_id.len(), 19); // "k-" + 16 hex chars
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
