//! mosaic-bridge-discord — Discord bot bridge.
//!
//! Connects Mosaic ↔ Discord via the Discord Gateway WebSocket and REST API.
//!
//! # Endpoints
//!
//! - `GET /health` — Health check
//! - `POST /send` — Send a message to a Discord channel
//!
//! # Environment
//!
//! - `DISCORD_BOT_TOKEN` — Discord bot token (required)
//! - `DISCORD_GUILD_ID` — Restrict to a specific guild (optional)
//! - `MIS_URL` — Mosaic Identity Service URL (default: `http://localhost:8081`)
//! - `DISCORD_PORT` — HTTP server port (default: 8086)
//! - `RUST_LOG` — Logging filter (default: `info`)

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;

use axum::{
    extract::State,
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::signal;
use tokio::sync::Mutex;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;
use tracing::{error, info, trace, warn};

// ─── Discord Gateway Constants ───────────────────────────────────────────────

/// Discord Gateway WebSocket URL (v10, JSON encoding).
const GATEWAY_URL: &str = "wss://gateway.discord.gg/?v=10&encoding=json";

/// Discord REST API base URL (v10).
const REST_API_BASE: &str = "https://discord.com/api/v10";

/// Gateway opcodes.
mod op {
    pub const DISPATCH: u8 = 0;
    pub const HEARTBEAT: u8 = 1;
    pub const IDENTIFY: u8 = 2;
    #[allow(dead_code)]
    pub const RESUME: u8 = 6; // reserved for resume reconnect logic
    pub const RECONNECT: u8 = 7;
    pub const INVALID_SESSION: u8 = 9;
    pub const HELLO: u8 = 10;
    pub const HEARTBEAT_ACK: u8 = 11;
}

/// Gateway intents.
const INTENT_GUILD_MESSAGES: u64 = 1 << 9;
const INTENT_MESSAGE_CONTENT: u64 = 1 << 15;
const DEFAULT_INTENTS: u64 = INTENT_GUILD_MESSAGES | INTENT_MESSAGE_CONTENT;

// ─── Configuration ───────────────────────────────────────────────────────────

/// Runtime configuration loaded from environment variables.
struct Config {
    token: String,
    guild_id: Option<String>,
    port: u16,
}

impl Config {
    fn from_env() -> Self {
        let token = std::env::var("DISCORD_BOT_TOKEN")
            .expect("DISCORD_BOT_TOKEN must be set");
        let guild_id = std::env::var("DISCORD_GUILD_ID").ok();
        let port = std::env::var("DISCORD_PORT")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(8086);
        Self { token, guild_id, port }
    }
}

// ─── Application State ───────────────────────────────────────────────────────

/// Shared state accessible from HTTP route handlers and the gateway task.
#[derive(Clone)]
struct AppState {
    token: String,
    guild_id: Option<String>,
    http_client: reqwest::Client,
    guild_count: Arc<AtomicU32>,
    mis_client: mosaic_client::MosaicClient,
}

// ─── Discord Gateway Payload Types ───────────────────────────────────────────

/// A raw message from the Discord Gateway.
#[derive(Debug, Deserialize)]
struct GatewayPayload {
    op: u8,
    #[serde(default)]
    d: Option<Value>,
    #[serde(default)]
    s: Option<u64>,
    #[serde(default)]
    t: Option<String>,
}

/// Data carried by the Hello (opcode 10) payload.
#[derive(Debug, Deserialize)]
struct HelloData {
    heartbeat_interval: u64,
}

/// Data carried by the Ready (t: "READY") dispatch event.
#[derive(Debug, Deserialize)]
struct ReadyData {
    user: UserData,
    session_id: String,
    guilds: Vec<UnavailableGuild>,
}

/// User info within Ready payload.
#[derive(Debug, Deserialize)]
struct UserData {
    id: String,
    username: String,
}

/// A guild listed in the Ready payload (may be unavailable during start-up).
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct UnavailableGuild {
    id: String,
}

/// Data from a MESSAGE_CREATE dispatch event.
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct MessageCreateData {
    id: String,
    channel_id: String,
    guild_id: Option<String>,
    author: MessageAuthor,
    content: String,
}

/// Author info within a message.
#[derive(Debug, Deserialize)]
struct MessageAuthor {
    id: String,
    username: String,
    global_name: Option<String>,
}

// ─── HTTP Request / Response Types ───────────────────────────────────────────

#[derive(Debug, Serialize)]
struct HealthResponse {
    status: String,
    service: String,
    guilds: u32,
}

#[derive(Debug, Deserialize)]
struct SendRequest {
    channel_id: String,
    content: String,
}

#[derive(Debug, Serialize)]
struct SendResponse {
    ok: bool,
    message_id: Option<String>,
    error: Option<String>,
}

#[derive(Debug, Serialize)]
struct ErrorResponse {
    error: String,
}

// ─── HTTP Route Handlers ─────────────────────────────────────────────────────

/// `GET /health` — liveness and readiness probe.
async fn health(State(state): State<AppState>) -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok".into(),
        service: "discord-bridge".into(),
        guilds: state.guild_count.load(Ordering::Relaxed),
    })
}

