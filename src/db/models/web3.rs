//! Astral Key - Web3 wallet model

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

use crate::error::Result;

/// Web3 wallet model
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
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
        pool: &PgPool,
        user_id: Uuid,
        address: &str,
        chain_id: i32,
    ) -> Result<Self> {
        let wallet = sqlx::query_as::<_, Web3Wallet>(
            r#"
            INSERT INTO web3_wallets (user_id, address, chain_id)
            VALUES ($1, $2, $3)
            RETURNING *
            "#,
        )
        .bind(user_id)
        .bind(address)
        .bind(chain_id)
        .fetch_one(pool)
        .await?;

        Ok(wallet)
    }

    /// Get wallet by address and chain
    pub async fn get_by_address_and_chain(
        pool: &PgPool,
        address: &str,
        chain_id: i32,
    ) -> Result<Option<Self>> {
        let wallet = sqlx::query_as::<_, Web3Wallet>(
            "SELECT * FROM web3_wallets WHERE address = $1 AND chain_id = $2",
        )
        .bind(address)
        .bind(chain_id)
        .fetch_optional(pool)
        .await?;

        Ok(wallet)
    }

    /// Get all wallets for a user
    pub async fn get_by_user(pool: &PgPool, user_id: Uuid) -> Result<Vec<Self>> {
        let wallets = sqlx::query_as::<_, Web3Wallet>(
            "SELECT * FROM web3_wallets WHERE user_id = $1 ORDER BY created_at DESC",
        )
        .bind(user_id)
        .fetch_all(pool)
        .await?;

        Ok(wallets)
    }

    /// Update last used timestamp
    pub async fn update_last_used(&self, pool: &PgPool) -> Result<()> {
        sqlx::query("UPDATE web3_wallets SET last_used_at = NOW() WHERE id = $1")
            .bind(self.id)
            .execute(pool)
            .await?;

        Ok(())
    }

    /// Delete wallet
    pub async fn delete(&self, pool: &PgPool) -> Result<()> {
        sqlx::query("DELETE FROM web3_wallets WHERE id = $1")
            .bind(self.id)
            .execute(pool)
            .await?;

        Ok(())
    }
}
