//! Astral Key — Integration tests
//!
//! Tests the public API surface against an in-memory SQLite database.
//! Each test starts a fresh server instance with a clean database,
//! avoiding test-to-test state leakage.

use axum::{body::Body, http::Request, response::IntoResponse, routing::get, Router};
use serde_json::Value;
use tower::ServiceExt;

use astral_key::config::Config;
use astral_key::state::AppState;

/// Build a test app state with an in-memory SQLite database.
/// Sets JWT_SECRET to a minimal valid key for the test environment.
/// rate_limit::init() is called inside api::routes() — no separate setup needed.
async fn test_state() -> AppState {
    // JWT_SECRET is required by AppState::new() with a minimum 32-byte check.
    // Set it before creating state so Config::from_env() picks it up.
    std::env::set_var(
        "JWT_SECRET",
        "abcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890",
    );

    let mut config = Config::from_env().expect("config from env");
    config.database.url = "sqlite::memory:?mode=rwc".to_string();
    config.database.max_connections = 1;
    AppState::new(config).await.expect("AppState init")
}

/// Build the full router for testing (same structure as main.rs).
/// api::routes() calls rate_limit::init() internally.
fn test_app(state: AppState) -> Router {
    // rate_limit::init() is called inside api::routes() below — no separate setup needed.

    async fn health_handler() -> &'static str {
        "OK"
    }

    async fn readiness_handler(
        axum::extract::State(state): axum::extract::State<AppState>,
    ) -> impl IntoResponse {
        if let Err(e) = state.db.health_check().await {
            tracing::error!("Database health check failed: {e}");
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
            "checks": { "database": true }
        });
        (axum::http::StatusCode::OK, axum::Json(body))
    }

    let app = Router::new()
        .route("/health", get(health_handler))
        .route("/ready", get(readiness_handler));

    astral_key::api::routes(app, state)
}

#[tokio::test]
async fn test_health_endpoint() {
    let state = test_state().await;
    let mut app = test_app(state);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), 200);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    assert_eq!(&body[..], b"OK");
}

#[tokio::test]
async fn test_ready_endpoint() {
    let state = test_state().await;
    let mut app = test_app(state);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/ready")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), 200);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["status"], "ready");
    assert_eq!(json["checks"]["database"], true);
}

#[tokio::test]
async fn test_web3_chains_endpoint() {
    let state = test_state().await;
    let mut app = test_app(state);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/auth/web3/chains")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), 200);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: Value = serde_json::from_slice(&body).unwrap();
    let chains = json["chains"].as_array().unwrap();
    assert!(!chains.is_empty(), "expected at least one chain");
    assert!(chains.iter().any(|c| c["name"] == "ethereum"));
}

#[tokio::test]
async fn test_web3_nonce_endpoint() {
    let state = test_state().await;
    let mut app = test_app(state);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/auth/web3/nonce")
                .method("POST")
                .header("Content-Type", "application/json")
                .body(Body::from(
                    serde_json::json!({ "address": "0x742d35Cc6634C0532925a3b844Bc9e7595f2bD18" })
                        .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), 200);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: Value = serde_json::from_slice(&body).unwrap();
    assert!(json["nonce"].as_str().unwrap_or_default().len() >= 16);
}

#[tokio::test]
async fn test_unknown_route_returns_404() {
    let state = test_state().await;
    let mut app = test_app(state);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/auth/nonexistent")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), 404);
}

#[tokio::test]
async fn test_rate_limit_does_not_block_normal_traffic() {
    let state = test_state().await;
    let app = test_app(state);

    for i in 0..5 {
        let mut app = app.clone();
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), 200, "request {} was blocked", i + 1);
    }
}
