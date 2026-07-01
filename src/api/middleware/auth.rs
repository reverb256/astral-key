//! Astral Key - Authentication middleware
//!
//! JWT validation for protected routes.

use axum::{
    extract::{Request, State},
    middleware::Next,
    response::Response,
};

use crate::error::{AuthError, Result};
use crate::state::AppState;

/// User ID extracted from JWT
#[derive(Debug, Clone)]
pub struct AuthenticatedUser {
    pub user_id: uuid::Uuid,
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

    // Check Bearer scheme
    if !auth_header.starts_with("Bearer ") {
        return Err(AuthError::Unauthorized(
            "Invalid Authorization header format".to_string(),
        ));
    }

    // Extract token
    let token = &auth_header[7..]; // Skip "Bearer "

    // Get JWT service
    let jwt = state
        .jwt
        .as_ref()
        .ok_or_else(|| AuthError::Internal("JWT service not initialized".to_string()))?;

    // Validate token
    let claims = jwt.validate_access_token(token)?;

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
    fn test_auth_header_parsing() {
        let valid_header = "Bearer eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.test";
        assert!(valid_header.starts_with("Bearer "));

        let invalid_header = "Basic dGVzdA==";
        assert!(!invalid_header.starts_with("Bearer "));
    }
}
