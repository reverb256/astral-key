//! Astral Key - Ultra-minimal Passkey + Web3 (SIWE) auth sidecar
//!
//! Standalone auth sidecar with SQLite, no Redis/PostgreSQL needed.

pub mod api;
pub mod auth;
pub mod config;
pub mod db;
pub mod error;
pub mod state;
pub mod utils;

pub use config::Config;
pub use error::{AuthError, Result};
pub use state::AppState;
