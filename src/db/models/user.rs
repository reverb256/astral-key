//! Astral Key - User model

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

/// User model
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: Uuid,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl User {
    /// Create a new user
    pub async fn create(pool: &SqlitePool) -> Result<Self> {
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();

        sqlx::query("INSERT INTO users (id, created_at, updated_at) VALUES (?1, ?2, ?3)")
            .bind(&id)
            .bind(&now)
            .bind(&now)
            .execute(pool)
            .await?;

        Ok(Self {
            id: Uuid::parse_str(&id).map_err(|e| {
                AuthError::Database(sqlx::Error::Protocol(format!(
                    "invalid UUID '{}': {}",
                    id, e
                )))
            })?,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        })
    }

    /// Get user by ID
    pub async fn get_by_id(pool: &SqlitePool, id: Uuid) -> Result<Option<Self>> {
        let id_str = id.to_string();
        let row = sqlx::query("SELECT id, created_at, updated_at FROM users WHERE id = ?1")
            .bind(&id_str)
            .fetch_optional(pool)
            .await?;

        match row {
            Some(r) => {
                let row_id: &str = r.get("id");
                Ok(Some(User {
                    id: Uuid::parse_str(row_id).map_err(|e| {
                        AuthError::Database(sqlx::Error::Protocol(format!(
                            "invalid UUID '{}': {}",
                            row_id, e
                        )))
                    })?,
                    created_at: parse_dt(r.get::<&str, _>("created_at"), "created_at")?,
                    updated_at: parse_dt(r.get::<&str, _>("updated_at"), "updated_at")?,
                }))
            }
            None => Ok(None),
        }
    }

    /// Delete user
    #[allow(dead_code)]
    pub async fn delete(&self, pool: &SqlitePool) -> Result<()> {
        sqlx::query("DELETE FROM users WHERE id = ?1")
            .bind(self.id.to_string())
            .execute(pool)
            .await?;
        Ok(())
    }
}
