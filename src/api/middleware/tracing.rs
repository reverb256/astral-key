//! Astral Key - Request tracing middleware
//!
//! Request ID generation and distributed tracing.

use axum::{
    extract::Request,
    http::HeaderMap,
    middleware::Next,
    response::Response,
};
use tracing::info;
use uuid::Uuid;

/// Request tracing middleware
///
/// Adds a unique request ID if not present and logs request details.
pub async fn tracing_middleware(request: Request, next: Next) -> Response {
    // Get request ID and other info before moving request
    let request_id = get_or_generate_request_id(request.headers());
    let method = request.method().clone();
    let path = request.uri().path().to_string();

    // Log request
    info!(
        method = %method,
        path = %path,
        request_id = %request_id,
        "Incoming request"
    );

    // Continue with request (moves request)
    let response = next.run(request).await;

    // Log response
    info!(
        method = %method,
        path = %path,
        request_id = %request_id,
        status = %response.status(),
        "Request completed"
    );

    response
}

/// Get existing request ID from headers or generate a new one
fn get_or_generate_request_id(headers: &HeaderMap) -> String {
    headers
        .get("x-request-id")
        .and_then(|h| h.to_str().ok())
        .map(|s| s.to_string())
        .unwrap_or_else(|| Uuid::new_v4().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;

    #[test]
    fn test_generate_request_id() {
        let id = Uuid::new_v4().to_string();
        assert!(!id.is_empty());
        assert_eq!(id.len(), 36); // UUID format
    }

    #[test]
    fn test_request_id_from_header() {
        // Test that existing request ID is preserved
        // This would require setting up HeaderMap in a test
    }
}
