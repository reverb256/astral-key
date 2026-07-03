//! Astral Key - Application state
//!
//! Shared state across the application.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;

use crate::auth::fido2::Fido2Service;
use crate::auth::jwt::JwtService;
use crate::config::Config;
use crate::db::pool::DbPool;
use crate::error::{AuthError, Result};

/// In-memory FIDO2 challenge state store (replaces Redis)
#[derive(Clone, Default)]
pub struct Fido2StateStore {
    store: Arc<Mutex<HashMap<String, (String, std::time::Instant)>>>,
}

impl Fido2StateStore {
    /// Store state with TTL
    pub async fn store(&self, key: &str, value: String, ttl_secs: u64) {
        let mut store = self.store.lock().await;
        store.insert(
            key.to_string(),
            (
                value,
                std::time::Instant::now() + Duration::from_secs(ttl_secs),
            ),
        );
    }

    /// Retrieve state
    pub async fn get(&self, key: &str) -> Option<String> {
        let mut store = self.store.lock().await;
        match store.get(key) {
            Some((value, expiry)) if *expiry > std::time::Instant::now() => Some(value.clone()),
            _ => {
                store.remove(key);
                None
            }
        }
    }

    /// Delete state
    pub async fn delete(&self, key: &str) {
        let mut store = self.store.lock().await;
        store.remove(key);
    }
}

/// Shared application state
#[derive(Clone)]
pub struct AppState {
    pub config: std::sync::Arc<Config>,

    // Database
    pub db: DbPool,

    // In-memory FIDO2 state store
    pub fido2_state: Fido2StateStore,

    // Auth services
    pub jwt: Option<JwtService>,
    pub fido2: Option<Fido2Service>,
}

impl AppState {
    /// Create a new application state
    pub async fn new(config: Config) -> Result<Self> {
        tracing::info!("Initializing application state");

        // Initialize database pool
        let db = DbPool::new(&config.database).await?;

        // Run migrations
        db.run_migrations().await?;

        // Initialize JWT service — STRICT: refuses to start without a real secret.
        let jwt_secret = std::env::var("JWT_SECRET")
            .map(|s| s.trim().to_string())
            .ok()
            .filter(|s| s.len() >= 32)
            .unwrap_or_else(|| {
                panic!(
                    "JWT_SECRET is required (>=32 bytes). Generate one with: openssl rand -hex 32"
                )
            });

        tracing::info!("JWT signing secret loaded from env:JWT_SECRET");

        let jwt = Some(
            JwtService::new(
                jwt_secret.as_bytes(),
                Duration::from_secs(config.jwt.access_token_ttl),
                Duration::from_secs(config.jwt.refresh_token_ttl),
            )
            .map_err(|e| AuthError::Internal(format!("Failed to initialize JWT service: {}", e)))?,
        );

        // Initialize FIDO2 service (no cache/Redis needed)
        let fido2_state = Fido2StateStore::default();
        let fido2 = match Fido2Service::new(&config.fido2) {
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
            config: std::sync::Arc::new(config),
            db,
            fido2_state,
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
        self.db.health_check().await
    }
}
