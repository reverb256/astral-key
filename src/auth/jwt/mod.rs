//! Astral Key - JWT authentication
//!
//! JWT token generation and validation (HMAC HS256 only).

pub mod claims;
pub mod middleware;

pub use claims::{Claims, TokenKind};
pub use middleware::AuthenticatedUser;

use crate::error::{AuthError, Result};
use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use uuid::Uuid;

/// JWT service — HMAC HS256 only
#[derive(Clone)]
pub struct JwtService {
    encoding_key: EncodingKey,
    decoding_key: DecodingKey,
    access_ttl: Duration,
    refresh_ttl: Duration,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TokenPair {
    pub access_token: String,
    pub refresh_token: String,
}

impl JwtService {
    /// Create a new JWT service with HMAC key
    pub fn new(secret: &[u8], access_ttl: Duration, refresh_ttl: Duration) -> Result<Self> {
        if secret.len() < 32 {
            return Err(AuthError::Internal(
                "JWT secret must be at least 32 bytes".to_string(),
            ));
        }

        Ok(Self {
            encoding_key: EncodingKey::from_secret(secret),
            decoding_key: DecodingKey::from_secret(secret),
            access_ttl,
            refresh_ttl,
        })
    }

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
    #[allow(dead_code)]
    pub fn extract_user_id(&self, token: &str) -> Result<Uuid> {
        let claims = self.validate_access_token(token)?;
        Uuid::parse_str(&claims.sub)
            .map_err(|_| AuthError::Internal("Invalid user ID in token".to_string()))
    }

    /// Validate any JWT token (access or refresh) — returns claims without checking token kind.
    /// Used by the token verification endpoint for external services (e.g., Quill MCP).
    pub fn validate_token(&self, token: &str) -> Result<Claims> {
        let mut validation = Validation::new(Algorithm::HS256);
        validation.validate_exp = true;

        let claims = decode::<Claims>(token, &self.decoding_key, &validation)
            .map_err(|e| AuthError::Unauthorized(format!("Invalid token: {}", e)))?
            .claims;

        Ok(claims)
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
}
