//! Astral Key - FIDO2 credential model (SQLite)

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

/// FIDO2 credential model
#[derive(Debug, Clone, Serialize, Deserialize)]
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

fn row_to_credential(r: sqlx::sqlite::SqliteRow) -> Result<Fido2Credential> {
    let row_id: &str = r.get("id");
    let row_user_id: &str = r.get("user_id");
    Ok(Fido2Credential {
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
        credential_id: r.get("credential_id"),
        public_key: r.get("public_key"),
        counter: r.get("counter"),
        transport: r.get("transport"),
        attestation_type: r.get("attestation_type"),
        created_at: parse_dt(r.get::<&str, _>("created_at"), "created_at")?,
        last_used_at: match r.get::<Option<&str>, _>("last_used_at") {
            Some(s) => Some(parse_dt(s, "last_used_at")?),
            None => None,
        },
        name: r.get("name"),
    })
}

impl Fido2Credential {
    /// Create a new credential
    pub async fn create(
        pool: &SqlitePool,
        user_id: Uuid,
        credential_id: &str,
        public_key: &str,
    ) -> Result<Self> {
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();

        sqlx::query(
            r#"INSERT INTO fido2_credentials (id, user_id, credential_id, public_key, created_at)
               VALUES (?1, ?2, ?3, ?4, ?5)"#,
        )
        .bind(&id)
        .bind(user_id.to_string())
        .bind(credential_id)
        .bind(public_key)
        .bind(&now)
        .execute(pool)
        .await?;

        Ok(Fido2Credential {
            id: Uuid::parse_str(&id).map_err(|e| {
                AuthError::Database(sqlx::Error::Protocol(format!(
                    "invalid UUID '{}': {}",
                    id, e
                )))
            })?,
            user_id,
            credential_id: credential_id.to_string(),
            public_key: public_key.to_string(),
            counter: 0,
            transport: None,
            attestation_type: None,
            created_at: Utc::now(),
            last_used_at: None,
            name: None,
        })
    }

    /// Get credential by credential ID
    pub async fn get_by_credential_id(
        pool: &SqlitePool,
        credential_id: &str,
    ) -> Result<Option<Self>> {
        let row = sqlx::query("SELECT * FROM fido2_credentials WHERE credential_id = ?1")
            .bind(credential_id)
            .fetch_optional(pool)
            .await?;

        match row {
            Some(r) => Ok(Some(row_to_credential(r)?)),
            None => Ok(None),
        }
    }

    /// Get credential by internal ID
    pub async fn get_by_id(pool: &SqlitePool, id: Uuid) -> Result<Option<Self>> {
        let row = sqlx::query("SELECT * FROM fido2_credentials WHERE id = ?1")
            .bind(id.to_string())
            .fetch_optional(pool)
            .await?;

        match row {
            Some(r) => Ok(Some(row_to_credential(r)?)),
            None => Ok(None),
        }
    }

    /// Get all credentials for a user
    pub async fn get_by_user(pool: &SqlitePool, user_id: Uuid) -> Result<Vec<Self>> {
        let rows = sqlx::query(
            "SELECT * FROM fido2_credentials WHERE user_id = ?1 ORDER BY created_at DESC",
        )
        .bind(user_id.to_string())
        .fetch_all(pool)
        .await?;

        rows.into_iter().map(row_to_credential).collect()
    }

    /// Update counter and last used timestamp
    pub async fn update_usage(&self, pool: &SqlitePool, new_counter: i64) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        sqlx::query(
            r#"UPDATE fido2_credentials SET counter = ?1, last_used_at = ?2 WHERE id = ?3"#,
        )
        .bind(new_counter)
        .bind(&now)
        .bind(self.id.to_string())
        .execute(pool)
        .await?;
        Ok(())
    }

    /// Delete credential
    pub async fn delete(&self, pool: &SqlitePool) -> Result<()> {
        sqlx::query("DELETE FROM fido2_credentials WHERE id = ?1")
            .bind(self.id.to_string())
            .execute(pool)
            .await?;
        Ok(())
    }

    /// Update credential name
    pub async fn update_name(&self, pool: &SqlitePool, name: &str) -> Result<()> {
        sqlx::query("UPDATE fido2_credentials SET name = ?1 WHERE id = ?2")
            .bind(name)
            .bind(self.id.to_string())
            .execute(pool)
            .await?;
        Ok(())
    }
}
