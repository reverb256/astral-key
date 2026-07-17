//! mosaic-client — Shared Mosaic client library.
//!
//! Provides an HTTP client for the Mosaic Identity Service (MIS) REST API
//! and shared types used across transport bridge daemons.
//!
//! # Usage
//!
//! ```rust,no_run
//! use mosaic_client::MosaicClient;
//!
//! # async fn example() -> Result<(), mosaic_client::Error> {
//! let client = MosaicClient::new("http://mosaic-identity:8080".parse().unwrap());
//! let key = client.generate_key().await?;
//! println!("{:?}", key);
//! # Ok(())
//! # }
//! ```

use serde::{Deserialize, Serialize};
use std::time::Duration;

// ─── Error type ─────────────────────────────────────────────────────────────

/// Errors that can occur when communicating with the Mosaic Identity Service.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// HTTP request failed (network, TLS, timeout)
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    /// The MIS returned a non-success status code
    #[error("MIS returned {status}: {body}")]
    Mis {
        /// HTTP status code
        status: reqwest::StatusCode,
        /// Response body (truncated)
        body: String,
    },

    /// URL parsing error
    #[error("Invalid URL: {0}")]
    Url(#[from] url::ParseError),
}

// ─── Response types ─────────────────────────────────────────────────────────

/// Response from GET /health.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthResponse {
    pub status: String,
}

/// Response from POST /keys/generate.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyGenerateResponse {
    pub key_id: String,
    pub pubkey_hex: String,
}

/// Key info from GET /keys or GET /keys/{key_id}.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyInfo {
    pub key_id: String,
    pub pubkey_hex: String,
    pub algorithm: Option<String>,
    pub created_at: Option<String>,
}

/// Response from POST /sign.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignResponse {
    pub signature_hex: String,
}

/// Response from POST /verify.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerifyResponse {
    pub valid: bool,
}

/// A single identity binding entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Binding {
    pub protocol: String,
    pub external_id: String,
    pub bound_at: Option<String>,
}

/// Response from GET /keys/{key_id}/bindings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BindingsResponse {
    pub bindings: Vec<Binding>,
}

/// Response from GET /resolve.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolveBindingResponse {
    pub key_id: String,
    pub pubkey_hex: String,
    pub algorithm: Option<String>,
}

/// Response from POST /nostr/resolve.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NpubResolveResponse {
    pub npub: String,
    pub hex_pubkey: String,
    pub algorithm: String,
    pub is_valid: bool,
}

// ─── Client ─────────────────────────────────────────────────────────────────

/// HTTP client for the Mosaic Identity Service.
#[derive(Debug, Clone)]
pub struct MosaicClient {
    base_url: url::Url,
    client: reqwest::Client,
}

