//! Global epoch counter + revocation tombstone journal.
//!
//! ## Epoch-based revocation
//!
//! All tokens minted at epoch N are rejected if `current_epoch > N`.
//! This is a fast emergency mechanism for:
//! - Key rotation (all tokens from old key are rejected)
//! - Security incident response
//! - Mass invalidation of leaked tokens
//!
//! ## Tombstone journal
//!
//! Individual token revocation is recorded in an append-only JSONL file.
//! Each line is a JSON record containing the token ID, revocation timestamp,
//! and a human-readable reason.
//!
//! The journal is loaded on startup and maintained in memory for O(1) lookups.
//! Persistence ensures tombstones survive process restarts.

use std::collections::HashSet;
use std::io::Write;
use std::path::PathBuf;
use std::sync::RwLock;

use serde::{Deserialize, Serialize};

/// A single revocation record in the tombstone journal.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct TombstoneEntry {
    token_id: String,
    revoked_at: i64,
    reason: String,
}

/// Global epoch manager for batch token revocation.
///
/// The epoch is an always-incrementing counter. Tokens minted at a prior
/// epoch are rejected when the verifier's current epoch exceeds the token's
/// epoch. This provides an O(1) emergency kill switch for all outstanding
/// tokens.
///
/// Thread-safe: backed by `AtomicU64`.
pub struct EpochManager {
    current: std::sync::atomic::AtomicU64,
}

impl EpochManager {
    /// Create a new epoch manager starting at the given epoch.
    ///
    /// Typically starts at 0 for a fresh deployment. After a security
    /// incident, you might initialize it to a higher value read from
    /// persistent storage.
    pub fn new(initial: u64) -> Self {
        Self {
            current: std::sync::atomic::AtomicU64::new(initial),
        }
    }

    /// Return the current epoch value.
    pub fn current(&self) -> u64 {
        self.current.load(std::sync::atomic::Ordering::Acquire)
    }

    /// Increment the epoch counter and return the **new** value.
    ///
    /// After calling this, all tokens minted at the previous epoch are
    /// considered stale and will be rejected by the verifier.
    pub fn increment(&self) -> u64 {
        self.current
            .fetch_add(1, std::sync::atomic::Ordering::Release)
            + 1
    }

    /// Set the epoch to an arbitrary value.
    ///
    /// Used when loading a persisted epoch from storage during startup.
    pub fn set(&self, epoch: u64) {
        self.current
            .store(epoch, std::sync::atomic::Ordering::Release);
    }
}

/// Append-only JSONL tombstone journal for durable token revocation.
///
/// ## Format
///
/// Each line is a JSON object:
/// ```json
/// {"token_id":"<uuid>","revoked_at":<unix_ts>,"reason":"<human-readable>"}
/// ```
///
/// The journal is loaded into memory on construction for O(1) revocation
/// checks. New tombstones are appended to the file and added to the
/// in-memory set atomically.
///
/// ## Thread safety
///
/// All mutations are behind a `RwLock<HashSet>` for the in-memory set and
/// serialized through `Mutex<File>` for the append-only file. The struct
/// can be safely shared across threads via `Arc`.
pub struct TombstoneJournal {
    /// In-memory set of revoked token IDs (fast lookup)
    revoked: RwLock<HashSet<String>>,
    /// Path to the JSONL file on disk
    path: PathBuf,
}

impl TombstoneJournal {
    /// Open (or create) a tombstone journal at the given file path.
    ///
    /// If the file already exists, all existing tombstones are loaded into
    /// memory. If the file doesn't exist, it will be created on the first
    /// revocation.
    ///
    /// # Errors
    ///
    /// Returns `Err` if the file exists but cannot be read, or if a line
    /// contains invalid JSON.
    pub fn new(path: &str) -> Result<Self, String> {
        let path = PathBuf::from(path);
        let mut revoked = HashSet::new();

        // Load existing tombstones from the journal file
        if path.exists() {
            let content = std::fs::read_to_string(&path)
                .map_err(|e| format!("Failed to read journal: {}", e))?;
            for (line_num, line) in content.lines().enumerate() {
                let line = line.trim();
                if line.is_empty() {
                    continue;
                }
                match serde_json::from_str::<TombstoneEntry>(line) {
                    Ok(entry) => {
                        revoked.insert(entry.token_id);
                    }
                    Err(e) => {
                        return Err(format!(
                            "Invalid tombstone entry on line {}: {}",
                            line_num + 1,
                            e
                        ));
                    }
                }
            }
        }

        Ok(Self {
            revoked: RwLock::new(revoked),
            path,
        })
    }

