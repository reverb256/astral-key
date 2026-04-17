//! Astral Key - Application state
//!
//! Shared state across the application, including database connections,
/// cache clients, and configuration.

use std::sync::Arc;
use std::time::Duration;

use crate::auth::fido2::Fido2Service;
use crate::auth::jwt::JwtService;
use crate::cache::pool::RedisPool;
use crate::config::Config;
use crate::db::pool::DbPool;
use crate::error::{AuthError, Result};

/// Shared application state
#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,

    // Database and cache
    pub db: DbPool,
    pub cache: RedisPool,

    // Auth services
    pub jwt: Option<JwtService>,
    pub fido2: Option<Fido2Service>,

    // TODO: Add Web3 service
    // TODO: Add Vaultwarden client
}

impl AppState {
    /// Create a new application state
    pub async fn new(config: Config) -> Result<Self> {
        tracing::info!("Initializing application state");

        // Initialize database pool
        let db = DbPool::new(&config.database).await?;

        // Initialize Redis pool
        let cache = RedisPool::new(&config.redis).await?;

        // Initialize JWT service (with a placeholder secret for now)
        // In production, load from file
        let jwt_secret = std::env::var("JWT_SECRET").unwrap_or_else(|_| {
            "placeholder_jwt_secret_change_in_production_32_bytes!!".to_string()
        });

        let jwt = if !jwt_secret.is_empty() {
            Some(
                JwtService::new(
                    jwt_secret.as_bytes(),
                    Duration::from_secs(config.jwt.access_token_ttl),
                    Duration::from_secs(config.jwt.refresh_token_ttl),
                )
                .map_err(|e| AuthError::Internal(format!("Failed to initialize JWT service: {}", e)))?,
            )
        } else {
            tracing::warn!("JWT_SECRET not set, JWT authentication will be unavailable");
            None
        };

        // TODO: Initialize FIDO2 service
        // TODO: Initialize Web3 service
        // TODO: Initialize Vaultwarden client

        // Initialize FIDO2 service
        let fido2 = match Fido2Service::new(&config.fido2, cache.clone()) {
            Ok(service) => {
                tracing::info!("FIDO2 service initialized");
                Some(service)
            }
            Err(e) => {
                tracing::warn!("Failed to initialize FIDO2 service: {}", e);
                None
            }
        };

        tracing::info!("Application state initialized");

        Ok(Self {
            config: Arc::new(config),
            db,
            cache,
            jwt,
            fido2,
        })
    }

    /// Run migrations on the database
    pub async fn run_migrations(&self) -> Result<()> {
        self.db.run_migrations().await
    }

    /// Health check for all services
    pub async fn health_check(&self) -> Result<bool> {
        // Check database
        self.db.health_check().await?;

        // Check Redis
        self.cache.health_check().await?;

        // TODO: Check Vaultwarden

        Ok(true)
    }
}
