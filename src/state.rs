//! Astral Key - Application state
//!
//! Shared state across the application.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;

use crate::auth::fido2::Fido2Service;
use crate::auth::jit::{JitIssuer, JitVerifier};
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

    // ZK JIT capability token issuer (wrapped in Arc for Clone)
    pub jit_issuer: Option<Arc<JitIssuer>>,

    // ZK JIT capability token verifier (wrapped in Arc for Clone)
    pub jit_verifier: Option<Arc<JitVerifier>>,

    // In-memory OAuth state store (for PKCE/state parameter verification)
    pub oauth_state: Fido2StateStore,
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

        // Initialize in-memory stores
        let fido2_state = Fido2StateStore::default();
        let oauth_state = Fido2StateStore::default();
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

        // Initialize JIT issuer and verifier (optional — requires JIT_ISSUER_KEY)
        let (jit_issuer, jit_verifier) = match config.jit.issuer_key_hex.as_ref() {
            Some(key_hex) => {
                match JitIssuer::new(key_hex, &config.jit.issuer_id) {
                    Ok(issuer) => {
                        tracing::info!(
                            "JIT issuer initialized (issuer_id={}, default_ttl={}s)",
                            config.jit.issuer_id,
                            config.jit.default_ttl,
                        );

                        // Derive the public key from the signing key and register it
                        // with the verifier so POST /auth/jit/verify works.
                        let key_bytes = hex::decode(key_hex)
                            .expect("JIT_ISSUER_KEY already validated during issuer init");
                        let mut arr = [0u8; 32];
                        arr.copy_from_slice(&key_bytes);
                        let signing_key = ed25519_dalek::SigningKey::from_bytes(&arr);
                        let vk_bytes = signing_key.verifying_key().to_bytes();

                        let verifier = JitVerifier::new();
                        verifier.add_issuer_key(&config.jit.issuer_id, &vk_bytes);

                        (Some(Arc::new(issuer)), Some(Arc::new(verifier)))
                    }
                    Err(e) => {
                        tracing::error!("Failed to initialize JIT issuer: {}", e);
                        (None, None)
                    }
                }
            }
            None => {
                tracing::info!("JIT issuer not configured — set JIT_ISSUER_KEY to enable");
                (None, None)
            }
        };

        tracing::info!("Application state initialized");

        Ok(Self {
            config: std::sync::Arc::new(config),
            db,
            fido2_state,
            oauth_state,
            jwt,
            fido2,
            jit_issuer,
            jit_verifier,
        })
    }

    /// Run migrations on the database
    #[allow(dead_code)]
    pub async fn run_migrations(&self) -> Result<()> {
        self.db.run_migrations().await
    }

    /// Health check for all services
    #[allow(dead_code)]
    pub async fn health_check(&self) -> Result<bool> {
        self.db.health_check().await
    }
}
