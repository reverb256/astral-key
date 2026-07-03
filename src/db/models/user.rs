//! Astral Key - User model

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::sqlite::SqlitePool;
use sqlx::Row;
use uuid::Uuid;

use crate::error::Result;

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

        sqlx::query(
            "INSERT INTO users (id, created_at, updated_at) VALUES (?1, ?2, ?3)",
        )
        .bind(&id)
        .bind(&now)
        .bind(&now)
        .execute(pool)
        .await?;

        Ok(Self {
            id: Uuid::parse_str(&id).unwrap(),
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
            Some(r) => Ok(Some(User {
                id: Uuid::parse_str(r.get::<&str, _>("id")).unwrap(),
                created_at: chrono::DateTime::parse_from_rfc3339(r.get::<&str, _>("created_at"))
                    .unwrap()
                    .with_timezone(&Utc),
                updated_at: chrono::DateTime::parse_from_rfc3339(r.get::<&str, _>("updated_at"))
                    .unwrap()
                    .with_timezone(&Utc),
            })),
            None => Ok(None),
        }
    }

    /// Delete user
    pub async fn delete(&self, pool: &SqlitePool) -> Result<()> {
        sqlx::query("DELETE FROM users WHERE id = ?1")
            .bind(self.id.to_string())
            .execute(pool)
            .await?;
        Ok(())
    }
}
