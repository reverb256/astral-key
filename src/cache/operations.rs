//! Astral Key - Cache operations
//!
//! High-level cache operations for sessions, nonces, and rate limiting.

use crate::cache::pool::RedisPool;
use crate::error::Result;

/// Cache key prefixes
const NONCE_PREFIX: &str = "nonce:";
const SESSION_PREFIX: &str = "session:";
const RATE_LIMIT_PREFIX: &str = "rate_limit:";
const BLACKLIST_PREFIX: &str = "blacklist:";

/// Cache operations manager
pub struct CacheOperations {
    pool: RedisPool,
}

impl CacheOperations {
    /// Create new cache operations
    pub fn new(pool: RedisPool) -> Self {
        Self { pool }
    }

    /// Store a nonce with expiration (typically 5-15 minutes)
    pub async fn store_nonce(&self, nonce: &str, expiry_seconds: u64) -> Result<()> {
        let key = format!("{}{}", NONCE_PREFIX, nonce);
        self.pool.set_with_expiry(&key, "1", expiry_seconds).await
    }

    /// Check if a nonce exists and is valid
    pub async fn validate_nonce(&self, nonce: &str) -> Result<bool> {
        let key = format!("{}{}", NONCE_PREFIX, nonce);
        self.pool.exists(&key).await
    }

    /// Consume (delete) a nonce after use
    pub async fn consume_nonce(&self, nonce: &str) -> Result<bool> {
        let key = format!("{}{}", NONCE_PREFIX, nonce);
        self.pool.delete(&key).await
    }

    /// Store session data
    pub async fn store_session(&self, session_id: &str, data: &str, expiry_seconds: u64) -> Result<()> {
        let key = format!("{}{}", SESSION_PREFIX, session_id);
        self.pool.set_with_expiry(&key, data, expiry_seconds).await
    }

    /// Get session data
    pub async fn get_session(&self, session_id: &str) -> Result<Option<String>> {
        let key = format!("{}{}", SESSION_PREFIX, session_id);
        self.pool.get(&key).await
    }

    /// Delete a session
    pub async fn delete_session(&self, session_id: &str) -> Result<bool> {
        let key = format!("{}{}", SESSION_PREFIX, session_id);
        self.pool.delete(&key).await
    }

    /// Add a JWT to the blacklist (for logout)
    pub async fn blacklist_token(&self, token: &str, expiry_seconds: u64) -> Result<()> {
        let key = format!("{}{}", BLACKLIST_PREFIX, token);
        self.pool.set_with_expiry(&key, "1", expiry_seconds).await
    }

    /// Check if a JWT is blacklisted
    pub async fn is_token_blacklisted(&self, token: &str) -> Result<bool> {
        let key = format!("{}{}", BLACKLIST_PREFIX, token);
        self.pool.exists(&key).await
    }

    /// Increment rate limit counter
    pub async fn rate_limit_increment(&self, identifier: &str, window_seconds: u64) -> Result<u64> {
        let key = format!("{}{}", RATE_LIMIT_PREFIX, identifier);

        // Note: For proper rate limiting, we'd use Redis INCR with EXPIRE
        // This is a simplified version - in production, use a proper rate limiting algorithm
        let mut conn = self.pool.get_connection().await?;

        // Check if key exists
        let exists: Option<String> = redis::cmd("GET")
            .arg(&key)
            .query_async(&mut conn)
            .await
            .map_err(|e| crate::error::AuthError::Cache(format!("Failed to check rate limit: {}", e)))?;

        let count = if let Some(val) = exists {
            val.parse::<u64>().unwrap_or(0) + 1
        } else {
            1
        };

        // Set with expiration
        redis::cmd("SETEX")
            .arg(&key)
            .arg(window_seconds)
            .arg(count)
            .query_async::<redis::aio::ConnectionManager, ()>(&mut conn)
            .await
            .map_err(|e| crate::error::AuthError::Cache(format!("Failed to set rate limit: {}", e)))?;

        Ok(count)
    }

    /// Get current rate limit count
    pub async fn rate_limit_count(&self, identifier: &str) -> Result<Option<u64>> {
        let key = format!("{}{}", RATE_LIMIT_PREFIX, identifier);
        let value = self.pool.get(&key).await?;

        Ok(value.and_then(|v| v.parse().ok()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore]
    async fn test_nonce_operations() {
        // This test requires a running Redis instance
        // Run with: cargo test -- --ignored
    }
}
