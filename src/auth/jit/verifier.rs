//! JIT token verifier — validates capability tokens WITHOUT database access.
//!
//! Verification is pure crypto:
//! 1. Decode base64 `header.payload.signature`
//! 2. Verify Ed25519 signature against issuer's public key
//! 3. Check expiry (with configurable leeway)
//! 4. Check global epoch (reject stale tokens)
//! 5. Check revocation tombstones
//!
//! ## Thread safety
//! All mutation is behind `RwLock` or `AtomicU64`, so a single `JitVerifier`
//! can be shared across threads via `Arc`.

use std::collections::{HashMap, HashSet};

use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use ed25519_dalek::{Signature, Verifier, VerifyingKey};

use crate::auth::jit::{Capability, VerificationError, VerifiedClaims};

/// JIT token verifier.
///
/// Validates capability tokens using only cryptographic operations and
/// in-memory state (epoch counter, revocation set, known issuer keys).
pub struct JitVerifier {
    /// Mapping of `issuer_id -> VerifyingKey` for known issuers
    verifying_keys: std::sync::RwLock<HashMap<String, VerifyingKey>>,
    /// Current global epoch — tokens with `epoch < current` are rejected
    current_epoch: std::sync::atomic::AtomicU64,
    /// Set of revoked token IDs (maintained by `revoke_token()`)
    revoked_tokens: std::sync::RwLock<HashSet<String>>,
    /// Clock skew leeway in seconds (default: 5)
    leeway_seconds: i64,
}

impl JitVerifier {
    /// Create a new verifier with default leeway (5 seconds).
    pub fn new() -> Self {
        Self {
            verifying_keys: std::sync::RwLock::new(HashMap::new()),
            current_epoch: std::sync::atomic::AtomicU64::new(0),
            revoked_tokens: std::sync::RwLock::new(HashSet::new()),
            leeway_seconds: 5,
        }
    }

    /// Register a trusted issuer's public key.
    ///
    /// The `public_key` must be a 32-byte Ed25519 verifying key.
    /// If the issuer is already registered, the key is **updated**.
    pub fn add_issuer_key(&self, issuer_id: &str, public_key: &[u8; 32]) {
        // `from_bytes` only fails for invalid curve points; we unwrap
        // because `add_issuer_key` is called with a known-good key.
        let vk = VerifyingKey::from_bytes(public_key).expect("Invalid Ed25519 verifying key bytes");
        let mut keys = self.verifying_keys.write().expect("Verifier lock poisoned");
        keys.insert(issuer_id.to_string(), vk);
    }

    /// Set the current global epoch.
    ///
    /// Tokens minted at an epoch **lower** than this value will be rejected
    /// with [`VerificationError::StaleEpoch`].
    pub fn set_epoch(&self, epoch: u64) {
        self.current_epoch
            .store(epoch, std::sync::atomic::Ordering::Release);
    }

    /// Get the current global epoch.
    pub fn current_epoch(&self) -> u64 {
        self.current_epoch
            .load(std::sync::atomic::Ordering::Acquire)
    }

    /// Set the clock skew leeway in seconds (default: 5).
    ///
    /// A token that expired within the leeway window is still considered
    /// valid. This prevents failures from minor clock drift between
    /// the issuer and verifier.
    pub fn set_leeway(&mut self, seconds: i64) {
        self.leeway_seconds = seconds;
    }

