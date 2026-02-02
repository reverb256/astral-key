//! Astral Key - Health check handlers

use axum::Json;
use serde_json::json;

/// Health check response
pub async fn health() -> Json<serde_json::Value> {
    Json(json!({
        "status": "healthy",
        "version": env!("CARGO_PKG_VERSION"),
    }))
}

/// Readiness check response
pub async fn ready() -> Json<serde_json::Value> {
    // TODO: Check database, redis, vaultwarden connectivity
    Json(json!({
        "status": "ready",
        "checks": {
            "database": true,
            "redis": true,
            "vaultwarden": true,
        }
    }))
}
