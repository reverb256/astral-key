use axum::{
    extract::State,
    http::Method,
    routing::{get, post},
    Json, Router,
};
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;
use tracing::{info, warn};

mod as_server;
mod matrix_client;
mod room_mapper;

use as_server::AsState;
use matrix_client::MatrixClient;
use mosaic_client::MosaicClient;
use room_mapper::RoomMapper;

/// Application configuration from environment variables.
struct Config {
    mis_url: String,
    homeserver_url: String,
    _as_token: String,
    hs_token: String,
    _bot_token: String,
    _bot_user_id: String,
    domain: String,
    as_port: u16,
    mapping_file: String,
}

impl Config {
    fn from_env() -> Self {
        Self {
            mis_url: std::env::var("MIS_URL")
                .unwrap_or_else(|_| "http://localhost:8081".to_string()),
            homeserver_url: std::env::var("MATRIX_HOMESERVER_URL")
                .unwrap_or_else(|_| "https://matrix.local".to_string()),
            _as_token: std::env::var("MATRIX_AS_TOKEN")
                .unwrap_or_else(|_| "mosaic-bridge-as-token".to_string()),
            hs_token: std::env::var("MATRIX_HS_TOKEN")
                .unwrap_or_else(|_| "mosaic-bridge-hs-token".to_string()),
            _bot_token: std::env::var("MATRIX_BOT_TOKEN").unwrap_or_default(),
            _bot_user_id: std::env::var("MATRIX_BOT_USER")
                .unwrap_or_else(|_| "@mosaic-bridge:matrix.local".to_string()),
            domain: std::env::var("MATRIX_DOMAIN").unwrap_or_else(|_| "matrix.local".to_string()),
            as_port: std::env::var("MATRIX_AS_PORT")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(8082),
            mapping_file: std::env::var("MATRIX_MAPPING_FILE")
                .unwrap_or_else(|_| "data/room-mappings.json".to_string()),
        }
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // ─── Tracing / logging ─────────────────────────────────────────────────
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,mosaic_bridge_matrix=debug".into()),
        )
        .init();

    // ─── Configuration ─────────────────────────────────────────────────────
    let cfg = Config::from_env();

    info!("=== Mosaic ↔ Matrix Bridge ===");
    info!("MIS: {}", cfg.mis_url);
    info!("AS server port: {}", cfg.as_port);
    info!("Matrix domain: {}", cfg.domain);
    info!("Homeserver: {}", cfg.homeserver_url);

    // ─── MIS client ────────────────────────────────────────────────────────
    let mis = MosaicClient::from_url(&cfg.mis_url).expect("Invalid MIS_URL");
    match mis.health().await {
        Ok(h) => info!("[MATRIX] MIS health: {}", h.status),
        Err(e) => warn!(
            "[MATRIX] Cannot reach MIS — binding resolution disabled: {}",
            e
        ),
    }

    // ─── Matrix client ─────────────────────────────────────────────────────
    let matrix = MatrixClient::new(
        cfg.homeserver_url.clone(),
        cfg._bot_token.clone(),
        cfg._bot_user_id.clone(),
    );

    // ─── Room mapper ───────────────────────────────────────────────────────
    let room_mapper = RoomMapper::new(&cfg.mapping_file, &cfg.domain);

    // ─── Build router ──────────────────────────────────────────────────────
    let as_state = Arc::new(AsState {
        mis,
        matrix,
        room_mapper,
        hs_token: cfg.hs_token,
        domain: cfg.domain.clone(),
    });

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods([Method::GET, Method::POST, Method::PUT])
        .allow_headers(Any);

    let app = Router::new()
        // Health check (unauthenticated)
        .route("/health", get(health_handler))
        // Mappings status
        .route("/mappings", get(mappings_handler))
        // Matrix Application Service endpoints
        .route(
            "/matrix/transactions/{txn_id}",
            post(as_server::handle_transaction),
        )
        .route("/matrix/users/{user_id}", get(as_server::handle_user_query))
        // Middleware
        .layer(TraceLayer::new_for_http())
        .layer(cors)
        .with_state(as_state);

    // ─── Start server ──────────────────────────────────────────────────────
    let addr = format!("0.0.0.0:{}", cfg.as_port);
    info!("[MATRIX] AS server listening on {}", addr);
    info!("[MATRIX] Registration endpoint: POST /matrix/transactions/{{txnId}}");
    info!("[MATRIX] User namespace: @mosaic_<pubkey>:{}", cfg.domain);
    info!("[MATRIX] Ready. Waiting for homeserver events...");

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    Ok(())
}

/// GET /health — generic health check.
async fn health_handler() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "status": "ok",
        "service": "matrix-bridge",
    }))
}

/// GET /mappings — list all channel ↔ room mappings.
async fn mappings_handler(State(state): State<Arc<AsState>>) -> Json<serde_json::Value> {
    let mappings = state.room_mapper.list_mappings();
    Json(serde_json::json!({
        "mappings": mappings,
    }))
}

/// Signal handler for graceful shutdown.
async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("Failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("Failed to install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }

    info!("[MATRIX] Shutting down");
}
