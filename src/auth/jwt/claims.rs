//! Astral Key - JWT claims

use serde::{Deserialize, Serialize};

/// JWT claims for HMAC-signed tokens
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Claims {
    /// Subject (user ID)
    pub sub: String,
    /// Expiration time
    pub exp: usize,
    /// Issued at
    pub iat: usize,
    /// Token kind (access or refresh)
    pub kind: TokenKind,
}

/// Token kind to distinguish between access and refresh tokens
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum TokenKind {
    Access,
    Refresh,
}