/// `POST /send` — send a message to a Discord channel via the REST API.
///
/// Accepts `{ "channel_id": "...", "content": "..." }` and forwards
/// it as a bot message to the given Discord channel.
async fn send_message(
    State(state): State<AppState>,
    Json(req): Json<SendRequest>,
) -> Result<Json<SendResponse>, (StatusCode, Json<ErrorResponse>)> {
    let url = format!("{}/channels/{}/messages", REST_API_BASE, req.channel_id);

    let resp = match state
        .http_client
        .post(&url)
        .header("Authorization", format!("Bot {}", state.token))
        .header("Content-Type", "application/json")
        .json(&serde_json::json!({ "content": req.content }))
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => {
            error!("HTTP request to Discord REST API failed: {}", e);
            return Err((
                StatusCode::BAD_GATEWAY,
                Json(ErrorResponse {
                    error: format!("Discord API request failed: {}", e),
                }),
            ));
        }
    };

    let status = resp.status();
    let body: Value = resp.json().await.unwrap_or_default();

    if status.is_success() {
        let message_id = body["id"].as_str().map(|s| s.to_string());
        info!(
            "Sent message to channel {} (message_id={:?})",
            req.channel_id, message_id
        );
        Ok(Json(SendResponse {
            ok: true,
            message_id,
            error: None,
        }))
    } else {
        let err_msg = body["message"]
            .as_str()
            .unwrap_or("unknown Discord API error")
            .to_string();
        warn!("Discord REST API returned {}: {}", status, err_msg);
        Err((
            status,
            Json(ErrorResponse { error: err_msg }),
        ))
    }
}

// ─── Event Handling ──────────────────────────────────────────────────────────

/// Process a MESSAGE_CREATE dispatch event: log it and attempt MIS user resolution.
async fn handle_message_create(state: &AppState, data: Value) {
    let msg: MessageCreateData = match serde_json::from_value(data) {
        Ok(m) => m,
        Err(e) => {
            warn!("Failed to parse MESSAGE_CREATE payload: {}", e);
            return;
        }
    };

    // If the bridge is restricted to a single guild, filter here
    if let Some(ref allowed_guild) = state.guild_id {
        if msg.guild_id.as_deref() != Some(allowed_guild) {
            return;
        }
    }

    let display_name = msg
        .author
        .global_name
        .as_deref()
        .unwrap_or(&msg.author.username);

    info!(
        "[MESSAGE_CREATE] {} ({}) in channel {} [guild={:?}]: {}",
        display_name, msg.author.id, msg.channel_id, msg.guild_id, msg.content
    );

    // Attempt to resolve the author's identity via the Mosaic Identity Service
    try_resolve_user(state, &msg.author.id, &msg.channel_id, &msg.guild_id).await;
}

/// Try to look up a Discord user in MIS. This is informational — the bridge
/// does not block on a failed resolution (the user may not be bound yet).
async fn try_resolve_user(
    state: &AppState,
    user_id: &str,
    _channel_id: &str,
    guild_id: &Option<String>,
) {
    let external_id = match guild_id {
        Some(gid) => format!("discord:{}:{}", gid, user_id),
        None => format!("discord:dm:{}", user_id),
    };

    match state.mis_client.resolve_binding("discord", &external_id).await {
        Ok(binding) => {
            info!(
                "Resolved Discord user {} → Mosaic key {} (pubkey: {})",
                user_id, binding.key_id, binding.pubkey_hex
            );
        }
        Err(e) => {
            trace!(
                "No MIS binding for Discord user {} ({}): {}",
                user_id,
                external_id,
                e
            );
        }
    }
}

// ─── Gateway WebSocket Connection ────────────────────────────────────────────

