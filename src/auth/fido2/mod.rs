//! Astral Key - FIDO2/WebAuthn authentication
//!
//! Passwordless authentication using WebAuthn standard with webauthn-rs.

pub mod authentication;
pub mod registration;
pub mod types;

pub use authentication::{finish_authentication, start_authentication};
pub use registration::{finish_registration, start_registration};

use url::Url;
use uuid::Uuid;
use webauthn_rs::prelude::*;

use crate::cache::pool::RedisPool;
use crate::config::Fido2Config;
use crate::error::{AuthError, Result};

/// Challenge storage key prefix
const CHALLENGE_PREFIX: &str = "fido2_challenge:";

/// FIDO2 service with webauthn-rs
#[derive(Clone)]
pub struct Fido2Service {
    /// WebAuthn instance
    webauthn: Webauthn,
    /// Cache pool for state storage
    cache: RedisPool,
}

impl Fido2Service {
    /// Create a new FIDO2 service with webauthn-rs
    pub fn new(config: &Fido2Config, cache: RedisPool) -> Result<Self> {
        // Parse origins from config
        let mut origins = Vec::new();
        for origin_str in &config.origins {
            let origin = Url::parse(origin_str).map_err(|e| {
                AuthError::Config(format!("Invalid origin URL '{}': {}", origin_str, e))
            })?;
            origins.push(origin);
        }

        // Create WebAuthn builder
        let builder = WebauthnBuilder::new(&config.rp_id, &origins[0])
            .map_err(|e| AuthError::Config(format!("Invalid WebAuthn configuration: {}", e)))?;

        // Add additional origins
        let builder = origins[1..]
            .iter()
            .fold(builder, |b, origin| b.append_allowed_origin(origin));

        // Set RP name
        let builder = builder.rp_name(&config.rp_name);

        // Build WebAuthn instance
        let webauthn = builder
            .build()
            .map_err(|e| AuthError::Config(format!("Failed to build WebAuthn: {}", e)))?;

        Ok(Self { webauthn, cache })
    }

    /// Get WebAuthn instance
    pub fn webauthn(&self) -> &Webauthn {
        &self.webauthn
    }

    /// Get cache pool
    pub fn cache(&self) -> &RedisPool {
        &self.cache
    }

    /// Store state in cache (for registration/authentication states)
    pub async fn store_state(
        &self,
        user_id: Uuid,
        state_type: &str,
        state_json: String,
    ) -> Result<()> {
        let key = format!("{}{}:{}", CHALLENGE_PREFIX, user_id, state_type);
        self.cache.set_with_expiry(&key, &state_json, 300).await?;
        Ok(())
    }

    /// Retrieve state from cache
    pub async fn get_state(&self, user_id: Uuid, state_type: &str) -> Result<Option<String>> {
        let key = format!("{}{}:{}", CHALLENGE_PREFIX, user_id, state_type);
        self.cache.get(&key).await.map_err(Into::into)
    }

    /// Consume state from cache (one-time use)
    pub async fn consume_state(&self, user_id: Uuid, state_type: &str) -> Result<()> {
        let key = format!("{}{}:{}", CHALLENGE_PREFIX, user_id, state_type);
        self.cache.delete(&key).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
}
