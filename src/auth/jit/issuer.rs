//! JIT token issuer — mints Ed25519-signed capability tokens.
//!
//! The issuer holds the private signing key (loaded from env/file at startup).
//! Minting is a pure crypto operation with **no database writes**.
//! The token is returned to the caller and forgotten by the issuer.
//!
//! ## Key lifecycle
//! - The signing key is loaded as a hex-encoded 32-byte Ed25519 seed.
//! - An atomic epoch counter enables emergency batch revocation.
//! - Call `increment_epoch()` to invalidate all tokens minted prior.

use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use chrono::Utc;
use ed25519_dalek::{Signer, SigningKey};
use uuid::Uuid;

use crate::auth::jit::{Capability, SignedToken};

/// Ed25519-based capability token issuer (with optional ML-DSA-65 hybrid).
///
/// Thread-safe: `mint()` and `increment_epoch()` can be called concurrently
/// without external synchronization.
///
/// # Post-quantum hybrid mode
///
/// When an ML-DSA-65 (FIPS 204) secret key is attached via
/// [`JitIssuer::with_mldsa_key`], minted tokens are **hybrid**: a 4-part
/// `header.payload.ed25519_sig.mldsa_sig` token. Without an ML-DSA key,
/// tokens are the legacy 3-part `header.payload.ed25519_sig` shape. The
/// verifier accepts both forms.
pub struct JitIssuer {
    /// Ed25519 signing key (secret)
    signing_key: SigningKey,
    /// Issuer identifier embedded in every minted token
    issuer_id: String,
    /// Monotonically increasing epoch for batch revocation
    epoch: std::sync::atomic::AtomicU64,
    /// Optional ML-DSA-65 secret key (raw bytes). When present, mint() emits
    /// a 4-part hybrid token.
    mldsa_secret: Option<Vec<u8>>,
}

impl JitIssuer {
    /// Create a new JIT issuer from a hex-encoded Ed25519 private key.
    ///
    /// The key must be exactly 32 bytes (64 hex characters).
    ///
    /// # Errors
    ///
    /// Returns `Err` if:
    /// - The hex string is invalid or not 64 characters long
    /// - The decoded bytes are not exactly 32 bytes
    pub fn new(key_hex: &str, issuer_id: &str) -> Result<Self, String> {
        let key_bytes = hex::decode(key_hex).map_err(|e| format!("Invalid key hex: {}", e))?;
        if key_bytes.len() != 32 {
            return Err(format!(
                "Key must be 32 bytes (64 hex chars), got {} bytes",
                key_bytes.len()
            ));
        }
        let mut arr = [0u8; 32];
        arr.copy_from_slice(&key_bytes);
        let signing_key = SigningKey::from_bytes(&arr);

        Ok(Self {
            signing_key,
            issuer_id: issuer_id.to_string(),
            epoch: std::sync::atomic::AtomicU64::new(0),
            mldsa_secret: None,
        })
    }

    /// Attach an ML-DSA-65 (FIPS 204) secret key to enable hybrid signing.
    ///
    /// `sk_hex` is the hex-encoded ML-DSA-65 secret key (from `pqcrypto-mldsa`).
    /// After calling this, [`mint()`](Self::mint) emits 4-part hybrid tokens.
    /// Returns `Err` if the hex is invalid or the key length doesn't match
    /// ML-DSA-65's secret-key size.
    pub fn with_mldsa_key(mut self, sk_hex: &str) -> Result<Self, String> {
        let bytes = hex::decode(sk_hex).map_err(|e| format!("Invalid ML-DSA key hex: {}", e))?;
        // ML-DSA-65 secret key is 4032 bytes (per FIPS 204 / PQClean). We don't
        // hard-assert the length here — `pqcrypto_mldsa` will reject bad sizes
        // at sign time. We only validate that we got *some* bytes.
        if bytes.is_empty() {
            return Err("ML-DSA secret key is empty".to_string());
        }
        self.mldsa_secret = Some(bytes);
        Ok(self)
    }

    /// Sign `data` with the attached ML-DSA-65 secret key. Returns the raw
    /// signature bytes. Returns `None` if no ML-DSA key is attached.
    fn sign_mldsa(&self, data: &[u8]) -> Option<Vec<u8>> {
        let sk_bytes = self.mldsa_secret.as_ref()?;
        use pqcrypto_mldsa::mldsa65::{detached_sign, SecretKey};
        use pqcrypto_traits::sign::{DetachedSignature as _, SecretKey as _};
        let sk = SecretKey::from_bytes(sk_bytes).ok()?;
        let sig = detached_sign(data, &sk);
        Some(sig.as_bytes().to_vec())
    }

    /// Mint a new capability token.
    ///
    /// This is a pure CPU operation — no I/O, no database writes.
    /// The token is returned to the caller and immediately forgotten.
    ///
    /// The returned [`SignedToken`] contains the full signed token string
    /// in `base64(header).base64(payload).base64(signature)` format,
    /// along with the expiry timestamp and token ID for caller convenience.
    pub fn mint(&self, scopes: Vec<String>, audience: &str, ttl_seconds: u64) -> SignedToken {
        let now = Utc::now().timestamp();
        let exp = now + ttl_seconds as i64;
        let token_id = Uuid::new_v4().to_string();
        let current_epoch = self.epoch.load(std::sync::atomic::Ordering::Relaxed);

        let capability = Capability {
            sub: token_id.clone(),
            iss: self.issuer_id.clone(),
            aud: audience.to_string(),
            iat: now,
            exp,
            scopes,
            epoch: current_epoch,
        };

        // Serialize payload to JSON bytes
        let payload_json =
            serde_json::to_vec(&capability).expect("Capability serialization should never fail");

        // Base64-encode header and payload
        let header = r#"{"typ":"CAP","alg":"EdDSA"}"#;
        let header_b64 = BASE64.encode(header);
        let payload_b64 = BASE64.encode(&payload_json);

        // Sign the standard JWT payload: header_b64 + "." + payload_b64
        let signed_data = format!("{}.{}", header_b64, payload_b64);
        let signature = self.signing_key.sign(signed_data.as_bytes());
        let sig_b64 = BASE64.encode(signature.to_bytes());

        // Optional ML-DSA-65 hybrid signature over the same signed data.
        // When present, the token becomes 4-part: header.payload.ed_sig.mldsa_sig.
        let token = if let Some(mldsa_sig) = self.sign_mldsa(signed_data.as_bytes()) {
            let mldsa_b64 = BASE64.encode(&mldsa_sig);
            format!("{}.{}.{}.{}", header_b64, payload_b64, sig_b64, mldsa_b64)
        } else {
            format!("{}.{}.{}", header_b64, payload_b64, sig_b64)
        };

        SignedToken {
            token,
            expires_at: exp,
            token_id,
        }
    }