/// Read and parse the next Gateway payload from the WebSocket stream.
///
/// Handles WebSocket framing (Text, Ping, Pong, Close, Binary) transparently
/// and returns the parsed `GatewayPayload`.
async fn receive_payload<S>(read: &mut S) -> Result<GatewayPayload, String>
where
    S: StreamExt<Item = Result<Message, tokio_tungstenite::tungstenite::Error>> + Unpin,
{
    loop {
        match read.next().await {
            Some(Ok(Message::Text(text))) => {
                return serde_json::from_str(&text)
                    .map_err(|e| format!("Failed to parse Gateway payload: {}", e));
            }
            Some(Ok(Message::Ping(data))) => {
                // tokio-tungstenite auto-pongs; just log
                trace!("Received WebSocket ping ({} bytes)", data.len());
            }
            Some(Ok(Message::Pong(_))) => {
                trace!("Received WebSocket pong");
            }
            Some(Ok(Message::Close(frame))) => {
                return Err(format!("WebSocket closed: {:?}", frame));
            }
            Some(Ok(Message::Binary(_))) => {
                trace!("Ignored unexpected binary frame");
            }
            Some(Ok(Message::Frame(_))) => {}
            Some(Err(e)) => {
                return Err(format!("WebSocket error: {}", e));
            }
            None => {
                return Err("WebSocket stream ended".into());
            }
        }
    }
}

/// Run a single Gateway session through its full lifecycle.
///
/// 1. Connect to the Gateway WebSocket
/// 2. Receive Hello and start heartbeating
/// 3. Send Identify with intents
/// 4. Process dispatch events until disconnect or error
async fn run_gateway_session(state: &AppState) -> Result<(), String> {
    info!("Connecting to Discord Gateway...");
    let (ws, _) = connect_async(GATEWAY_URL)
        .await
        .map_err(|e| format!("Failed to connect to Gateway: {}", e))?;
    info!("WebSocket connected to Discord Gateway");

    let (mut write, mut read) = ws.split();

    // Shared sequence number — updated on each dispatch event, sent with heartbeats.
    let seq = Arc::new(Mutex::new(None::<u64>));

    // ── Step 1: Wait for Hello ──────────────────────────────────────────────
    let hello_payload = receive_payload(&mut read).await?;
    if hello_payload.op != op::HELLO {
        return Err(format!("Expected Hello (op 10), got op {}", hello_payload.op));
    }
    let hello_data: HelloData = serde_json::from_value(
        hello_payload
            .d
            .ok_or("Hello payload missing data field")?,
    )
    .map_err(|e| format!("Failed to parse Hello data: {}", e))?;
    info!(
        "Received Hello — heartbeat interval: {} ms",
        hello_data.heartbeat_interval
    );

    // ── Step 2: Send Identify ───────────────────────────────────────────────
    let identify = serde_json::json!({
        "op": op::IDENTIFY,
        "d": {
            "token": state.token,
            "intents": DEFAULT_INTENTS,
            "properties": {
                "os": "linux",
                "browser": "mosaic-bridge-discord",
                "device": "mosaic-bridge-discord"
            }
        }
    });
    write
        .send(Message::Text(identify.to_string()))
        .await
        .map_err(|e| format!("Failed to send Identify: {}", e))?;
    info!("Sent Identify");

    // ── Step 3: Heartbeat interval timer ────────────────────────────────────
    let hb_seq = seq.clone();
    let mut heartbeat = tokio::time::interval(Duration::from_millis(hello_data.heartbeat_interval));
    heartbeat.tick().await; // consume the immediate first tick

    // ── Step 4: Event loop (poll heartbeat timer + WebSocket concurrently) ──
    loop {
        tokio::select! {
            biased; // process heartbeats first to keep gateway alive

            _ = heartbeat.tick() => {
                let current_seq = { *hb_seq.lock().await };
                let hb = serde_json::json!({
                    "op": op::HEARTBEAT,
                    "d": current_seq
                });
                if write.send(Message::Text(hb.to_string())).await.is_err() {
                    return Err("Heartbeat send failed".into());
                }
                trace!("Heartbeat sent (seq={:?})", current_seq);
            }

            payload = receive_payload(&mut read) => {
                let payload = payload?;

                match payload.op {
                    op::DISPATCH => {
                        // Track sequence number for heartbeats
                        if let Some(s) = payload.s {
                            *seq.lock().await = Some(s);
                        }

                        let event_type = payload.t.as_deref().unwrap_or("unknown");
                        trace!("Dispatch event: {} (seq={:?})", event_type, payload.s);

                        match event_type {
                            "READY" => {
                                if let Some(d) = payload.d {
                                    match serde_json::from_value::<ReadyData>(d) {
                                        Ok(ready) => {
                                            let guild_count = ready.guilds.len() as u32;
                                            state.guild_count.store(guild_count, Ordering::Relaxed);
                                            info!(
                                                "Ready! Logged in as {} ({}) — {} guilds, session_id: {}",
                                                ready.user.username,
                                                ready.user.id,
                                                guild_count,
                                                ready.session_id,
                                            );
                                        }
                                        Err(e) => {
                                            warn!("Failed to parse Ready event: {}", e);
                                        }
                                    }
                                }
                            }

                            "MESSAGE_CREATE" => {
                                if let Some(d) = payload.d {
                                    handle_message_create(state, d).await;
                                }
                            }

                            _ => {
                                trace!("Unhandled dispatch event: {}", event_type);
                            }
                        }
                    }

                    op::HEARTBEAT_ACK => {
                        trace!("Heartbeat ACK received");
                    }

                    op::RECONNECT => {
                        warn!("Gateway requested reconnect (opcode 7)");
                        return Err("Reconnect requested by gateway".into());
                    }

                    op::INVALID_SESSION => {
                        let resumable = payload.d.and_then(|v| v.as_bool()).unwrap_or(false);
                        if resumable {
                            warn!("Invalid session (resumable) — reconnecting");
                            return Err("Invalid session (resumable)".into());
                        } else {
                            error!("Invalid session (not resumable) — full re-identify needed");
                            return Err("Invalid session (not resumable)".into());
                        }
                    }

                    other => {
                        trace!("Unhandled opcode: {}", other);
                    }
                }
            }
        }
    }
}

