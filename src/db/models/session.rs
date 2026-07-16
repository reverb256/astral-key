//! Astral Key - Session model (SQLite)
//!
//! Session management with refresh token rotation support.
//! Refresh tokens are stored as SHA-256 hashes for lookup; the JWT itself
//! provides cryptographic integrity.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::sqlite::SqlitePool;
use sqlx::Row;
use uuid::Uuid;

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

/// Session model with refresh token rotation support.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: Uuid,
    pub user_id: Uuid,
    pub refresh_token_hash: String,
    pub device_info: String,
    pub ip_address: String,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub revoked_at: Option<DateTime<Utc>>,
}

/// Summary returned to clients (no token hash).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionSummary {
    pub id: Uuid,
    pub device_info: String,
    pub ip_address: String,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub revoked_at: Option<DateTime<Utc>>,
}

fn row_to_session(r: sqlx::sqlite::SqliteRow) -> Result<Session> {
    let row_id: &str = r.get("id");
    let row_user_id: &str = r.get("user_id");
    Ok(Session {
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
        refresh_token_hash: r.get("refresh_token_hash"),
        device_info: r.get("device_info"),
        ip_address: r.get("ip_address"),
        created_at: parse_dt(r.get::<&str, _>("created_at"), "created_at")?,
        expires_at: parse_dt(r.get::<&str, _>("expires_at"), "expires_at")?,
        revoked_at: match r.get::<Option<&str>, _>("revoked_at") {
            Some(s) => Some(parse_dt(s, "revoked_at")?),
            None => None,
        },
    })
}

impl Session {
    /// Create a new session.
    ///
    /// The `refresh_token` is SHA-256 hashed before storage;
    /// the JWT itself provides integrity.
    pub async fn create(
        pool: &SqlitePool,
        user_id: Uuid,
        refresh_token: &str,
        device_info: &str,
        ip_address: &str,
        expires_in: chrono::Duration,
    ) -> Result<Self> {
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        let expires_at = (Utc::now() + expires_in).to_rfc3339();
        let token_hash = hash_refresh_token(refresh_token);

        sqlx::query(
            r#"INSERT INTO sessions (id, user_id, refresh_token_hash, device_info, ip_address, created_at, expires_at)
               VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)"#,
        )
        .bind(&id)
        .bind(user_id.to_string())
        .bind(&token_hash)
        .bind(device_info)
        .bind(ip_address)
        .bind(&now)
        .bind(&expires_at)
        .execute(pool)
        .await?;

        Ok(Session {
            id: Uuid::parse_str(&id).map_err(|e| {
                AuthError::Database(sqlx::Error::Protocol(format!(
                    "invalid UUID '{}': {}",
                    id, e
                )))
            })?,
            user_id,
            refresh_token_hash: token_hash,
            device_info: device_info.to_string(),
            ip_address: ip_address.to_string(),
            created_at: Utc::now(),
            expires_at: Utc::now() + expires_in,
            revoked_at: None,
        })
    }

    /// Find a session by its internal UUID.
    pub async fn find_by_id(pool: &SqlitePool, id: Uuid) -> Result<Option<Self>> {
        let row = sqlx::query("SELECT * FROM sessions WHERE id = ?1")
            .bind(id.to_string())
            .fetch_optional(pool)
            .await?;

        match row {
            Some(r) => Ok(Some(row_to_session(r)?)),
            None => Ok(None),
        }
    }

    /// Find all active (non-revoked) sessions for a user.
    pub async fn find_by_user(pool: &SqlitePool, user_id: Uuid) -> Result<Vec<SessionSummary>> {
        let rows = sqlx::query(
            "SELECT * FROM sessions WHERE user_id = ?1 AND revoked_at IS NULL ORDER BY created_at DESC",
        )
        .bind(user_id.to_string())
        .fetch_all(pool)
        .await?;

        rows.into_iter()
            .map(|r| {
                let s = row_to_session(r)?;
                Ok(SessionSummary {
                    id: s.id,
                    device_info: s.device_info,
                    ip_address: s.ip_address,
                    created_at: s.created_at,
                    expires_at: s.expires_at,
                    revoked_at: s.revoked_at,
                })
            })
            .collect()
    }

    /// Find a session by refresh token hash (deterministic SHA-256 lookup).
    pub async fn find_by_refresh_hash(pool: &SqlitePool, hash: &str) -> Result<Option<Self>> {
        let now = Utc::now().to_rfc3339();
        let row = sqlx::query(
            "SELECT * FROM sessions WHERE refresh_token_hash = ?1 AND revoked_at IS NULL AND expires_at > ?2",
        )
        .bind(hash)
        .bind(&now)
        .fetch_optional(pool)
        .await?;

        match row {
            Some(r) => Ok(Some(row_to_session(r)?)),
            None => Ok(None),
        }
    }

    /// Soft-delete this session by setting `revoked_at`.
    pub async fn revoke(&self, pool: &SqlitePool) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        sqlx::query("UPDATE sessions SET revoked_at = ?1 WHERE id = ?2")
            .bind(&now)
            .bind(self.id.to_string())
            .execute(pool)
            .await?;
        Ok(())
    }

    /// Hard-delete this session from the database.
    pub async fn delete(&self, pool: &SqlitePool) -> Result<()> {
        sqlx::query("DELETE FROM sessions WHERE id = ?1")
            .bind(self.id.to_string())
            .execute(pool)
            .await?;
        Ok(())
    }

    /// Rotate the refresh token (for refresh token rotation on each refresh).
    ///
    /// Stores a new SHA-256 hash and updates the expiry time.
    pub async fn rotate_refresh_token(
        &self,
        pool: &SqlitePool,
        new_refresh_token: &str,
        new_expires_at: DateTime<Utc>,
    ) -> Result<()> {
        let new_hash = hash_refresh_token(new_refresh_token);
        sqlx::query("UPDATE sessions SET refresh_token_hash = ?1, expires_at = ?2 WHERE id = ?3")
            .bind(&new_hash)
            .bind(new_expires_at.to_rfc3339())
            .bind(self.id.to_string())
            .execute(pool)
            .await?;
        Ok(())
    }
}

/// SHA-256 hash a refresh token for deterministic lookup.
fn hash_refresh_token(token: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    hex::encode(hasher.finalize())
}