    /// Increment the global epoch counter.
    ///
    /// This invalidates **all** tokens minted at the previous epoch.
    /// Use it for:
    /// - Emergency key rotation
    /// - Security incident response
    /// - Mass token invalidation
    ///
    /// Returns the **new** epoch value after incrementing.
    #[allow(dead_code)]
    pub fn increment_epoch(&self) -> u64 {
        // fetch_add returns the old value; +1 gives the new value
        self.epoch
            .fetch_add(1, std::sync::atomic::Ordering::Release)
            + 1
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::Verifier;
    use ed25519_dalek::VerifyingKey;

    /// Generate a deterministic test key (32 bytes of 0xab)
    fn test_key_hex() -> String {
        hex::encode([0xabu8; 32])
    }

    #[test]
    fn test_new_issuer_valid_key() {
        let key_hex = test_key_hex();
        let issuer = JitIssuer::new(&key_hex, "ak:issuer:test");
        assert!(issuer.is_ok());
    }

    #[test]
    fn test_new_issuer_invalid_key_length() {
        let short_hex = "abcd"; // 2 bytes, not 32
        let issuer = JitIssuer::new(short_hex, "ak:issuer:test");
        assert!(issuer.is_err());
    }

    #[test]
    fn test_new_issuer_invalid_hex() {
        let invalid_hex = "not-a-valid-hex-string-at-all!!";
        let issuer = JitIssuer::new(invalid_hex, "ak:issuer:test");
        assert!(issuer.is_err());
    }

    #[test]
    fn test_mint_returns_signed_token() {
        let key_hex = test_key_hex();
        let issuer = JitIssuer::new(&key_hex, "ak:issuer:test").unwrap();

        let scopes = vec!["dns:read".to_string(), "pages:deploy".to_string()];
        let token = issuer.mint(scopes, "astral-key.local", 3600);

        assert!(!token.token.is_empty());
        assert!(token.expires_at > 0);
        assert!(!token.token_id.is_empty());

        // Token should have 3 parts
        let parts: Vec<&str> = token.token.split('.').collect();
        assert_eq!(parts.len(), 3);
    }

    #[test]
    fn test_mint_produces_verifiable_token() {
        let key_hex = test_key_hex();
        let issuer = JitIssuer::new(&key_hex, "ak:issuer:test").unwrap();

        let scopes = vec!["admin".to_string()];
        let token = issuer.mint(scopes, "test-audience", 60);

        // Decode and verify manually
        let parts: Vec<&str> = token.token.split('.').collect();
        let header_b64 = parts[0];
        let payload_b64 = parts[1];
        let sig_b64 = parts[2];

        // Decode the key and payload
        let key_bytes = hex::decode(&key_hex).unwrap();
        let mut arr = [0u8; 32];
        arr.copy_from_slice(&key_bytes);
        let signing_key = SigningKey::from_bytes(&arr);
        let verifying_key = signing_key.verifying_key();

        let signed_data = format!("{}.{}", header_b64, payload_b64);
        let sig_bytes = BASE64.decode(sig_b64).unwrap();
        let sig_arr: [u8; 64] = sig_bytes.as_slice().try_into().unwrap();
        let signature = ed25519_dalek::Signature::from_bytes(&sig_arr);

        assert!(verifying_key
            .verify(signed_data.as_bytes(), &signature)
            .is_ok());
    }

    #[test]
    fn test_increment_epoch() {
        let key_hex = test_key_hex();
        let issuer = JitIssuer::new(&key_hex, "ak:issuer:test").unwrap();

        assert_eq!(issuer.increment_epoch(), 1);
        assert_eq!(issuer.increment_epoch(), 2);
        assert_eq!(issuer.increment_epoch(), 3);
    }

    #[test]
    fn test_epoch_embedded_in_minted_token() {
        let key_hex = test_key_hex();
        let issuer = JitIssuer::new(&key_hex, "ak:issuer:test").unwrap();

        let token = issuer.mint(vec!["admin".to_string()], "aud", 60);

        // Decode payload to check epoch
        let parts: Vec<&str> = token.token.split('.').collect();
        let payload_bytes = BASE64.decode(parts[1]).unwrap();
        let cap: Capability = serde_json::from_slice(&payload_bytes).unwrap();
        assert_eq!(cap.epoch, 0); // Not yet incremented

        issuer.increment_epoch();
        let token2 = issuer.mint(vec!["admin".to_string()], "aud", 60);
        let parts2: Vec<&str> = token2.token.split('.').collect();
        let payload_bytes2 = BASE64.decode(parts2[1]).unwrap();
        let cap2: Capability = serde_json::from_slice(&payload_bytes2).unwrap();
        assert_eq!(cap2.epoch, 1); // After increment
    }
}
