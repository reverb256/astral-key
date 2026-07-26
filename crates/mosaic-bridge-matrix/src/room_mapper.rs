use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use tracing::{error, info};

/// A single channel ↔ room mapping entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomMapping {
    #[serde(default)]
    pub room_alias: String,
    /// Matrix room ID (null/empty until the room is created).
    #[serde(default)]
    pub room_id: Option<String>,
    #[serde(default)]
    pub created_at: String,
}

/// On-disk format for the mapping file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MappingsFile {
    pub channels: HashMap<String, RoomMapping>,
}

/// Bi-directional Mosaic channel ↔ Matrix room mapper.
///
/// Backed by a JSON file on disk for persistence across restarts.
/// All operations are held behind a `Mutex` — this is a single-threaded
/// bridge with low throughput requirements so the lock is acceptable.
pub struct RoomMapper {
    /// File path for the JSON mapping file.
    path: PathBuf,
    /// Domain for generating room aliases (e.g. `matrix.local`).
    domain: String,
    /// In-memory state protected by a mutex.
    state: Mutex<MappingsFile>,
}

impl RoomMapper {
    /// Create a new room mapper.
    ///
    /// If the mapping file already exists, it is loaded into memory.
    /// Otherwise an empty state is initialised.
    pub fn new<P: AsRef<Path>>(path: P, domain: impl Into<String>) -> Self {
        let path = path.as_ref().to_path_buf();
        let domain = domain.into();

        let state = if path.exists() {
            match fs::read_to_string(&path) {
                Ok(content) => match serde_json::from_str::<MappingsFile>(&content) {
                    Ok(m) => {
                        info!(
                            "Loaded {} room mappings from {}",
                            m.channels.len(),
                            path.display()
                        );
                        m
                    }
                    Err(e) => {
                        error!(
                            "Failed to parse mappings file {}: {}. Starting fresh.",
                            path.display(),
                            e
                        );
                        MappingsFile {
                            channels: HashMap::new(),
                        }
                    }
                },
                Err(e) => {
                    error!(
                        "Failed to read mappings file {}: {}. Starting fresh.",
                        path.display(),
                        e
                    );
                    MappingsFile {
                        channels: HashMap::new(),
                    }
                }
            }
        } else {
            // Ensure parent directory exists
            if let Some(parent) = path.parent() {
                let _ = fs::create_dir_all(parent);
            }
            let mappings = MappingsFile {
                channels: HashMap::new(),
            };
            // Write initial empty file
            Self::write_file(&path, &mappings);
            mappings
        };

        Self {
            path,
            domain,
            state: Mutex::new(state),
        }
    }

    /// Get or create a mapping for a Mosaic channel code to a Matrix room alias.
    pub fn get_room_for_alias(&self, channel_code: &str) -> RoomMapping {
        let mut state = self.state.lock().expect("RoomMapper lock poisoned");

        if let Some(entry) = state.channels.get(channel_code) {
            return entry.clone();
        }

        // Create new mapping
        let alias = format!("#mosaic-{}:{}", channel_code, self.domain);
        let entry = RoomMapping {
            room_alias: alias,
            room_id: None,
            created_at: chrono_now(),
        };

        state
            .channels
            .insert(channel_code.to_string(), entry.clone());
        Self::write_file(&self.path, &state);

        entry
    }

    /// Store/update the Matrix room ID for a given channel code.
    pub fn store_mapping(&self, channel_code: &str, room_id: &str) {
        let mut state = self.state.lock().expect("RoomMapper lock poisoned");

        let entry = state
            .channels
            .entry(channel_code.to_string())
            .or_insert_with(|| RoomMapping {
                room_alias: format!("#mosaic-{}:{}", channel_code, self.domain),
                room_id: None,
                created_at: chrono_now(),
            });
        entry.room_id = Some(room_id.to_string());

        Self::write_file(&self.path, &state);
    }

    /// Look up a Mosaic channel code from a Matrix room ID.
    pub fn get_channel_for_room(&self, room_id: &str) -> Option<String> {
        let state = self.state.lock().expect("RoomMapper lock poisoned");
        for (code, entry) in &state.channels {
            if entry.room_id.as_deref() == Some(room_id) {
                return Some(code.clone());
            }
        }
        None
    }

    /// Look up a Mosaic channel code from a Matrix room alias.
    pub fn get_channel_for_alias(&self, alias: &str) -> Option<String> {
        let state = self.state.lock().expect("RoomMapper lock poisoned");
        for (code, entry) in &state.channels {
            if entry.room_alias == alias {
                return Some(code.clone());
            }
        }
        None
    }

    /// List all channel → room mappings.
    pub fn list_mappings(&self) -> HashMap<String, RoomMapping> {
        let state = self.state.lock().expect("RoomMapper lock poisoned");
        state.channels.clone()
    }

    /// Generate a Matrix room alias for a channel code.
    pub fn alias_for_channel(&self, channel_code: &str) -> String {
        format!("#mosaic-{}:{}", channel_code, self.domain)
    }

    /// Extract a channel code from a Matrix room alias.
    pub fn channel_from_alias(alias: &str) -> Option<String> {
        // Format: #mosaic-<channel_code>:<domain>
        alias
            .strip_prefix("#mosaic-")
            .and_then(|rest| rest.split(':').next())
            .map(|s| s.to_string())
    }

    // ─── Helpers ───────────────────────────────────────────────────────────

    fn write_file(path: &Path, mappings: &MappingsFile) {
        match serde_json::to_string_pretty(mappings) {
            Ok(json) => {
                if let Err(e) = fs::write(path, &json) {
                    error!("Failed to write mappings file {}: {}", path.display(), e);
                }
            }
            Err(e) => {
                error!("Failed to serialize mappings: {}", e);
            }
        }
    }
}

/// Get the current time as an ISO 8601 string (UTC).
fn chrono_now() -> String {
    // Use system clock without pulling in chrono crate
    use std::time::{SystemTime, UNIX_EPOCH};
    let dur = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let secs = dur.as_secs();
    // Format as ISO 8601
    let days = secs / 86400;
    let time_secs = secs % 86400;
    let hours = time_secs / 3600;
    let minutes = (time_secs % 3600) / 60;
    let seconds = time_secs % 60;

    // Compute year/month/day from days since epoch (simplified)
    let (year, month, day) = days_to_date(days as i64);
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        year, month, day, hours, minutes, seconds
    )
}

/// Convert days since Unix epoch to (year, month, day).
/// Uses a simple civil date algorithm.
fn days_to_date(days: i64) -> (i64, u32, u32) {
    // Civil date algorithm
    let z = days + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y, m as u32, d as u32)
}
