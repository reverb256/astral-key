//! Astral Key - Contact model (pubkey-based contact graph)

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

/// Contact entry saved by a user
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Contact {
    pub id: Uuid,
    pub owner_user_id: Uuid,
    pub pubkey: String,
    pub label: Option<String>,
    pub discovered_via: String,
    pub first_seen_at: DateTime<Utc>,
    pub last_seen_at: Option<DateTime<Utc>>,
}

fn row_to_contact(r: sqlx::sqlite::SqliteRow) -> Result<Contact> {
    let row_id: &str = r.get("id");
    let row_owner_id: &str = r.get("owner_user_id");
    Ok(Contact {
        id: Uuid::parse_str(row_id).map_err(|e| {
            AuthError::Database(sqlx::Error::Protocol(format!(
                "invalid UUID '{}': {}",
                row_id, e
            )))
        })?,
        owner_user_id: Uuid::parse_str(row_owner_id).map_err(|e| {
            AuthError::Database(sqlx::Error::Protocol(format!(
                "invalid UUID '{}': {}",
                row_owner_id, e
            )))
        })?,
        pubkey: r.get("pubkey"),
        label: r.get("label"),
        discovered_via: r.get("discovered_via"),
        first_seen_at: parse_dt(r.get::<&str, _>("first_seen_at"), "first_seen_at")?,
        last_seen_at: match r.get::<Option<&str>, _>("last_seen_at") {
            Some(s) => Some(parse_dt(s, "last_seen_at")?),
            None => None,
        },
    })
}

impl Contact {
    /// Create or update a contact
    pub async fn upsert(
        pool: &SqlitePool,
        owner_user_id: Uuid,
        pubkey: &str,
        label: Option<&str>,
        discovered_via: Option<&str>,
    ) -> Result<Self> {
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        let source = discovered_via.unwrap_or("qr");

        sqlx::query(
            r#"INSERT INTO contacts (id, owner_user_id, pubkey, label, discovered_via, first_seen_at, last_seen_at)
               VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6)
               ON CONFLICT(owner_user_id, pubkey) DO UPDATE SET
                 label = excluded.label,
                 last_seen_at = excluded.last_seen_at"#,
        )
        .bind(&id)
        .bind(owner_user_id.to_string())
        .bind(pubkey)
        .bind(label)
        .bind(source)
        .bind(&now)
        .execute(pool)
        .await?;

        let row = sqlx::query("SELECT * FROM contacts WHERE owner_user_id = ?1 AND pubkey = ?2")
            .bind(owner_user_id.to_string())
            .bind(pubkey)
            .fetch_one(pool)
            .await?;

        row_to_contact(row)
    }

    /// Get all contacts for a user
    pub async fn get_by_owner(pool: &SqlitePool, owner_user_id: Uuid) -> Result<Vec<Self>> {
        let rows = sqlx::query(
            "SELECT * FROM contacts WHERE owner_user_id = ?1 ORDER BY first_seen_at DESC",
        )
        .bind(owner_user_id.to_string())
        .fetch_all(pool)
        .await?;

        rows.into_iter().map(row_to_contact).collect()
    }

    /// Delete a contact
    pub async fn delete(pool: &SqlitePool, owner_user_id: Uuid, pubkey: &str) -> Result<()> {
        sqlx::query("DELETE FROM contacts WHERE owner_user_id = ?1 AND pubkey = ?2")
            .bind(owner_user_id.to_string())
            .bind(pubkey)
            .execute(pool)
            .await?;
        Ok(())
    }
}
