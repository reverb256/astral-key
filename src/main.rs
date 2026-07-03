//! Astral Key - Ultra-minimal Passkey + Web3 (SIWE) Auth Sidecar
//!
//! Standalone auth sidecar with SQLite, no Redis/PostgreSQL needed.

use std::net::SocketAddr;

use axum::{response::IntoResponse, routing::get, Router};
use tracing::info;

mod api;
mod auth;
mod config;
mod db;
mod error;
mod state;
mod utils;

use crate::config::Config;
use crate::state::AppState;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,astral_key=debug".into()),
        )
        .init();

    info!("Starting Astral Key v{}", env!("CARGO_PKG_VERSION"));

    // Load configuration
    let config = Config::from_env()?;
    info!("Configuration loaded successfully");

    // Initialize application state
    let state = AppState::new(config.clone()).await?;
    info!("Application state initialized");

    // Build router
    let app = Router::new()
        .route("/health", get(health_handler))
        .route("/ready", get(readiness_handler));

    // Add API routes
    let app = api::routes(app, state);

    // Determine bind address
    let addr: SocketAddr = format!("{}:{}", config.server.host, config.server.port).parse()?;
    info!("Binding to {}", addr);

    // Start server
    let listener = tokio::net::TcpListener::bind(addr).await?;
    info!("Server listening on {}", addr);

    axum::serve(listener, app).await?;

    info!("Server shutdown complete");
    Ok(())
}

/// Health check endpoint
async fn health_handler() -> &'static str {
    "OK"
}

/// Readiness check endpoint
async fn readiness_handler(
    axum::extract::State(state): axum::extract::State<AppState>,
) -> impl IntoResponse {
    // Check database connectivity
    if let Err(e) = state.db.health_check().await {
        tracing::error!("Database health check failed: {}", e);
        let body = serde_json::json!({
            "status": "not_ready",
            "error": "database_unavailable"
        });
        return (
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            axum::Json(body),
        );
    }

    let body = serde_json::json!({
        "status": "ready",
        "checks": {
            "database": true,
        }
    });
    (axum::http::StatusCode::OK, axum::Json(body))
}
