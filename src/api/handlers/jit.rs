//! Astral Key - ZK JIT capability token REST handlers
//!
//! REST API endpoint for minting and verifying capability tokens.
//! Wraps the same `JitIssuer` / `JitVerifier` used by the MCP server,
//! but available without the `mcp` feature flag.

use axum::{extract::State, Json};
use serde::{Deserialize, Serialize};

use crate::auth::jit::VerifiedClaims;
use crate::auth::jwt::AuthenticatedUser;
use crate::error::{AuthError, Result};
use crate::state::AppState;

/// Request body for minting a new capability token.
#[derive(Deserialize)]
pub struct MintTokenRequest {
    /// Permission scopes to grant (e.g. ["dns:read", "pages:deploy"])
    pub scopes: Vec<String>,
    /// Target audience
    pub audience: String,
    /// TTL in seconds (defaults to the config-level default if omitted)
    pub ttl_seconds: Option<u64>,
}

/// Response body for a successful mint.
#[derive(Serialize)]
pub struct MintTokenResponse {
    /// The full signed token string: `base64(header).base64(payload).base64(signature)`
    pub token: String,
    /// Unix timestamp when the token expires
    pub expires_at: i64,
    /// Token ID (UUID v4)
    pub token_id: String,
}

/// `POST /auth/jit/mint` — Mint a new capability token.
///
/// Requires JWT authentication.
/// The JIT issuer must be configured via `JIT_ISSUER_KEY` env var.
pub async fn mint_token(
    State(state): State<AppState>,
    _auth_user: AuthenticatedUser,
    Json(request): Json<MintTokenRequest>,
) -> Result<Json<MintTokenResponse>> {
    let issuer = state
        .jit_issuer
        .as_ref()
        .ok_or_else(|| AuthError::Internal("JIT issuer not configured".to_string()))?;

    let ttl = request.ttl_seconds.unwrap_or(state.config.jit.default_ttl);

    let signed = issuer.mint(request.scopes, &request.audience, ttl);

    Ok(Json(MintTokenResponse {
        token: signed.token,
        expires_at: signed.expires_at,
        token_id: signed.token_id,
    }))
}

/// Request body for verifying a capability token.
#[derive(Deserialize)]
pub struct VerifyTokenRequest {
    /// The full signed token string to verify
    pub token: String,
}

/// Response body for a successful verification.
#[derive(Serialize)]
pub struct VerifyTokenResponse {
    pub subject: String,
    pub issuer: String,
    pub audience: String,
    pub scopes: Vec<String>,
    pub issued_at: i64,
    pub expires_at: i64,
    pub epoch: u64,
}

impl From<VerifiedClaims> for VerifyTokenResponse {
    fn from(claims: VerifiedClaims) -> Self {
        Self {
            subject: claims.subject,
            issuer: claims.issuer,
            audience: claims.audience,
            scopes: claims.scopes,
            issued_at: claims.issued_at,
            expires_at: claims.expires_at,
            epoch: claims.epoch,
        }
    }
}

/// `POST /auth/jit/verify` — Verify a capability token.
///
/// Returns the verified claims on success.
/// This endpoint is **public** (no JWT required) so that delegated services
/// can validate tokens without having to authenticate first.
///
/// The JIT verifier must be configured via `JIT_ISSUER_KEY` env var.
pub async fn verify_token(
    State(state): State<AppState>,
    Json(request): Json<VerifyTokenRequest>,
) -> Result<Json<VerifyTokenResponse>> {
    let verifier = state
        .jit_verifier
        .as_ref()
        .ok_or_else(|| AuthError::Internal("JIT verifier not configured".to_string()))?;

    let claims = verifier
        .verify(&request.token)
        .map_err(|e| AuthError::Unauthorized(format!("Token verification failed: {}", e)))?;

    Ok(Json(VerifyTokenResponse::from(claims)))
}
