//! mosaic-bridge-telegram — Mosaic ↔ Telegram HTTP Bot API bridge.
//!
//! Polls the Telegram Bot API (`getUpdates` with long-polling) for incoming
//! messages, relays them to Mosaic, and exposes a REST API for Mosaic to send
//! messages into Telegram chats.
//!
//! # Environment
//!
//! | Variable             | Default                   | Description                       |
//! |----------------------|---------------------------|-----------------------------------|
//! | `TELEGRAM_BOT_TOKEN` | *(required)*              | Bot token from @BotFather         |
//! | `MIS_URL`            | `http://localhost:8081`    | Mosaic Identity Service base URL  |
//! | `TELEGRAM_PORT`      | `8085`                    | HTTP server port                  |
//! | `RUST_LOG`           | `info`                    | Logging filter                    |

use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use axum::{
    extract::State,
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use tokio::sync::{watch, Semaphore};
use tracing::{debug, error, info, warn};

// ─── Telegram Bot API types (subset) ───────────────────────────────────────

#[derive(Debug, Deserialize)]
struct GetUpdatesResponse {
    ok: bool,
    #[serde(default)]
    result: Vec<Update>,
}

#[derive(Debug, Deserialize)]
struct Update {
    update_id: i64,
    #[serde(default)]
    message: Option<Message>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct Message {
    message_id: i64,
    chat: Chat,
    #[serde(default)]
    from: Option<User>,
    #[serde(default)]
    text: Option<String>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct Chat {
    id: i64,
    #[serde(rename = "type")]
    chat_type: Option<String>,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    first_name: Option<String>,
    #[serde(default)]
    last_name: Option<String>,
    #[serde(default)]
    username: Option<String>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct User {
    id: i64,
    #[serde(default)]
    first_name: Option<String>,
    #[serde(default)]
    last_name: Option<String>,
    #[serde(default)]
    username: Option<String>,
    #[serde(default)]
    is_bot: bool,
}

#[derive(Debug, Deserialize)]
struct SendMessageResponse {
    ok: bool,
    #[serde(default)]
    description: Option<String>,
}

// ─── Bridge REST API types ─────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct SendRequest {
    chat_id: i64,
    text: String,
}

#[derive(Debug, Serialize)]
struct HealthResponse {
    status: String,
}

// ─── Configuration ─────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
struct Config {
    bot_token: String,
    mis_url: String,
    port: u16,
}

impl Config {
    fn from_env() -> Result<Self> {
        Ok(Self {
            bot_token: std::env::var("TELEGRAM_BOT_TOKEN")
                .context("TELEGRAM_BOT_TOKEN must be set")?,
            mis_url: std::env::var("MIS_URL")
                .unwrap_or_else(|_| "http://localhost:8081".to_string()),
            port: std::env::var("TELEGRAM_PORT")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(8085),
        })
    }
}

// ─── Rate limiter (leaky bucket, 30 msgs/sec per Telegram limits) ─────────

struct RateLimiter {
    semaphore: Semaphore,
}

impl RateLimiter {
    /// Create a rate limiter that allows up to `rate` operations per second.
    fn new(rate: usize) -> Arc<Self> {
        let limiter = Arc::new(Self {
            semaphore: Semaphore::new(rate),
        });

        // Background refill: release one permit every (1000/rate) ms.
        let r = limiter.clone();
        tokio::spawn(async move {
            let ms = 1000u64 / rate as u64;
            let mut tick = tokio::time::interval(Duration::from_millis(ms.max(1)));
            loop {
                tick.tick().await;
                r.semaphore.add_permits(1);
            }
        });

        limiter
    }

    /// Acquire one permit, blocking until available.
    async fn acquire(&self) {
        // `forget()` prevents the permit from being returned on drop.
        self.semaphore
            .acquire()
            .await
            .expect("rate-limiter semaphore closed")
            .forget();
    }
}

// ─── Telegram Bot API helpers ──────────────────────────────────────────────

fn bot_api_url(token: &str, method: &str) -> String {
    format!("https://api.telegram.org/bot{}/{}", token, method)
}

/// Send a text message to a Telegram chat. Called from the HTTP handler.
async fn send_telegram_message(
    client: &reqwest::Client,
    token: &str,
    chat_id: i64,
    text: &str,
) -> Result<()> {
    let body = serde_json::json!({ "chat_id": chat_id, "text": text });
    let resp = client
        .post(&bot_api_url(token, "sendMessage"))
        .json(&body)
        .send()
        .await
        .context("sendMessage HTTP failed")?;

    let result: SendMessageResponse = resp
        .json()
        .await
        .context("sendMessage response parse failed")?;

    if !result.ok {
        anyhow::bail!(
            "Telegram API error: {}",
            result.description.as_deref().unwrap_or("unknown")
        );
    }
    Ok(())
}

/// Fetch updates from Telegram via long-poll (30 s timeout).
async fn fetch_updates(
    client: &reqwest::Client,
    token: &str,
    offset: i64,
) -> Result<Vec<Update>> {
    let url = format!(
        "{}?timeout=30&offset={}",
        bot_api_url(token, "getUpdates"),
        offset
    );

    let resp = client
        .get(&url)
        .timeout(Duration::from_secs(35)) // slightly above Telegram's 30 s
        .send()
        .await
        .context("getUpdates HTTP failed")?;

    let data: GetUpdatesResponse = resp.json().await.context("getUpdates parse failed")?;
    if !data.ok {
        anyhow::bail!("Telegram getUpdates returned ok=false");
    }
    Ok(data.result)
}

// ─── Retry helper — exponential-ish backoff, max N attempts ────────────────

async fn retry<F, Fut, T>(f: F, max_attempts: u32, backoff: Duration) -> Result<T>
where
    F: Fn() -> Fut,
    Fut: std::future::Future<Output = Result<T>>,
{
    let mut last_err = None;
    for i in 0..max_attempts {
        match f().await {
            Ok(val) => return Ok(val),
            Err(e) => {
                warn!("Attempt {}/{} failed: {e}", i + 1, max_attempts);
                if i + 1 < max_attempts {
                    tokio::time::sleep(backoff).await;
                }
                last_err = Some(e);
            }
        }
    }
    Err(last_err.unwrap())
}

// ─── Polling loop ──────────────────────────────────────────────────────────

/// Long-poll loop: fetches updates every ~2 s, logs incoming messages, and
/// tracks the update offset to avoid re-processing.
async fn polling_loop(
    config: Config,
    client: reqwest::Client,
    mut shutdown: watch::Receiver<bool>,
) {
    let mut offset = 0i64;
    let mut tick = tokio::time::interval(Duration::from_secs(2));

    loop {
        tokio::select! {
            _ = shutdown.changed() => {
                if *shutdown.borrow() {
                    info!("Polling loop: shutting down");
                    return;
                }
            }
            _ = tick.tick() => {}
        }

        match retry(
            || fetch_updates(&client, &config.bot_token, offset),
            3,
            Duration::from_secs(5),
        )
        .await
        {
            Ok(updates) => {
                for u in &updates {
                    if u.update_id >= offset {
                        offset = u.update_id + 1;
                    }
                    if let Some(ref msg) = u.message {
                        let chat = &msg.chat;
                        let sender = msg.from.as_ref().map(|u| {
                            format!(
                                "user_id={} name=\"{}\" @{}",
                                u.id,
                                u.first_name.as_deref().unwrap_or("?"),
                                u.username.as_deref().unwrap_or("?"),
                            )
                        });
                        let text = msg.text.as_deref().unwrap_or("<non-text>");
                        info!(
                            "Telegram msg [chat={} title={:?}] from {:?}: {text}",
                            chat.id,
                            chat.title,
                            sender,
                        );
                        debug!(
                            "Full message: chat_type={:?} chat_username={:?} from_bot={}",
                            chat.chat_type, chat.username,
                            msg.from.as_ref().map(|u| u.is_bot).unwrap_or(false),
                        );
                    }
                }
            }
            Err(e) => error!("Polling error after retries: {e}"),
        }
    }
}

// ─── MIS probe ─────────────────────────────────────────────────────────────

async fn probe_mis(mis_url: String) {
    let url = format!("{}/health", mis_url.trim_end_matches('/'));
    info!("Probing MIS health at {url}...");

    let client = match reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            warn!("Failed to build MIS probe client (non-fatal): {e}");
            return;
        }
    };

    match client.get(&url).send().await {
        Ok(resp) if resp.status().is_success() => {
            info!("MIS health check OK");
        }
        Ok(resp) => warn!("MIS returned HTTP {} (non-fatal)", resp.status()),
        Err(e) => warn!("MIS unreachable (non-fatal, continuing): {e}"),
    }
}

// ─── Axum HTTP server ──────────────────────────────────────────────────────

#[derive(Clone)]
struct AppState {
    bot_token: String,
    client: reqwest::Client,
    rate_limiter: Arc<RateLimiter>,
}

/// GET /health
async fn health() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok".to_string(),
    })
}

