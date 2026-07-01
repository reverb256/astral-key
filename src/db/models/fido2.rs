//! Astral Key - FIDO2 credential model

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

use crate::error::Result;

/// FIDO2 credential model
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Fido2Credential {
    pub id: Uuid,
    pub user_id: Uuid,
    pub credential_id: String,
    pub public_key: String,
    pub counter: i64,
    pub transport: Option<String>,
    pub attestation_type: Option<String>,
    pub created_at: DateTime<Utc>,
    pub last_used_at: Option<DateTime<Utc>>,
    pub name: Option<String>,
}

impl Fido2Credential {
    /// Create a new credential
    pub async fn create(
        pool: &PgPool,
        user_id: Uuid,
        credential_id: &str,
        public_key: &str,
    ) -> Result<Self> {
        let credential = sqlx::query_as::<_, Fido2Credential>(
            r#"
            INSERT INTO fido2_credentials (user_id, credential_id, public_key)
            VALUES ($1, $2, $3)
            RETURNING *
            "#,
        )
        .bind(user_id)
        .bind(credential_id)
        .bind(public_key)
        .fetch_one(pool)
        .await?;

        Ok(credential)
    }

    /// Get credential by credential ID
    pub async fn get_by_credential_id(pool: &PgPool, credential_id: &str) -> Result<Option<Self>> {
        let credential = sqlx::query_as::<_, Fido2Credential>(
            "SELECT * FROM fido2_credentials WHERE credential_id = $1",
        )
        .bind(credential_id)
        .fetch_optional(pool)
        .await?;

        Ok(credential)
    }

    /// Get credential by internal ID
    pub async fn get_by_id(pool: &PgPool, id: Uuid) -> Result<Option<Self>> {
        let credential =
            sqlx::query_as::<_, Fido2Credential>("SELECT * FROM fido2_credentials WHERE id = $1")
                .bind(id)
                .fetch_optional(pool)
                .await?;

        Ok(credential)
    }

    /// Get all credentials for a user
    pub async fn get_by_user(pool: &PgPool, user_id: Uuid) -> Result<Vec<Self>> {
        let credentials = sqlx::query_as::<_, Fido2Credential>(
            "SELECT * FROM fido2_credentials WHERE user_id = $1 ORDER BY created_at DESC",
        )
        .bind(user_id)
        .fetch_all(pool)
        .await?;

        Ok(credentials)
    }

    /// Update counter and last used timestamp
    pub async fn update_usage(&self, pool: &PgPool, new_counter: i64) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE fido2_credentials
            SET counter = $1, last_used_at = NOW()
            WHERE id = $2
            "#,
        )
        .bind(new_counter)
        .bind(self.id)
        .execute(pool)
        .await?;

        Ok(())
    }

    /// Delete credential
    pub async fn delete(&self, pool: &PgPool) -> Result<()> {
        sqlx::query("DELETE FROM fido2_credentials WHERE id = $1")
            .bind(self.id)
            .execute(pool)
            .await?;

        Ok(())
    }

    /// Update credential name
    pub async fn update_name(&self, pool: &PgPool, name: &str) -> Result<()> {
        sqlx::query("UPDATE fido2_credentials SET name = $1 WHERE id = $2")
            .bind(name)
            .bind(self.id)
            .execute(pool)
            .await?;

        Ok(())
    }
}
