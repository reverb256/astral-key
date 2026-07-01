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
///
/// This extracts the user ID from request extensions after JWT middleware has run.
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
        // Try to get AuthenticatedUser from extensions
        parts
            .extensions
            .get::<AuthenticatedUser>()
            .cloned()
            .ok_or_else(|| AuthError::Unauthorized("Not authenticated".to_string()))
    }
}

/// JWT authentication middleware
///
/// Validates JWT token from Authorization header and adds user_id to request extensions.
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

    // Check Bearer scheme
    if !auth_header.starts_with("Bearer ") {
        return Err(AuthError::Unauthorized(
            "Invalid Authorization header format".to_string(),
        ));
    }

    // Extract token
    let token = &auth_header[7..]; // Skip "Bearer "

    // Validate token
    let claims = state
        .jwt
        .as_ref()
        .ok_or_else(|| AuthError::Internal("JWT service not initialized".to_string()))?
        .validate_access_token(token)?;

    // Check if token is blacklisted
    if state
        .cache
        .is_token_blacklisted(token)
        .await
        .unwrap_or(false)
    {
        return Err(AuthError::Unauthorized(
            "Token has been revoked".to_string(),
        ));
    }

    // Extract user ID
    let user_id = uuid::Uuid::parse_str(&claims.sub)
        .map_err(|_| AuthError::Internal("Invalid user ID in token".to_string()))?;

    // Add user ID to request extensions
    request
        .extensions_mut()
        .insert(AuthenticatedUser { user_id });

    Ok(next.run(request).await)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_auth_header_validation() {
        let valid_header = "Bearer eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.test";
        assert!(valid_header.starts_with("Bearer "));

        let invalid_header = "Basic dGVzdA==";
        assert!(!invalid_header.starts_with("Bearer "));
    }
}
