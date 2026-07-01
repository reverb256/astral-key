//! Astral Key - JWT authentication
//!
//! JWT token generation and validation.
//! Supports HMAC (HS256) for internal API tokens and RSA (RS256) for OIDC tokens.

pub mod claims;
pub mod middleware;

pub use claims::{Claims, TokenKind};
pub use middleware::AuthenticatedUser;

use crate::error::{AuthError, Result};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use jsonwebtoken::jwk::{
    CommonParameters, Jwk, JwkSet, KeyAlgorithm, PublicKeyUse, RSAKeyParameters, RSAKeyType,
};
use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use rsa::pkcs1::EncodeRsaPrivateKey;
use rsa::traits::PublicKeyParts;
use rsa::RsaPrivateKey;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use uuid::Uuid;

/// JWT service
#[derive(Clone)]
pub struct JwtService {
    // HMAC keys (HS256) for internal API tokens
    encoding_key: EncodingKey,
    decoding_key: DecodingKey,
    access_ttl: Duration,
    refresh_ttl: Duration,

    // RSA keys (RS256) for OIDC tokens + JWKS
    rsa_encoding_key: Option<EncodingKey>,
    rsa_decoding_key: Option<DecodingKey>,
    rsa_jwk_n: String, // base64url-encoded modulus
    rsa_jwk_e: String, // base64url-encoded exponent
    kid: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TokenPair {
    pub access_token: String,
    pub refresh_token: String,
}

impl JwtService {
    /// Create a new JWT service with HMAC keys and RSA keys for OIDC.
    pub fn new(secret: &[u8], access_ttl: Duration, refresh_ttl: Duration) -> Result<Self> {
        if secret.len() < 32 {
            return Err(AuthError::Internal(
                "JWT secret must be at least 32 bytes".to_string(),
            ));
        }

        // Generate RSA key pair for OIDC token signing
        let (rsa_encoding_key, rsa_decoding_key, rsa_jwk_n, rsa_jwk_e) =
            Self::generate_rsa_keypair().unwrap_or_else(|e| {
                tracing::warn!("Failed to generate RSA key pair for OIDC: {}", e);
                (None, None, String::new(), String::new())
            });

        let kid = uuid::Uuid::new_v4().to_string();

        Ok(Self {
            encoding_key: EncodingKey::from_secret(secret),
            decoding_key: DecodingKey::from_secret(secret),
            access_ttl,
            refresh_ttl,
            rsa_encoding_key,
            rsa_decoding_key,
            rsa_jwk_n,
            rsa_jwk_e,
            kid,
        })
    }

    /// Generate an RSA key pair and return the encoding/decoding keys + JWK components.
    fn generate_rsa_keypair() -> Result<(Option<EncodingKey>, Option<DecodingKey>, String, String)>
    {
        let mut rng = rand::thread_rng();
        let private_key = RsaPrivateKey::new(&mut rng, 2048)
            .map_err(|e| AuthError::Internal(format!("RSA key generation failed: {}", e)))?;

        // Export private key as PKCS#1 DER for jsonwebtoken (ring expects this format)
        let der = private_key
            .to_pkcs1_der()
            .map_err(|e| AuthError::Internal(format!("PKCS#1 encoding failed: {}", e)))?;

        let encoding = EncodingKey::from_rsa_der(der.as_bytes());

        // Extract public key components for JWK and decoding
        let n_bytes = private_key.n().to_bytes_be();
        let e_bytes = private_key.e().to_bytes_be();

        let decoding = DecodingKey::from_rsa_raw_components(&n_bytes, &e_bytes);

        // Base64url-encode for JWK format
        let jwk_n = URL_SAFE_NO_PAD.encode(&n_bytes);
        let jwk_e = URL_SAFE_NO_PAD.encode(&e_bytes);

        tracing::info!("RSA key pair generated successfully for OIDC JWKS");

        Ok((Some(encoding), Some(decoding), jwk_n, jwk_e))
    }

    // ── HMAC token methods (for internal API) ──────────────

    /// Generate an access token (HS256)
    pub fn generate_access_token(&self, user_id: Uuid) -> Result<String> {
        let expiration = chrono::Utc::now()
            + chrono::Duration::from_std(self.access_ttl)
                .map_err(|e| AuthError::Internal(format!("Invalid duration: {}", e)))?;

        let claims = Claims {
            sub: user_id.to_string(),
            exp: expiration.timestamp() as usize,
            iat: chrono::Utc::now().timestamp() as usize,
            kind: TokenKind::Access,
            name: None,
            preferred_username: None,
            iss: None,
            aud: None,
        };

        encode(&Header::default(), &claims, &self.encoding_key)
            .map_err(|e| AuthError::Internal(format!("Failed to encode access token: {}", e)))
    }

