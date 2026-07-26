//! mosaic-bridge-atproto — AT Protocol DID resolver daemon.
//!
//! Bridges Mosaic identity ↔ Bluesky/AT Protocol.
//!
//! Resolves `did:plc:...` handles and DIDs via the PLC directory,
//! returning Mosaic-compatible identity summaries.
//!
//! # Endpoints
//!
//! - `GET /` — List endpoints
//! - `GET /health` — Health check
//! - `POST /resolve` — Resolve a DID or handle to a Mosaic identity
//!
//! # Environment
//!
//! - `ATPROTO_PORT` — Listening port (default: 8083)

use axum::{
    extract::State,
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use tokio::signal;
use tower_http::trace::TraceLayer;
use tracing::{info, warn};

// ─── Application state ──────────────────────────────────────────────────────

/// Shared application state.
#[derive(Clone)]
struct AppState {
    /// Reusable reqwest client for outbound HTTP calls.
    http_client: reqwest::Client,
}

impl AppState {
    fn new() -> Self {
        Self {
            http_client: reqwest::Client::builder()
                .user_agent("mosaic-atproto-bridge/1.0")
                .build()
                .expect("Failed to create reqwest Client"),
        }
    }
}

// ─── Request / response types ───────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct ResolveRequest {
    /// A DID (`did:plc:...`, `did:web:...`) or handle (`bsky.app`, `@bsky.app`)
    did_or_handle: String,
}

/// Full resolved identity response.
#[derive(Debug, Serialize)]
struct ResolveResponse {
    /// The resolved DID
    did: String,
    /// Handle extracted from `alsoKnownAs` (at:// handle)
    handle: Option<String>,
    /// PDS endpoint URL
    pds: Option<String>,
    /// Signing key (multibase-encoded, e.g. `zQ3sh...`)
    signing_key: Option<String>,
    /// Signing key decoded to hex
    signing_key_hex: Option<String>,
    /// Verification method type (e.g. "Multikey", "EcdsaSecp256k1VerificationKey2019")
    signing_key_type: Option<String>,
    /// Recovery key (multibase-encoded)
    recovery_key: Option<String>,
    /// Recovery key decoded to hex
    recovery_key_hex: Option<String>,
    /// Mosaic-compatible identity summary
    mosaic: MosaicInfo,
}

/// Mosaic identity summary stored in the external identities table.
#[derive(Debug, Serialize)]
struct MosaicInfo {
    /// The DID used as external_id
    external_id: String,
    /// Suggested display name
    display_name: String,
    /// Key in Mosaic's `external:<type>:<key>` format
    external_pubkey: Option<String>,
}

#[derive(Debug, Serialize)]
struct HealthResponse {
    status: String,
    service: String,
}

/// Generic error response.
#[derive(Debug, Serialize)]
struct ErrorResponse {
    error: String,
}

// ─── Route handlers ─────────────────────────────────────────────────────────

/// `GET /` — list available endpoints.
async fn index() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "service": "mosaic-bridge-atproto",
        "version": "0.1.0",
        "endpoints": {
            "GET /": "List available endpoints",
            "GET /health": "Health check",
            "POST /resolve": "Resolve a DID or handle to a Mosaic identity"
        }
    }))
}

/// `GET /health` — liveness probe.
async fn health() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok".into(),
        service: "atproto-bridge".into(),
    })
}

/// `POST /resolve` — resolve a DID or handle.
///
/// Accepts `{ "did_or_handle": "did:plc:..." }` or `{ "did_or_handle": "bsky.app" }`.
/// Returns a complete identity summary with Mosaic-compatible fields.
async fn resolve(
    State(state): State<AppState>,
    Json(req): Json<ResolveRequest>,
) -> Result<Json<ResolveResponse>, (StatusCode, Json<ErrorResponse>)> {
    match resolve_identity(&state.http_client, &req.did_or_handle).await {
        Ok(identity) => Ok(Json(identity)),
        Err(e) => {
            warn!("Resolve failed for '{}': {}", req.did_or_handle, e);
            Err((StatusCode::BAD_REQUEST, Json(ErrorResponse { error: e })))
        }
    }
}

// ─── DID resolution logic ───────────────────────────────────────────────────

