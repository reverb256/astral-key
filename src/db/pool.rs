//! Astral Key - Database connection pool
//!
//! Manages PostgreSQL connection pool using SQLx.

use sqlx::{PgPool, Pool, Postgres};
use sqlx::postgres::PgPoolOptions;

use crate::config::DatabaseConfig;
use crate::error::{AuthError, Result};

/// Database connection pool
#[derive(Clone)]
pub struct DbPool {
    pool: PgPool,
}

impl DbPool {
    /// Create a new database connection pool
    pub async fn new(config: &DatabaseConfig) -> Result<Self> {
        tracing::debug!("Creating database connection pool");

        let pool = PgPoolOptions::new()
            .max_connections(config.max_connections)
            .min_connections(config.min_connections)
            .acquire_timeout(std::time::Duration::from_secs(30))
            .idle_timeout(std::time::Duration::from_secs(600))
            .max_lifetime(std::time::Duration::from_secs(1800))
            .connect(&config.url)
            .await?;

        tracing::info!(
            "Database connection pool created (min: {}, max: {})",
            config.min_connections,
            config.max_connections
        );

        Ok(Self { pool })
    }

    /// Get the underlying SQLx pool
    pub fn inner(&self) -> &Pool<Postgres> {
        &self.pool
    }

    /// Health check for the database connection
    pub async fn health_check(&self) -> Result<bool> {
        sqlx::query("SELECT 1")
            .fetch_one(&self.pool)
            .await?;

        Ok(true)
    }

    /// Run database migrations
    pub async fn run_migrations(&self) -> Result<()> {
        tracing::info!("Running database migrations");

        sqlx::migrate!("./migrations")
            .run(&self.pool)
            .await
            .map_err(|e| AuthError::Internal(format!("Migration failed: {}", e)))?;

        tracing::info!("Database migrations completed successfully");
        Ok(())
    }
}

/// Tests
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore] // Requires actual database
    async fn test_pool_creation() {
        let config = DatabaseConfig {
            url: "postgresql://localhost/astral_key".to_string(),
            max_connections: 5,
            min_connections: 1,
        };

        let pool = DbPool::new(&config).await;
        assert!(pool.is_ok());
    }

    #[tokio::test]
    #[ignore]
    async fn test_health_check() {
        let config = DatabaseConfig {
            url: "postgresql://localhost/astral_key".to_string(),
            max_connections: 5,
            min_connections: 1,
        };

        let pool = DbPool::new(&config).await.unwrap();
        let health = pool.health_check().await;
        assert!(health.is_ok());
        assert!(health.unwrap());
    }
}