    /// Verify and decode a capability token.
    ///
    /// Performs the full validation pipeline:
    /// 1. Decode the 3-part base64 token
    /// 2. Verify Ed25519 signature
    /// 3. Deserialize the capability payload
    /// 4. Check token expiry (with leeway)
    /// 5. Check global epoch staleness
    /// 6. Check token revocation
    ///
    /// # Errors
    ///
    /// Returns [`VerificationError`] on any validation failure.
    pub fn verify(&self, token: &str) -> Result<VerifiedClaims, VerificationError> {
        // Step 1: Split the token into 3 parts
        let parts: Vec<&str> = token.split('.').collect();
        if parts.len() != 3 {
            return Err(VerificationError::Decode(
                "Token must have exactly 3 dot-separated parts".to_string(),
            ));
        }
        let header_b64 = parts[0];
        let payload_b64 = parts[1];
        let sig_b64 = parts[2];

        // Decode base64 payload
        let payload_bytes = BASE64
            .decode(payload_b64)
            .map_err(|e| VerificationError::Decode(format!("Payload base64: {}", e)))?;

        // Decode base64 signature
        let sig_bytes = BASE64
            .decode(sig_b64)
            .map_err(|e| VerificationError::Decode(format!("Signature base64: {}", e)))?;

        // Decode base64 header (validate it's proper JSON, but contents aren't critical)
        let _header_bytes = BASE64
            .decode(header_b64)
            .map_err(|e| VerificationError::Decode(format!("Header base64: {}", e)))?;

        // Step 2: Reconstruct signed data (header_b64 + "." + payload_b64)
        let signed_data = format!("{}.{}", header_b64, payload_b64);

        // Parse signature bytes
        let sig_array: [u8; 64] = sig_bytes.as_slice().try_into().map_err(|_| {
            VerificationError::Decode("Signature must be exactly 64 bytes".to_string())
        })?;
        let signature = Signature::from_bytes(&sig_array);

        // Parse capability from payload
        let capability: Capability = serde_json::from_slice(&payload_bytes)
            .map_err(|e| VerificationError::Decode(format!("Payload JSON: {}", e)))?;

        // Step 2 (cont): Find the issuer's verifying key
        let vk = {
            let keys = self
                .verifying_keys
                .read()
                .map_err(|_| VerificationError::Decode("Lock poisoned".to_string()))?;
            keys.get(&capability.iss)
                .copied()
                .ok_or_else(|| VerificationError::UnknownIssuer(capability.iss.clone()))?
        };

        // Step 3: Verify Ed25519 signature
        vk.verify(signed_data.as_bytes(), &signature)
            .map_err(|_| VerificationError::InvalidSignature)?;

        // Step 4: Check expiry (with leeway)
        let now = chrono::Utc::now().timestamp();
        if now > capability.exp + self.leeway_seconds {
            return Err(VerificationError::Expired(capability.exp));
        }

        // Step 5: Check global epoch
        let current_epoch = self
            .current_epoch
            .load(std::sync::atomic::Ordering::Acquire);
        if capability.epoch < current_epoch {
            return Err(VerificationError::StaleEpoch(
                capability.epoch,
                current_epoch,
            ));
        }

        // Step 6: Check revocation
        {
            let revoked = self
                .revoked_tokens
                .read()
                .map_err(|_| VerificationError::Decode("Lock poisoned".to_string()))?;
            if revoked.contains(&capability.sub) {
                return Err(VerificationError::Revoked(capability.sub));
            }
        }

        // All checks passed — return verified claims
        Ok(VerifiedClaims {
            subject: capability.sub,
            issuer: capability.iss,
            audience: capability.aud,
            scopes: capability.scopes,
            issued_at: capability.iat,
            expires_at: capability.exp,
            epoch: capability.epoch,
        })
    }

    /// Revoke a token by its ID (the `sub` field in the capability).
    ///
    /// The revoked token ID is added to an in-memory set. This set is
    /// **not** persisted — use [`TombstoneJournal`](super::epoch::TombstoneJournal)
    /// for durable revocation tracking.
    pub fn revoke_token(&self, token_id: &str) {
        let mut revoked = self.revoked_tokens.write().expect("Verifier lock poisoned");
        revoked.insert(token_id.to_string());
    }

    /// Check if a token ID has been revoked.
    pub fn is_revoked(&self, token_id: &str) -> bool {
        self.revoked_tokens
            .read()
            .map_or(false, |r| r.contains(token_id))
    }
}

impl Default for JitVerifier {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::jit::issuer::JitIssuer;

    fn test_verifier_and_issuer() -> (JitVerifier, JitIssuer) {
        let key_hex = hex::encode([0xabu8; 32]);
        let issuer = JitIssuer::new(&key_hex, "ak:issuer:test").unwrap();

        // Derive the public key
        let key_bytes = hex::decode(&key_hex).unwrap();
        let mut arr = [0u8; 32];
        arr.copy_from_slice(&key_bytes);
        let signing_key = ed25519_dalek::SigningKey::from_bytes(&arr);
        let vk_bytes = signing_key.verifying_key().to_bytes();

        let verifier = JitVerifier::new();
        verifier.add_issuer_key("ak:issuer:test", &vk_bytes);

        (verifier, issuer)
    }

