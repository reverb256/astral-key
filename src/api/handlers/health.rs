//! Astral Key - Health check handlers

use axum::{extract::State, Json};
use serde_json::json;

use crate::state::AppState;

/// Health check response
pub async fn health() -> Json<serde_json::Value> {
    Json(json!({
        "status": "healthy",
        "version": env!("CARGO_PKG_VERSION"),
    }))
}

/// Readiness check response
pub async fn ready(State(state): State<AppState>) -> Json<serde_json::Value> {
    let db_ok = state.db.health_check().await.is_ok();
    Json(json!({
        "status": if db_ok { "ready" } else { "not_ready" },
        "checks": {
            "database": db_ok,
        }
    }))
}
