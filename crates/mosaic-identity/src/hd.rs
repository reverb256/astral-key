//! BIP-39 mnemonic → HD key derivation for the Mosaic Identity Foundation.
//!
//! Implements:
//! - BIP-39 mnemonic generation (24 words, English)
//! - PBKDF2 seed derivation (SLIP-10 / BIP32-Ed25519 master key)
//! - Ed25519 keypair derivation from seed
//! - ML-DSA-65 keypair derivation from the same seed
//!
//! When the user loses their MIS database, the 24-word phrase is all they
//! need to re-derive every key. The SQLite database is a fast cache — the
//! mnemonic is the canonical source of truth.
//!
//! # Key hierarchy
//!
//! ```text
//! Mnemonic (24 BIP-39 words)
//!   └─► PBKDF2 (mnemonic, "mnemonic" + passphrase)
//!         └─► 512-bit master seed
//!               ├─► HMAC-SHA512("ed25519 seed", seed) → I_left = Ed25519 seed
//!               └─► SHA512("mosaic-mldsa-65" || seed) → ML-DSA-65 key material
//! ```

use ring::hmac;
use ring::signature::KeyPair;
use sha2::{Digest, Sha512};

use crate::error::Error;

/// Number of words in a generated mnemonic.
const MNEMONIC_WORD_COUNT: usize = 24;

/// Generate a new 24-word BIP-39 mnemonic phrase in English.
///
/// The entropy is 256 bits (33 bytes with 8-bit checksum → 264 bits ÷ 11
/// bits per word = 24 words).
pub fn generate_mnemonic() -> String {
    use bip39::Mnemonic;
    let mnemonic = Mnemonic::generate(MNEMONIC_WORD_COUNT)
        .expect("BIP-39 mnemonic generation should never fail with 24 words");
    mnemonic.to_string()
}

/// Validate a BIP-39 mnemonic phrase and convert it to a 512-bit seed.
///
/// `passphrase` is an optional additional secret (BIP-39 "password"). An empty
/// string is the default.
pub fn mnemonic_to_seed(phrase: &str, passphrase: &str) -> Result<[u8; 64], Error> {
    use bip39::Mnemonic;
    use std::str::FromStr;
    let mnemonic = Mnemonic::from_str(phrase)
        .map_err(|e| Error::Crypto(format!("Invalid BIP-39 mnemonic: {}", e)))?;
    Ok(mnemonic.to_seed(passphrase))
}

/// Derive an Ed25519 keypair from a 512-bit master seed (SLIP-10).
///
/// Returns (seed_hex, pubkey_hex, key_id). The seed is the raw 32-bytes of
/// clamped SLIP-10 output — NOT PKCS#8 format. Use
/// `ring::signature::Ed25519KeyPair::from_seed_unchecked` to reconstruct
/// the signing key from this seed.
pub fn derive_ed25519_from_seed(seed: &[u8; 64]) -> (String, String, String) {
    // SLIP-10 master key derivation for Ed25519:
    //   I = HMAC-SHA512(key="ed25519 seed", data=seed)
    //   k_master = clamp(I_left)
    let signing_key = hmac::Key::new(hmac::HMAC_SHA512, b"ed25519 seed");
    let tag = hmac::sign(&signing_key, seed);
    let hmac_bytes = tag.as_ref();
    debug_assert_eq!(hmac_bytes.len(), 64);

    // Split: first 32 bytes = raw seed (needs clamping for Ed25519)
    let mut raw_seed = [0u8; 32];
    raw_seed.copy_from_slice(&hmac_bytes[..32]);
    clamp_ed25519_seed(&mut raw_seed);

    // Create keypair from seed to extract the public key
    let key_pair = ring::signature::Ed25519KeyPair::from_seed_unchecked(&raw_seed)
        .expect("SLIP-10-derived seed produced an invalid Ed25519 keypair");
    let pubkey_bytes = key_pair.public_key().as_ref();
    let pubkey_hex = hex::encode(pubkey_bytes);
    let seed_hex = hex::encode(&raw_seed);
    let key_id = crate::crypto::derive_key_id(pubkey_bytes);

    (seed_hex, pubkey_hex, key_id)
}

