//! Astral Key - Error types
//!
//! Centralized error handling for the application.

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use thiserror::Error;

/// Main application error type
#[derive(Error, Debug)]
pub enum AuthError {
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("Cache error: {0}")]
    Cache(String),

    #[error("Redis error: {0}")]
    Redis(String),

    #[error("JWT error: {0}")]
    Jwt(String),

    #[error("Web3 error: {0}")]
    Web3(String),

    #[error("FIDO2 error: {0}")]
    Fido2(String),

    #[error("Vaultwarden error: {0}")]
    Vaultwarden(String),

    #[error("Config error: {0}")]
    Config(String),

    #[error("Validation error: {0}")]
    Validation(String),

    #[error("Unauthorized: {0}")]
    Unauthorized(String),

    #[error("Forbidden: {0}")]
    Forbidden(String),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Rate limited")]
    RateLimited,

    #[error("Internal error: {0}")]
    Internal(String),

    #[error("Bad request: {0}")]
    BadRequest(String),

    #[error("Not implemented: {0}")]
    NotImplemented(String),
}

/// Result type alias
pub type Result<T> = std::result::Result<T, AuthError>;

impl IntoResponse for AuthError {
    fn into_response(self) -> Response {
        let (status, error_message) = match self {
            AuthError::Database(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Internal database error".to_string(),
            ),
            AuthError::Cache(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Internal cache error".to_string(),
            ),
            AuthError::Redis(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Internal cache error".to_string(),
            ),
            AuthError::Jwt(msg) => (StatusCode::UNAUTHORIZED, msg),
            AuthError::Web3(msg) => (StatusCode::BAD_REQUEST, msg),
            AuthError::Fido2(msg) => (StatusCode::BAD_REQUEST, msg),
            AuthError::Vaultwarden(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg),
            AuthError::Config(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg),
            AuthError::Validation(msg) => (StatusCode::BAD_REQUEST, msg),
            AuthError::Unauthorized(msg) => (StatusCode::UNAUTHORIZED, msg),
            AuthError::Forbidden(msg) => (StatusCode::FORBIDDEN, msg),
            AuthError::NotFound(msg) => (StatusCode::NOT_FOUND, msg),
            AuthError::RateLimited => (
                StatusCode::TOO_MANY_REQUESTS,
                "Rate limit exceeded".to_string(),
            ),
            AuthError::Internal(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg),
            AuthError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg),
            AuthError::NotImplemented(msg) => (StatusCode::NOT_IMPLEMENTED, msg),
        };

        let body = Json(json!({
            "error": {
                "code": status.as_u16(),
                "message": error_message,
            }
        }));

        (status, body).into_response()
    }
}

impl From<redis::RedisError> for AuthError {
    fn from(err: redis::RedisError) -> Self {
        AuthError::Redis(err.to_string())
    }
}

impl From<jsonwebtoken::errors::Error> for AuthError {
    fn from(err: jsonwebtoken::errors::Error) -> Self {
        AuthError::Jwt(err.to_string())
    }
}

impl From<reqwest::Error> for AuthError {
    fn from(err: reqwest::Error) -> Self {
        AuthError::Internal(format!("HTTP client error: {}", err))
    }
}
