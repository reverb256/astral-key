//! Astral Key - JWT claims

use serde::{Deserialize, Serialize};

/// JWT claims — supports HMAC tokens and OIDC RS256 tokens
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

    /// OIDC: display name
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,

    /// OIDC: preferred username
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preferred_username: Option<String>,

    /// OIDC: issuer URL
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub iss: Option<String>,

    /// OIDC: audience (required by Convex and other JWT verifiers)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub aud: Option<String>,
}

/// Token kind to distinguish between access and refresh tokens
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum TokenKind {
    Access,
    Refresh,
}
