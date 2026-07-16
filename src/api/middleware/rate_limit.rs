//! Per-key token-bucket rate limiting middleware.
//!
//! Uses API key prefix (from `Authorization: Bearer <key>` header) + client IP
//! as the rate-limit key. Falls back to IP-only when no API key is present.
//!
//! The limiter is a global singleton initialized via [`init()`], which must be
//! called once before the first request (e.g. in route setup).

use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::OnceLock;
use std::time::Instant;

use axum::{
    extract::Request,
    http::{HeaderMap, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};

// ---------------------------------------------------------------------------
// Token bucket
// ---------------------------------------------------------------------------

#[derive(Debug)]
struct TokenBucket {
    tokens: f64,
    last_refill: Instant,
}

/// Per-key token bucket rate limiter.
///
/// Thread-safe via `Mutex` (homelab single-replica — sufficient).
#[derive(Debug)]
pub struct RateLimiter {
    buckets: Mutex<HashMap<String, TokenBucket>>,
    max_rps: u32,
    max_burst: u32,
}

/// Rate-limit exceeded — returned with a `Retry-After` header.
#[derive(Debug)]
pub struct RateLimitExceeded {
    pub retry_after_secs: u64,
}

impl IntoResponse for RateLimitExceeded {
    fn into_response(self) -> Response {
        let retry_after = self.retry_after_secs.to_string();
        (
            StatusCode::TOO_MANY_REQUESTS,
            [("Retry-After", retry_after.as_str())],
        )
            .into_response()
    }
}

impl RateLimiter {
    /// Create a new rate limiter.
    ///
    /// - `max_rps` — maximum requests per second per key.
    /// - `max_burst` — maximum burst size (initial token count).
    pub fn new(max_rps: u32, max_burst: u32) -> Self {
        Self {
            buckets: Mutex::new(HashMap::new()),
            max_rps,
            max_burst,
        }
    }

    /// Check whether a request for the given key is allowed.
    ///
    /// Returns `Ok(())` if the request can proceed, or
    /// [`RateLimitExceeded`] with the recommended retry delay.
    pub fn check(&self, key: &str) -> Result<(), RateLimitExceeded> {
        let mut buckets = self.buckets.lock().unwrap();
        let now = Instant::now();
        let bucket = buckets
            .entry(key.to_string())
            .or_insert_with(|| TokenBucket {
                tokens: self.max_burst as f64,
                last_refill: now,
            });

        // Refill tokens proportionally to elapsed time.
        let elapsed = now.duration_since(bucket.last_refill).as_secs_f64();
        bucket.tokens = (bucket.tokens + elapsed * self.max_rps as f64).min(self.max_burst as f64);
        bucket.last_refill = now;

        if bucket.tokens >= 1.0 {
            bucket.tokens -= 1.0;
            Ok(())
        } else {
            Err(RateLimitExceeded {
                retry_after_secs: (1.0 / self.max_rps as f64).ceil() as u64,
            })
        }
    }
}

// ---------------------------------------------------------------------------
// Global singleton
// ---------------------------------------------------------------------------

static RATE_LIMITER: OnceLock<RateLimiter> = OnceLock::new();

/// Initialize the global rate limiter.
///
/// Must be called **once** before any middleware runs (e.g. at the top of
/// the route-setup function).  Panics if called a second time.
pub fn init(max_rps: u32, max_burst: u32) {
    RATE_LIMITER
        .set(RateLimiter::new(max_rps, max_burst))
        .expect("RateLimiter has already been initialized");
}

fn rate_limiter() -> &'static RateLimiter {
    RATE_LIMITER
        .get()
        .expect("RateLimiter not initialised — call rate_limit::init() first")
}

// ---------------------------------------------------------------------------
// Key derivation
// ---------------------------------------------------------------------------

/// Build a rate-limit key from the request headers.
///
/// Format: `{api_key_prefix}:{client_ip}` when a Bearer token is present,
/// or just `{client_ip}` when there is no API key.
fn build_key(headers: &HeaderMap) -> String {
    let ip = headers
        .get("X-Forwarded-For")
        .and_then(|v| v.to_str().ok())
        .or_else(|| headers.get("X-Real-IP").and_then(|v| v.to_str().ok()))
        .unwrap_or("unknown")
        .to_string();

    let api_key_prefix = headers
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .map(|key| {
            if key.len() >= 8 {
                key[..8].to_string()
            } else {
                key.to_string()
            }
        });

    match api_key_prefix {
        Some(prefix) => format!("{}:{}", prefix, ip),
        None => ip,
    }
}

// ---------------------------------------------------------------------------
// Axum middleware
// ---------------------------------------------------------------------------

/// Axum middleware for per-key rate limiting.
///
/// Reads the `Authorization: Bearer <key>` header and uses the key prefix
/// plus client IP (`X-Forwarded-For` / `X-Real-IP`) as the rate-limit key.
///
/// Returns **429 Too Many Requests** with a `Retry-After` header when the
/// rate limit is exceeded.
pub async fn rate_limit_middleware(request: Request, next: Next) -> Response {
    let key = build_key(request.headers());
    match rate_limiter().check(&key) {
        Ok(()) => next.run(request).await,
        Err(exceeded) => exceeded.into_response(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_rate_limiter_allows_burst() {
        let limiter = RateLimiter::new(10, 5);
        // First 5 requests should be allowed (burst capacity)
        for _ in 0..5 {
            assert!(limiter.check("test-key").is_ok());
        }
        // 6th should be denied (burst exhausted, no refill time elapsed)
        assert!(limiter.check("test-key").is_err());
    }

    #[test]
    fn test_rate_limiter_refills() {
        let limiter = RateLimiter::new(100, 10);
        // Exhaust burst
        for _ in 0..10 {
            assert!(limiter.check("test-key").is_ok());
        }
        assert!(limiter.check("test-key").is_err());

        // Wait for refill (~10ms at 100 RPS → 1 token)
        std::thread::sleep(Duration::from_millis(15));
        assert!(limiter.check("test-key").is_ok()); // should have 1 token back
    }

    #[test]
    fn test_build_key_with_bearer_token() {
        let mut headers = HeaderMap::new();
        headers.insert(
            "Authorization",
            "Bearer ak_abcdef1234567890".parse().unwrap(),
        );
        headers.insert("X-Forwarded-For", "10.0.0.1".parse().unwrap());

        let key = build_key(&headers);
        assert_eq!(key, "ak_abcd:10.0.0.1");
    }

    #[test]
    fn test_build_key_without_token() {
        let mut headers = HeaderMap::new();
        headers.insert("X-Forwarded-For", "10.0.0.1".parse().unwrap());

        let key = build_key(&headers);
        assert_eq!(key, "10.0.0.1");
    }

    #[test]
    fn test_build_key_fallback_to_real_ip() {
        let mut headers = HeaderMap::new();
        headers.insert("X-Real-IP", "10.0.0.2".parse().unwrap());

        let key = build_key(&headers);
        assert_eq!(key, "10.0.0.2");
    }

    #[test]
    fn test_build_key_unknown_ip() {
        let headers = HeaderMap::new();
        let key = build_key(&headers);
        assert_eq!(key, "unknown");
    }

    #[test]
    fn test_retry_after_header_value() {
        // With max_rps=10, retry_after should be ceil(1/10) = 1
        let err = RateLimitExceeded {
            retry_after_secs: 1,
        };
        let response = err.into_response();
        assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(
            response
                .headers()
                .get("Retry-After")
                .and_then(|v| v.to_str().ok()),
            Some("1")
        );
    }
}
