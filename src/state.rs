//! Astral Key - Application state
//!
//! Shared state across the application, including database connections,
/// cache clients, and configuration.
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
    pub config: std::sync::Arc<Config>,

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

        // Initialize JWT service — STRICT: refuses to start without a real secret.
        //
        // Resolution order:
        //   1. `JWT_SECRET` env var (>=32 bytes after trim)
        //   2. `JWT_SECRET_FILE` env var → read path; trim must be >=32 bytes
        //
        // Both missing, too short, or unreadable → process panics with an
        // actionable error. This replaces the previous behaviour of silently
        // using a known placeholder secret if env vars were unset.
        let jwt_secret = load_jwt_secret(&config).unwrap_or_else(|err| {
            panic!(
                "{}\n\
                 \nResolution order:\n\
                 \n\
                 1. Set JWT_SECRET to a value of at least 32 bytes:\n\
                    export JWT_SECRET=$(openssl rand -hex 32)\n\
                 \n\
                 2. Or set JWT_SECRET_FILE to a file containing the secret:\n\
                    export JWT_SECRET_FILE=/var/lib/astral-key/jwt_secret\n\
                    echo \"$(openssl rand -hex 32)\" > \"$JWT_SECRET_FILE\" && chmod 600 \"$JWT_SECRET_FILE\"\n\
                 \nGenerate one with: openssl rand -hex 32",
                err
            );
        });

        let jwt = Some(
            JwtService::new(
                jwt_secret.as_bytes(),
                Duration::from_secs(config.jwt.access_token_ttl),
                Duration::from_secs(config.jwt.refresh_token_ttl),
            )
            .map_err(|e| AuthError::Internal(format!("Failed to initialize JWT service: {}", e)))?,
        );

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
            config: std::sync::Arc::new(config),
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

/// Resolve a JWT signing secret from environment variables.
///
/// Resolution order:
///   1. `JWT_SECRET` env var — must be >= 32 bytes after trim
///   2. `JWT_SECRET_FILE` env var — path to file whose contents are >= 32 bytes
///
/// Returns `Err` with a human-readable explanation when neither path yields a
/// valid secret. Callers should map the error to a process-level panic with
/// actionable instructions rather than continuing with an insecure default.
fn load_jwt_secret(config: &Config) -> std::result::Result<String, String> {
    const MIN_BYTES: usize = 32;

    if let Ok(s) = std::env::var("JWT_SECRET") {
        let trimmed = s.trim();
        if trimmed.len() >= MIN_BYTES {
            return Ok(trimmed.to_string());
        }
        return Err(format!(
            "JWT_SECRET env var must be at least {} bytes after trim (got {} bytes)",
            MIN_BYTES,
            trimmed.len()
        ));
    }

    if let Ok(path) = std::env::var("JWT_SECRET_FILE") {
        return match std::fs::read_to_string(&path) {
            Ok(contents) => {
                let trimmed = contents.trim();
                if trimmed.len() >= MIN_BYTES {
                    Ok(trimmed.to_string())
                } else {
                    Err(format!(
                        "JWT secret in {} must be at least {} bytes after trim (got {} bytes)",
                        path,
                        MIN_BYTES,
                        trimmed.len()
                    ))
                }
            }
            Err(e) => Err(format!("Failed to read JWT_SECRET_FILE={}: {}", path, e)),
        };
    }

    Err(format!(
        "JWT_SECRET is required. Set JWT_SECRET env var or JWT_SECRET_FILE=<path>. \
         (config.jwt.secret_file defaults to {}).",
        config.jwt.secret_file
    ))
}
