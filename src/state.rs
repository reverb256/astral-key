//! Astral Key - Application state
//!
//! Shared state across the application, including database connections,
/// cache clients, and configuration.

use std::sync::Arc;

use crate::config::Config;
use crate::error::{AuthError, Result};

/// Shared application state
#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,
    // TODO: Add database pool, redis client, vaultwarden client
}

impl AppState {
    /// Create a new application state
    pub async fn new(config: Config) -> Result<Self> {
        // TODO: Initialize database pool
        // TODO: Initialize redis client
        // TODO: Initialize vaultwarden client

        Ok(Self {
            config: Arc::new(config),
        })
    }
}