/// Resolve a DID or handle to a complete Mosaic identity.
async fn resolve_identity(
    client: &reqwest::Client,
    input: &str,
) -> Result<ResolveResponse, String> {
    // Step 1: Normalize input and resolve to a DID string
    let did = resolve_to_did(client, input).await?;

    // Step 2: Fetch the DID document from plc.directory
    let doc = fetch_did_document(client, &did).await?;

    // Step 3: Extract handle from alsoKnownAs
    let handle = doc
        .also_known_as
        .as_ref()
        .and_then(|aka| aka.iter().find(|a| a.starts_with("at://")).cloned())
        .map(|a| a.trim_start_matches("at://").to_string());

    // Step 4: Extract PDS endpoint
    let pds = doc.service.as_ref().and_then(|services| {
        services
            .iter()
            .find(|s| s.type_ == "AtprotoPersonalDataServer")
            .map(|s| s.service_endpoint.clone())
    });

    // Step 5: Extract signing key (Multikey or legacy EcdsaSecp*)
    let vm = doc.verification_method.as_ref().and_then(|vms| {
        vms.iter()
            .find(|vm| vm.type_ == "Multikey" || vm.type_.starts_with("Ecdsa"))
    });

    let signing_key = vm.and_then(|vm| vm.public_key_multibase.clone());
    let signing_key_type = vm.map(|vm| vm.type_.clone());
    let signing_key_hex = signing_key.as_deref().and_then(multibase_to_hex);

    // Step 6: Extract recovery key (verification method with #recovery in id)
    let recovery = doc.verification_method.as_ref().and_then(|vms| {
        vms.iter().find(|vm| {
            vm.id
                .as_deref()
                .map_or(false, |id| id.contains("#recovery"))
        })
    });
    let recovery_key = recovery.and_then(|vm| vm.public_key_multibase.clone());
    let recovery_key_hex = recovery_key.as_deref().and_then(multibase_to_hex);

    // Step 7: Determine the key type prefix for Mosaic external_pubkey
    let key_type_prefix = signing_key_hex.as_deref().and_then(|hex| {
        if hex.len() >= 2 {
            // First byte of decoded multibase key determines algorithm
            match &hex[..2] {
                "e7" => Some("secp256k1"), // 0xe7 = secp256k1
                "01" => Some("secp256k1"), // 0x01 = secp256k1 (compressed)
                "ed" => Some("ed25519"),   // 0xed = Ed25519
                "00" => Some("p256"),      // 0x00 = P-256
                "02" | "03" => {
                    // Compressed SEC1-encoded key — check next byte for curve
                    if hex.len() >= 4 {
                        match &hex[2..4] {
                            "00" => Some("p256"),   // secp256r1
                            _ => Some("secp256k1"), // secp256k1
                        }
                    } else {
                        Some("unknown")
                    }
                }
                _ => Some("unknown"),
            }
        } else {
            None
        }
    });

    let external_pubkey = match (key_type_prefix, &signing_key_hex) {
        (Some(prefix), Some(hex)) => Some(format!("{}:{}", prefix, hex)),
        _ => None,
    };

    let display_name = handle.clone().unwrap_or_else(|| did.clone());

    Ok(ResolveResponse {
        did: did.clone(),
        handle,
        pds,
        signing_key,
        signing_key_hex,
        signing_key_type,
        recovery_key,
        recovery_key_hex,
        mosaic: MosaicInfo {
            external_id: did,
            display_name,
            external_pubkey,
        },
    })
}

/// Resolve a handle or DID to a DID string.
///
/// If the input starts with `did:`, it is returned as-is.
/// Otherwise, it is treated as a handle and resolved via bsky.social.
async fn resolve_to_did(client: &reqwest::Client, input: &str) -> Result<String, String> {
    let input = input.trim();

    // Already a DID — return as-is
    if input.starts_with("did:") {
        return Ok(input.to_string());
    }

    // Normalize: strip @, protocol prefixes, trailing slash
    let handle = input
        .trim_start_matches('@')
        .trim_start_matches("https://")
        .trim_start_matches("at://")
        .trim_end_matches('/')
        .to_lowercase();

    // Must contain a dot to be a valid handle
    if !handle.contains('.') {
        return Err(format!("Not a DID or handle: {}", input));
    }

    // Resolve handle via bsky.social
    let url = format!(
        "https://bsky.social/xrpc/com.atproto.identity.resolveHandle?handle={}",
        urlencode(&handle)
    );

    let resp = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("Handle resolution failed: {}", e))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("Handle resolution returned {}: {}", status, body));
    }

    let body: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("Invalid handle resolution response: {}", e))?;

    body["did"]
        .as_str()
        .map(|s| s.to_string())
        .ok_or_else(|| format!("No DID returned for handle: {}", handle))
}

