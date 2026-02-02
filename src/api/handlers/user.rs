//! Astral Key - User management handlers

use axum::{extract::State, Json};
use serde::Deserialize;
use serde_json::json;

use crate::error::{AuthError, Result};
use crate::state::AppState;

/// Get current user info
pub async fn me(State(_state): State<AppState>) -> Result<Json<serde_json::Value>> {
    // TODO: Return current user info
    Err(AuthError::Internal(
        "User info not yet implemented".to_string(),
    ))
}

/// Update current user
pub async fn update(
    State(_state): State<AppState>,
    Json(_request): Json<UpdateUserRequest>,
) -> Result<Json<serde_json::Value>> {
    // TODO: Update user info
    Err(AuthError::Internal(
        "User update not yet implemented".to_string(),
    ))
}

/// Delete current user
pub async fn delete(State(_state): State<AppState>) -> Result<Json<serde_json::Value>> {
    // TODO: Delete user account
    Err(AuthError::Internal(
        "User deletion not yet implemented".to_string(),
    ))
}

/// List user's security keys (passkeys)
pub async fn security_keys(State(_state): State<AppState>) -> Result<Json<serde_json::Value>> {
    // TODO: List security keys
    Err(AuthError::Internal(
        "Security key listing not yet implemented".to_string(),
    ))
}

/// Update user request
#[derive(Deserialize)]
pub struct UpdateUserRequest {
    pub display_name: Option<String>,
    pub email: Option<String>,
}
