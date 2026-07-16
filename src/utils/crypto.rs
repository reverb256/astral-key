//! Astral Key - Cryptographic utilities

use ring::rand::{SecureRandom, SystemRandom};
use ring::signature::{Ed25519KeyPair, KeyPair};

/// Generate a new Ed25519 key pair
pub fn generate_keypair() -> Ed25519KeyPair {
    let rng = SystemRandom::new();
    let mut seed = [0u8; 32];
    rng.fill(&mut seed).expect("SystemRandom is infallible");
    Ed25519KeyPair::from_seed_unchecked(&seed).expect("32 bytes is valid seed length")
}

/// Restore a signing key from a hex-encoded 32‑byte seed
pub fn signing_key_from_hex(hex: &str) -> Result<Ed25519KeyPair, String> {
    let bytes = hex::decode(hex).map_err(|e| format!("invalid hex: {}", e))?;
    Ed25519KeyPair::from_seed_unchecked(&bytes).map_err(|e| format!("invalid key material: {}", e))
}

/// Get the public key (verifying key) bytes from a key pair
pub fn verifying_key(key: &Ed25519KeyPair) -> &[u8] {
    key.public_key().as_ref()
}
