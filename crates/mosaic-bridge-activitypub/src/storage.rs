//! JSON file-backed storage for followers and outbox data.
//!
//! The ActivityPub bridge persists two data sets on disk:
//!
//! - `followers.json` — list of actor IRIs that follow the Mosaic bridge actor
//! - `outbox.json` — ordered list of sent activities (as JSON Values)
//! - `keys.json` — Ed25519 seed for the actor's signing key
//!
//! All operations are protected by a `tokio::sync::RwLock` to allow
//! concurrent read access with safe writes.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

// ─── Persistent data structures ──────────────────────────────────────────────

/// A single follower record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Follower {
    /// The actor IRI (e.g. `https://mastodon.social/users/alice`)
    pub actor_id: String,
    /// When the follow was accepted (ISO 8601)
    pub accepted_at: String,
    /// The inbox URL for delivering activities
    pub inbox_url: String,
    /// The shared inbox URL (preferred for delivery, if available)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shared_inbox_url: Option<String>,
}

/// The on-disk followers list.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct FollowerList {
    followers: Vec<Follower>,
}

/// A single outbox entry — an Activity JSON Value.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct OutboxEntry {
    activity: serde_json::Value,
    published: String,
}

/// The on-disk outbox list.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct OutboxList {
    entries: Vec<OutboxEntry>,
}

/// Stored Ed25519 key material (32-byte seed hex + 32-byte public key hex).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyMaterial {
    pub seed_hex: String,
    pub public_key_hex: String,
}

// ─── ActivityPubStore ────────────────────────────────────────────────────────

/// Thread-safe, file-backed store for ActivityPub bridge data.
///
/// All mutations are serialised via `tokio::sync::RwLock`; reads are
/// concurrent. Data is persisted to JSON files on every write.
#[derive(Clone)]
pub struct ActivityPubStore {
    inner: Arc<RwLock<StoreInner>>,
    data_dir: PathBuf,
}

#[derive(Debug)]
struct StoreInner {
    followers: Vec<Follower>,
    outbox: Vec<serde_json::Value>,
}

impl ActivityPubStore {
    /// Create a new store backed by the given data directory.
    ///
    /// Attempts to load existing state from `followers.json`, `outbox.json`;
    /// missing files are treated as empty state.
    pub async fn load(data_dir: &std::path::Path) -> Result<Self> {
        tokio::fs::create_dir_all(data_dir)
            .await
            .context("Failed to create data directory")?;

        let followers_path = data_dir.join("followers.json");
        let outbox_path = data_dir.join("outbox.json");

        let followers = if followers_path.exists() {
            let data = tokio::fs::read_to_string(&followers_path)
                .await
                .context("Failed to read followers.json")?;
            let list: FollowerList =
                serde_json::from_str(&data).context("Failed to parse followers.json")?;
            list.followers
        } else {
            Vec::new()
        };

        let outbox = if outbox_path.exists() {
            let data = tokio::fs::read_to_string(&outbox_path)
                .await
                .context("Failed to read outbox.json")?;
            let list: OutboxList =
                serde_json::from_str(&data).context("Failed to parse outbox.json")?;
            list.entries.into_iter().map(|e| e.activity).collect()
        } else {
            Vec::new()
        };

        tracing::info!(
            "Store loaded: {} followers, {} outbox entries",
            followers.len(),
            outbox.len()
        );

        Ok(Self {
            inner: Arc::new(RwLock::new(StoreInner { followers, outbox })),
            data_dir: data_dir.to_path_buf(),
        })
    }

    /// Persist followers to disk.
    async fn save_followers(&self, followers: &[Follower]) -> Result<()> {
        let path = self.data_dir.join("followers.json");
        let list = FollowerList {
            followers: followers.to_vec(),
        };
        let data = serde_json::to_string_pretty(&list).context("Failed to serialize followers")?;
        tokio::fs::write(&path, &data)
            .await
            .context("Failed to write followers.json")?;
        Ok(())
    }

