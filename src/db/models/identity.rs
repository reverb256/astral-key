//! Astral Key - Identity model (Ed25519 public-key identity)

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
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

/// Ed25519 public-key identity
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Identity {
    pub id: Uuid,
    pub user_id: Uuid,
    pub pubkey: String,
    pub label: Option<String>,
    pub is_current: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

fn row_to_identity(r: sqlx::sqlite::SqliteRow) -> Result<Identity> {
    let row_id: &str = r.get("id");
    let row_user_id: &str = r.get("user_id");
    Ok(Identity {
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
        pubkey: r.get("pubkey"),
        label: r.get("label"),
        is_current: r.get::<i64, _>("is_current") != 0,
        created_at: parse_dt(r.get::<&str, _>("created_at"), "created_at")?,
        updated_at: parse_dt(r.get::<&str, _>("updated_at"), "updated_at")?,
    })
}

impl Identity {
    /// Create a new identity. The first identity for a user becomes current.
    pub async fn create(
        pool: &SqlitePool,
        user_id: Uuid,
        pubkey: &str,
        label: Option<&str>,
    ) -> Result<Self> {
        let id = Uuid::new_v4().to_string();
        let now_dt = Utc::now();
        let now = now_dt.to_rfc3339();

        let mut tx = pool.begin().await?;

        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM identities WHERE user_id = ?1")
            .bind(user_id.to_string())
            .fetch_one(&mut *tx)
            .await?;
        let is_current = count == 0;

        sqlx::query(
            r#"INSERT INTO identities (id, user_id, pubkey, label, is_current, created_at, updated_at)
               VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6)"#,
        )
        .bind(&id)
        .bind(user_id.to_string())
        .bind(pubkey)
        .bind(label)
        .bind(if is_current { 1i64 } else { 0i64 })
        .bind(&now)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;

        Ok(Identity {
            id: Uuid::parse_str(&id).map_err(|e| {
                AuthError::Database(sqlx::Error::Protocol(format!(
                    "invalid UUID '{}': {}",
                    id, e
                )))
            })?,
            user_id,
            pubkey: pubkey.to_string(),
            label: label.map(|s| s.to_string()),
            is_current,
            created_at: now_dt,
            updated_at: now_dt,
        })
    }

    /// Get identity by ID
    pub async fn get_by_id(pool: &SqlitePool, id: Uuid) -> Result<Option<Self>> {
        let row = sqlx::query("SELECT * FROM identities WHERE id = ?1")
            .bind(id.to_string())
            .fetch_optional(pool)
            .await?;

        match row {
            Some(r) => Ok(Some(row_to_identity(r)?)),
            None => Ok(None),
        }
    }

    /// Get all identities for a user
    pub async fn get_by_user(pool: &SqlitePool, user_id: Uuid) -> Result<Vec<Self>> {
        let rows =
            sqlx::query("SELECT * FROM identities WHERE user_id = ?1 ORDER BY created_at DESC")
                .bind(user_id.to_string())
                .fetch_all(pool)
                .await?;

        rows.into_iter().map(row_to_identity).collect()
    }

    /// Get the current identity for a user
    pub async fn get_current(pool: &SqlitePool, user_id: Uuid) -> Result<Option<Self>> {
        let row =
            sqlx::query("SELECT * FROM identities WHERE user_id = ?1 AND is_current = 1 LIMIT 1")
                .bind(user_id.to_string())
                .fetch_optional(pool)
                .await?;

        match row {
            Some(r) => Ok(Some(row_to_identity(r)?)),
            None => Ok(None),
        }
    }

    /// Set this identity as the current one for the user
    pub async fn set_current(&self, pool: &SqlitePool) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        let mut tx = pool.begin().await?;

        sqlx::query("UPDATE identities SET is_current = 0 WHERE user_id = ?1")
            .bind(self.user_id.to_string())
            .execute(&mut *tx)
            .await?;

        sqlx::query("UPDATE identities SET is_current = 1, updated_at = ?1 WHERE id = ?2")
            .bind(&now)
            .bind(self.id.to_string())
            .execute(&mut *tx)
            .await?;

        tx.commit().await?;
        Ok(())
    }

    /// Delete identity
    pub async fn delete(&self, pool: &SqlitePool) -> Result<()> {
        sqlx::query("DELETE FROM identities WHERE id = ?1")
            .bind(self.id.to_string())
            .execute(pool)
            .await?;
        Ok(())
    }
}
