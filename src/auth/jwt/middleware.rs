//! Astral Key - JWT authentication middleware
//!
//! Axum middleware for validating JWT tokens on protected routes.

use axum::{
    async_trait,
    extract::{FromRequestParts, Request, State},
    http::request::Parts,
    middleware::Next,
    response::Response,
};

use crate::error::{AuthError, Result};
use crate::state::AppState;

/// User ID extractor - extracts authenticated user ID from JWT token
#[derive(Debug, Clone)]
pub struct AuthenticatedUser {
    pub user_id: uuid::Uuid,
}

/// Axum extractor for AuthenticatedUser
#[async_trait]
impl<S> FromRequestParts<S> for AuthenticatedUser
where
    S: Send + Sync,
{
    type Rejection = AuthError;

    async fn from_request_parts(
        parts: &mut Parts,
        _state: &S,
    ) -> std::result::Result<Self, Self::Rejection> {
        parts
            .extensions
            .get::<AuthenticatedUser>()
            .cloned()
            .ok_or_else(|| AuthError::Unauthorized("Not authenticated".to_string()))
    }
}

/// JWT authentication middleware
pub async fn jwt_auth_middleware(
    State(state): State<AppState>,
    mut request: Request,
    next: Next,
) -> Result<Response> {
    // Extract Authorization header
    let auth_header = request
        .headers()
        .get("Authorization")
        .and_then(|h| h.to_str().ok())
        .ok_or_else(|| AuthError::Unauthorized("Missing Authorization header".to_string()))?;

    if !auth_header.starts_with("Bearer ") {
        return Err(AuthError::Unauthorized(
            "Invalid Authorization header format".to_string(),
        ));
    }

    let token = &auth_header[7..];

    let claims = state
        .jwt
        .as_ref()
        .ok_or_else(|| AuthError::Internal("JWT service not initialized".to_string()))?
        .validate_access_token(token)?;

    let user_id = uuid::Uuid::parse_str(&claims.sub)
        .map_err(|_| AuthError::Internal("Invalid user ID in token".to_string()))?;

    request
        .extensions_mut()
        .insert(AuthenticatedUser { user_id });

    Ok(next.run(request).await)
}
