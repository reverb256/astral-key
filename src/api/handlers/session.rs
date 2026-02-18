//! Astral Key - Session management handlers

use axum::{extract::State, Json};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::auth::jwt::{JwtService, AuthenticatedUser};
use crate::db::models::Session;
use crate::error::{AuthError, Result};
use crate::state::AppState;

/// Refresh session tokens
pub async fn refresh(
    State(state): State<AppState>,
    Json(request): Json<RefreshRequest>,
) -> Result<Json<TokenResponse>> {
    // Get JWT service
    let jwt = state.jwt.as_ref().ok_or_else(|| {
        AuthError::Internal("JWT service not initialized".to_string())
    })?;

    // Validate refresh token
    let claims = jwt.validate_refresh_token(&request.refresh_token)?;
    let user_id = Uuid::parse_str(&claims.sub).map_err(|_| {
        AuthError::Internal("Invalid user ID in token".to_string())
    })?;

    // Check if refresh token is blacklisted
    if state.cache.is_token_blacklisted(&request.refresh_token).await.unwrap_or(false) {
        return Err(AuthError::Unauthorized("Token has been revoked".to_string()));
    }

    // Get session from database
    let pool = state.db.inner();
    let old_token_hash = hash_token(&request.refresh_token);

    let session = Session::get_by_refresh_token_hash(pool, &old_token_hash)
        .await?
        .ok_or_else(|| AuthError::Unauthorized("Invalid refresh token".to_string()))?;

    // Check if session is valid
    if !session.is_valid() {
        return Err(AuthError::Unauthorized("Session expired or revoked".to_string()));
    }

    // Check user ID matches
    if session.user_id != user_id {
        return Err(AuthError::Unauthorized("User ID mismatch".to_string()));
    }

    // Generate new token pair
    let tokens = jwt.generate_token_pair(user_id)?;

    // Update session with new refresh token hash
    let new_token_hash = hash_token(&tokens.refresh_token);
    session.update_refresh_token(pool, &new_token_hash).await?;

    // Blacklist old refresh token
    // Calculate TTL from expiration
    let ttl = (session.expires_at - chrono::Utc::now()).num_seconds().max(0) as u64;
    state.cache.blacklist_token(&request.refresh_token, ttl).await?;

    Ok(Json(TokenResponse {
        access_token: tokens.access_token,
        refresh_token: tokens.refresh_token,
    }))
}

/// Logout current session
pub async fn logout(
    State(state): State<AppState>,
    Json(request): Json<LogoutRequest>,
) -> Result<Json<serde_json::Value>> {
    // Get JWT service
    let jwt = state.jwt.as_ref().ok_or_else(|| {
        AuthError::Internal("JWT service not initialized".to_string())
    })?;

    // Validate refresh token
    let claims = jwt.validate_refresh_token(&request.refresh_token)?;
    let user_id = Uuid::parse_str(&claims.sub).map_err(|_| {
        AuthError::Internal("Invalid user ID in token".to_string())
    })?;

    // Get session and revoke it
    let pool = state.db.inner();
    let token_hash = hash_token(&request.refresh_token);

    if let Some(session) = Session::get_by_refresh_token_hash(pool, &token_hash).await? {
        if session.user_id == user_id {
            session.revoke(pool).await?;
        }
    }

    // Blacklist the refresh token
    state.cache.blacklist_token(&request.refresh_token, 86400).await?;

    Ok(Json(serde_json::json!({
        "message": "Logged out successfully",
    })))
}

/// List user sessions
pub async fn list(
    State(state): State<AppState>,
    auth_user: AuthenticatedUser,
) -> Result<Json<Vec<SessionInfo>>> {
    // Get user ID from authenticated context
    let user_id = auth_user.user_id;

    // Get active sessions
    let pool = state.db.inner();
    let sessions = Session::get_active_by_user(pool, user_id).await?;

    let session_infos: Vec<SessionInfo> = sessions
        .into_iter()
        .map(|s| SessionInfo {
            id: s.id,
            created_at: s.created_at,
            expires_at: s.expires_at,
            user_agent: s.user_agent,
            ip_address: s.ip_address,
        })
        .collect();

    Ok(Json(session_infos))
}

/// Delete a session
pub async fn delete(
    State(state): State<AppState>,
    auth_user: AuthenticatedUser,
    axum::extract::Path(session_id): axum::extract::Path<Uuid>,
) -> Result<Json<serde_json::Value>> {
    // Get user ID from authenticated context
    let user_id = auth_user.user_id;

    // Get session
    let pool = state.db.inner();
    let session = Session::get_by_id(pool, session_id)
        .await?
        .ok_or_else(|| AuthError::NotFound("Session not found".to_string()))?;

    // Check ownership
    if session.user_id != user_id {
        return Err(AuthError::Forbidden("Not your session".to_string()));
    }

    // Delete session
    session.delete(pool).await?;

    Ok(Json(serde_json::json!({
        "message": "Session deleted successfully",
    })))
}

/// Helper: Hash token for storage
fn hash_token(token: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    format!("{:x}", hasher.finalize())
}

// Request/Response types

#[derive(Debug, Deserialize)]
pub struct RefreshRequest {
    pub refresh_token: String,
}

#[derive(Debug, Deserialize)]
pub struct LogoutRequest {
    pub refresh_token: String,
}

#[derive(Debug, Serialize)]
pub struct TokenResponse {
    pub access_token: String,
    pub refresh_token: String,
}

#[derive(Debug, Serialize)]
pub struct SessionInfo {
    pub id: Uuid,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub expires_at: chrono::DateTime<chrono::Utc>,
    pub user_agent: Option<String>,
    pub ip_address: Option<String>,
}
