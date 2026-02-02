//! Astral Key - Web3 authentication handlers

use axum::{extract::State, Json};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::error::{AuthError, Result};
use crate::state::AppState;

/// Request a nonce for SIWE
pub async fn nonce(State(_state): State<AppState>) -> Result<Json<NonceResponse>> {
    // TODO: Generate and store nonce
    Ok(Json(NonceResponse {
        nonce: "placeholder_nonce".to_string(),
        message_template: "Sign in to Astral Key".to_string(),
        domain: "localhost".to_string(),
    }))
}

/// Verify Web3 signature
pub async fn verify(
    State(_state): State<AppState>,
    Json(_request): Json<VerifyRequest>,
) -> Result<Json<AuthResponse>> {
    // TODO: Verify SIWE signature
    Err(AuthError::NotImplemented(
        "Web3 verification not yet implemented".to_string(),
    ))
}

/// Get supported chains
pub async fn chains(State(_state): State<AppState>) -> Result<Json<serde_json::Value>> {
    Ok(Json(json!({
        "chains": [
            { "id": 1, "name": "ethereum", "display_name": "Ethereum" },
            { "id": 137, "name": "polygon", "display_name": "Polygon" },
            { "id": 42161, "name": "arbitrum", "display_name": "Arbitrum" },
            { "id": 10, "name": "optimism", "display_name": "Optimism" },
        ]
    })))
}

/// Nonce response
#[derive(Serialize)]
pub struct NonceResponse {
    pub nonce: String,
    pub message_template: String,
    pub domain: String,
}

/// Verify request
#[derive(Deserialize)]
pub struct VerifyRequest {
    pub message: String,
    pub signature: String,
    pub chain_id: u64,
}

/// Authentication response
#[derive(Serialize)]
pub struct AuthResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub user: UserInfo,
}

/// User info
#[derive(Serialize)]
pub struct UserInfo {
    pub id: String,
    pub address: String,
    pub chain_id: u64,
}

use crate::error::AuthError as NotImplementedError;

impl AuthError {
    fn NotImplemented(msg: String) -> Self {
        AuthError::Internal(msg)
    }
}
