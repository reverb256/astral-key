//! Astral Key - Argon2id hashing helpers for API keys.
//!
//! Key generation: 32 random bytes → base58 (~44 chars) → prefixed → Argon2id hash.
//! Only the hash is stored; the plaintext is returned to the caller exactly once.

use argon2::password_hash::SaltString;
use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier};
use rand::rngs::OsRng;
use rand::RngCore;

use crate::error::{AuthError, Result};

/// Generate an API key with Argon2id hashing.
///
/// Returns `(hash, prefix, raw_key)`:
/// - `hash`: Argon2id PHC string to store in the database
/// - `prefix`: e.g. `"ak_prod_"` for prefix-based lookup
/// - `raw_key`: full plaintext key (shown once, never stored)
pub fn generate_api_key(environment: &str) -> Result<(String, String, String)> {
    // 1. Generate 32 random bytes
    let mut bytes = [0u8; 32];
    OsRng.fill_bytes(&mut bytes);

    // 2. Base58 encode → ~44 chars
    let encoded = bs58::encode(&bytes).into_string();

    // 3. Prefix with environment marker
    let prefix = format!("ak_{}_", environment);
    let raw_key = format!("{}{}", prefix, encoded);

    // 4. Hash with Argon2id (random salt, default params)
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    let hash = argon2
        .hash_password(raw_key.as_bytes(), &salt)
        .map_err(|e| AuthError::Internal(format!("Failed to hash API key: {}", e)))?
        .to_string();

    Ok((hash, prefix, raw_key))
}

/// Verify an API key against its stored Argon2id hash.
///
/// Returns `true` if the key matches the hash, `false` otherwise.
#[allow(dead_code)]
pub(crate) fn verify_api_key(key: &str, hash: &str) -> Result<bool> {
    let parsed_hash = PasswordHash::new(hash)
        .map_err(|e| AuthError::Internal(format!("Invalid password hash: {}", e)))?;
    let argon2 = Argon2::default();
    Ok(argon2.verify_password(key.as_bytes(), &parsed_hash).is_ok())
}

/// Extract the key prefix from a full API key.
///
/// e.g. `"ak_prod_xxxxxxxxx"` → `"ak_prod_"`
#[allow(dead_code)]
pub fn extract_prefix(key: &str) -> &str {
    // Find the last underscore (separates environment from base58 payload)
    if let Some(pos) = key.rfind('_') {
        &key[..=pos]
    } else {
        // Fallback — should never happen with valid keys
        "ak_prod_"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_api_key_format() {
        let (hash, prefix, raw_key) = generate_api_key("prod").unwrap();
        assert!(prefix == "ak_prod_");
        assert!(raw_key.starts_with("ak_prod_"));
        assert!(raw_key.len() > 44);
        assert!(raw_key.len() < 64);
        assert!(hash.starts_with("$argon2id$"));
    }

    #[test]
    fn test_generate_api_key_env_dev() {
        let (_, prefix, raw_key) = generate_api_key("dev").unwrap();
        assert_eq!(prefix, "ak_dev_");
        assert!(raw_key.starts_with("ak_dev_"));
    }

    #[test]
    fn test_verify_api_key_valid() {
        let (hash, _, raw_key) = generate_api_key("prod").unwrap();
        let result = verify_api_key(&raw_key, &hash).unwrap();
        assert!(result);
    }

    #[test]
    fn test_verify_api_key_invalid() {
        let (hash, _, _) = generate_api_key("prod").unwrap();
        let result = verify_api_key("ak_prod_wrongkey123", &hash).unwrap();
        assert!(!result);
    }

    #[test]
    fn test_extract_prefix_standard() {
        assert_eq!(extract_prefix("ak_prod_abc123"), "ak_prod_");
        assert_eq!(extract_prefix("ak_dev_xyz789"), "ak_dev_");
        assert_eq!(extract_prefix("ak_ci_test123"), "ak_ci_");
    }
}
