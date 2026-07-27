//! Astral Key - Error types
//!
//! Centralized error handling for the application.

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;

use thiserror::Error;

/// Machine-readable error code constants
#[derive(Debug, Clone, Copy, Serialize)]
pub struct ErrorCode(&'static str);

impl ErrorCode {
    #[allow(dead_code)]
    pub const HEALTH_OK: Self = Self("HEALTH_OK");
    pub const INVALID_SIGNATURE: Self = Self("AUTH_INVALID_SIGNATURE");
    #[allow(dead_code)]
    pub const EXPIRED_TOKEN: Self = Self("AUTH_EXPIRED_TOKEN");
    #[allow(dead_code)]
    pub const INSUFFICIENT_SCOPE: Self = Self("AUTH_INSUFFICIENT_SCOPE");
    pub const NOT_FOUND: Self = Self("AUTH_NOT_FOUND");
    pub const INTERNAL: Self = Self("AUTH_INTERNAL_ERROR");
    pub const BAD_REQUEST: Self = Self("AUTH_BAD_REQUEST");
    pub const UNAUTHORIZED: Self = Self("AUTH_UNAUTHORIZED");
    pub const FORBIDDEN: Self = Self("AUTH_FORBIDDEN");
    pub const CONFLICT: Self = Self("AUTH_CONFLICT");
    pub const NOT_IMPLEMENTED: Self = Self("AUTH_NOT_IMPLEMENTED");
}

impl std::fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub code: &'static str,
    pub detail: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub docs_url: Option<&'static str>,
}

/// Main application error type
#[derive(Error, Debug)]
pub enum AuthError {
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("JWT error: {0}")]
    Jwt(String),

    #[allow(dead_code)]
    #[error("Web3 error: {0}")]
    Web3(String),

    #[allow(dead_code)]
    #[error("FIDO2 error: {0}")]
    Fido2(String),

    #[error("Config error: {0}")]
    Config(String),

    #[allow(dead_code)]
    #[error("Validation error: {0}")]
    Validation(String),

    #[error("Unauthorized: {0}")]
    Unauthorized(String),

    #[error("Forbidden: {0}")]
    Forbidden(String),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Internal error: {0}")]
    Internal(String),

    #[error("Bad request: {0}")]
    BadRequest(String),

    #[allow(dead_code)]
    #[error("Not implemented: {0}")]
    NotImplemented(String),

    #[allow(dead_code)]
    #[error("Conflict: {0}")]
    Conflict(String),
}

/// Result type alias
pub type Result<T> = std::result::Result<T, AuthError>;

impl AuthError {
    /// Get the machine-readable error code
    pub fn code(&self) -> &'static str {
        match self {
            AuthError::Database(_) => ErrorCode::INTERNAL.0,
            AuthError::Jwt(_) => ErrorCode::INVALID_SIGNATURE.0,
            AuthError::Web3(_) => ErrorCode::BAD_REQUEST.0,
            AuthError::Fido2(_) => ErrorCode::BAD_REQUEST.0,
            AuthError::Config(_) => ErrorCode::INTERNAL.0,
            AuthError::Validation(_) => ErrorCode::BAD_REQUEST.0,
            AuthError::Unauthorized(_) => ErrorCode::UNAUTHORIZED.0,
            AuthError::Forbidden(_) => ErrorCode::FORBIDDEN.0,
            AuthError::NotFound(_) => ErrorCode::NOT_FOUND.0,
            AuthError::Internal(_) => ErrorCode::INTERNAL.0,
            AuthError::BadRequest(_) => ErrorCode::BAD_REQUEST.0,
            AuthError::NotImplemented(_) => ErrorCode::NOT_IMPLEMENTED.0,
            AuthError::Conflict(_) => ErrorCode::CONFLICT.0,
        }
    }

    /// Get the human-readable detail message
    pub fn detail(&self) -> String {
        match self {
            AuthError::Database(e) => format!("Database error: {}", e),
            AuthError::Jwt(e) => e.clone(),
            AuthError::Web3(e) => e.clone(),
            AuthError::Fido2(e) => e.clone(),
            AuthError::Config(e) => e.clone(),
            AuthError::Validation(e) => e.clone(),
            AuthError::Unauthorized(e) => e.clone(),
            AuthError::Forbidden(e) => e.clone(),
            AuthError::NotFound(e) => e.clone(),
            AuthError::Internal(e) => e.clone(),
            AuthError::BadRequest(e) => e.clone(),
            AuthError::NotImplemented(e) => e.clone(),
            AuthError::Conflict(e) => e.clone(),
        }
    }
}

impl IntoResponse for AuthError {
    fn into_response(self) -> Response {
        let status = match self {
            AuthError::Database(_) | AuthError::Config(_) | AuthError::Internal(_) => {
                StatusCode::INTERNAL_SERVER_ERROR
            }
            AuthError::Jwt(_) | AuthError::Unauthorized(_) => StatusCode::UNAUTHORIZED,
            AuthError::Forbidden(_) => StatusCode::FORBIDDEN,
            AuthError::NotFound(_) => StatusCode::NOT_FOUND,
            AuthError::NotImplemented(_) => StatusCode::NOT_IMPLEMENTED,
            AuthError::Conflict(_) => StatusCode::CONFLICT,
            _ => StatusCode::BAD_REQUEST,
        };
        let body = Json(ErrorResponse {
            code: self.code(),
            detail: self.detail(),
            docs_url: Some("https://github.com/reverb256/astral-key/docs/errors.md"),
        });
        (status, body).into_response()
    }
}

impl From<jsonwebtoken::errors::Error> for AuthError {
    fn from(err: jsonwebtoken::errors::Error) -> Self {
        AuthError::Jwt(err.to_string())
    }
}
