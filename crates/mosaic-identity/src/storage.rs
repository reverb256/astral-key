//! SQLite storage for the Mosaic Identity Service.
//!
//! Uses sqlx (same as astral-key workspace). Schema is versioned via
//! `migrations/001_init.sql`. All methods are async.

use sqlx::sqlite::SqlitePoolOptions;
use sqlx::{FromRow, SqlitePool};

use crate::error::Error;

/// Storage backend for the identity service.
#[derive(Clone)]
pub struct Storage {
    pool: SqlitePool,
}

#[derive(Debug, Clone, FromRow, serde::Serialize, serde::Deserialize)]
pub struct KeyRecord {
    pub key_id: String,
    pub pubkey_hex: String,
    /// PKCS#8 v2 private key (hex-encoded). Only present for keys we generated.
    pub privkey_pkcs8_hex: Option<String>,
    pub algorithm: String,
    pub created_at: String,
    pub rotated_from: Option<String>,
}

#[derive(Debug, Clone, FromRow, serde::Serialize, serde::Deserialize)]
pub struct BindingRecord {
    pub key_id: String,
    pub protocol: String,
    pub external_id: String,
    pub proof: Option<String>,
    pub claimed_at: String,
}

impl Storage {
    /// Open or create the database, run migrations.
    pub async fn open(database_url: &str) -> Result<Self, Error> {
        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect(database_url)
            .await?;

        let storage = Self { pool };
        storage.migrate().await?;
        Ok(storage)
    }

    async fn migrate(&self) -> Result<(), Error> {
        sqlx::raw_sql(include_str!("../migrations/001_init.sql"))
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    // ─── Keys ────────────────────────────────────────────────────────────────

    /// Insert a new key record.
    pub async fn insert_key(
        &self,
        pubkey: &str,
        privkey: Option<&str>,
        key_id: &str,
        rotated_from: Option<&str>,
    ) -> Result<(), Error> {
        sqlx::query(
            "INSERT OR IGNORE INTO keys (key_id, pubkey_hex, privkey_pkcs8_hex, algorithm, created_at, rotated_from)
             VALUES ($1, $2, $3, 'Ed25519', $4, $5)",
        )
        .bind(key_id)
        .bind(pubkey)
        .bind(privkey)
        .bind(chrono::Utc::now().to_rfc3339())
        .bind(rotated_from)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Get a key record by key_id.
    pub async fn get_key(&self, key_id: &str) -> Result<KeyRecord, Error> {
        sqlx::query_as::<_, KeyRecord>(
            "SELECT key_id, pubkey_hex, privkey_pkcs8_hex, algorithm, created_at, rotated_from
             FROM keys WHERE key_id = $1",
        )
        .bind(key_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| Error::NotFound(format!("Key not found: {}", key_id)))
    }

    /// Get key by public key hex.
    pub async fn get_key_by_pubkey(&self, pubkey_hex: &str) -> Result<KeyRecord, Error> {
        sqlx::query_as::<_, KeyRecord>(
            "SELECT key_id, pubkey_hex, privkey_pkcs8_hex, algorithm, created_at, rotated_from
             FROM keys WHERE pubkey_hex = $1",
        )
        .bind(pubkey_hex)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| Error::NotFound(format!("Key not found for pubkey: {}", pubkey_hex)))
    }

    /// List all keys (never exposes private key material).
    pub async fn list_keys(&self) -> Result<Vec<KeyRecord>, Error> {
        let rows = sqlx::query_as::<_, KeyRecord>(
            "SELECT key_id, pubkey_hex, privkey_pkcs8_hex, algorithm, created_at, rotated_from
             FROM keys ORDER BY created_at DESC",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    // ─── Bindings ────────────────────────────────────────────────────────────

    /// Insert or replace an identity binding.
    pub async fn upsert_binding(
        &self,
        key_id: &str,
        protocol: &str,
        external_id: &str,
        proof: Option<&str>,
    ) -> Result<(), Error> {
        sqlx::query(
            "INSERT OR REPLACE INTO bindings (key_id, protocol, external_id, proof, claimed_at)
             VALUES ($1, $2, $3, $4, $5)",
        )
        .bind(key_id)
        .bind(protocol)
        .bind(external_id)
        .bind(proof)
        .bind(chrono::Utc::now().to_rfc3339())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Get all bindings for a key.
    pub async fn get_bindings(&self, key_id: &str) -> Result<Vec<BindingRecord>, Error> {
        let rows = sqlx::query_as::<_, BindingRecord>(
            "SELECT key_id, protocol, external_id, proof, claimed_at
             FROM bindings WHERE key_id = $1 ORDER BY protocol",
        )
        .bind(key_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// Resolve an external protocol ID to a Mosaic key.
    pub async fn resolve_external(
        &self,
        protocol: &str,
        external_id: &str,
    ) -> Result<BindingRecord, Error> {
        sqlx::query_as::<_, BindingRecord>(
            "SELECT key_id, protocol, external_id, proof, claimed_at
             FROM bindings WHERE protocol = $1 AND external_id = $2",
        )
        .bind(protocol)
        .bind(external_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| Error::NotFound(format!("No binding for {}:{}", protocol, external_id)))
    }

    // ─── Rotations ──────────────────────────────────────────────────────────

    /// Record a key rotation.
    pub async fn insert_rotation(
        &self,
        old_key_id: &str,
        new_key_id: &str,
        cross_sig: &str,
    ) -> Result<(), Error> {
        sqlx::query(
            "INSERT INTO rotations (old_key_id, new_key_id, cross_sig, rotated_at)
             VALUES ($1, $2, $3, $4)",
        )
        .bind(old_key_id)
        .bind(new_key_id)
        .bind(cross_sig)
        .bind(chrono::Utc::now().to_rfc3339())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Get rotation history for a key.
    pub async fn get_rotation_history(
        &self,
        key_id: &str,
    ) -> Result<Vec<(String, String, String)>, Error> {
        #[derive(sqlx::FromRow)]
        struct Rotation {
            old_key_id: String,
            new_key_id: String,
            rotated_at: String,
        }

        let rows = sqlx::query_as::<_, Rotation>(
            "SELECT old_key_id, new_key_id, rotated_at
             FROM rotations WHERE old_key_id = $1 OR new_key_id = $1
             ORDER BY rotated_at",
        )
        .bind(key_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| (r.old_key_id, r.new_key_id, r.rotated_at))
            .collect())
    }
}
