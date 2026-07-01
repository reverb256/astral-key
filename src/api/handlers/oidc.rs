//! Astral Key - OIDC Provider Handlers
//!
//! OpenID Connect endpoints for oauth2-proxy integration.
//! Supports both WebAuthn passkey and Web3 SIWE authentication.
//!
//! Endpoints:
//! - `/.well-known/openid-configuration` — Discovery document
//! - `/oidc/auth` — Authorization redirect
//! - `/oidc/token` — Token exchange (accepts existing JWT as Bearer token)
//! - `/oidc/userinfo` — User info from access token
//! - `/oidc/jwks` — JSON Web Key Set for token verification

use axum::{
    extract::{Query, State},
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Json, Redirect},
    routing::get,
    Router,
};
use serde::{Deserialize, Serialize};
use tracing::warn;

use crate::error::Result;
use crate::state::AppState;

/// OIDC well-known discovery document
#[derive(Serialize)]
pub struct OidcDiscovery {
    issuer: String,
    authorization_endpoint: String,
    token_endpoint: String,
    userinfo_endpoint: String,
    jwks_uri: String,
    response_types_supported: Vec<String>,
    subject_types_supported: Vec<String>,
    id_token_signing_alg_values_supported: Vec<String>,
    scopes_supported: Vec<String>,
    token_endpoint_auth_methods_supported: Vec<String>,
    claims_supported: Vec<String>,
}

/// Build OIDC routes
pub fn oidc_routes() -> Router<AppState> {
    Router::new()
        .route("/.well-known/openid-configuration", get(discovery))
        .route("/oidc/auth", get(authorize))
        .route("/oidc/token", get(token))
        .route("/oidc/userinfo", get(userinfo))
        .route("/oidc/jwks", get(jwks))
}

/// GET /.well-known/openid-configuration
async fn discovery(State(state): State<AppState>) -> Json<OidcDiscovery> {
    let issuer = state
        .config
        .oidc
        .issuer_url
        .trim_end_matches('/')
        .to_string();

    Json(OidcDiscovery {
        issuer: issuer.clone(),
        authorization_endpoint: format!("{}/oidc/auth", issuer),
        token_endpoint: format!("{}/oidc/token", issuer),
        userinfo_endpoint: format!("{}/oidc/userinfo", issuer),
        jwks_uri: format!("{}/oidc/jwks", issuer),
        response_types_supported: vec!["code".into()],
        subject_types_supported: vec!["public".into()],
        id_token_signing_alg_values_supported: vec!["RS256".into()],
        scopes_supported: vec!["openid".into(), "profile".into(), "email".into()],
        token_endpoint_auth_methods_supported: vec![
            "client_secret_basic".into(),
            "client_secret_post".into(),
        ],
        claims_supported: vec!["sub".into(), "name".into(), "preferred_username".into()],
    })
}

/// Authorization request parameters
#[derive(Deserialize)]
pub struct AuthRequest {
    response_type: Option<String>,
    client_id: Option<String>,
    redirect_uri: Option<String>,
    scope: Option<String>,
    state: Option<String>,
}

/// GET /oidc/auth — redirect to the login portal
async fn authorize(Query(params): Query<AuthRequest>) -> impl IntoResponse {
    // Store the OIDC params in a session or redirect to the login portal
    // For now, redirect to a login page that the user can use
    // The login page will redirect back with a code

    let login_url = format!(
        "/login?redirect_uri={}&state={}&client_id={}",
        params.redirect_uri.as_deref().unwrap_or(""),
        params.state.as_deref().unwrap_or(""),
        params.client_id.as_deref().unwrap_or(""),
    );

    Redirect::to(&login_url)
}

/// Token request (query parameters)
#[derive(Deserialize)]
pub struct TokenRequest {
    grant_type: Option<String>,
    code: Option<String>,
    redirect_uri: Option<String>,
    client_id: Option<String>,
}

/// Token response
#[derive(Serialize)]
pub struct TokenResponse {
    access_token: String,
    token_type: String,
    expires_in: u64,
    id_token: Option<String>,
}