    #[test]
    fn test_verify_valid_token() {
        let (verifier, issuer) = test_verifier_and_issuer();
        let token = issuer.mint(vec!["dns:read".to_string()], "test-aud", 3600);
        let result = verifier.verify(&token.token);
        assert!(result.is_ok());
        let claims = result.unwrap();
        assert_eq!(claims.subject, token.token_id);
        assert_eq!(claims.issuer, "ak:issuer:test");
        assert_eq!(claims.audience, "test-aud");
        assert_eq!(claims.scopes, vec!["dns:read"]);
    }

    #[test]
    fn test_verify_expired_token() {
        let (verifier, issuer) = test_verifier_and_issuer();
        // Mint with 0 TTL — already expired
        let token = issuer.mint(vec!["admin".to_string()], "aud", 0);
        // Sleep briefly to ensure the token is expired
        std::thread::sleep(std::time::Duration::from_millis(10));
        let result = verifier.verify(&token.token);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), VerificationError::Expired(_)));
    }

    #[test]
    fn test_verify_wrong_issuer_key() {
        let (verifier, _issuer) = test_verifier_and_issuer();

        // Create a token with a different key
        let other_hex = hex::encode([0x01u8; 32]);
        let other_issuer = JitIssuer::new(&other_hex, "ak:issuer:other").unwrap();
        let token = other_issuer.mint(vec!["admin".to_string()], "aud", 3600);

        // Verifier only knows about ak:issuer:test — should fail as unknown issuer
        // Actually, the token has iss="ak:issuer:other" which is not registered
        let result = verifier.verify(&token.token);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            VerificationError::UnknownIssuer(_)
        ));
    }

    #[test]
    fn test_verify_stale_epoch() {
        let (verifier, issuer) = test_verifier_and_issuer();
        let token = issuer.mint(vec!["admin".to_string()], "aud", 3600);

        // Bump the epoch on the verifier
        verifier.set_epoch(1);

        let result = verifier.verify(&token.token);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            VerificationError::StaleEpoch(0, 1)
        ));
    }

    #[test]
    fn test_verify_revoked_token() {
        let (verifier, issuer) = test_verifier_and_issuer();
        let token = issuer.mint(vec!["admin".to_string()], "aud", 3600);

        // Revoke the token
        verifier.revoke_token(&token.token_id);

        let result = verifier.verify(&token.token);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), VerificationError::Revoked(_)));
    }

    #[test]
    fn test_verify_malformed_token() {
        let (verifier, _issuer) = test_verifier_and_issuer();

        assert!(matches!(
            verifier.verify("not-a-valid-token"),
            Err(VerificationError::Decode(_))
        ));

        assert!(matches!(
            verifier.verify("only.two"),
            Err(VerificationError::Decode(_))
        ));
    }

    #[test]
    fn test_add_issuer_key_then_verify() {
        let verifier = JitVerifier::new();

        // Generate a fresh key, register it
        let key_hex = hex::encode([0xabu8; 32]);
        let key_bytes = hex::decode(&key_hex).unwrap();
        let mut arr = [0u8; 32];
        arr.copy_from_slice(&key_bytes);
        let signing_key = ed25519_dalek::SigningKey::from_bytes(&arr);
        let vk_bytes = signing_key.verifying_key().to_bytes();

        verifier.add_issuer_key("ak:issuer:dynamic", &vk_bytes);

        // Now mint and verify
        let issuer = JitIssuer::new(&key_hex, "ak:issuer:dynamic").unwrap();
        let token = issuer.mint(vec!["pages:read".to_string()], "app", 300);

        let result = verifier.verify(&token.token);
        assert!(result.is_ok());
        let claims = result.unwrap();
        assert_eq!(claims.issuer, "ak:issuer:dynamic");
    }

    #[test]
    fn test_is_revoked() {
        let verifier = JitVerifier::new();
        assert!(!verifier.is_revoked("nonexistent"));
        verifier.revoke_token("token-123");
        assert!(verifier.is_revoked("token-123"));
    }
}
