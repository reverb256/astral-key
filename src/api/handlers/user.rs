//! Astral Key - User management handlers

use axum::{extract::State, Json};
use serde::Deserialize;

use crate::error::{AuthError, Result};
use crate::state::AppState;

/// Get current user info — reserved for future implementation.
/// Returns HTTP 501 with a structured `not_implemented` error so callers can
/// distinguish "feature reserved" from "service down". See ROADMAP.md.
pub async fn me(State(_state): State<AppState>) -> Result<Json<serde_json::Value>> {
    Err(AuthError::NotImplemented("user_get_reserved".to_string()))
}

/// Update current user — reserved. Identity is rooted in passkey / SIWE
/// wallet; only `display_name` updates will eventually be supported.
pub async fn update(
    State(_state): State<AppState>,
    Json(_request): Json<UpdateUserRequest>,
) -> Result<Json<serde_json::Value>> {
    Err(AuthError::NotImplemented(
        "user_update_reserved".to_string(),
    ))
}

/// Delete current user — reserved. Self-hosters control deletion via DB ops.
pub async fn delete(State(_state): State<AppState>) -> Result<Json<serde_json::Value>> {
    Err(AuthError::NotImplemented(
        "user_delete_reserved".to_string(),
    ))
}

/// List user's security keys (passkeys) — reserved; use
/// `/v1/auth/passkey/credentials` on maplespike instead.
pub async fn security_keys(State(_state): State<AppState>) -> Result<Json<serde_json::Value>> {
    Err(AuthError::NotImplemented(
        "security_keys_reserved".to_string(),
    ))
}

/// Update user request
///
/// Decision 0002: `email` field REMOVED. Astral-key never
/// stores plaintext email; identity is rooted in the
/// passkey public key or SIWE wallet address. Display name
/// remains optional and is treated as a user-chosen label,
/// not PII.
#[derive(Deserialize)]
pub struct UpdateUserRequest {
    pub display_name: Option<String>,
}
