//! Astral Key - API Key REST handlers
//!
//! Endpoints for creating, listing, revoking, and deleting API keys.

use axum::{
    extract::{Path, State},
    Json,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::auth::jwt::AuthenticatedUser;
use crate::auth::keys::KeyService;
use crate::db::models::api_key::ApiKeySummary;
use crate::error::Result;
use crate::state::AppState;

/// Request body for creating a new API key.
#[derive(Deserialize)]
pub struct CreateKeyRequest {
    pub name: String,
    #[serde(default)]
    pub scopes: Vec<String>,
    #[serde(default = "default_environment")]
    pub environment: String,
    pub expires_in_seconds: Option<i64>,
}

fn default_environment() -> String {
    "prod".to_string()
}

/// Response body for successful key creation.
///
/// Contains the plaintext `api_key` shown exactly once.
#[derive(Serialize)]
pub struct CreateKeyResponse {
    pub id: Uuid,
    pub api_key: String,
    pub key_prefix: String,
    pub name: String,
    pub scopes: String,
    pub environment: String,
}

/// Response body for listing API keys.
#[derive(Serialize)]
pub struct ListKeysResponse {
    pub keys: Vec<ApiKeySummary>,
}

/// Request body for verifying an API key.
#[derive(Deserialize)]
pub struct VerifyKeyRequest {
    pub api_key: String,
}

/// Response body for API key verification.
///
/// Always returns HTTP 200. On failure `valid` is `false` and `error` is set
/// (mirrors the `/auth/verify` JWT endpoint for consistent downstream handling).
#[derive(Serialize)]
pub struct VerifyKeyResponse {
    pub valid: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<Uuid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scopes: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub environment: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_id: Option<Uuid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// `POST /auth/keys` — Create a new API key.
///
/// Requires JWT authentication.
/// Returns the plaintext key exactly once (not stored).
pub async fn create_key(
    State(state): State<AppState>,
    auth_user: AuthenticatedUser,
    Json(request): Json<CreateKeyRequest>,
) -> Result<Json<CreateKeyResponse>> {
    let db = state.db.inner();

    let expires_in = request.expires_in_seconds.map(chrono::Duration::seconds);

    let scopes: Vec<&str> = request.scopes.iter().map(|s| s.as_str()).collect();

    let (summary, raw_key) = KeyService::create_key(
        db,
        auth_user.user_id,
        &request.name,
        &scopes,
        &request.environment,
        expires_in,
    )
    .await?;

    Ok(Json(CreateKeyResponse {
        id: summary.id,
        api_key: raw_key,
        key_prefix: summary.key_prefix,
        name: summary.name,
        scopes: summary.scopes,
        environment: summary.environment,
    }))
}

/// `GET /auth/keys` — List all API keys for the authenticated user.
///
/// Returns summaries only (no plaintext keys, no hashes).
pub async fn list_keys(
    State(state): State<AppState>,
    auth_user: AuthenticatedUser,
) -> Result<Json<ListKeysResponse>> {
    let db = state.db.inner();
    let keys = KeyService::list_keys(db, auth_user.user_id).await?;
    Ok(Json(ListKeysResponse { keys }))
}

/// `DELETE /auth/keys/:id` — Hard-delete an API key.
pub async fn delete_key(
    State(state): State<AppState>,
    auth_user: AuthenticatedUser,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>> {
    let db = state.db.inner();
    KeyService::delete_key(db, id, auth_user.user_id).await?;
    Ok(Json(serde_json::json!({"deleted": true})))
}

/// `POST /auth/keys/:id/revoke` — Revoke an API key (soft delete).
///
/// Sets `revoked_at` so the key can no longer be used for authentication.
pub async fn revoke_key(
    State(state): State<AppState>,
    auth_user: AuthenticatedUser,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>> {
    let db = state.db.inner();
    KeyService::revoke_key(db, id, auth_user.user_id).await?;
    Ok(Json(serde_json::json!({"revoked": true})))
}

/// `POST /auth/keys/verify` — Verify an API key (public).
///
/// Used by external services (e.g. the AI Inference Gateway) to validate
/// `ak_*` API keys without needing a JWT of their own. Returns key metadata
/// (name, scopes, environment, owner) on success, or `{ "valid": false, ... }`.
pub async fn verify_key(
    State(state): State<AppState>,
    Json(request): Json<VerifyKeyRequest>,
) -> Result<Json<VerifyKeyResponse>> {
    let db = state.db.inner();

    match KeyService::validate_key(db, &request.api_key).await {
        Ok(key) => {
            // Best-effort: refresh the last-used timestamp on successful verify.
            let _ = key.update_last_used(db).await;

            let scopes: Vec<String> = key
                .scopes
                .split(',')
                .filter(|s| !s.is_empty())
                .map(|s| s.to_string())
                .collect();

            Ok(Json(VerifyKeyResponse {
                valid: true,
                id: Some(key.id),
                name: Some(key.name),
                scopes: Some(scopes),
                environment: Some(key.environment),
                user_id: Some(key.user_id),
                expires_at: key.expires_at,
                error: None,
            }))
        }
        Err(e) => Ok(Json(VerifyKeyResponse {
            valid: false,
            id: None,
            name: None,
            scopes: None,
            environment: None,
            user_id: None,
            expires_at: None,
            error: Some(e.detail()),
        })),
    }
}