impl MosaicClient {
    /// Create a new client pointing at the given MIS base URL.
    pub fn new(base_url: url::Url) -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .expect("Failed to build HTTP client");
        Self { base_url, client }
    }

    /// Create a new client from a string URL.
    pub fn from_url(url: &str) -> Result<Self, Error> {
        Ok(Self::new(url.parse()?))
    }

    /// Create a client from the MIS_URL environment variable,
    /// falling back to `http://localhost:8081`.
    pub fn from_env() -> Self {
        let url = std::env::var("MIS_URL")
            .unwrap_or_else(|_| "http://localhost:8081".to_string());
        Self::from_url(&url).expect("Invalid MIS_URL")
    }

    /// Create a new client with a custom reqwest Client.
    pub fn new_with_client(base_url: url::Url, client: reqwest::Client) -> Self {
        Self { base_url, client }
    }

    /// Get the base URL.
    pub fn base_url(&self) -> &url::Url {
        &self.base_url
    }

    // ─── Internal helpers ───────────────────────────────────────────────────

    /// Build a full URL for a relative path.
    fn url(&self, path: &str) -> url::Url {
        // Join the relative path onto the base URL
        let path = path.trim_start_matches('/');
        // If base URL doesn't end with /, we need to be careful
        let base_str = self.base_url.to_string();
        let full = if base_str.ends_with('/') {
            format!("{}{}", base_str, path)
        } else {
            format!("{}/{}", base_str.trim_end_matches('/'), path)
        };
        url::Url::parse(&full).expect("Failed to build MIS URL")
    }

    async fn get_raw(&self, path: &str) -> Result<reqwest::Response, Error> {
        let url = self.url(path);
        tracing::debug!("MIS GET {}", url);
        Ok(self.client.get(url).send().await?)
    }

    async fn post_raw<T: Serialize + ?Sized>(
        &self,
        path: &str,
        body: &T,
    ) -> Result<reqwest::Response, Error> {
        let url = self.url(path);
        tracing::debug!("MIS POST {}", url);
        Ok(self.client.post(url).json(body).send().await?)
    }

    async fn check_response(
        resp: reqwest::Response,
    ) -> Result<serde_json::Value, Error> {
        let status = resp.status();
        let body: serde_json::Value = resp.json().await?;
        if status.is_client_error() || status.is_server_error() {
            let msg = body
                .get("error")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown error");
            return Err(Error::Mis {
                status,
                body: msg.to_string(),
            });
        }
        Ok(body)
    }

    async fn get_json(&self, path: &str) -> Result<serde_json::Value, Error> {
        let resp = self.get_raw(path).await?;
        Self::check_response(resp).await
    }

    async fn post_json<T: Serialize + ?Sized>(
        &self,
        path: &str,
        body: &T,
    ) -> Result<serde_json::Value, Error> {
        let resp = self.post_raw(path, body).await?;
        Self::check_response(resp).await
    }

    // ─── MIS API methods ───────────────────────────────────────────────────

    /// Check MIS health.
    pub async fn health(&self) -> Result<HealthResponse, Error> {
        let val = self.get_json("/health").await?;
        Ok(serde_json::from_value(val)?)
    }

    /// Generate a new Ed25519 key pair.
    pub async fn generate_key(&self) -> Result<KeyGenerateResponse, Error> {
        let val = self.post_json("/keys/generate", &serde_json::json!({})).await?;
        Ok(serde_json::from_value(val)?)
    }

    /// List all keys.
    pub async fn list_keys(&self) -> Result<Vec<KeyInfo>, Error> {
        let val = self.get_json("/keys").await?;
        Ok(serde_json::from_value(val)?)
    }

    /// Get a specific key by ID.
    pub async fn get_key(&self, key_id: &str) -> Result<KeyInfo, Error> {
        let val = self.get_json(&format!("/keys/{}", urlencoding(key_id))).await?;
        Ok(serde_json::from_value(val)?)
    }

    /// Get bindings for a key.
    pub async fn get_key_bindings(&self, key_id: &str) -> Result<BindingsResponse, Error> {
        let val = self
            .get_json(&format!("/keys/{}/bindings", urlencoding(key_id)))
            .await?;
        Ok(serde_json::from_value(val)?)
    }

    /// Sign a message (hex-encoded) with a key.
    pub async fn sign(&self, key_id: &str, message_hex: &str) -> Result<SignResponse, Error> {
        let body = serde_json::json!({ "key_id": key_id, "message_hex": message_hex });
        let val = self.post_json("/sign", &body).await?;
        Ok(serde_json::from_value(val)?)
    }

    /// Verify a signature.
    pub async fn verify(
        &self,
        pubkey_hex: &str,
        message_hex: &str,
        signature_hex: &str,
    ) -> Result<VerifyResponse, Error> {
        let body = serde_json::json!({
            "pubkey_hex": pubkey_hex,
            "message_hex": message_hex,
            "signature_hex": signature_hex,
        });
        let val = self.post_json("/verify", &body).await?;
        Ok(serde_json::from_value(val)?)
    }

    /// Resolve an npub to its hex public key via /nostr/resolve.
    pub async fn resolve_npub(&self, npub: &str) -> Result<NpubResolveResponse, Error> {
        let body = serde_json::json!({ "npub": npub });
        let val = self.post_json("/nostr/resolve", &body).await?;
        Ok(serde_json::from_value(val)?)
    }

    /// Resolve an external identity to a Mosaic key.
    pub async fn resolve_binding(
        &self,
        protocol: &str,
        external_id: &str,
    ) -> Result<ResolveBindingResponse, Error> {
        let encoded_id = urlencoding(external_id);
        let encoded_proto = urlencoding(protocol);
        let path = format!("/resolve?protocol={}&id={}", encoded_proto, encoded_id);
        let val = self.get_json(&path).await?;
        Ok(serde_json::from_value(val)?)
    }
}

// ─── Shared types (cross-bridge) ────────────────────────────────────────────

/// A resolved atproto identity — used across atproto bridge daemons.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolvedAtprotoIdentity {
    /// The resolved DID
    pub did: String,
    /// The user's handle (from alsoKnownAs)
    pub handle: Option<String>,
    /// PDS endpoint URL
    pub pds: Option<String>,
    /// Verification method public key (multibase)
    pub signing_key: Option<String>,
    /// Verification method type (e.g. "Multikey", "EcdsaSecp256k1VerificationKey2019")
    pub signing_key_type: Option<String>,
    /// Signing key decoded to hex
    pub signing_key_hex: Option<String>,
    /// Recovery key (multibase)
    pub recovery_key: Option<String>,
    /// Recovery key decoded to hex
    pub recovery_key_hex: Option<String>,
    /// Mosaic-compatible identity summary
    pub mosaic: ResolvedMosaicInfo,
}

/// Mosaic identity summary — used in external identity bindings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolvedMosaicInfo {
    /// The DID used as external_id
    pub external_id: String,
    /// Suggested display name
    pub display_name: String,
    /// Key in Mosaic's `external:<type>:<key>` format
    pub external_pubkey: Option<String>,
}

// ─── Helpers ────────────────────────────────────────────────────────────────

fn urlencoding(s: &str) -> String {
    url::form_urlencoded::byte_serialize(s.as_bytes()).collect()
}

impl From<serde_json::Error> for Error {
    fn from(e: serde_json::Error) -> Self {
        Error::Mis {
            status: reqwest::StatusCode::OK,
            body: format!("JSON error: {}", e),
        }
    }
}
