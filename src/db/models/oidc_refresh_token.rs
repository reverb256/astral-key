//! Astral Key - OIDC refresh tokens
//!
//! Opaque bearer values issued by /oidc/token. Only the SHA-256 hash is
//! stored; the raw token never reaches the database.

use chrono::{DateTime, Utc};
use sqlx::sqlite::SqlitePool;
use sqlx::Row;
use uuid::Uuid;

use crate::error::{AuthError, Result};

/// A stored refresh token (hash only — never the token itself).
#[derive(Debug, Clone)]
pub struct OidcRefreshToken {
    pub client_id: String,
    pub user_id: Uuid,
    pub scope: String,
    pub expires_at: DateTime<Utc>,
}

impl OidcRefreshToken {
    /// True when the token is past its expiry.
    pub fn is_expired(&self, now: DateTime<Utc>) -> bool {
        self.expires_at <= now
    }
}

/// Store a new refresh token hash.
pub async fn insert(
    pool: &SqlitePool,
    token_hash: &str,
    client_id: &str,
    user_id: Uuid,
    scope: &str,
    expires_at: DateTime<Utc>,
) -> Result<()> {
    sqlx::query(
        "INSERT INTO oidc_refresh_tokens (token_hash, client_id, user_id, scope, expires_at) \
         VALUES (?1, ?2, ?3, ?4, ?5)",
    )
    .bind(token_hash)
    .bind(client_id)
    .bind(user_id.to_string())
    .bind(scope)
    .bind(expires_at.to_rfc3339())
    .execute(pool)
    .await?;

    Ok(())
}

/// Look up a refresh token by its hash.
pub async fn find(pool: &SqlitePool, token_hash: &str) -> Result<Option<OidcRefreshToken>> {
    let row = sqlx::query(
        "SELECT client_id, user_id, scope, expires_at FROM oidc_refresh_tokens WHERE token_hash = ?1",
    )
    .bind(token_hash)
    .fetch_optional(pool)
    .await?;

    row.map(|row| {
        let user_id_raw: &str = row.get("user_id");
        let user_id = Uuid::parse_str(user_id_raw).map_err(|e| {
            AuthError::Database(sqlx::Error::Protocol(format!(
                "invalid user_id '{}' in oidc_refresh_tokens: {e}",
                user_id_raw
            )))
        })?;
        let expires_raw: &str = row.get("expires_at");
        let expires_at = DateTime::parse_from_rfc3339(expires_raw)
            .map(|dt| dt.with_timezone(&Utc))
            .map_err(|e| {
                AuthError::Database(sqlx::Error::Protocol(format!(
                    "invalid expires_at '{}' in oidc_refresh_tokens: {e}",
                    expires_raw
                )))
            })?;

        Ok(OidcRefreshToken {
            client_id: row.get::<&str, _>("client_id").to_string(),
            user_id,
            scope: row.get::<&str, _>("scope").to_string(),
            expires_at,
        })
    })
    .transpose()
}

/// Delete a refresh token by hash (rotation — a token is single-use).
pub async fn delete(pool: &SqlitePool, token_hash: &str) -> Result<()> {
    sqlx::query("DELETE FROM oidc_refresh_tokens WHERE token_hash = ?1")
        .bind(token_hash)
        .execute(pool)
        .await?;

    Ok(())
}