    /// Persist outbox to disk.
    async fn save_outbox(&self, entries: &[serde_json::Value]) -> Result<()> {
        let path = self.data_dir.join("outbox.json");
        let list = OutboxList {
            entries: entries
                .iter()
                .map(|activity| OutboxEntry {
                    activity: activity.clone(),
                    published: chrono::Utc::now().to_rfc3339(),
                })
                .collect(),
        };
        let data = serde_json::to_string_pretty(&list).context("Failed to serialize outbox")?;
        tokio::fs::write(&path, &data)
            .await
            .context("Failed to write outbox.json")?;
        Ok(())
    }

    // ─── Follower operations ───────────────────────────────────────────────

    /// Get a copy of all followers.
    pub async fn get_followers(&self) -> Vec<Follower> {
        self.inner.read().await.followers.clone()
    }

    /// Get the count of followers.
    pub async fn follower_count(&self) -> usize {
        self.inner.read().await.followers.len()
    }

    /// Check if an actor is already following.
    pub async fn is_follower(&self, actor_id: &str) -> bool {
        let guard = self.inner.read().await;
        guard.followers.iter().any(|f| f.actor_id == actor_id)
    }

    /// Add a follower. Persists to disk.
    pub async fn add_follower(
        &self,
        actor_id: &str,
        inbox_url: &str,
        shared_inbox: Option<&str>,
    ) -> Result<bool> {
        let mut guard = self.inner.write().await;
        // Idempotent — no-op if already a follower
        if guard.followers.iter().any(|f| f.actor_id == actor_id) {
            return Ok(false);
        }
        guard.followers.push(Follower {
            actor_id: actor_id.to_string(),
            accepted_at: chrono::Utc::now().to_rfc3339(),
            inbox_url: inbox_url.to_string(),
            shared_inbox_url: shared_inbox.map(String::from),
        });
        self.save_followers(&guard.followers).await?;
        tracing::info!("Follower added: {}", actor_id);
        Ok(true)
    }

    /// Remove a follower. Persists to disk.
    pub async fn remove_follower(&self, actor_id: &str) -> Result<bool> {
        let mut guard = self.inner.write().await;
        let len_before = guard.followers.len();
        guard.followers.retain(|f| f.actor_id != actor_id);
        let removed = guard.followers.len() < len_before;
        if removed {
            self.save_followers(&guard.followers).await?;
            tracing::info!("Follower removed: {}", actor_id);
        }
        Ok(removed)
    }

    // ─── Outbox operations ──────────────────────────────────────────────────

    /// Get a copy of all outbox entries (most recent first).
    pub async fn get_outbox(&self) -> Vec<serde_json::Value> {
        let guard = self.inner.read().await;
        let mut entries = guard.outbox.clone();
        entries.reverse();
        entries
    }

    /// Get the count of outbox entries.
    pub async fn outbox_count(&self) -> usize {
        self.inner.read().await.outbox.len()
    }

    /// Append an activity to the outbox. Persists to disk.
    pub async fn add_to_outbox(&self, activity: serde_json::Value) -> Result<()> {
        let mut guard = self.inner.write().await;
        guard.outbox.push(activity);
        self.save_outbox(&guard.outbox).await?;
        Ok(())
    }

    // ─── Key storage ────────────────────────────────────────────────────────

    /// Load Ed25519 key material from `keys.json`, or return `None`.
    pub async fn load_key_material(data_dir: &std::path::Path) -> Result<Option<KeyMaterial>> {
        let path = data_dir.join("keys.json");
        if !path.exists() {
            return Ok(None);
        }
        let data = tokio::fs::read_to_string(&path)
            .await
            .context("Failed to read keys.json")?;
        let km: KeyMaterial = serde_json::from_str(&data).context("Failed to parse keys.json")?;
        Ok(Some(km))
    }

    /// Save Ed25519 key material to `keys.json`.
    pub async fn save_key_material(data_dir: &std::path::Path, km: &KeyMaterial) -> Result<()> {
        let path = data_dir.join("keys.json");
        let data = serde_json::to_string_pretty(km).context("Failed to serialize key material")?;
        tokio::fs::write(&path, &data)
            .await
            .context("Failed to write keys.json")?;
        Ok(())
    }
}
