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
    pub jit: JitConfig,
    pub oauth: OAuthConfig,
    pub oidc: OidcConfig,
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
    /// Canonical domain used when building SIWE (EIP-4361) challenge messages.
    /// Replaces the previous behaviour of trusting the raw `domain` field from the
    /// inbound HTTP request (which defaulted to `localhost`). The API proxy now
    /// sends the real hostname, but we validate it against this configured domain
    /// so a spoofed `domain` header cannot mint a challenge for an attacker origin.
    #[serde(default = "default_web3_domain")]
    pub domain: String,
}

fn default_chains() -> Vec<String> {
    vec![
        "ethereum".to_string(),
        "polygon".to_string(),
        "arbitrum".to_string(),
        "optimism".to_string(),
    ]
}

fn default_web3_domain() -> String {
    "maplespike.ca".to_string()
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

/// ZK JIT capability token configuration.
///
/// Optional — only set `JIT_ISSUER_KEY` to enable JIT token minting.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct JitConfig {
    /// Hex-encoded 32-byte Ed25519 private key (64 hex chars).
    /// When set, a `JitIssuer` is initialized in AppState.
    #[serde(default)]
    pub issuer_key_hex: Option<String>,
    /// Issuer identifier embedded in minted tokens (e.g. "ak:issuer:01").
    #[serde(default = "default_jit_issuer_id")]
    pub issuer_id: String,
    /// Default TTL in seconds for minted tokens.
    #[serde(default = "default_jit_ttl")]
    pub default_ttl: u64,
}

fn default_jit_issuer_id() -> String {
    "ak:issuer:01".to_string()
}

fn default_jit_ttl() -> u64 {
    3600 // 1 hour
}

/// OAuth provider configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthProviderConfig {
    pub client_id: String,
    pub client_secret: String,
    pub redirect_uri: String,
    pub authorize_url: String,
    pub token_url: String,
    pub user_info_url: String,
    pub scopes: String,
}

/// OAuth configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthConfig {
    /// Base URL used to build redirect URIs and state parameters.
    #[serde(default = "default_oauth_base_url")]
    pub base_url: String,
    /// GitHub OAuth provider configuration.
    #[serde(default)]
    pub github: Option<OAuthProviderConfig>,
}

fn default_oauth_base_url() -> String {
    "http://localhost:8080".to_string()
}

/// OIDC provider client registration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OidcClientConfig {
    pub client_id: String,
    pub client_secret: String,
    pub redirect_uris: Vec<String>,
}

/// OIDC provider (identity-provider side) configuration.
///
/// Astral Key can act as an OIDC **provider** so that oauth2-proxy and other
/// relying parties can authenticate users with passkeys. Enabled by setting
/// `OIDC_ENABLED=true`. The signing key is an Ed25519 seed (64 hex chars);
/// if omitted a fresh ephemeral key is generated at startup (fine for dev,
/// but production deployments MUST pin one via `OIDC_SIGNING_KEY` or
/// `OIDC_SIGNING_KEY_FILE` so JWKS stays stable across restarts).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct OidcConfig {
    pub enabled: bool,
    /// Public issuer URL, e.g. `https://auth.lan`. Must match what relying
    /// parties are configured with.
    pub issuer: String,
    /// Hex-encoded 32-byte Ed25519 seed (64 hex chars).
    pub signing_key_hex: Option<String>,
    pub clients: Vec<OidcClientConfig>,
    /// Lifetime of OIDC access tokens (seconds).
    pub access_token_ttl: u64,
    /// Lifetime of OIDC id_tokens (seconds).
    pub id_token_ttl: u64,
}