    /// Generate a refresh token (HS256)
    pub fn generate_refresh_token(&self, user_id: Uuid) -> Result<String> {
        let expiration = chrono::Utc::now()
            + chrono::Duration::from_std(self.refresh_ttl)
                .map_err(|e| AuthError::Internal(format!("Invalid duration: {}", e)))?;

        let claims = Claims {
            sub: user_id.to_string(),
            exp: expiration.timestamp() as usize,
            iat: chrono::Utc::now().timestamp() as usize,
            kind: TokenKind::Refresh,
            name: None,
            preferred_username: None,
            iss: None,
            aud: None,
        };

        encode(&Header::default(), &claims, &self.encoding_key)
            .map_err(|e| AuthError::Internal(format!("Failed to encode refresh token: {}", e)))
    }

    /// Generate a token pair (access + refresh)
    pub fn generate_token_pair(&self, user_id: Uuid) -> Result<TokenPair> {
        Ok(TokenPair {
            access_token: self.generate_access_token(user_id)?,
            refresh_token: self.generate_refresh_token(user_id)?,
        })
    }

    /// Validate an access token (HS256)
    pub fn validate_access_token(&self, token: &str) -> Result<Claims> {
        let mut validation = Validation::new(Algorithm::HS256);
        validation.validate_exp = true;

        let claims = decode::<Claims>(token, &self.decoding_key, &validation)
            .map_err(|e| AuthError::Unauthorized(format!("Invalid access token: {}", e)))?
            .claims;

        if claims.kind != TokenKind::Access {
            return Err(AuthError::Unauthorized(
                "Token is not an access token".to_string(),
            ));
        }

        Ok(claims)
    }

    /// Validate a refresh token (HS256)
    pub fn validate_refresh_token(&self, token: &str) -> Result<Claims> {
        let mut validation = Validation::new(Algorithm::HS256);
        validation.validate_exp = true;

        let claims = decode::<Claims>(token, &self.decoding_key, &validation)
            .map_err(|e| AuthError::Unauthorized(format!("Invalid refresh token: {}", e)))?
            .claims;

        if claims.kind != TokenKind::Refresh {
            return Err(AuthError::Unauthorized(
                "Token is not a refresh token".to_string(),
            ));
        }

        Ok(claims)
    }

    /// Extract user ID from HMAC-signed token
    pub fn extract_user_id(&self, token: &str) -> Result<Uuid> {
        let claims = self.validate_access_token(token)?;
        Uuid::parse_str(&claims.sub)
            .map_err(|_| AuthError::Internal("Invalid user ID in token".to_string()))
    }

    // ── OIDC / RS256 token methods ─────────────────────────

    /// Generate an OIDC-compatible token signed with RS256.
    /// Can be used as both `access_token` and `id_token`.
    pub fn generate_oidc_token(
        &self,
        user_id: Uuid,
        issuer: &str,
        audience: Option<String>,
        name: Option<String>,
        preferred_username: Option<String>,
    ) -> Result<String> {
        let rsa_enc = self.rsa_encoding_key.as_ref().ok_or_else(|| {
            AuthError::Internal("RSA key not available — OIDC not initialized".to_string())
        })?;

        let expiration = chrono::Utc::now()
            + chrono::Duration::from_std(self.access_ttl)
                .map_err(|e| AuthError::Internal(format!("Invalid duration: {}", e)))?;

        let claims = Claims {
            sub: user_id.to_string(),
            exp: expiration.timestamp() as usize,
            iat: chrono::Utc::now().timestamp() as usize,
            kind: TokenKind::Access,
            name,
            preferred_username,
            iss: Some(issuer.to_string()),
            aud: audience,
        };

        let header = Header::new(Algorithm::RS256);

        encode(&header, &claims, rsa_enc)
            .map_err(|e| AuthError::Internal(format!("Failed to encode OIDC token: {}", e)))
    }

    /// Validate an OIDC token (RS256) and return its claims.
    pub fn validate_oidc_token(&self, token: &str) -> Result<Claims> {
        let rsa_dec = self.rsa_decoding_key.as_ref().ok_or_else(|| {
            AuthError::Internal("RSA key not available — OIDC not initialized".to_string())
        })?;

        let mut validation = Validation::new(Algorithm::RS256);
        validation.validate_exp = true;

        let token_data = decode::<Claims>(token, rsa_dec, &validation)
            .map_err(|e| AuthError::Unauthorized(format!("Invalid OIDC token: {}", e)))?;

        Ok(token_data.claims)
    }

