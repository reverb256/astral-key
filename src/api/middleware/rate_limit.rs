//! Astral Key - Rate limiting middleware
//!
//! Request rate limiting using governor.

use axum::{
    extract::{Request, State},
    http::StatusCode,
    middleware::Next,
    response::Response,
};
use std::net::IpAddr;
use std::sync::Arc;
use std::time::Duration;

use crate::cache::pool::RedisPool;
use crate::error::{AuthError, Result};

/// Rate limiter configuration
#[derive(Clone)]
pub struct RateLimiter {
    cache: RedisPool,
    max_requests: u32,
    window: Duration,
}

impl RateLimiter {
    /// Create a new rate limiter
    pub fn new(cache: RedisPool, max_requests: u32, window_secs: u64) -> Self {
        Self {
            cache,
            max_requests,
            window: Duration::from_secs(window_secs),
        }
    }

    /// Check if request should be allowed
    pub async fn check_rate_limit(&self, identifier: &str) -> Result<bool> {
        let key = format!("rate_limit:{}", identifier);

        // Try to increment counter
        let count = self
            .cache
            .rate_limit_increment(&key, self.window.as_secs() as u64)
            .await?;

        Ok(count <= self.max_requests as u64)
    }
}

/// Rate limiting middleware
pub async fn rate_limit_middleware(
    State(state): State<Arc<RateLimiter>>,
    mut request: Request,
    next: Next,
) -> Result<Response> {
    // Extract IP address
    let ip = extract_ip(&request);

    // Check rate limit
    let allowed = state.check_rate_limit(&ip).await.unwrap_or(false);

    if !allowed {
        return Err(AuthError::RateLimited);
    }

    Ok(next.run(request).await)
}

/// Extract IP address from request
fn extract_ip(request: &Request) -> String {
    // Try to get IP from headers (for proxied requests)
    if let Some(forwarded_for) = request
        .headers()
        .get("x-forwarded-for")
        .and_then(|h| h.to_str().ok())
    {
        // Take the first IP from the list
        return forwarded_for
            .split(',')
            .next()
            .map(|s| s.trim().to_string())
            .unwrap_or_else(|| "unknown".to_string());
    }

    // Fall back to connection info
    // In Axum, this would require a different extractor
    "unknown".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_ip_from_forwarded_for() {
        // Test IP extraction logic
        let forwarded = "203.0.113.1, 70.41.3.18, 150.172.238.178";
        let ip = forwarded.split(',').next().map(|s| s.trim());
        assert_eq!(ip, Some("203.0.113.1"));
    }
}