impl Default for OidcConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            issuer: "https://auth.lan".to_string(),
            signing_key_hex: None,
            clients: Vec::new(),
            access_token_ttl: 3600,
            id_token_ttl: 600,
        }
    }
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
                domain: std::env::var("ASTRAL_WEB3_DOMAIN")
                    .unwrap_or_else(|_| default_web3_domain()),
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
            jit: JitConfig {
                issuer_key_hex: std::env::var("JIT_ISSUER_KEY").ok(),
                issuer_id: std::env::var("JIT_ISSUER_ID")
                    .unwrap_or_else(|_| default_jit_issuer_id()),
                default_ttl: std::env::var("JIT_DEFAULT_TTL")
                    .ok()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or_else(default_jit_ttl),
            },
            jwt: JwtConfig {
                access_token_ttl: default_access_token_ttl(),
                refresh_token_ttl: default_refresh_token_ttl(),
            },
            oauth: OAuthConfig {
                base_url: std::env::var("OAUTH_BASE_URL")
                    .unwrap_or_else(|_| default_oauth_base_url()),
                github: Self::load_github_oauth_config(),
            },
            oidc: Self::load_oidc_config(),
        };

        Ok(config)
    }

    fn load_oidc_config() -> OidcConfig {
        let enabled = std::env::var("OIDC_ENABLED")
            .map(|v| matches!(v.as_str(), "1" | "true" | "TRUE" | "yes" | "on"))
            .unwrap_or(false);

        if !enabled {
            return OidcConfig::default();
        }

        let issuer =
            std::env::var("OIDC_ISSUER").unwrap_or_else(|_| "https://auth.lan".to_string());

        // Signing key: explicit hex env first, then key file, then ephemeral.
        let signing_key_hex = match std::env::var("OIDC_SIGNING_KEY") {
            Ok(k) if !k.is_empty() => Some(k),
            _ => std::env::var("OIDC_SIGNING_KEY_FILE")
                .ok()
                .filter(|p| !p.is_empty())
                .and_then(|p| std::fs::read_to_string(p).ok())
                .map(|s| s.trim().to_string()),
        };

        let client_id =
            std::env::var("OIDC_CLIENT_ID").unwrap_or_else(|_| "astral-key-oidc".to_string());
        let client_secret = std::env::var("OIDC_CLIENT_SECRET")
            .ok()
            .filter(|s| !s.is_empty())
            .or_else(|| {
                std::env::var("OIDC_CLIENT_SECRET_FILE")
                    .ok()
                    .and_then(|p| std::fs::read_to_string(p).ok())
                    .map(|s| s.trim().to_string())
            })
            .unwrap_or_default();
        let redirect_uris = std::env::var("OIDC_REDIRECT_URIS")
            .ok()
            .map(|s| s.split(',').map(|s| s.trim().to_string()).collect())
            .unwrap_or_else(|| vec!["https://auth.lan/oauth2/callback".to_string()]);

        OidcConfig {
            enabled,
            issuer,
            signing_key_hex,
            clients: vec![OidcClientConfig {
                client_id,
                client_secret,
                redirect_uris,
            }],
            access_token_ttl: std::env::var("OIDC_ACCESS_TOKEN_TTL")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(3600),
            id_token_ttl: std::env::var("OIDC_ID_TOKEN_TTL")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(600),
        }
    }

    fn load_github_oauth_config() -> Option<OAuthProviderConfig> {
        let client_id = std::env::var("OAUTH_GITHUB_CLIENT_ID").ok()?;
        let client_secret = std::env::var("OAUTH_GITHUB_CLIENT_SECRET").ok()?;

        Some(OAuthProviderConfig {
            client_id,
            client_secret,
            redirect_uri: std::env::var("OAUTH_GITHUB_REDIRECT_URI").unwrap_or_else(|_| {
                format!("{}/auth/oauth/github/callback", default_oauth_base_url())
            }),
            authorize_url: "https://github.com/login/oauth/authorize".into(),
            token_url: "https://github.com/login/oauth/access_token".into(),
            user_info_url: "https://api.github.com/user".into(),
            scopes: "read:user".into(),
        })
    }
}