/// POST /send — relay a message from Mosaic to a Telegram chat.
async fn send_message(
    State(st): State<AppState>,
    Json(req): Json<SendRequest>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    if req.text.trim().is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }

    st.rate_limiter.acquire().await;

    // Retry the Telegram API call with backoff.
    let token = st.bot_token.clone();
    let client = st.client.clone();
    let text = req.text.clone();

    retry(
        || send_telegram_message(&client, &token, req.chat_id, &text),
        3,
        Duration::from_secs(5),
    )
    .await
    .map(|()| Json(serde_json::json!({"ok": true})))
    .map_err(|e| {
        error!("Failed to send Telegram message after retries: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })
}

fn build_router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/send", post(send_message))
        .with_state(state)
}

// ─── Main entry point ──────────────────────────────────────────────────────

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let config = Config::from_env()?;
    info!("Starting mosaic-bridge-telegram on port {}", config.port);

    // Non-blocking MIS probe (failures are non-fatal).
    tokio::spawn(probe_mis(config.mis_url.clone()));

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .context("Failed to build reqwest Client")?;

    let rate_limiter = RateLimiter::new(30);
    let (shutdown_tx, shutdown_rx) = watch::channel(false);

    // Spawn the long-poll background task.
    let poll_handle = {
        let cfg = config.clone();
        let cl = client.clone();
        let rx = shutdown_rx.clone();
        tokio::spawn(async move { polling_loop(cfg, cl, rx).await })
    };

    let state = AppState {
        bot_token: config.bot_token.clone(),
        client,
        rate_limiter,
    };

    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", config.port))
        .await
        .context("Failed to bind TCP listener")?;

    info!("HTTP server listening on 0.0.0.0:{}", config.port);

    axum::serve(listener, build_router(state))
        .with_graceful_shutdown(async move {
            tokio::signal::ctrl_c().await.expect("ctrl_c listener");
            info!("Shutdown signal received — stopping...");
            let _ = shutdown_tx.send(true);
        })
        .await
        .context("axum::serve failed")?;

    // Wait for the polling loop to finish.
    let _ = poll_handle.await;
    info!("mosaic-bridge-telegram stopped cleanly");
    Ok(())
}
