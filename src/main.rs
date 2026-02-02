//! Astral Key - Web3 & FIDO2 Authentication Microservice
//!
//! A next-generation authentication service built with Rust, NixOS, and Vaultwarden.

use std::net::SocketAddr;

use axum::{routing::get, Router};
use tokio::signal;
use tracing::{info, warn};

mod api;
mod auth;
mod cache;
mod config;
mod db;
mod error;
mod state;
mod utils;
mod vaultwarden;

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
        .route("/ready", get(readiness_handler))
        .with_state(state.clone());

    // Add API routes
    let app = api::routes(app, state);

    // Determine bind address
    let addr: SocketAddr = format!("{}:{}", config.server.host, config.server.port).parse()?;
    info!("Binding to {}", addr);

    // Start server with graceful shutdown
    let listener = tokio::net::TcpListener::bind(addr).await?;
    info!("Server listening on {}", addr);

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    info!("Server shutdown complete");
    Ok(())
}

/// Health check endpoint
async fn health_handler() -> &'static str {
    "OK"
}

/// Readiness check endpoint
async fn readiness_handler() -> &'static str {
    // TODO: Check database, redis, vaultwarden connectivity
    "READY"
}

/// Graceful shutdown signal handler
async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("Failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("Failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {
            warn!("Received Ctrl+C, starting graceful shutdown");
        }
        _ = terminate => {
            warn!("Received SIGTERM, starting graceful shutdown");
        }
    }
}
