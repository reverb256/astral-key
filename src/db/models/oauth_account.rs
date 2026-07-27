//! Astral Key - OAuth Account model
//!
//! Links external OAuth provider accounts to internal user records.

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

/// OAuth account linking an external provider identity to an Astral Key user.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthAccount {
    pub id: Uuid,
    pub user_id: Uuid,
    pub provider: String,
    pub provider_user_id: String,
    pub email: Option<String>,
    pub name: Option<String>,
    pub avatar_url: Option<String>,
    pub access_token: Option<String>,
    pub refresh_token: Option<String>,
    pub expires_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

impl OAuthAccount {
    /// Create a new OAuth account link.
    pub async fn create(
        pool: &SqlitePool,
        user_id: Uuid,
        provider: &str,
        provider_user_id: &str,
        email: Option<&str>,
        name: Option<&str>,
        avatar_url: Option<&str>,
        access_token: Option<&str>,
        refresh_token: Option<&str>,
        expires_at: Option<&str>,
    ) -> Result<Self> {
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();

        sqlx::query(
            r#"INSERT INTO oauth_accounts
               (id, user_id, provider, provider_user_id, email, name, avatar_url,
                access_token, refresh_token, expires_at, created_at)
               VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)"#,
        )
        .bind(&id)
        .bind(user_id.to_string())
        .bind(provider)
        .bind(provider_user_id)
        .bind(email)
        .bind(name)
        .bind(avatar_url)
        .bind(access_token)
        .bind(refresh_token)
        .bind(expires_at)
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
            user_id,
            provider: provider.to_string(),
            provider_user_id: provider_user_id.to_string(),
            email: email.map(String::from),
            name: name.map(String::from),
            avatar_url: avatar_url.map(String::from),
            access_token: access_token.map(String::from),
            refresh_token: refresh_token.map(String::from),
            expires_at: expires_at.and_then(|s| {
                DateTime::parse_from_rfc3339(s)
                    .ok()
                    .map(|dt| dt.with_timezone(&Utc))
            }),
            created_at: Utc::now(),
        })
    }

    /// Find an OAuth account by provider and provider-side user ID.
    pub async fn get_by_provider_and_user_id(
        pool: &SqlitePool,
        provider: &str,
        provider_user_id: &str,
    ) -> Result<Option<Self>> {
        let row = sqlx::query(
            "SELECT * FROM oauth_accounts WHERE provider = ?1 AND provider_user_id = ?2",
        )
        .bind(provider)
        .bind(provider_user_id)
        .fetch_optional(pool)
        .await?;

        match row {
            Some(r) => Ok(Some(Self::from_row(r)?)),
            None => Ok(None),
        }
    }

    /// Find all OAuth accounts for a user.
    #[allow(dead_code)]
    pub async fn find_by_user(pool: &SqlitePool, user_id: Uuid) -> Result<Vec<Self>> {
        let rows =
            sqlx::query("SELECT * FROM oauth_accounts WHERE user_id = ?1 ORDER BY created_at DESC")
                .bind(user_id.to_string())
                .fetch_all(pool)
                .await?;

        rows.into_iter().map(|r| Self::from_row(r)).collect()
    }

    /// Delete this OAuth account link.
    #[allow(dead_code)]
    pub async fn delete(&self, pool: &SqlitePool) -> Result<()> {
        sqlx::query("DELETE FROM oauth_accounts WHERE id = ?1")
            .bind(self.id.to_string())
            .execute(pool)
            .await?;
        Ok(())
    }

    /// Build from a SQLite row.
    fn from_row(r: sqlx::sqlite::SqliteRow) -> Result<Self> {
        let row_id: &str = r.get("id");
        let row_user_id: &str = r.get("user_id");
        Ok(Self {
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
            provider: r.get("provider"),
            provider_user_id: r.get("provider_user_id"),
            email: r.get("email"),
            name: r.get("name"),
            avatar_url: r.get("avatar_url"),
            access_token: r.get("access_token"),
            refresh_token: r.get("refresh_token"),
            expires_at: match r.get::<Option<&str>, _>("expires_at") {
                Some(s) => Some(parse_dt(s, "expires_at")?),
                None => None,
            },
            created_at: parse_dt(r.get::<&str, _>("created_at"), "created_at")?,
        })
    }
}
