//! Astral Key - Configuration management
//!
//! Handles loading and validation of application configuration from environment variables
//! and configuration files.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Main application configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub server: ServerConfig,
    pub database: DatabaseConfig,
    pub redis: RedisConfig,
    pub vaultwarden: VaultwardenConfig,
    pub web3: Web3Config,
    pub fido2: Fido2Config,
    pub jwt: JwtConfig,
    pub rate_limit: RateLimitConfig,
}

/// Server configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    #[serde(default = "default_workers")]
    pub workers: usize,
}

fn default_workers() -> usize {
    num_cpus::get()
}

/// Database configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConfig {
    pub url: String,
    #[serde(default = "default_max_connections")]
    pub max_connections: u32,
    #[serde(default = "default_min_connections")]
    pub min_connections: u32,
}

fn default_max_connections() -> u32 {
    10
}

fn default_min_connections() -> u32 {
    2
}

/// Redis configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedisConfig {
    pub url: String,
    #[serde(default = "default_pool_size")]
    pub pool_size: usize,
}

fn default_pool_size() -> usize {
    10
}

/// Vaultwarden configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VaultwardenConfig {
    pub url: String,
    #[serde(rename = "admin_token_file")]
    pub admin_token_file: Option<String>,
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
    pub origin: String,
    #[serde(default = "default_attestation")]
    pub attestation: String,
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
    #[serde(rename = "secret_file")]
    pub secret_file: String,
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

/// Rate limiting configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitConfig {
    #[serde(default = "default_requests_per_minute")]
    pub requests_per_minute: u32,
    #[serde(default = "default_burst_size")]
    pub burst_size: u32,
}

fn default_requests_per_minute() -> u32 {
    60
}

fn default_burst_size() -> u32 {
    10
}

impl Config {
    /// Load configuration from environment variables
    pub fn from_env() -> anyhow::Result<Self> {
        // TODO: Implement proper configuration loading from env vars
        // This is a placeholder for initial structure

        let config = Config {
            server: ServerConfig {
                host: std::env::var("SERVER_HOST").unwrap_or_else(|_| "127.0.0.1".to_string()),
                port: std::env::var("SERVER_PORT")
                    .ok()
                    .and_then(|p| p.parse().ok())
                    .unwrap_or(8080),
                workers: std::env::var("SERVER_WORKERS")
                    .ok()
                    .and_then(|w| w.parse().ok())
                    .unwrap_or_else(num_cpus::get),
            },
            database: DatabaseConfig {
                url: std::env::var("DATABASE_URL")
                    .unwrap_or_else(|_| "postgresql://localhost/astral_key".to_string()),
                max_connections: std::env::var("DATABASE_MAX_CONNECTIONS")
                    .ok()
                    .and_then(|c| c.parse().ok())
                    .unwrap_or(10),
                min_connections: std::env::var("DATABASE_MIN_CONNECTIONS")
                    .ok()
                    .and_then(|c| c.parse().ok())
                    .unwrap_or(2),
            },
            redis: RedisConfig {
                url: std::env::var("REDIS_URL")
                    .unwrap_or_else(|_| "redis://localhost:6379".to_string()),
                pool_size: std::env::var("REDIS_POOL_SIZE")
                    .ok()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(10),
            },
            vaultwarden: VaultwardenConfig {
                url: std::env::var("VAULTWARDEN_URL")
                    .unwrap_or_else(|_| "http://localhost:8000".to_string()),
                admin_token_file: std::env::var("VAULTWARDEN_ADMIN_TOKEN_FILE").ok(),
            },
            web3: Web3Config {
                chains: default_chains(),
                rpc_endpoints: HashMap::new(),
            },
            fido2: Fido2Config {
                rp_id: std::env::var("FIDO2_RP_ID")
                    .unwrap_or_else(|_| "localhost".to_string()),
                rp_name: std::env::var("FIDO2_RP_NAME")
                    .unwrap_or_else(|_| "Astral Key".to_string()),
                origin: std::env::var("FIDO2_ORIGIN")
                    .unwrap_or_else(|_| "http://localhost:8080".to_string()),
                attestation: std::env::var("FIDO2_ATTESTATION")
                    .unwrap_or_else(|_| "indirect".to_string()),
            },
            jwt: JwtConfig {
                secret_file: std::env::var("JWT_SECRET_FILE")
                    .unwrap_or_else(|_| "/var/lib/astral-key/jwt_secret".to_string()),
                access_token_ttl: default_access_token_ttl(),
                refresh_token_ttl: default_refresh_token_ttl(),
            },
            rate_limit: RateLimitConfig {
                requests_per_minute: default_requests_per_minute(),
                burst_size: default_burst_size(),
            },
        };

        Ok(config)
    }
}
