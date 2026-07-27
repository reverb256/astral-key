//! mosaic-bridge-haven — Haven adapter bridge.
//!
//! Connects the Mosaic Identity Service (MIS) to a [Haven] server
//! (the self-hosted Discord alternative, a Node/Socket.IO app) so that
//! Haven accounts are bound to Mosaic Ed25519 keys and messages can be
//! identity-verified across the federation.
//!
//! # Haven wire contract (extracted from the Haven fork — see git history of
//! # the removed `identity/mosaic/server.js`)
//!
//! - Transport: **Socket.IO** (`require('socket.io')`), protocol v4 over
//!   WebSocket. Rooms are named `channel:<code>`.
//! - Server → client events of interest:
//!   - `new-message`      → `{ channelCode, message }`
//!   - `message-deleted`  → `{ channelCode, messageId }`
//!   - `play-sound`       → `{ channelCode, soundUrl, soundName }`
//!   - `kicked`           → `{ channelCode, reason }`
//! - A `user` object is attached to each authenticated socket:
//!   `{ id, username, display_name }` (resolved from the REST
//!   `Authorization: Bearer <token>` → `verifyToken`).
//! - Messages are persisted as
//!   `INSERT INTO messages (channel_id, user_id, content, ...)`.
//!
//! # Endpoints (this bridge)
//!
//! - `GET /health` — liveness probe
//! - `POST /send` — `{ "channelCode": "...", "content": "..." }` relays a
//!   message into the Haven `channel:<code>` room via the MIS-bridged user.
//!
//! # Environment
//!
//! - `HAVEN_URL`        — Haven Socket.IO endpoint (default `http://localhost:4000`)
//! - `HAVEN_TOKEN`      — Bearer token used to authenticate to Haven (required)
//! - `MIS_URL`          — Mosaic Identity Service URL (default `http://localhost:8081`)
//! - `HAVEN_PORT`       — HTTP server port (default `8087`)
//! - `RUST_LOG`         — tracing filter (default `info`)

use std::sync::Arc;

use anyhow::Result;
use axum::{
    extract::State,
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use tracing::{error, info, warn};

// ─── Configuration ─────────────────────────────────────────────────────────

struct Config {
    haven_url: String,
    haven_token: String,
    port: u16,
}

impl Config {
    fn from_env() -> Self {
        let haven_url =
            std::env::var("HAVEN_URL").unwrap_or_else(|_| "http://localhost:4000".to_string());
        let haven_token =
            std::env::var("HAVEN_TOKEN").expect("HAVEN_TOKEN must be set (Haven Bearer token)");
        let port = std::env::var("HAVEN_PORT")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(8087);
        Self {
            haven_url,
            haven_token,
            port,
        }
    }
}

// ─── Application State ─────────────────────────────────────────────────────

#[derive(Clone)]
struct AppState {
    config: Arc<Config>,
    mis_client: mosaic_client::MosaicClient,
    http_client: reqwest::Client,
}

// ─── HTTP Request / Response Types ─────────────────────────────────────────

#[derive(Debug, Serialize)]
struct HealthResponse {
    status: String,
    service: String,
}

#[derive(Debug, Deserialize)]
struct SendRequest {
    channel_code: String,
    content: String,
}

#[derive(Debug, Serialize)]
struct SendResponse {
    ok: bool,
    error: Option<String>,
}

#[derive(Debug, Serialize)]
struct ErrorResponse {
    error: String,
}

// ─── HTTP Route Handlers ───────────────────────────────────────────────────

/// `GET /health` — liveness probe.
async fn health() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok".into(),
        service: "haven-bridge".into(),
    })
}

/// `POST /send` — relay a message into a Haven channel room.
///
/// Forwards `{ "channel_code": "...", "content": "..." }` to the Haven server
/// using the configured Bearer token. The bridge authenticates as the
/// MIS-bridged Haven user; message identity is verifiable via MIS bindings.
async fn send_message(
    State(state): State<AppState>,
    Json(req): Json<SendRequest>,
) -> Result<Json<SendResponse>, (StatusCode, Json<ErrorResponse>)> {
    let url = format!(
        "{}/api/channels/{}/messages",
        state.config.haven_url, req.channel_code
    );
    let resp = match state
        .http_client
        .post(&url)
        .header(
            "Authorization",
            format!("Bearer {}", state.config.haven_token),
        )
        .header("Content-Type", "application/json")
        .json(&serde_json::json!({ "content": req.content }))
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => {
            error!("HTTP request to Haven failed: {}", e);
            return Err((
                StatusCode::BAD_GATEWAY,
                Json(ErrorResponse {
                    error: format!("Haven request failed: {e}"),
                }),
            ));
        }
    };

    if resp.status().is_success() {
        info!("Relayed message to channel {}", req.channel_code);
        Ok(Json(SendResponse {
            ok: true,
            error: None,
        }))
    } else {
        let status = resp.status();
        let body: serde_json::Value = resp.json().await.unwrap_or_default();
        let err_msg = body["error"]
            .as_str()
            .unwrap_or("unknown Haven error")
            .to_string();
        warn!("Haven returned {}: {}", status, err_msg);
        Err((status, Json(ErrorResponse { error: err_msg })))
    }
}

