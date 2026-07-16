//! Astral Key - Session REST handlers
//!
//! Endpoints for refreshing tokens, listing sessions, and revoking sessions.

use axum::{
    extract::{Path, State},
    Json,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::auth::jwt::AuthenticatedUser;
use crate::db::models::session::{Session, SessionSummary};
use crate::error::{AuthError, Result};
use crate::state::AppState;

/// Request body for refresh token exchange.
#[derive(Deserialize)]
pub struct RefreshTokenRequest {
    pub refresh_token: String,
    #[serde(default)]
    pub device_info: String,
    #[serde(default)]
    pub ip_address: String,
}

/// Response body for a successful token refresh.
#[derive(Serialize)]
pub struct RefreshTokenResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub session_id: Uuid,
}

/// Response body for listing sessions.
#[derive(Serialize)]
pub struct ListSessionsResponse {
    pub sessions: Vec<SessionSummary>,
}

/// `POST /auth/token/refresh` — Exchange a refresh token for a new token pair.
///
/// Validates the refresh JWT, finds the matching session, issues a new
/// access+refresh pair, and rotates the stored refresh token hash.
/// This implements refresh token rotation — each refresh invalidates the
/// previous refresh token.
pub async fn refresh_token(
    State(state): State<AppState>,
    Json(request): Json<RefreshTokenRequest>,
) -> Result<Json<RefreshTokenResponse>> {
    let db = state.db.inner();
    let jwt = state
        .jwt
        .as_ref()
        .ok_or_else(|| AuthError::Internal("JWT service not initialized".to_string()))?;

    // 1. Validate the refresh token JWT
    let claims = jwt.validate_refresh_token(&request.refresh_token)?;
    let user_id = Uuid::parse_str(&claims.sub)
        .map_err(|_| AuthError::Internal("Invalid user ID in refresh token".to_string()))?;

    // 2. Hash the refresh token to find the matching session
    let token_hash = hash_refresh_token(&request.refresh_token);

    let session = Session::find_by_refresh_hash(db, &token_hash)
        .await?
        .ok_or_else(|| AuthError::Unauthorized("Session not found or revoked".to_string()))?;

    // 3. Issue new token pair
    let new_pair = jwt.generate_token_pair(user_id)?;

    // 4. Rotate the refresh token hash in the session store
    let refresh_ttl = chrono::Duration::seconds(state.config.jwt.refresh_token_ttl as i64);
    let new_expires = chrono::Utc::now() + refresh_ttl;
    session
        .rotate_refresh_token(db, &new_pair.refresh_token, new_expires)
        .await?;

    Ok(Json(RefreshTokenResponse {
        access_token: new_pair.access_token,
        refresh_token: new_pair.refresh_token,
        session_id: session.id,
    }))
}

/// `GET /auth/sessions` — List active sessions for the authenticated user.
pub async fn list_sessions(
    State(state): State<AppState>,
    auth_user: AuthenticatedUser,
) -> Result<Json<ListSessionsResponse>> {
    let db = state.db.inner();
    let sessions = Session::find_by_user(db, auth_user.user_id).await?;
    Ok(Json(ListSessionsResponse { sessions }))
}

/// `DELETE /auth/sessions/:id` — Revoke a session.
pub async fn revoke_session(
    State(state): State<AppState>,
    auth_user: AuthenticatedUser,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>> {
    let db = state.db.inner();

    let session = Session::find_by_id(db, id)
        .await?
        .ok_or_else(|| AuthError::NotFound("Session not found".to_string()))?;

    if session.user_id != auth_user.user_id {
        return Err(AuthError::Forbidden(
            "Cannot revoke another user's session".to_string(),
        ));
    }

    session.revoke(db).await?;
    Ok(Json(serde_json::json!({"revoked": true})))
}

/// SHA-256 hash a refresh token (deterministic — matches Session model).
fn hash_refresh_token(token: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    hex::encode(hasher.finalize())
}