/// Sign a message using a raw 32-byte Ed25519 seed (hex) via from_seed_unchecked.
/// This is the HD path — no PKCS#8 wrapping needed.
pub fn sign_with_seed(seed_hex: &str, msg: &[u8]) -> Result<String, Error> {
    let seed = hex::decode(seed_hex)
        .map_err(|_| Error::Crypto("Invalid hex in seed".into()))?;
    if seed.len() != 32 {
        return Err(Error::Crypto("Seed must be 32 bytes".into()));
    }
    let mut arr = [0u8; 32];
    arr.copy_from_slice(&seed);
    let key_pair = ring::signature::Ed25519KeyPair::from_seed_unchecked(&arr)
        .map_err(|e| Error::Crypto(format!("Invalid HD seed: {}", e)))?;
    let signature = key_pair.sign(msg);
    Ok(hex::encode(signature.as_ref()))
}

/// Clamp a raw 32-byte seed for Ed25519 (clears high bits, low bits).
///
/// Ed25519 requires:
/// - Clear bits 0, 1, 2 of the first byte
/// - Clear bit 7 of the last byte
/// - Set bit 6 of the last byte
fn clamp_ed25519_seed(seed: &mut [u8; 32]) {
    seed[0] &= 0b1111_1000;
    seed[31] &= 0b0011_1111;
    seed[31] |= 0b0100_0000;
}

// ─── ML-DSA-65 derivation (PQ, feature-gated) ──────────────────────────

/// Derive an ML-DSA-65 keypair from a 512-bit master seed.
///
/// Uses a domain-separated hash to ensure the ML-DSA key is distinct from
/// the Ed25519 key: SHA512("mosaic-mldsa-65" || seed) truncated to 4032
/// bytes (required by ML-DSA-65).
///
/// When the `pq` feature is not enabled, returns empty strings (caller
/// should check).
#[cfg(feature = "pq")]
pub fn derive_mldsa_from_seed(seed: &[u8; 64]) -> (String, String) {
    use pqcrypto_mldsa::mldsa65::{keypair_from_secret, SecretKey};
    use pqcrypto_traits::sign::{PublicKey as _, SecretKey as _};

    // Domain-separated hash: SHA512("mosaic-mldsa-65" || master_seed)
    let mut hasher = Sha512::new();
    hasher.update(b"mosaic-mldsa-65");
    hasher.update(seed);
    let hash = hasher.finalize(); // 64 bytes

    // ML-DSA-65 needs 4032 bytes of secret key material. We stretch the
    // 64-byte hash via repeated HMAC-SHA256(K=hash, counter) into 4032
    // bytes. This is NOT a standard derivation — there is no SLIP for
    // lattice-based HD keys yet. We use this as a deterministic, auditable
    // scheme that can be replaced once a standard emerges.
    let mut sk_bytes = Vec::with_capacity(4032);
    let hmac_key = hmac::Key::new(hmac::HMAC_SHA256, hash.as_slice());
    for i in 0..63u8 {
        let tag = hmac::sign(&hmac_key, &[i]);
        sk_bytes.extend_from_slice(tag.as_ref());
    }
    // Last chunk: take the first 64 bytes (4032 = 63 × 64 + 0 → exact)
    let remainder = 4032 - sk_bytes.len();
    let tag = hmac::sign(&hmac_key, &[63]);
    sk_bytes.extend_from_slice(&tag.as_ref()[..remainder]);
    debug_assert_eq!(sk_bytes.len(), 4032);

    // Import into pqcrypto to get the public key
    let sk = match SecretKey::from_bytes(&sk_bytes) {
        Ok(sk) => sk,
        Err(_) => return (String::new(), String::new()),
    };
    let pk = sk.public_key();
    (hex::encode(pk.as_bytes()), hex::encode(sk_bytes))
}

