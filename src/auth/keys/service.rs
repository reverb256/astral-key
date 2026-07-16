//! Astral Key - API Key service layer
//!
//! Create, validate, revoke, and list API keys.
//! Stateless — operates directly on the database pool.

use chrono::Utc;
use sqlx::sqlite::SqlitePool;
use uuid::Uuid;

use crate::db::models::api_key::{ApiKey, ApiKeySummary};
use crate::error::{AuthError, Result};

/// Service for managing API keys.
///
/// Stateless design — all methods accept a `&SqlitePool` directly
/// rather than holding pool state in the service.
#[derive(Clone)]
pub struct KeyService;

impl KeyService {
    /// Create a new API key for the given user.
    ///
    /// Returns the `ApiKeySummary` (safe for clients) and the raw plaintext key
    /// (shown exactly once to the caller, never stored).
    pub async fn create_key(
        pool: &SqlitePool,
        user_id: Uuid,
        name: &str,
        scopes: &[&str],
        environment: &str,
        expires_in: Option<chrono::Duration>,
    ) -> Result<(ApiKeySummary, String)> {
        let (model, raw_key) =
            ApiKey::create(pool, user_id, name, scopes, environment, expires_in).await?;

        let summary = ApiKeySummary {
            id: model.id,
            key_prefix: model.key_prefix,
            name: model.name,
            scopes: model.scopes,
            environment: model.environment,
            created_at: model.created_at,
            expires_at: model.expires_at,
            last_used_at: model.last_used_at,
            revoked_at: model.revoked_at,
        };

        Ok((summary, raw_key))
    }

    /// Validate an API key by finding it via prefix match and verifying its Argon2id hash.
    ///
    /// Returns the matching `ApiKey` (with hash) if valid, or an `Unauthorized` error.
    pub async fn validate_key(pool: &SqlitePool, key: &str) -> Result<ApiKey> {
        let model = ApiKey::find_by_prefix_and_verify(pool, key)
            .await?
            .ok_or_else(|| AuthError::Unauthorized("Invalid API key".to_string()))?;

        // Check if expired
        if let Some(expires_at) = model.expires_at {
            if Utc::now() > expires_at {
                return Err(AuthError::Unauthorized("API key has expired".to_string()));
            }
        }

        Ok(model)
    }

    /// List all API keys for a user (summary only — no hash, no plaintext).
    pub async fn list_keys(pool: &SqlitePool, user_id: Uuid) -> Result<Vec<ApiKeySummary>> {
        ApiKey::find_by_user(pool, user_id).await
    }

    /// Revoke an API key by ID (soft delete — sets `revoked_at`).
    pub async fn revoke_key(pool: &SqlitePool, key_id: Uuid, user_id: Uuid) -> Result<()> {
        let key = ApiKey::find_by_id(pool, key_id)
            .await?
            .ok_or_else(|| AuthError::NotFound("API key not found".to_string()))?;

        if key.user_id != user_id {
            return Err(AuthError::Forbidden(
                "Cannot revoke another user's API key".to_string(),
            ));
        }

        key.revoke(pool).await
    }

    /// Delete an API key (hard delete from database).
    pub async fn delete_key(pool: &SqlitePool, key_id: Uuid, user_id: Uuid) -> Result<()> {
        let key = ApiKey::find_by_id(pool, key_id)
            .await?
            .ok_or_else(|| AuthError::NotFound("API key not found".to_string()))?;

        if key.user_id != user_id {
            return Err(AuthError::Forbidden(
                "Cannot delete another user's API key".to_string(),
            ));
        }

        key.delete(pool).await
    }
}
