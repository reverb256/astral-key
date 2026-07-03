//! Astral Key - Database connection pool
//!
//! Manages SQLite connection pool using SQLx.

use sqlx::sqlite::{SqlitePool, SqlitePoolOptions};

use crate::config::DatabaseConfig;
use crate::error::{AuthError, Result};

/// Database connection pool
#[derive(Clone)]
pub struct DbPool {
    pool: SqlitePool,
}

impl DbPool {
    /// Create a new SQLite connection pool
    pub async fn new(config: &DatabaseConfig) -> Result<Self> {
        tracing::debug!("Creating SQLite connection pool");

        let pool = SqlitePoolOptions::new()
            .max_connections(config.max_connections)
            .acquire_timeout(std::time::Duration::from_secs(30))
            .connect(&config.url)
            .await?;

        tracing::info!(
            "SQLite connection pool created (max: {})",
            config.max_connections
        );

        Ok(Self { pool })
    }

    /// Get the underlying SQLx pool
    pub fn inner(&self) -> &SqlitePool {
        &self.pool
    }

    /// Health check for the database connection
    pub async fn health_check(&self) -> Result<bool> {
        sqlx::query("SELECT 1").fetch_one(&self.pool).await?;
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