/// Non-PQ stub.
#[cfg(not(feature = "pq"))]
pub fn derive_mldsa_from_seed(_seed: &[u8; 64]) -> (String, String) {
    (String::new(), String::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_mnemonic_24_words() {
        let phrase = generate_mnemonic();
        let word_count = phrase.split_whitespace().count();
        assert_eq!(word_count, 24);
    }

    #[test]
    fn test_mnemonic_roundtrip() {
        let phrase = generate_mnemonic();
        let seed = mnemonic_to_seed(&phrase, "").unwrap();
        assert_eq!(seed.len(), 64);

        // Same phrase + passphrase → same seed
        let seed2 = mnemonic_to_seed(&phrase, "").unwrap();
        assert_eq!(seed, seed2);

        // Different passphrase → different seed
        let seed3 = mnemonic_to_seed(&phrase, "extra").unwrap();
        assert_ne!(seed, seed3);
    }

    #[test]
    fn test_invalid_mnemonic() {
        let result = mnemonic_to_seed("not a valid mnemonic phrase at all", "");
        assert!(result.is_err());
    }

    #[test]
    fn test_derive_ed25519_deterministic() {
        let seed = [0xabu8; 64];
        let (seed_hex, pubkey, kid) = derive_ed25519_from_seed(&seed);
        assert_eq!(seed_hex.len(), 64); // 32 bytes as hex
        assert_eq!(pubkey.len(), 64); // 32 bytes as hex
        assert_eq!(kid.len(), 18); // "k-" + 16 hex chars

        // Same seed → same key
        let (seed2, pubkey2, kid2) = derive_ed25519_from_seed(&seed);
        assert_eq!(seed_hex, seed2);
        assert_eq!(pubkey, pubkey2);
        assert_eq!(kid, kid2);
    }

    #[test]
    fn test_deterministic_phrase_produces_key() {
        // A known test mnemonic (BIP-39 test vector)
        let phrase = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
        let seed = mnemonic_to_seed(phrase, "TREZOR").unwrap();
        let (_seed, pubkey, _kid) = derive_ed25519_from_seed(&seed);
        // Just ensure it produces *some* valid public key
        assert_eq!(pubkey.len(), 64);
        assert!(hex::decode(&pubkey).is_ok());
    }

    #[cfg(feature = "pq")]
    #[test]
    fn test_derive_mldsa_deterministic() {
        let seed = [0xabu8; 64];
        let (pk, sk) = derive_mldsa_from_seed(&seed);
        assert_eq!(pk.len(), 64); // 32 bytes as hex
        assert!(sk.len() > 8000); // 4032 bytes as hex ≈ 8064+ chars

        // Same seed → same key
        let (pk2, sk2) = derive_mldsa_from_seed(&seed);
        assert_eq!(pk, pk2);
        assert_eq!(sk, sk2);
    }

    #[test]
    fn test_sign_with_seed_roundtrip() {
        let seed = [0xabu8; 32];
        let seed_hex = hex::encode(&seed);
        let msg = b"hello mosaic";
        let sig = sign_with_seed(&seed_hex, msg).unwrap();
        assert_eq!(sig.len(), 128); // 64 bytes as hex

        // Verify using the public key derived from the same seed
        let key_pair = ring::signature::Ed25519KeyPair::from_seed_unchecked(&seed).unwrap();
        let pk_bytes = key_pair.public_key().as_ref();
        let sig_bytes = hex::decode(&sig).unwrap();
        let peer_key = ring::signature::UnparsedPublicKey::new(&ring::signature::ED25519, pk_bytes);
        assert!(peer_key.verify(msg, &sig_bytes).is_ok());
    }
}
