//! Astral Key - API Key REST handlers
//!
//! Endpoints for creating, listing, revoking, and deleting API keys.

use axum::{
    extract::{Path, State},
    Json,
};
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
