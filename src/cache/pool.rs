//! Astral Key - Redis cache pool
//!
//! Manages Redis connection pool for caching sessions, nonces, and rate limiting.

use redis::aio::ConnectionManager;
use redis::Client;

use crate::config::RedisConfig;
use crate::error::{AuthError, Result};

/// Redis connection pool manager
#[derive(Clone)]
pub struct RedisPool {
    client: Client,
    config: RedisConfig,
}

impl RedisPool {
    /// Create a new Redis pool
    pub async fn new(config: &RedisConfig) -> Result<Self> {
        tracing::debug!("Creating Redis connection pool");

        let client = Client::open(config.url.clone())
            .map_err(|e| AuthError::Cache(format!("Failed to create Redis client: {}", e)))?;

        // Test connection
        let mut conn = ConnectionManager::new(client.clone())
            .await
            .map_err(|e| AuthError::Cache(format!("Failed to connect to Redis: {}", e)))?;

        // Ping to verify connection
        redis::cmd("PING")
            .query_async::<ConnectionManager, String>(&mut conn)
            .await
            .map_err(|e| AuthError::Cache(format!("Redis ping failed: {}", e)))?;

        tracing::info!("Redis connection pool created (pool size: {})", config.pool_size);

        Ok(Self {
            client,
            config: config.clone(),
        })
    }

    /// Get the underlying Redis client
    pub fn client(&self) -> &Client {
        &self.client
    }

    /// Get a connection from the pool
    pub async fn get_connection(&self) -> Result<ConnectionManager> {
        ConnectionManager::new(self.client.clone())
            .await
            .map_err(|e| AuthError::Cache(format!("Failed to get Redis connection: {}", e)))
    }

    /// Health check for Redis
    pub async fn health_check(&self) -> Result<bool> {
        let mut conn = self.get_connection().await?;

        redis::cmd("PING")
            .query_async::<ConnectionManager, String>(&mut conn)
            .await
            .map_err(|e| AuthError::Cache(format!("Redis health check failed: {}", e)))?;

        Ok(true)
    }

    /// Set a key with expiration time (in seconds)
    pub async fn set_with_expiry(&self, key: &str, value: &str, expiry_seconds: u64) -> Result<()> {
        let mut conn = self.get_connection().await?;

        redis::cmd("SETEX")
            .arg(key)
            .arg(expiry_seconds)
            .arg(value)
            .query_async::<ConnectionManager, ()>(&mut conn)
            .await
            .map_err(|e| AuthError::Cache(format!("Failed to set key: {}", e)))?;

        Ok(())
    }

    /// Get a value by key
    pub async fn get(&self, key: &str) -> Result<Option<String>> {
        let mut conn = self.get_connection().await?;

        let value: Option<String> = redis::cmd("GET")
            .arg(key)
            .query_async(&mut conn)
            .await
            .map_err(|e| AuthError::Cache(format!("Failed to get key: {}", e)))?;

        Ok(value)
    }

    /// Delete a key
    pub async fn delete(&self, key: &str) -> Result<bool> {
        let mut conn = self.get_connection().await?;

        let deleted: i32 = redis::cmd("DEL")
            .arg(key)
            .query_async(&mut conn)
            .await
            .map_err(|e| AuthError::Cache(format!("Failed to delete key: {}", e)))?;

        Ok(deleted > 0)
    }

    /// Check if a key exists
    pub async fn exists(&self, key: &str) -> Result<bool> {
        let mut conn = self.get_connection().await?;

        let exists: i32 = redis::cmd("EXISTS")
            .arg(key)
            .query_async(&mut conn)
            .await
            .map_err(|e| AuthError::Cache(format!("Failed to check key existence: {}", e)))?;

        Ok(exists > 0)
    }

    /// Add a JWT to the blacklist (for logout)
    pub async fn blacklist_token(&self, token: &str, expiry_seconds: u64) -> Result<()> {
        let key = format!("blacklist:{}", token);
        self.set_with_expiry(&key, "1", expiry_seconds).await
    }

    /// Check if a JWT is blacklisted
    pub async fn is_token_blacklisted(&self, token: &str) -> Result<bool> {
        let key = format!("blacklist:{}", token);
        self.exists(&key).await
    }

    /// Increment rate limit counter
    pub async fn rate_limit_increment(&self, identifier: &str, window_seconds: u64) -> Result<u64> {
        let key = format!("rate_limit:{}", identifier);

        let mut conn = self.get_connection().await?;

        // Check if key exists
        let exists: Option<String> = redis::cmd("GET")
            .arg(&key)
            .query_async(&mut conn)
            .await
            .map_err(|e| AuthError::Cache(format!("Failed to check rate limit: {}", e)))?;

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
            .map_err(|e| AuthError::Cache(format!("Failed to set rate limit: {}", e)))?;

        Ok(count)
    }

    /// Get current rate limit count
    pub async fn rate_limit_count(&self, identifier: &str) -> Result<Option<u64>> {
        let key = format!("rate_limit:{}", identifier);
        let value = self.get(&key).await?;

        Ok(value.and_then(|v| v.parse().ok()))
    }
}

/// Tests
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore] // Requires actual Redis
    async fn test_redis_pool_creation() {
        let config = RedisConfig {
            url: "redis://localhost:6379".to_string(),
            pool_size: 10,
        };

        let pool = RedisPool::new(&config).await;
        assert!(pool.is_ok());
    }

    #[tokio::test]
    #[ignore]
    async fn test_set_get() {
        let config = RedisConfig {
            url: "redis://localhost:6379".to_string(),
            pool_size: 10,
        };

        let pool = RedisPool::new(&config).await.unwrap();

        // Set a value
        pool.set_with_expiry("test_key", "test_value", 60).await.unwrap();

        // Get the value
        let value = pool.get("test_key").await.unwrap();
        assert_eq!(value, Some("test_value".to_string()));

        // Delete the key
        pool.delete("test_key").await.unwrap();
    }
}
