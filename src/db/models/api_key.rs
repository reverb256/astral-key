//! Astral Key - API Key model (SQLite)
//!
//! API key management with Argon2id hashing.
//! Only the key hash and prefix are stored — never the plaintext key.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::sqlite::SqlitePool;
use sqlx::Row;
use uuid::Uuid;

use crate::auth::keys::hashing::{generate_api_key, verify_api_key};
use crate::error::{AuthError, Result};

/// Parse RFC 3339 datetime from SQLite TEXT column
fn parse_dt(s: &str, field: &str) -> Result<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(s)
        .map(|dt| dt.with_timezone(&Utc))
        .map_err(|e| {
            AuthError::Database(sqlx::Error::Protocol(format!(
                "invalid timestamp for {}: '{}' — {}",
                field, s, e
            )))
        })
}

/// API key model.
///
/// Stores only the Argon2id hash and prefix — never the plaintext key value.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiKey {
    pub id: Uuid,
    pub user_id: Uuid,
    pub key_hash: String,
    pub key_prefix: String,
    pub name: String,
    pub scopes: String,
    pub environment: String,
    pub created_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
    pub last_used_at: Option<DateTime<Utc>>,
    pub revoked_at: Option<DateTime<Utc>>,
}

/// Summary returned to clients (no hash, no plaintext).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiKeySummary {
    pub id: Uuid,
    pub key_prefix: String,
    pub name: String,
    pub scopes: String,
    pub environment: String,
    pub created_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
    pub last_used_at: Option<DateTime<Utc>>,
    pub revoked_at: Option<DateTime<Utc>>,
}

fn row_to_api_key(r: sqlx::sqlite::SqliteRow) -> Result<ApiKey> {
    let row_id: &str = r.get("id");
    let row_user_id: &str = r.get("user_id");
    Ok(ApiKey {
        id: Uuid::parse_str(row_id).map_err(|e| {
            AuthError::Database(sqlx::Error::Protocol(format!(
                "invalid UUID '{}': {}",
                row_id, e
            )))
        })?,
        user_id: Uuid::parse_str(row_user_id).map_err(|e| {
            AuthError::Database(sqlx::Error::Protocol(format!(
                "invalid UUID '{}': {}",
                row_user_id, e
            )))
        })?,
        key_hash: r.get("key_hash"),
        key_prefix: r.get("key_prefix"),
        name: r.get("name"),
        scopes: r.get("scopes"),
        environment: r.get("environment"),
        created_at: parse_dt(r.get::<&str, _>("created_at"), "created_at")?,
        expires_at: match r.get::<Option<&str>, _>("expires_at") {
            Some(s) => Some(parse_dt(s, "expires_at")?),
            None => None,
        },
        last_used_at: match r.get::<Option<&str>, _>("last_used_at") {
            Some(s) => Some(parse_dt(s, "last_used_at")?),
            None => None,
        },
        revoked_at: match r.get::<Option<&str>, _>("revoked_at") {
            Some(s) => Some(parse_dt(s, "revoked_at")?),
            None => None,
        },
    })
}

impl ApiKey {
    /// Generate a new API key.
    ///
    /// Internally generates 32 random bytes, base58-encodes them, prepends an
    /// environment prefix (e.g. `ak_prod_`), and hashes the result with Argon2id.
    /// Only the hash and prefix are stored — the plaintext key is returned to the
    /// caller exactly once.
    pub async fn create(
        pool: &SqlitePool,
        user_id: Uuid,
        name: &str,
        scopes: &[&str],
        environment: &str,
        expires_in: Option<chrono::Duration>,
    ) -> Result<(Self, String)> {
        let (hash, prefix, raw_key) = generate_api_key(environment)?;
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        let scopes_str = scopes.join(",");

        let expires_at = expires_in.map(|d| (Utc::now() + d).to_rfc3339());

        sqlx::query(
            r#"INSERT INTO api_keys (id, user_id, key_hash, key_prefix, name, scopes, environment, created_at, expires_at)
               VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)"#,
        )
        .bind(&id)
        .bind(user_id.to_string())
        .bind(&hash)
        .bind(&prefix)
        .bind(name)
        .bind(&scopes_str)
        .bind(environment)
        .bind(&now)
        .bind(&expires_at)
        .execute(pool)
        .await?;

