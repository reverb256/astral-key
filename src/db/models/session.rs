//! Astral Key - Session model

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

use crate::error::Result;

/// Session model
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Session {
    pub id: Uuid,
    pub user_id: Uuid,
    pub refresh_token_hash: String,
    pub expires_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub user_agent: Option<String>,
    pub ip_address: Option<String>,
    pub revoked_at: Option<DateTime<Utc>>,
}

impl Session {
    /// Create a new session
    pub async fn create(
        pool: &PgPool,
        user_id: Uuid,
        refresh_token_hash: &str,
        expires_at: DateTime<Utc>,
        user_agent: Option<String>,
        ip_address: Option<String>,
    ) -> Result<Self> {
        let session = sqlx::query_as::<_, Session>(
            r#"
            INSERT INTO sessions (user_id, refresh_token_hash, expires_at, user_agent, ip_address)
            VALUES ($1, $2, $3, $4, $5)
            RETURNING *
            "#
        )
        .bind(user_id)
        .bind(refresh_token_hash)
        .bind(expires_at)
        .bind(user_agent)
        .bind(ip_address)
        .fetch_one(pool)
        .await?;

        Ok(session)
    }

    /// Get session by ID
    pub async fn get_by_id(pool: &PgPool, id: Uuid) -> Result<Option<Self>> {
        let session = sqlx::query_as::<_, Session>("SELECT * FROM sessions WHERE id = $1")
            .bind(id)
            .fetch_optional(pool)
            .await?;

        Ok(session)
    }

    /// Get session by refresh token hash
    pub async fn get_by_refresh_token_hash(
        pool: &PgPool,
        hash: &str,
    ) -> Result<Option<Self>> {
        let session = sqlx::query_as::<_, Session>(
            "SELECT * FROM sessions WHERE refresh_token_hash = $1"
        )
        .bind(hash)
        .fetch_optional(pool)
        .await?;

        Ok(session)
    }

    /// Get all active sessions for a user
    pub async fn get_active_by_user(pool: &PgPool, user_id: Uuid) -> Result<Vec<Self>> {
        let sessions = sqlx::query_as::<_, Session>(
            r#"
            SELECT * FROM sessions
            WHERE user_id = $1
            AND revoked_at IS NULL
            AND expires_at > NOW()
            ORDER BY created_at DESC
            "#
        )
        .bind(user_id)
        .fetch_all(pool)
        .await?;

        Ok(sessions)
    }

    /// Check if session is valid (not revoked and not expired)
    pub fn is_valid(&self) -> bool {
        if self.revoked_at.is_some() {
            return false;
        }

        if self.expires_at < Utc::now() {
            return false;
        }

        true
    }

    /// Revoke session
    pub async fn revoke(&self, pool: &PgPool) -> Result<()> {
        sqlx::query("UPDATE sessions SET revoked_at = NOW() WHERE id = $1")
            .bind(self.id)
            .execute(pool)
            .await?;

        Ok(())
    }

    /// Delete session
    pub async fn delete(&self, pool: &PgPool) -> Result<()> {
        sqlx::query("DELETE FROM sessions WHERE id = $1")
            .bind(self.id)
            .execute(pool)
            .await?;

        Ok(())
    }

    /// Delete expired sessions
    pub async fn delete_expired(pool: &PgPool) -> Result<u64> {
        let result = sqlx::query("DELETE FROM sessions WHERE expires_at < NOW()")
            .execute(pool)
            .await?;

        Ok(result.rows_affected())
    }

    /// Update refresh token hash
    pub async fn update_refresh_token(
        &self,
        pool: &PgPool,
        new_hash: &str,
    ) -> Result<()> {
        sqlx::query("UPDATE sessions SET refresh_token_hash = $1 WHERE id = $2")
            .bind(new_hash)
            .bind(self.id)
            .execute(pool)
            .await?;

        Ok(())
    }
}