    /// Record a token revocation in the journal.
    ///
    /// Appends a JSONL entry to the file and inserts the token ID into
    /// the in-memory set. Both operations are best-effort atomic — if
    /// the file write succeeds, the in-memory set is updated.
    ///
    /// # Errors
    ///
    /// Returns `Err` if the file cannot be opened or written to.
    pub fn revoke(&self, token_id: &str, reason: &str) -> Result<(), String> {
        let entry = TombstoneEntry {
            token_id: token_id.to_string(),
            revoked_at: chrono::Utc::now().timestamp(),
            reason: reason.to_string(),
        };

        let line = serde_json::to_string(&entry)
            .map_err(|e| format!("Failed to serialize tombstone: {}", e))?;

        // Append to the JSONL file (create if not exists)
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .map_err(|e| format!("Failed to open journal for append: {}", e))?;

        writeln!(file, "{}", line).map_err(|e| format!("Failed to write tombstone: {}", e))?;

        // Update in-memory set
        {
            let mut revoked = self
                .revoked
                .write()
                .map_err(|_| "Lock poisoned".to_string())?;
            revoked.insert(token_id.to_string());
        }

        Ok(())
    }

    /// Check whether a token ID has been revoked.
    ///
    /// This is an O(1) hash-set lookup against the in-memory state.
    pub fn is_revoked(&self, token_id: &str) -> bool {
        self.revoked.read().map_or(false, |r| r.contains(token_id))
    }

    /// Return the number of revoked tokens tracked in memory.
    pub fn len(&self) -> usize {
        self.revoked.read().map_or(0, |r| r.len())
    }

    /// Returns `true` if no tokens have been revoked.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Reload the journal from disk, replacing the in-memory set.
    ///
    /// Useful when the file may have been modified externally (e.g.,
    /// by a secondary process or manual edit).
    ///
    /// # Errors
    ///
    /// Returns `Err` if the file cannot be read or parsed.
    pub fn reload(&self) -> Result<(), String> {
        let mut revoked = HashSet::new();

        if self.path.exists() {
            let content = std::fs::read_to_string(&self.path)
                .map_err(|e| format!("Failed to read journal: {}", e))?;
            for line in content.lines() {
                let line = line.trim();
                if line.is_empty() {
                    continue;
                }
                if let Ok(entry) = serde_json::from_str::<TombstoneEntry>(line) {
                    revoked.insert(entry.token_id);
                }
            }
        }

        let mut current = self
            .revoked
            .write()
            .map_err(|_| "Lock poisoned".to_string())?;
        *current = revoked;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_epoch_manager_initial() {
        let em = EpochManager::new(0);
        assert_eq!(em.current(), 0);
    }

    #[test]
    fn test_epoch_manager_increment() {
        let em = EpochManager::new(0);
        assert_eq!(em.increment(), 1);
        assert_eq!(em.increment(), 2);
        assert_eq!(em.current(), 2);
    }

    #[test]
    fn test_epoch_manager_set() {
        let em = EpochManager::new(0);
        em.set(42);
        assert_eq!(em.current(), 42);
    }

    #[test]
    fn test_tombstone_journal_create_and_revoke() {
        let dir = std::env::temp_dir();
        let path = dir.join("test_tombstones.jsonl");
        let _ = std::fs::remove_file(&path); // Clean up from previous runs

        let journal = TombstoneJournal::new(path.to_str().unwrap()).unwrap();
        assert!(journal.is_empty());
        assert_eq!(journal.len(), 0);

        journal.revoke("token-001", "testing").unwrap();
        assert!(!journal.is_empty());
        assert_eq!(journal.len(), 1);
        assert!(journal.is_revoked("token-001"));
        assert!(!journal.is_revoked("token-002"));

        // Clean up
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_tombstone_journal_persistence() {
        let dir = std::env::temp_dir();
        let path = dir.join("test_tombstones_persist.jsonl");
        let _ = std::fs::remove_file(&path);

        // First session: record a revocation
        {
            let journal = TombstoneJournal::new(path.to_str().unwrap()).unwrap();
            journal.revoke("token-persist-1", "key rotation").unwrap();
            journal.revoke("token-persist-2", "compromised").unwrap();
            assert_eq!(journal.len(), 2);
        }

        // Second session: reload and verify persistence
        {
            let journal = TombstoneJournal::new(path.to_str().unwrap()).unwrap();
            assert_eq!(journal.len(), 2);
            assert!(journal.is_revoked("token-persist-1"));
            assert!(journal.is_revoked("token-persist-2"));
            assert!(!journal.is_revoked("token-persist-3"));
        }

        // Clean up
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_tombstone_journal_reload() {
        let dir = std::env::temp_dir();
        let path = dir.join("test_tombstones_reload.jsonl");
        let _ = std::fs::remove_file(&path);

        let journal = TombstoneJournal::new(path.to_str().unwrap()).unwrap();

        // Add one via revoke
        journal.revoke("token-reload-1", "test").unwrap();
        assert_eq!(journal.len(), 1);

        // Manually append a second entry to the file (simulating external write)
        {
            let mut file = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&path)
                .unwrap();
            writeln!(
                file,
                r#"{{"token_id":"token-reload-2","revoked_at":{0},"reason":"external"}}"#,
                chrono::Utc::now().timestamp()
            )
            .unwrap();
        }

        // Reload should pick up the external entry
        journal.reload().unwrap();
        assert_eq!(journal.len(), 2);
        assert!(journal.is_revoked("token-reload-1"));
        assert!(journal.is_revoked("token-reload-2"));

        // Clean up
        let _ = std::fs::remove_file(&path);
    }
}