/// GET /oidc/token — exchange an existing JWT (Bearer) for an OIDC-compatible token
///
/// Accepts an existing HMAC-signed JWT in the Authorization header (Bearer <token>),
/// validates it, and returns an RS256-signed OIDC token response.
async fn token(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<TokenRequest>,
) -> impl IntoResponse {
    // Get the JWT service
    let jwt = match state.jwt.as_ref() {
        Some(jwt) => jwt,
        None => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": "jwt_service_not_initialized"})),
            )
                .into_response();
        }
    };

    // Check if OIDC (RSA keys) is available
    if !jwt.oidc_available() {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": "oidc_not_available"})),
        )
            .into_response();
    }

    // Extract the Bearer token from the Authorization header
    let bearer_token = headers
        .get(header::AUTHORIZATION)
        .and_then(|h| h.to_str().ok())
        .and_then(|h| h.strip_prefix("Bearer "))
        .ok_or_else(|| {
            (
                StatusCode::UNAUTHORIZED,
                Json(serde_json::json!({"error": "missing_or_invalid_authorization_header",
                                        "error_description": "Authorization header with Bearer token is required"})),
            )
        });

    let bearer_token = match bearer_token {
        Ok(t) => t,
        Err(e) => return e.into_response(),
    };

    // Validate the existing HMAC-signed access token
    let claims = match jwt.validate_access_token(bearer_token) {
        Ok(c) => c,
        Err(e) => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(serde_json::json!({"error": "invalid_token",
                                        "error_description": e.to_string()})),
            )
                .into_response();
        }
    };

    // Parse user_id from claims
    let user_id = match uuid::Uuid::parse_str(&claims.sub) {
        Ok(id) => id,
        Err(_) => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(serde_json::json!({"error": "invalid_token",
                                        "error_description": "Invalid subject in token"})),
            )
                .into_response();
        }
    };

    // Check if the token is blacklisted
    match state.cache.is_token_blacklisted(bearer_token).await {
        Ok(true) => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(serde_json::json!({"error": "token_revoked"})),
            )
                .into_response();
        }
        Err(e) => {
            tracing::warn!("Cache check failed in OIDC token endpoint: {}", e);
        }
        _ => {}
    }

    // Generate RS256-signed OIDC token
    let issuer = state.config.oidc.issuer_url.trim_end_matches('/');
    let audience = params.client_id.clone();
    let oidc_token = match jwt.generate_oidc_token(
        user_id,
        issuer,
        audience,
        claims.name.clone(),
        claims.preferred_username.clone(),
    ) {
        Ok(t) => t,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": "token_generation_failed",
                                        "error_description": e.to_string()})),
            )
                .into_response();
        }
    };

    let expires_in = state.config.jwt.access_token_ttl;

    Json(TokenResponse {
        access_token: oidc_token.clone(),
        token_type: "Bearer".to_string(),
        expires_in,
        id_token: Some(oidc_token),
    })
    .into_response()
}

/// GET /oidc/userinfo — return user info from RS256-signed JWT
async fn userinfo(State(state): State<AppState>, headers: HeaderMap) -> impl IntoResponse {
    // Get the JWT service
    let jwt = match state.jwt.as_ref() {
        Some(jwt) => jwt,
        None => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": "jwt_service_not_initialized"})),
            )
                .into_response();
        }
    };

    // Extract Bearer token
    let bearer_token = match headers
        .get(header::AUTHORIZATION)
        .and_then(|h| h.to_str().ok())
        .and_then(|h| h.strip_prefix("Bearer "))
    {
        Some(t) => t,
        None => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(serde_json::json!({"error": "missing_authorization_header"})),
            )
                .into_response();
        }
    };

    // Validate the RS256-signed OIDC token
    let claims = match jwt.validate_oidc_token(bearer_token) {
        Ok(c) => c,
        Err(e) => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(serde_json::json!({"error": "invalid_token",
                                        "error_description": e.to_string()})),
            )
                .into_response();
        }
    };

    // Return OIDC standard claims
    let mut response = serde_json::json!({
        "sub": claims.sub,
    });

    // Add optional fields if present
    if let Some(name) = claims.name {
        response["name"] = serde_json::Value::String(name);
    }
    if let Some(preferred_username) = claims.preferred_username {
        response["preferred_username"] = serde_json::Value::String(preferred_username);
    }

    Json(response).into_response()
}

/// JWKS response
#[derive(Serialize)]
pub struct JwksResponse {
    keys: Vec<serde_json::Value>,
}

/// GET /oidc/jwks — public keys for JWT verification
async fn jwks(State(state): State<AppState>) -> impl IntoResponse {
    let jwt = match state.jwt.as_ref() {
        Some(jwt) => jwt,
        None => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": "jwt_service_not_initialized"})),
            )
                .into_response();
        }
    };

    let jwk_set = jwt.get_jwk_set();

    if jwk_set.keys.is_empty() {
        return Json(JwksResponse { keys: vec![] }).into_response();
    }

    Json(serde_json::to_value(&jwk_set).unwrap_or_else(|_| serde_json::json!({"keys": []})))
        .into_response()
}
