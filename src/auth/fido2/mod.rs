//! Astral Key - FIDO2/WebAuthn authentication
//!
//! Passwordless authentication using WebAuthn standard.

pub mod registration;
pub mod authentication;
pub mod types;

pub use registration::{start_registration, finish_registration};
pub use authentication::{start_authentication, finish_authentication};

use crate::cache::pool::RedisPool;
use crate::config::Fido2Config;
use crate::error::{AuthError, Result};

/// FIDO2 service
#[derive(Clone)]
pub struct Fido2Service {
    cache: RedisPool,
}

impl Fido2Service {
    /// Create a new FIDO2 service
    pub fn new(_config: &Fido2Config, cache: RedisPool) -> Result<Self> {
        // TODO: Initialize WebAuthn when properly integrated
        Ok(Self { cache })
    }

    /// Get cache pool
    pub fn cache(&self) -> &RedisPool {
        &self.cache
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fido2_service_creation() {
        // This would require actual Redis connection
        // Unit tests would mock the cache
    }
}