        let model = ApiKey {
            id: Uuid::parse_str(&id).map_err(|e| {
                AuthError::Database(sqlx::Error::Protocol(format!(
                    "invalid UUID '{}': {}",
                    id, e
                )))
            })?,
            user_id,
            key_hash: hash,
            key_prefix: prefix,
            name: name.to_string(),
            scopes: scopes_str,
            environment: environment.to_string(),
            created_at: Utc::now(),
            expires_at: expires_in.map(|d| Utc::now() + d),
            last_used_at: None,
            revoked_at: None,
        };

        Ok((model, raw_key))
    }

    /// Find an API key by its internal UUID.
    pub async fn find_by_id(pool: &SqlitePool, id: Uuid) -> Result<Option<Self>> {
        let row = sqlx::query("SELECT * FROM api_keys WHERE id = ?1")
            .bind(id.to_string())
            .fetch_optional(pool)
            .await?;

        match row {
            Some(r) => Ok(Some(row_to_api_key(r)?)),
            None => Ok(None),
        }
    }

    /// Find all API keys for a user (summary only — no hash).
    pub async fn find_by_user(pool: &SqlitePool, user_id: Uuid) -> Result<Vec<ApiKeySummary>> {
        let rows =
            sqlx::query("SELECT * FROM api_keys WHERE user_id = ?1 ORDER BY created_at DESC")
                .bind(user_id.to_string())
                .fetch_all(pool)
                .await?;

        rows.into_iter()
            .map(|r| {
                let key = row_to_api_key(r)?;
                Ok(ApiKeySummary {
                    id: key.id,
                    key_prefix: key.key_prefix,
                    name: key.name,
                    scopes: key.scopes,
                    environment: key.environment,
                    created_at: key.created_at,
                    expires_at: key.expires_at,
                    last_used_at: key.last_used_at,
                    revoked_at: key.revoked_at,
                })
            })
            .collect()
    }

    /// Find an API key by prefix-matching and verifying its Argon2id hash.
    ///
    /// Extracts the prefix from the full key (e.g. `ak_prod_`), queries for
    /// non-revoked keys with that prefix, and returns the first one whose hash
    /// verifies against the provided key.
    #[allow(dead_code)]
    pub async fn find_by_prefix_and_verify(pool: &SqlitePool, key: &str) -> Result<Option<Self>> {
        let prefix = crate::auth::keys::hashing::extract_prefix(key);

        let rows =
            sqlx::query("SELECT * FROM api_keys WHERE key_prefix = ?1 AND revoked_at IS NULL")
                .bind(prefix)
                .fetch_all(pool)
                .await?;

        for row in rows {
            let model = row_to_api_key(row)?;
            if verify_api_key(key, &model.key_hash)? {
                return Ok(Some(model));
            }
        }

        Ok(None)
    }

    /// Soft-delete this API key by setting `revoked_at`.
    pub async fn revoke(&self, pool: &SqlitePool) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        sqlx::query("UPDATE api_keys SET revoked_at = ?1 WHERE id = ?2")
            .bind(&now)
            .bind(self.id.to_string())
            .execute(pool)
            .await?;
        Ok(())
    }

    /// Hard-delete this API key from the database.
    pub async fn delete(&self, pool: &SqlitePool) -> Result<()> {
        sqlx::query("DELETE FROM api_keys WHERE id = ?1")
            .bind(self.id.to_string())
            .execute(pool)
            .await?;
        Ok(())
    }

    /// Update the `last_used_at` timestamp to now.
    #[allow(dead_code)]
    pub async fn update_last_used(&self, pool: &SqlitePool) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        sqlx::query("UPDATE api_keys SET last_used_at = ?1 WHERE id = ?2")
            .bind(&now)
            .bind(self.id.to_string())
            .execute(pool)
            .await?;
        Ok(())
    }
}