/// Fetch the DID document from the PLC directory.
async fn fetch_did_document(client: &reqwest::Client, did: &str) -> Result<DidDocument, String> {
    let url = format!("https://plc.directory/{}", did);

    let resp = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("PLC directory unreachable: {}", e))?;

    if resp.status() == StatusCode::NOT_FOUND {
        return Err(format!("DID not registered: {}", did));
    }

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("PLC directory returned {}: {}", status, body));
    }

    let doc: DidDocument = resp
        .json()
        .await
        .map_err(|e| format!("Invalid DID document: {}", e))?;

    Ok(doc)
}

// ─── Multibase decoding ─────────────────────────────────────────────────────

/// Convert a multibase-encoded key to hex.
///
/// Supports:
/// - `z` prefix: base58btc (used by atproto for Multikey)
/// - `u` prefix: base64url (RFC 4648 §5)
/// - Raw hex: returned as-is
fn multibase_to_hex(mb: &str) -> Option<String> {
    if mb.is_empty() {
        return None;
    }

    let bytes: Vec<u8> = if mb.starts_with('z') {
        // base58btc — decode the payload after the 'z' prefix
        bs58::decode(&mb[1..]).into_vec().ok()?
    } else if mb.starts_with('u') {
        // base64url — decode the payload after the 'u' prefix
        use base64::Engine as _;
        base64::engine::general_purpose::URL_SAFE
            .decode(&mb[1..])
            .ok()?
    } else if mb.starts_with('f') {
        // hex (multibase prefix 'f') — decode as hex
        hex::decode(&mb[1..]).ok()?
    } else if mb.starts_with('v') {
        // base64 (RFC 4648 §4, multibase prefix 'v')
        use base64::Engine as _;
        base64::engine::general_purpose::STANDARD
            .decode(&mb[1..])
            .ok()?
    } else if mb.chars().all(|c| c.is_ascii_hexdigit()) {
        // Raw hex
        hex::decode(mb).ok()?
    } else {
        // Unknown encoding — return as-is (for non-standard formats)
        return Some(mb.to_string());
    };

    Some(hex::encode(&bytes))
}

// ─── DID document types ─────────────────────────────────────────────────────

/// A DID document as returned by plc.directory.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DidDocument {
    #[allow(dead_code)]
    id: String,
    #[serde(default)]
    also_known_as: Option<Vec<String>>,
    #[serde(default)]
    verification_method: Option<Vec<VerificationMethod>>,
    #[serde(default)]
    service: Option<Vec<Service>>,
}

/// A verification method entry in a DID document.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct VerificationMethod {
    id: Option<String>,
    #[serde(rename = "type")]
    type_: String,
    #[allow(dead_code)]
    controller: Option<String>,
    public_key_multibase: Option<String>,
}

/// A service entry in a DID document.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Service {
    #[serde(rename = "type")]
    type_: String,
    service_endpoint: String,
}

// ─── Utilities ──────────────────────────────────────────────────────────────

/// Simple URL encoding for handle characters.
fn urlencode(s: &str) -> String {
    s.replace('%', "%25")
        .replace(' ', "%20")
        .replace('@', "%40")
        .replace('#', "%23")
        .replace('&', "%26")
}

// ─── Server setup ───────────────────────────────────────────────────────────

fn build_router(state: AppState) -> Router {
    Router::new()
        .route("/", get(index))
        .route("/health", get(health))
        .route("/resolve", post(resolve))
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

/// Shutdown signal: SIGTERM (Unix) or CTRL+C (cross-platform).
async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("Failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("Failed to install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => info!("Received Ctrl+C, shutting down"),
        _ = terminate => info!("Received SIGTERM, shutting down"),
    }
}

// ─── Entry point ────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "mosaic_bridge_atproto=info,tower_http=info".into()),
        )
        .init();

    let port: u16 = std::env::var("ATPROTO_PORT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(8083u16);

    let state = AppState::new();
    let app = build_router(state);

    let addr = format!("0.0.0.0:{}", port);
    info!("Starting mosaic-bridge-atproto on {}", addr);

    let listener = tokio::net::TcpListener::bind(&addr).await?;

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    info!("Server stopped");
    Ok(())
}