// ─── MIS binding helper ────────────────────────────────────────────────────

/// Resolve a Haven user (`<id>@<username>`) to a Mosaic key via MIS.
///
/// Mirrors the pattern used by the other bridges: external id is formed as
/// `haven:<user_id>` and looked up through the shared `mosaic-client`.
async fn try_resolve_haven_user(state: &AppState, haven_user_id: &str) {
    let external_id = format!("haven:{}", haven_user_id);
    match state
        .mis_client
        .resolve_binding("haven", &external_id)
        .await
    {
        Ok(binding) => info!(
            "Resolved Haven user {} → Mosaic key {} (pubkey: {})",
            haven_user_id, binding.key_id, binding.pubkey_hex
        ),
        Err(e) => tracing::trace!(
            "No MIS binding for Haven user {} ({}): {}",
            haven_user_id,
            external_id,
            e
        ),
    }
}

// ─── Socket.IO subscription (Haven event ingestion) ─────────────────────────

/// Connect to the Haven Socket.IO server and subscribe to channel events.
///
/// This is the inbound side of the adapter: it listens for `new-message`
/// events, resolves the author against MIS, and is the hook point for
/// cross-protocol routing. The Socket.IO client connection lives for the
/// lifetime of the process.
async fn run_haven_socket(state: AppState) -> Result<()> {
    use futures_util::FutureExt;
    use rust_socketio::{
        asynchronous::{Client, ClientBuilder},
        Payload,
    };

    let state_for_cb = state.clone();
    let callback = move |payload: Payload, _client: Client| {
        if let Payload::Text(values) = payload {
            // Haven emits `["new-message",{"channelCode":"...","message":{...}}]`
            // (Socket.IO packet framing). Parse the inner event array.
            if let Some(event) = values.get(0).and_then(|v| v.as_str()) {
                if event == "new-message" {
                    if let Some(msg) = values.get(1).and_then(|v| v.get("message")) {
                        let author_id = msg
                            .get("user_id")
                            .and_then(|v| v.as_i64())
                            .map(|i| i.to_string());
                        if let Some(id) = author_id {
                            let st = state_for_cb.clone();
                            tokio::spawn(async move {
                                try_resolve_haven_user(&st, &id).await;
                            });
                        }
                    }
                }
            }
        }
        async {}.boxed()
    };

    let client = ClientBuilder::new(&state.config.haven_url)
        .namespace("/")
        .on("new-message", callback)
        .connect()
        .await
        .map_err(|e| anyhow::anyhow!("Failed to connect to Haven Socket.IO: {e}"))?;

    info!("Connected to Haven Socket.IO at {}", state.config.haven_url);

    // Emit an auth event so Haven binds this socket to the configured user.
    // Haven auth is via the REST Bearer token; pass it as the socket auth
    // payload (Haven reads `Authorization` from the handshake by convention).
    let _ = client
        .emit(
            "authenticate",
            Payload::Text(vec![
                serde_json::json!({ "token": state.config.haven_token }),
            ]),
        )
        .await;

    // Keep the connection alive; the bridge HTTP server runs separately.
    tokio::signal::ctrl_c().await.ok();
    Ok(())
}

// ─── Main ───────────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();

    let config = Arc::new(Config::from_env());
    let mis_client = mosaic_client::MosaicClient::from_env();
    let http_client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()?;

    let state = AppState {
        config: config.clone(),
        mis_client,
        http_client,
    };

    // Start the HTTP server (`/health`, `/send`).
    let app = Router::new()
        .route("/health", get(health))
        .route("/send", post(send_message))
        .with_state(state.clone());

    let addr = std::net::SocketAddr::from(([0, 0, 0, 0], config.port));
    info!("Haven adapter bridge listening on {}", addr);

    // Run the Socket.IO subscriber and the HTTP server concurrently.
    let http_task = tokio::spawn(async move {
        let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
        if let Err(e) = axum::serve(listener, app).await {
            error!("HTTP server error: {}", e);
        }
    });

    let socket_task = tokio::spawn(async move { run_haven_socket(state).await });

    // Wait for either to terminate.
    tokio::select! {
        r = http_task => { if let Err(e) = r { error!("http task join error: {e}"); } }
        r = socket_task => { if let Err(e) = r { error!("socket task join error: {e}"); } }
    }

    Ok(())
}