/// Gateway connection supervisor.
///
/// Runs the gateway session in a loop with exponential backoff (1s → 30s max)
/// so that transient failures (network blips, Discord restarts) are handled
/// automatically.
async fn gateway_supervisor(state: AppState) {
    let mut backoff_secs = 1u64;

    loop {
        let result = run_gateway_session(&state).await;

        match &result {
            Ok(()) => {
                info!("Gateway session ended cleanly — reconnecting");
                backoff_secs = 1;
            }
            Err(e) => {
                warn!("Gateway session failed: {}", e);
            }
        }

        let delay = Duration::from_secs(backoff_secs);
        info!("Reconnecting in {}s...", delay.as_secs());
        tokio::time::sleep(delay).await;

        if backoff_secs < 30 {
            backoff_secs = std::cmp::min(backoff_secs * 2, 30);
        }
    }
}

// ─── Router & Server ─────────────────────────────────────────────────────────

/// Build the axum router with all endpoints registered.
fn build_router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/send", post(send_message))
        .with_state(state)
}

/// Wait for a shutdown signal — SIGTERM (Unix) or Ctrl+C (cross-platform).
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

// ─── Entry Point ─────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialise structured logging
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "mosaic_bridge_discord=info".into()),
        )
        .init();

    let config = Config::from_env();

    if config.token.is_empty() {
        anyhow::bail!("DISCORD_BOT_TOKEN must not be empty");
    }

    // Build the shared HTTP client (reused for both Discord REST and MIS calls)
    let http_client = reqwest::Client::builder()
        .user_agent("mosaic-bridge-discord/1.0")
        .timeout(Duration::from_secs(15))
        .build()?;

    // Connect to MIS using the standard mosaic-client from_env helper
    let mis_client = mosaic_client::MosaicClient::from_env();

    let state = AppState {
        token: config.token,
        guild_id: config.guild_id,
        http_client,
        guild_count: Arc::new(AtomicU32::new(0)),
        mis_client,
    };

    // ── Start the Discord Gateway connection in the background ───────────────
    let gateway_handle = {
        let state = state.clone();
        tokio::spawn(async move {
            gateway_supervisor(state).await;
        })
    };

    // ── Start the HTTP server ────────────────────────────────────────────────
    let addr = format!("0.0.0.0:{}", config.port);
    info!("Starting mosaic-bridge-discord HTTP server on {}", addr);

    let listener = tokio::net::TcpListener::bind(&addr).await?;

    axum::serve(listener, build_router(state))
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    info!("HTTP server stopped — shutting down gateway task");
    gateway_handle.abort();

    Ok(())
}
