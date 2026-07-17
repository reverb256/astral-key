//! Structured audit logging middleware.
//!
//! Creates a JSON [`AuditEvent`] for every API request and emits it to
//! **stdout** (one line per event) after the response has been sent.

use std::time::SystemTime;

use axum::{extract::Request, http::HeaderMap, middleware::Next, response::Response};
use chrono::Utc;
use serde::Serialize;
use uuid::Uuid;

/// Structured audit event, emitted as JSON to stdout (one event per line).
#[derive(Debug, Serialize)]
pub struct AuditEvent {
    /// RFC 3339 timestamp of when the event was created.
    pub timestamp: String,
    /// Event type label (e.g. `"api_request"`).
    pub event: &'static str,
    /// Authenticated user ID, if available.
    pub user_id: Option<String>,
    /// Client IP address (from `X-Forwarded-For` or `X-Real-IP`).
    pub client_ip: String,
    /// Resource / action being accessed (e.g. `"GET /api/v1/auth/keys"`).
    pub resource: Option<String>,
    /// Outcome — `"allow"` or `"deny"`.
    pub result: &'static str,
    /// Human-readable reason for the outcome.
    pub reason: Option<String>,
    /// Unique request identifier (UUID v4).
    pub request_id: String,
}

impl AuditEvent {
    /// Serialise the event as JSON and write it to stdout.
    ///
    /// Silently ignores serialisation errors (should never happen for this
    /// struct).
    pub fn emit(&self) {
        if let Ok(json) = serde_json::to_string(self) {
            // Using `println!` to get an implicit newline after each event.
            println!("{}", json);
        }
    }
}

/// Extract the client IP from headers, falling back to `"unknown"`.
fn client_ip(headers: &HeaderMap) -> String {
    headers
        .get("X-Forwarded-For")
        .and_then(|v| v.to_str().ok())
        .or_else(|| headers.get("X-Real-IP").and_then(|v| v.to_str().ok()))
        .unwrap_or("unknown")
        .to_string()
}

/// Axum middleware for structured audit logging.
///
/// Records the HTTP method, URI, client IP, and status code of every
/// request, then emits a JSON [`AuditEvent`] after the response is sent.
///
/// The event is written to **stdout** so that external log shippers
/// (Filebeat, Vector, journald, etc.) can collect and forward it.
pub async fn audit_middleware(request: Request, next: Next) -> Response {
    let _start = SystemTime::now();
    let method = request.method().clone();
    let uri = request.uri().clone();
    let headers = request.headers().clone();
    let request_id = Uuid::new_v4().to_string();

    let ip = client_ip(&headers);

    // Build a resource string from method + path.
    let resource = Some(format!("{} {}", method, uri));

    // Run the inner handler.
    let response = next.run(request).await;

    let status = response.status();
    let (result, reason) = if status.is_success() || status.is_redirection() {
        ("allow", None)
    } else {
        (
            "deny",
            Some(status.canonical_reason().unwrap_or("unknown").to_string()),
        )
    };

    let event = AuditEvent {
        timestamp: Utc::now().to_rfc3339(),
        event: "api_request",
        user_id: None, // Set by a downstream middleware when authentication runs
        client_ip: ip,
        resource,
        result,
        reason,
        request_id,
    };
    event.emit();

    response
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audit_event_serialises() {
        let event = AuditEvent {
            timestamp: "2025-01-01T00:00:00Z".to_string(),
            event: "api_request",
            user_id: Some("user-abc".to_string()),
            client_ip: "10.0.0.1".to_string(),
            resource: Some("POST /api/v1/auth/keys".to_string()),
            result: "allow",
            reason: None,
            request_id: "550e8400-e29b-41d4-a716-446655440000".to_string(),
        };

        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("\"result\":\"allow\""));
        assert!(json.contains("\"event\":\"api_request\""));
        assert!(json.contains("\"client_ip\":\"10.0.0.1\""));
    }

    #[test]
    fn test_client_ip_from_forwarded_for() {
        let mut headers = HeaderMap::new();
        headers.insert("X-Forwarded-For", "192.168.1.1".parse().unwrap());
        assert_eq!(client_ip(&headers), "192.168.1.1");
    }

    #[test]
    fn test_client_ip_from_real_ip() {
        let mut headers = HeaderMap::new();
        headers.insert("X-Real-IP", "10.0.0.5".parse().unwrap());
        assert_eq!(client_ip(&headers), "10.0.0.5");
    }

    #[test]
    fn test_client_ip_unknown() {
        let headers = HeaderMap::new();
        assert_eq!(client_ip(&headers), "unknown");
    }

    #[test]
    fn test_client_ip_prefers_forwarded_for() {
        let mut headers = HeaderMap::new();
        headers.insert("X-Forwarded-For", "192.168.1.1".parse().unwrap());
        headers.insert("X-Real-IP", "10.0.0.5".parse().unwrap());
        assert_eq!(client_ip(&headers), "192.168.1.1");
    }
}