    /// Return the JWK Set containing the RSA public key for token verification.
    pub fn get_jwk_set(&self) -> JwkSet {
        if self.rsa_jwk_n.is_empty() || self.rsa_jwk_e.is_empty() {
            return JwkSet { keys: vec![] };
        }

        JwkSet {
            keys: vec![Jwk {
                common: CommonParameters {
                    public_key_use: Some(PublicKeyUse::Signature),
                    key_algorithm: Some(KeyAlgorithm::RS256),
                    key_id: Some(self.kid.clone()),
                    ..Default::default()
                },
                algorithm: jsonwebtoken::jwk::AlgorithmParameters::RSA(RSAKeyParameters {
                    key_type: RSAKeyType::RSA,
                    n: self.rsa_jwk_n.clone(),
                    e: self.rsa_jwk_e.clone(),
                }),
            }],
        }
    }

    /// Returns whether RSA (OIDC) keys are available.
    pub fn oidc_available(&self) -> bool {
        self.rsa_encoding_key.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_service() -> JwtService {
        let secret = b"test_secret_key_32_bytes_long_!!!";
        JwtService::new(
            secret,
            Duration::from_secs(900),
            Duration::from_secs(604800),
        )
        .unwrap()
    }

    #[test]
    fn test_generate_access_token() {
        let service = create_test_service();
        let user_id = Uuid::new_v4();

        let token = service.generate_access_token(user_id);
        assert!(token.is_ok());
    }

    #[test]
    fn test_generate_token_pair() {
        let service = create_test_service();
        let user_id = Uuid::new_v4();

        let tokens = service.generate_token_pair(user_id);
        assert!(tokens.is_ok());

        let pair = tokens.unwrap();
        assert!(!pair.access_token.is_empty());
        assert!(!pair.refresh_token.is_empty());
        assert_ne!(pair.access_token, pair.refresh_token);
    }

    #[test]
    fn test_validate_access_token() {
        let service = create_test_service();
        let user_id = Uuid::new_v4();

        let token = service.generate_access_token(user_id).unwrap();
        let claims = service.validate_access_token(&token);

        assert!(claims.is_ok());
        assert_eq!(claims.unwrap().sub, user_id.to_string());
    }

    #[test]
    fn test_refresh_token_cannot_be_used_as_access() {
        let service = create_test_service();
        let user_id = Uuid::new_v4();

        let refresh_token = service.generate_refresh_token(user_id).unwrap();
        let result = service.validate_access_token(&refresh_token);

        assert!(result.is_err());
    }

    #[test]
    fn test_oidc_available() {
        let service = create_test_service();
        assert!(service.oidc_available());
    }

    #[test]
    fn test_generate_and_validate_oidc_token() {
        let service = create_test_service();
        let user_id = Uuid::new_v4();

        let token = service.generate_oidc_token(
            user_id,
            "https://auth.example.com",
            Some("convex".to_string()),
            Some("Alice".to_string()),
            Some("alice".to_string()),
        );
        assert!(
            token.is_ok(),
            "OIDC token generation failed: {:?}",
            token.err()
        );

        let token = token.unwrap();
        let claims = service.validate_oidc_token(&token);
        assert!(
            claims.is_ok(),
            "OIDC token validation failed: {:?}",
            claims.err()
        );

        let claims = claims.unwrap();
        assert_eq!(claims.sub, user_id.to_string());
        assert_eq!(claims.name, Some("Alice".to_string()));
        assert_eq!(claims.preferred_username, Some("alice".to_string()));
        assert_eq!(claims.iss, Some("https://auth.example.com".to_string()));
        assert_eq!(claims.aud, Some("convex".to_string()));
    }

    #[test]
    fn test_get_jwk_set() {
        let service = create_test_service();
        let jwks = service.get_jwk_set();
        assert!(
            !jwks.keys.is_empty(),
            "JWK set should have at least one key"
        );
        assert_eq!(jwks.keys.len(), 1);

        let key = &jwks.keys[0];
        assert_eq!(key.common.key_algorithm, Some(KeyAlgorithm::RS256));
        assert_eq!(key.common.public_key_use, Some(PublicKeyUse::Signature));

        match &key.algorithm {
            jsonwebtoken::jwk::AlgorithmParameters::RSA(params) => {
                assert!(!params.n.is_empty());
                assert!(!params.e.is_empty());
            }
            _ => panic!("Expected RSA key parameters"),
        }
    }
}
