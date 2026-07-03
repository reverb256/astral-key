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

use crate::config::Fido2Config;
use crate::error::{AuthError, Result};

/// Challenge storage key prefix
const CHALLENGE_PREFIX: &str = "fido2_challenge:";

/// FIDO2 service with webauthn-rs (no Redis dependency)
#[derive(Clone)]
pub struct Fido2Service {
    /// WebAuthn instance
    webauthn: Webauthn,
}

impl Fido2Service {
    /// Create a new FIDO2 service with webauthn-rs
    pub fn new(config: &Fido2Config) -> Result<Self> {
        // Parse origins from config
        let mut origins = Vec::new();
        for origin_str in &config.origins {
            let origin = Url::parse(origin_str).map_err(|e| {
                AuthError::Config(format!("Invalid origin URL '{}': {}", origin_str, e))
            })?;
            origins.push(origin);
        }

        if origins.is_empty() {
            return Err(AuthError::Config(
                "At least one FIDO2 origin must be configured".to_string(),
            ));
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
            .require_resident_key(ResidentKeyRequirement::Preferred)
            .build()
            .map_err(|e| AuthError::Config(format!("Failed to build WebAuthn: {}", e)))?;

        Ok(Self { webauthn })
    }

    /// Get WebAuthn instance
    pub fn webauthn(&self) -> &Webauthn {
        &self.webauthn
    }

    /// Store state in-memory (via AppState) for registration/authentication states
    pub async fn store_state(
        &self,
        state: &crate::state::AppState,
        user_id: Uuid,
        state_type: &str,
        state_json: String,
    ) -> Result<()> {
        let key = format!("{}{}:{}", CHALLENGE_PREFIX, user_id, state_type);
        state.fido2_state.store(&key, state_json, 300).await;
        Ok(())
    }

    /// Retrieve state from in-memory store
    pub async fn get_state(
        &self,
        state: &crate::state::AppState,
        user_id: Uuid,
        state_type: &str,
    ) -> Result<Option<String>> {
        let key = format!("{}{}:{}", CHALLENGE_PREFIX, user_id, state_type);
        Ok(state.fido2_state.get(&key).await)
    }

    /// Consume state from in-memory store (one-time use)
    pub async fn consume_state(
        &self,
        state: &crate::state::AppState,
        user_id: Uuid,
        state_type: &str,
    ) -> Result<()> {
        let key = format!("{}{}:{}", CHALLENGE_PREFIX, user_id, state_type);
        state.fido2_state.delete(&key).await;
        Ok(())
    }
}
