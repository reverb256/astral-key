//! Astral Key - API Integration Tests
//!
//! Integration tests for all API endpoints

use axum::{
    body::Body,
    http::{header, Method, Request, StatusCode},
};
use serde_json::json;
use tower::ServiceExt;

/// Test helper: Create test app state
async fn create_test_app() -> axum::Router {
    use astral_key::config::Config;
    use astral_key::state::AppState;

    // Create test configuration
    let config = Config {
        server: astral_key::config::ServerConfig {
            host: "127.0.0.1".to_string(),
            port: 8080,
        },
        database: astral_key::config::DatabaseConfig {
            url: "postgresql://postgres:postgres@localhost/astral_key_test".to_string(),
            max_connections: 5,
            min_connections: 1,
        },
        redis: astral_key::config::RedisConfig {
            url: "redis://127.0.0.1:6379".to_string(),
            max_connections: 5,
            min_connections: 1,
        },
        jwt: astral_key::config::JwtConfig {
            secret: "test_secret_key_32_bytes_long_!!!".to_string(),
            access_token_ttl: 900,
            refresh_token_ttl: 604800,
        },
        fido2: astral_key::config::Fido2Config {
            rp_id: "localhost".to_string(),
            rp_name: "Astral Key Test".to_string(),
            origin: "http://localhost:8080".to_string(),
        },
        vaultwarden: astral_key::config::VaultwardenConfig {
            url: "http://localhost:8000".to_string(),
            admin_token: "test_token".to_string(),
        },
    };

    // Create test state
    let state = AppState::new(config).await.expect("Failed to create test state");

    // Create router
    let app = axum::Router::new()
        .route("/health", axum::routing::get(|| async { "OK" }))
        .nest("/api/v1", astral_key::api::routes::api_v1_routes())
        .with_state(state);

    app
}

#[tokio::test]
#[ignore] // Requires database and Redis
async fn test_health_endpoint() {
    let app = create_test_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
#[ignore] // Requires database and Redis
async fn test_web3_nonce_generation() {
    let app = create_test_app().await;

    let request_body = json!({
        "domain": "localhost",
        "address": "0x742d35Cc6634C0532925a3b844Bc9e7595f0bEb",
        "chain_id": 1
    });

    let response = app
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/v1/auth/web3/nonce")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(request_body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert!(json["nonce"].is_string());
    assert!(json["message_template"].is_string());
    assert_eq!(json["domain"], "localhost");
    assert_eq!(json["chain_id"], 1);
}

#[tokio::test]
#[ignore] // Requires database and Redis
async fn test_web3_verify_signature() {
    let app = create_test_app().await;

    // First, get a nonce
    let nonce_request = json!({
        "domain": "localhost",
        "address": "0x742d35Cc6634C0532925a3b844Bc9e7595f0bEb",
        "chain_id": 1
    });

    let nonce_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/v1/auth/web3/nonce")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(nonce_request.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    let nonce_body = hyper::body::to_bytes(nonce_response.into_body()).await.unwrap();
    let nonce_json: serde_json::Value = serde_json::from_slice(&nonce_body).unwrap();
    let nonce = nonce_json["nonce"].as_str().unwrap();
    let message_template = nonce_json["message_template"].as_str().unwrap();

    // Create a mock SIWE message
    let message = message_template.replace(nonce, nonce);

    // Note: In a real test, you would generate an actual Ethereum signature
    // For this test, we'll check that validation fails with an invalid signature
    let verify_request = json!({
        "message": message,
        "signature": "0x" + "0".repeat(130), // Invalid signature
        "chain_id": 1
    });

    let response = app
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/v1/auth/web3/verify")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(verify_request.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    // Should fail with invalid signature
    assert!(response.status() != StatusCode::OK);
}

#[tokio::test]
#[ignore] // Requires database and Redis
async fn test_fido2_registration_options() {
    let app = create_test_app().await;

    // This would require a valid JWT token from Web3 auth
    // For now, just test that unauthenticated request is rejected
    let request_body = json!({
        "username": "testuser",
        "display_name": "Test User"
    });

    let response = app
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/v1/auth/fido2/register/options")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(request_body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    // Should be unauthorized without JWT
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
#[ignore] // Requires database and Redis
async fn test_fido2_authenticate_options() {
    let app = create_test_app().await;

    let request_body = json!({
        "username": "00000000-0000-0000-0000-000000000000" // Test user UUID
    });

    let response = app
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/v1/auth/fido2/authenticate/options")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(request_body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    // User not found, should return error
    assert!(response.status() != StatusCode::OK);
}

#[tokio::test]
#[ignore] // Requires database and Redis
async fn test_session_refresh() {
    let app = create_test_app().await;

    // Test with invalid token
    let request_body = json!({
        "refresh_token": "invalid_token"
    });

    let response = app
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/v1/sessions/refresh")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(request_body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    // Should be unauthorized with invalid token
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
#[ignore] // Requires database and Redis
async fn test_protected_route_without_auth() {
    let app = create_test_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/users/me")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    // Should be unauthorized without JWT
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
#[ignore] // Requires database and Redis
async fn test_get_chains() {
    let app = create_test_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/auth/web3/chains")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert!(json["chains"].is_array());
    assert!(json["chains"].as_array().unwrap().len() > 0);
}
