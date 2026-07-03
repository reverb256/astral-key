//! Astral Key - Nonce model for SIWE (SQLite)

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::sqlite::SqlitePool;
use sqlx::Row;
use uuid::Uuid;

use crate::error::Result;

/// Nonce model
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Nonce {
    pub id: Uuid,
    pub nonce: String,
    pub expires_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub used_at: Option<DateTime<Utc>>,
    pub user_id: Option<Uuid>,
}

impl Nonce {
    /// Create a new nonce
    pub async fn create(
        pool: &SqlitePool,
        nonce: &str,
        expires_at: DateTime<Utc>,
        user_id: Option<Uuid>,
    ) -> Result<Self> {
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        let expires_str = expires_at.to_rfc3339();

        sqlx::query(
            r#"INSERT INTO nonces (id, nonce, expires_at, created_at, user_id)
               VALUES (?1, ?2, ?3, ?4, ?5)"#,
        )
        .bind(&id)
        .bind(nonce)
        .bind(&expires_str)
        .bind(&now)
        .bind(user_id.map(|u| u.to_string()))
        .execute(pool)
        .await?;

        Ok(Nonce {
            id: Uuid::parse_str(&id).unwrap(),
            nonce: nonce.to_string(),
            expires_at,
            created_at: Utc::now(),
            used_at: None,
            user_id,
        })
    }

    /// Get nonce by value
    pub async fn get_by_nonce(pool: &SqlitePool, nonce: &str) -> Result<Option<Self>> {
        let row = sqlx::query("SELECT * FROM nonces WHERE nonce = ?1")
            .bind(nonce)
            .fetch_optional(pool)
            .await?;

        match row {
            Some(r) => Ok(Some(Nonce {
                id: Uuid::parse_str(r.get::<&str, _>("id")).unwrap(),
                nonce: r.get("nonce"),
                expires_at: chrono::DateTime::parse_from_rfc3339(
                    r.get::<&str, _>("expires_at"),
                )
                .unwrap()
                .with_timezone(&Utc),
                created_at: chrono::DateTime::parse_from_rfc3339(
                    r.get::<&str, _>("created_at"),
                )
                .unwrap()
                .with_timezone(&Utc),
                used_at: r
                    .get::<Option<&str>, _>("used_at")
                    .map(|s| {
                        chrono::DateTime::parse_from_rfc3339(s)
                            .unwrap()
                            .with_timezone(&Utc)
                    }),
                user_id: r
                    .get::<Option<&str>, _>("user_id")
                    .map(|s| Uuid::parse_str(s).unwrap()),
            })),
            None => Ok(None),
        }
    }

    /// Check if nonce is valid (exists, not used, not expired)
    pub fn is_valid(&self) -> bool {
        if self.used_at.is_some() {
            return false;
        }
        if self.expires_at < Utc::now() {
            return false;
        }
        true
    }

    /// Mark nonce as used
    pub async fn mark_as_used(&self, pool: &SqlitePool) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        sqlx::query("UPDATE nonces SET used_at = ?1 WHERE id = ?2")
            .bind(&now)
            .bind(self.id.to_string())
            .execute(pool)
            .await?;
        Ok(())
    }

    /// Delete expired nonces
    pub async fn delete_expired(pool: &SqlitePool) -> Result<u64> {
        let now = Utc::now().to_rfc3339();
        let result = sqlx::query("DELETE FROM nonces WHERE expires_at < ?1")
            .bind(&now)
            .execute(pool)
            .await?;
        Ok(result.rows_affected())
    }

    /// Delete all used nonces
    pub async fn delete_old_used(pool: &SqlitePool, _days: i64) -> Result<u64> {
        // Simpler approach for SQLite: delete all used nonces
        let result = sqlx::query("DELETE FROM nonces WHERE used_at IS NOT NULL")
            .execute(pool)
            .await?;
        Ok(result.rows_affected())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_nonce_validity() {
        let nonce = Nonce {
            id: Uuid::new_v4(),
            nonce: "test_nonce".to_string(),
            created_at: Utc::now(),
            expires_at: Utc::now() + chrono::Duration::minutes(10),
            used_at: None,
            user_id: None,
        };

        assert!(nonce.is_valid());

        // Expired nonce
        let expired_nonce = Nonce {
            expires_at: Utc::now() - chrono::Duration::minutes(1),
            ..nonce.clone()
        };
        assert!(!expired_nonce.is_valid());

        // Used nonce
        let used_nonce = Nonce {
            used_at: Some(Utc::now()),
            ..nonce
        };
        assert!(!used_nonce.is_valid());
    }
}
