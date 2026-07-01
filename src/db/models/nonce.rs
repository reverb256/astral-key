//! Astral Key - Nonce model for SIWE

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

use crate::error::Result;

/// Nonce model
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
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
        pool: &PgPool,
        nonce: &str,
        expires_at: DateTime<Utc>,
        user_id: Option<Uuid>,
    ) -> Result<Self> {
        let nonce_record = sqlx::query_as::<_, Nonce>(
            r#"
            INSERT INTO nonces (nonce, expires_at, user_id)
            VALUES ($1, $2, $3)
            RETURNING *
            "#,
        )
        .bind(nonce)
        .bind(expires_at)
        .bind(user_id)
        .fetch_one(pool)
        .await?;

        Ok(nonce_record)
    }

    /// Get nonce by value
    pub async fn get_by_nonce(pool: &PgPool, nonce: &str) -> Result<Option<Self>> {
        let nonce_record = sqlx::query_as::<_, Nonce>("SELECT * FROM nonces WHERE nonce = $1")
            .bind(nonce)
            .fetch_optional(pool)
            .await?;

        Ok(nonce_record)
    }

    /// Check if nonce is valid (exists, not used, not expired)
    pub fn is_valid(&self) -> bool {
        // Check if already used
        if self.used_at.is_some() {
            return false;
        }

        // Check if expired
        if self.expires_at < Utc::now() {
            return false;
        }

        true
    }

    /// Mark nonce as used
    pub async fn mark_as_used(&self, pool: &PgPool) -> Result<()> {
        sqlx::query("UPDATE nonces SET used_at = NOW() WHERE id = $1")
            .bind(self.id)
            .execute(pool)
            .await?;

        Ok(())
    }

    /// Delete expired nonces
    pub async fn delete_expired(pool: &PgPool) -> Result<u64> {
        let result = sqlx::query("DELETE FROM nonces WHERE expires_at < NOW()")
            .execute(pool)
            .await?;

        Ok(result.rows_affected())
    }

    /// Clean up old used nonces (older than specified days)
    pub async fn delete_old_used(pool: &PgPool, days: i64) -> Result<u64> {
        let result = sqlx::query(
            r#"
            DELETE FROM nonces
            WHERE used_at IS NOT NULL
            AND used_at < NOW() - (MAKE_INTERVAL(days => $1))
            "#,
        )
        .bind(days)
        .execute(pool)
        .await?;

        Ok(result.rows_affected())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    #[tokio::test]
    #[ignore]
    async fn test_nonce_validity() {
        // Valid nonce (not used, not expired)
        let nonce = Nonce {
            id: Uuid::new_v4(),
            nonce: "test_nonce".to_string(),
            created_at: Utc::now(),
            expires_at: Utc::now() + Duration::minutes(10),
            used_at: None,
            user_id: None,
        };

        assert!(nonce.is_valid());

        // Expired nonce
        let expired_nonce = Nonce {
            expires_at: Utc::now() - Duration::minutes(1),
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
