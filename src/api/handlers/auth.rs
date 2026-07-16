//! Astral Key - Token verification handlers
//!
//! Endpoints for external services to verify JWT tokens.

use axum::{extract::State, Json};
use serde::Deserialize;
use serde_json::json;

use crate::state::AppState;

/// Token verification request
#[derive(Deserialize)]
pub struct VerifyTokenRequest {
    pub token: String,
}

/// Verify a JWT token (used by Quill MCP and other external services)
///
/// Returns `{ valid: true, sub, exp }` on success or `{ valid: false, error }` on failure.
pub async fn verify_token(
    State(state): State<AppState>,
    Json(request): Json<VerifyTokenRequest>,
) -> Json<serde_json::Value> {
    let jwt = match state.jwt.as_ref() {
        Some(jwt) => jwt,
        None => {
            return Json(json!({
                "valid": false,
                "error": "JWT service not initialized"
            }));
        }
    };

    match jwt.validate_token(&request.token) {
        Ok(claims) => Json(json!({
            "valid": true,
            "sub": claims.sub,
            "exp": claims.exp,
        })),
        Err(e) => Json(json!({
            "valid": false,
            "error": e.to_string(),
        })),
    }
}
