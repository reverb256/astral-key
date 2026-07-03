//! Astral Key - Web3 wallet model (SQLite)

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::sqlite::SqlitePool;
use sqlx::Row;
use uuid::Uuid;

use crate::error::Result;

/// Web3 wallet model
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Web3Wallet {
    pub id: Uuid,
    pub user_id: Uuid,
    pub address: String,
    pub chain_id: i32,
    pub created_at: DateTime<Utc>,
    pub last_used_at: Option<DateTime<Utc>>,
}

impl Web3Wallet {
    /// Create a new wallet for a user
    pub async fn create(
        pool: &SqlitePool,
        user_id: Uuid,
        address: &str,
        chain_id: i32,
    ) -> Result<Self> {
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();

        sqlx::query(
            r#"INSERT INTO web3_wallets (id, user_id, address, chain_id, created_at)
               VALUES (?1, ?2, ?3, ?4, ?5)"#,
        )
        .bind(&id)
        .bind(user_id.to_string())
        .bind(address)
        .bind(chain_id)
        .bind(&now)
        .execute(pool)
        .await?;

        Ok(Web3Wallet {
            id: Uuid::parse_str(&id).unwrap(),
            user_id,
            address: address.to_string(),
            chain_id,
            created_at: Utc::now(),
            last_used_at: None,
        })
    }

    /// Get wallet by address and chain
    pub async fn get_by_address_and_chain(
        pool: &SqlitePool,
        address: &str,
        chain_id: i32,
    ) -> Result<Option<Self>> {
        let row = sqlx::query(
            "SELECT * FROM web3_wallets WHERE address = ?1 AND chain_id = ?2",
        )
        .bind(address)
        .bind(chain_id)
        .fetch_optional(pool)
        .await?;

        Ok(row.map(|r| Web3Wallet {
            id: Uuid::parse_str(r.get::<&str, _>("id")).unwrap(),
            user_id: Uuid::parse_str(r.get::<&str, _>("user_id")).unwrap(),
            address: r.get("address"),
            chain_id: r.get("chain_id"),
            created_at: chrono::DateTime::parse_from_rfc3339(
                r.get::<&str, _>("created_at"),
            )
            .unwrap()
            .with_timezone(&Utc),
            last_used_at: r
                .get::<Option<&str>, _>("last_used_at")
                .map(|s| {
                    chrono::DateTime::parse_from_rfc3339(s)
                        .unwrap()
                        .with_timezone(&Utc)
                }),
        }))
    }

    /// Get all wallets for a user
    pub async fn get_by_user(pool: &SqlitePool, user_id: Uuid) -> Result<Vec<Self>> {
        let rows = sqlx::query(
            "SELECT * FROM web3_wallets WHERE user_id = ?1 ORDER BY created_at DESC",
        )
        .bind(user_id.to_string())
        .fetch_all(pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| Web3Wallet {
                id: Uuid::parse_str(r.get::<&str, _>("id")).unwrap(),
                user_id: Uuid::parse_str(r.get::<&str, _>("user_id")).unwrap(),
                address: r.get("address"),
                chain_id: r.get("chain_id"),
                created_at: chrono::DateTime::parse_from_rfc3339(
                    r.get::<&str, _>("created_at"),
                )
                .unwrap()
                .with_timezone(&Utc),
                last_used_at: r
                    .get::<Option<&str>, _>("last_used_at")
                    .map(|s| {
                        chrono::DateTime::parse_from_rfc3339(s)
                            .unwrap()
                            .with_timezone(&Utc)
                    }),
            })
            .collect())
    }

    /// Update last used timestamp
    pub async fn update_last_used(&self, pool: &SqlitePool) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        sqlx::query("UPDATE web3_wallets SET last_used_at = ?1 WHERE id = ?2")
            .bind(&now)
            .bind(self.id.to_string())
            .execute(pool)
            .await?;
        Ok(())
    }

    /// Delete wallet
    pub async fn delete(&self, pool: &SqlitePool) -> Result<()> {
        sqlx::query("DELETE FROM web3_wallets WHERE id = ?1")
            .bind(self.id.to_string())
            .execute(pool)
            .await?;
        Ok(())
    }
}
