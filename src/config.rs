//! Astral Key - Configuration management
//!
//! Handles loading of application configuration from environment variables.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Main application configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub server: ServerConfig,
    pub database: DatabaseConfig,
    pub web3: Web3Config,
    pub fido2: Fido2Config,
    pub jwt: JwtConfig,
}

/// Server configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
}

/// Database configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConfig {
    pub url: String,
    #[serde(default = "default_max_connections")]
    pub max_connections: u32,
}

fn default_max_connections() -> u32 {
    5
}

/// Web3 configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Web3Config {
    #[serde(default = "default_chains")]
    pub chains: Vec<String>,
    #[serde(default)]
    pub rpc_endpoints: HashMap<String, String>,
}

fn default_chains() -> Vec<String> {
    vec![
        "ethereum".to_string(),
        "polygon".to_string(),
        "arbitrum".to_string(),
        "optimism".to_string(),
    ]
}

/// FIDO2/WebAuthn configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Fido2Config {
    #[serde(rename = "rp_id")]
    pub rp_id: String,
    #[serde(rename = "rp_name", default = "default_rp_name")]
    pub rp_name: String,
    #[serde(default = "default_origins")]
    pub origins: Vec<String>,
    #[serde(default = "default_attestation")]
    pub attestation: String,
}

fn default_origins() -> Vec<String> {
    vec!["http://localhost:8080".to_string()]
}

fn default_rp_name() -> String {
    "Astral Key".to_string()
}

fn default_attestation() -> String {
    "indirect".to_string()
}

/// JWT configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JwtConfig {
    #[serde(default = "default_access_token_ttl")]
    pub access_token_ttl: u64,
    #[serde(default = "default_refresh_token_ttl")]
    pub refresh_token_ttl: u64,
}

fn default_access_token_ttl() -> u64 {
    900 // 15 minutes
}

fn default_refresh_token_ttl() -> u64 {
    604800 // 7 days
}

impl Config {
    /// Load configuration from environment variables
    pub fn from_env() -> anyhow::Result<Self> {
        let config = Config {
            server: ServerConfig {
                host: std::env::var("SERVER_HOST").unwrap_or_else(|_| "127.0.0.1".to_string()),
                port: std::env::var("SERVER_PORT")
                    .ok()
                    .and_then(|p| p.parse().ok())
                    .unwrap_or(8080),
            },
            database: DatabaseConfig {
                url: std::env::var("DATABASE_URL")
                    .unwrap_or_else(|_| "sqlite:astral_key.db?mode=rwc".to_string()),
                max_connections: std::env::var("DATABASE_MAX_CONNECTIONS")
                    .ok()
                    .and_then(|c| c.parse().ok())
                    .unwrap_or(5),
            },
            web3: Web3Config {
                chains: default_chains(),
                rpc_endpoints: HashMap::new(),
            },
            fido2: Fido2Config {
                rp_id: std::env::var("FIDO2_RP_ID").unwrap_or_else(|_| "localhost".to_string()),
                rp_name: std::env::var("FIDO2_RP_NAME")
                    .unwrap_or_else(|_| "Astral Key".to_string()),
                origins: std::env::var("FIDO2_ORIGINS")
                    .ok()
                    .map(|s| s.split(',').map(|s| s.trim().to_string()).collect())
                    .unwrap_or_else(default_origins),
                attestation: std::env::var("FIDO2_ATTESTATION")
                    .unwrap_or_else(|_| "indirect".to_string()),
            },
            jwt: JwtConfig {
                access_token_ttl: default_access_token_ttl(),
                refresh_token_ttl: default_refresh_token_ttl(),
            },
        };

        Ok(config)
    }
}
