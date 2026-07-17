use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct Config {
    /// Listen address
    pub host: String,

    /// Listen port
    pub port: u16,

    /// Path to SQLite database file
    pub database_url: PathBuf,
}

impl Config {
    /// Load config from environment variables.
    pub fn from_env() -> Self {
        Self {
            host: std::env::var("MIS_HOST").unwrap_or_else(|_| "0.0.0.0".into()),
            port: std::env::var("MIS_PORT")
                .ok()
                .and_then(|p| p.parse().ok())
                .unwrap_or(8081),
            database_url: std::env::var("MIS_DATABASE_URL")
                .map(PathBuf::from)
                .unwrap_or_else(|_| PathBuf::from("mosaic-identity.db")),
        }
    }
}
