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
    /// Stable login identifier (used as the OIDC `email` claim).
    pub email: Option<String>,
    /// Human-readable name surfaced in OIDC `name` / `preferred_username`.
    pub display_name: Option<String>,
}

/// SELECT column list shared by all user queries.
const USER_COLUMNS: &str = "id, created_at, updated_at, email, display_name";

/// Build a User from a sqlx row.
fn row_to_user(r: &sqlx::sqlite::SqliteRow) -> Result<User> {
    let row_id: &str = r.get("id");
    Ok(User {
        id: Uuid::parse_str(row_id).map_err(|e| {
            AuthError::Database(sqlx::Error::Protocol(format!(
                "invalid UUID '{}': {}",
                row_id, e
            )))
        })?,
        created_at: parse_dt(r.get::<&str, _>("created_at"), "created_at")?,
        updated_at: parse_dt(r.get::<&str, _>("updated_at"), "updated_at")?,
        email: r.get::<Option<&str>, _>("email").map(|s| s.to_string()),
        display_name: r
            .get::<Option<&str>, _>("display_name")
            .map(|s| s.to_string()),
    })
}

impl User {
    /// Create a new user (no email — legacy Web3/OAuth path).
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
            email: None,
            display_name: None,
        })
    }

    /// Create a user with an email + display name (OIDC / bootstrap path).
    ///
    /// Fails with `Conflict` if the email is already taken.
    pub async fn create_with_email(
        pool: &SqlitePool,
        id: Uuid,
        email: &str,
        display_name: &str,
    ) -> Result<Self> {
        let now = Utc::now().to_rfc3339();

        sqlx::query(
            "INSERT INTO users (id, created_at, updated_at, email, display_name) \
             VALUES (?1, ?2, ?3, ?4, ?5)",
        )
        .bind(id.to_string())
        .bind(&now)
        .bind(&now)
        .bind(email.trim().to_lowercase())
        .bind(display_name.trim())
        .execute(pool)
        .await?;

        Ok(Self {
            id,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            email: Some(email.trim().to_lowercase()),
            display_name: Some(display_name.trim().to_string()),
        })
    }

    /// Get user by ID
    pub async fn get_by_id(pool: &SqlitePool, id: Uuid) -> Result<Option<Self>> {
        let id_str = id.to_string();
        let row = sqlx::query(&format!("SELECT {USER_COLUMNS} FROM users WHERE id = ?1"))
            .bind(&id_str)
            .fetch_optional(pool)
            .await?;

        row.map(|r| row_to_user(&r)).transpose()
    }

    /// Get user by email (case-insensitive).
    pub async fn get_by_email(pool: &SqlitePool, email: &str) -> Result<Option<Self>> {
        let row = sqlx::query(&format!(
            "SELECT {USER_COLUMNS} FROM users WHERE lower(email) = lower(?1)"
        ))
        .bind(email.trim())
        .fetch_optional(pool)
        .await?;

        row.map(|r| row_to_user(&r)).transpose()
    }

    /// Count users that have at least one FIDO2 credential.
    ///
    /// Used to gate first-user bootstrap registration: once a single
    /// credentialed user exists, the open bootstrap endpoints are disabled.
    pub async fn credentialed_user_count(pool: &SqlitePool) -> Result<i64> {
        let row = sqlx::query("SELECT COUNT(DISTINCT user_id) AS n FROM fido2_credentials")
            .fetch_one(pool)
            .await?;
        Ok(row.get::<i64, _>("n"))
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
