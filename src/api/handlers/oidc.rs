//! Astral Key - OIDC provider handlers
//!
//! Implements the OIDC discovery, JWKS, authorization-code, token, and
//! userinfo endpoints that oauth2-proxy (and any other RP) talks to.

use axum::{
    extract::{Query, State},
    http::header,
    http::HeaderMap,
    response::{Html, IntoResponse},
    Form, Json,
};
use base64::Engine;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use uuid::Uuid;

use crate::auth::oidc::OidcService;
use crate::db::models::User;
use crate::error::{AuthError, Result};
use crate::state::AppState;

/// Prefixes used for the in-memory OIDC state store.
const SESSION_PREFIX: &str = "oidc:session:";
const CODE_PREFIX: &str = "oidc:code:";
/// Authorize sessions + authorization codes live for 10 minutes.
const OIDC_TTL_SECS: u64 = 600;

/// State carried through the authorization-code flow (serialized into the
/// in-memory store — nothing sensitive beyond short-lived random codes).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OidcSession {
    pub client_id: String,
    pub redirect_uri: String,
    pub scope: String,
    pub state: Option<String>,
    pub nonce: Option<String>,
    pub code_challenge: Option<String>,
    pub code_challenge_method: Option<String>,
    /// Authorization code minted after a successful login (one-time use).
    pub code: Option<String>,
    /// Authenticated user (set when the code is minted).
    pub user_id: Option<String>,
}

/// GET /oidc/authorize — query params from the RP.
#[derive(Debug, Deserialize)]
pub struct AuthorizeQuery {
    pub client_id: String,
    pub redirect_uri: String,
    #[serde(rename = "response_type")]
    pub response_type: String,
    pub scope: Option<String>,
    pub state: Option<String>,
    pub nonce: Option<String>,
    pub code_challenge: Option<String>,
    #[serde(rename = "code_challenge_method")]
    pub code_challenge_method: Option<String>,
}

/// POST /oidc/authorize — browser posts back after the WebAuthn ceremony.
#[derive(Debug, Deserialize)]
pub struct AuthorizeCompleteRequest {
    pub session_id: String,
    pub access_token: String,
}

/// POST /oidc/token — form-encoded token exchange.
#[derive(Debug, Deserialize)]
pub struct TokenRequest {
    #[serde(rename = "grant_type")]
    pub grant_type: String,
    pub code: Option<String>,
    #[serde(rename = "redirect_uri")]
    pub redirect_uri: Option<String>,
    #[serde(rename = "client_id")]
    pub client_id: Option<String>,
    #[serde(rename = "client_secret")]
    pub client_secret: Option<String>,
    #[serde(rename = "code_verifier")]
    pub code_verifier: Option<String>,
}

/// OIDC discovery document.
pub async fn discovery(State(state): State<AppState>) -> Result<Json<Value>> {
    let svc = require_oidc(&state)?;
    Ok(Json(svc.discovery_document()))
}

/// JWKS — public key(s) for verifying id_tokens.
pub async fn jwks(State(state): State<AppState>) -> Result<Json<Value>> {
    let svc = require_oidc(&state)?;
    Ok(Json(svc.jwks()))
}

/// Authorization endpoint (GET) — validates the RP request and serves the
/// WebAuthn login page.
pub async fn authorize_get(
    State(state): State<AppState>,
    Query(query): Query<AuthorizeQuery>,
) -> Result<impl IntoResponse> {
    let svc = require_oidc(&state)?;

    validate_authorize_request(svc, &query)?;

    // Persist the authorize session so the browser's completion POST can be
    // bound to this exact request (state, nonce, PKCE, redirect_uri).
    let session = OidcSession {
        client_id: query.client_id.clone(),
        redirect_uri: query.redirect_uri.clone(),
        scope: query.scope.clone().unwrap_or_else(|| "openid".to_string()),
        state: query.state.clone(),
        nonce: query.nonce.clone(),
        code_challenge: query.code_challenge.clone(),
        code_challenge_method: query.code_challenge_method.clone(),
        code: None,
        user_id: None,
    };
    let session_id = Uuid::new_v4().to_string();
    let key = format!("{SESSION_PREFIX}{session_id}");
    let value = serde_json::to_string(&session)
        .map_err(|e| AuthError::Internal(format!("Failed to serialize OIDC session: {e}")))?;
    state.oauth_state.store(&key, value, OIDC_TTL_SECS).await;

    // Serve the login page; the session id is embedded so the page can
    // complete the flow without exposing anything sensitive.
    let html = LOGIN_PAGE.replace("__SESSION_ID__", &session_id);
    Ok(Html(html))
}

/// Authorization endpoint (POST) — the login page posts the (already-verified)
/// Astral Key access token; we mint the authorization code and bounce the
/// browser back to the RP.
pub async fn authorize_post(
    State(state): State<AppState>,
    Json(request): Json<AuthorizeCompleteRequest>,
) -> Result<Json<Value>> {
    let _svc = require_oidc(&state)?;

    let session_key = format!("{SESSION_PREFIX}{}", request.session_id);
    let session_json = state.oauth_state.get(&session_key).await.ok_or_else(|| {
        AuthError::BadRequest("OIDC authorize session expired or not found".to_string())
    })?;
    let mut session: OidcSession = serde_json::from_str(&session_json)
        .map_err(|e| AuthError::Internal(format!("Failed to parse OIDC session: {e}")))?;

    // Validate the Astral Key access token presented by the browser.
    let jwt = state
        .jwt
        .as_ref()
        .ok_or_else(|| AuthError::Internal("JWT service not initialized".to_string()))?;
    let claims = jwt
        .validate_access_token(&request.access_token)
        .map_err(|e| AuthError::Unauthorized(format!("Invalid session token: {e}")))?;

    let user_id = Uuid::parse_str(&claims.sub)
        .map_err(|_| AuthError::Unauthorized("Invalid subject in session token".to_string()))?;
    let user = User::get_by_id(state.db.inner(), user_id)
        .await?
        .ok_or_else(|| AuthError::Unauthorized("User no longer exists".to_string()))?;

    // Mint the one-time authorization code.
    let code = state
        .oidc
        .as_ref()
        .map(|svc| svc.new_authorization_code())
        .ok_or_else(|| AuthError::Internal("OIDC provider not initialized".to_string()))?;

    session.code = Some(code.clone());
    session.user_id = Some(user.id.to_string());
    let session_json = serde_json::to_string(&session)
        .map_err(|e| AuthError::Internal(format!("Failed to serialize OIDC session: {e}")))?;
    state
        .oauth_state
        .store(&session_key, session_json, OIDC_TTL_SECS)
        .await;

    // Map code → session so the token endpoint can find it.
    state
        .oauth_state
        .store(
            &format!("{CODE_PREFIX}{code}"),
            request.session_id.clone(),
            OIDC_TTL_SECS,
        )
        .await;

    // Redirect back to the RP with code + state.
    let mut redirect = format!(
        "{}?code={}",
        session.redirect_uri,
        url::form_urlencoded::byte_serialize(code.as_bytes()).collect::<String>()
    );
    if let Some(state_param) = &session.state {
        redirect.push_str(&format!(
            "&state={}",
            url::form_urlencoded::byte_serialize(state_param.as_bytes()).collect::<String>()
        ));
    }

    Ok(Json(json!({ "redirect_url": redirect })))
}

/// Token endpoint — exchanges the authorization code for tokens.
pub async fn token(
    State(state): State<AppState>,
    headers: HeaderMap,
    Form(form): Form<TokenRequest>,
) -> Result<Json<Value>> {
    let svc = require_oidc(&state)?;

    if form.grant_type != "authorization_code" {
        return Err(AuthError::BadRequest(format!(
            "Unsupported grant_type: {}",
            form.grant_type
        )));
    }

    let code = form
        .code
        .as_deref()
        .ok_or_else(|| AuthError::BadRequest("Missing code".to_string()))?;

    // Resolve the code back to its session.
    let session_id = state
        .oauth_state
        .get(&format!("{CODE_PREFIX}{code}"))
        .await
        .ok_or_else(|| {
            AuthError::BadRequest("Invalid or expired authorization code".to_string())
        })?;
    let session_key = format!("{SESSION_PREFIX}{session_id}");
    let session_json = state.oauth_state.get(&session_key).await.ok_or_else(|| {
        AuthError::BadRequest("Authorization session expired or not found".to_string())
    })?;
    let session: OidcSession = serde_json::from_str(&session_json)
        .map_err(|e| AuthError::Internal(format!("Failed to parse OIDC session: {e}")))?;

    // The code must actually have been minted for this session.
    if session.code.as_deref() != Some(code) || session.user_id.is_none() {
        return Err(AuthError::BadRequest(
            "Authorization code was not issued for this session".to_string(),
        ));
    }

    // Client authentication: form fields or HTTP Basic.
    let (client_id, client_secret) = client_credentials(&headers, &form)?;
    if !svc.verify_client_secret(&client_id, &client_secret) {
        return Err(AuthError::Unauthorized(
            "Invalid client credentials".to_string(),
        ));
    }
    if client_id != session.client_id {
        return Err(AuthError::BadRequest(
            "Client mismatch with authorization".to_string(),
        ));
    }

    // Redirect URI must match the one used at authorize time.
    if let Some(redirect_uri) = &form.redirect_uri {
        if redirect_uri != &session.redirect_uri {
            return Err(AuthError::BadRequest("redirect_uri mismatch".to_string()));
        }
    }

    // PKCE verification (S256).
    if !svc.verify_pkce(
        session.code_challenge.as_deref(),
        session.code_challenge_method.as_deref(),
        form.code_verifier.as_deref(),
    ) {
        return Err(AuthError::BadRequest(
            "PKCE verification failed".to_string(),
        ));
    }

    // Load the user to populate id_token / userinfo claims.
    let user_id = Uuid::parse_str(
        session
            .user_id
            .as_deref()
            .ok_or_else(|| AuthError::BadRequest("No user on session".to_string()))?,
    )
    .map_err(|_| AuthError::Internal("Invalid user id on session".to_string()))?;
    let user = User::get_by_id(state.db.inner(), user_id)
        .await?
        .ok_or_else(|| AuthError::Unauthorized("User no longer exists".to_string()))?;

    let id_token = svc.sign_id_token(
        user.id,
        &client_id,
        session.nonce.clone(),
        user.email.clone(),
        user.display_name.clone(),
    )?;
    let access_token = svc.sign_access_token(
        user.id,
        &client_id,
        user.email.clone(),
        user.display_name.clone(),
    )?;

    // One-time use: consume both the code and its session.
    state
        .oauth_state
        .delete(&format!("{CODE_PREFIX}{code}"))
        .await;
    state.oauth_state.delete(&session_key).await;

    Ok(Json(json!({
        "access_token": access_token,
        "id_token": id_token,
        "token_type": "Bearer",
        "expires_in": svc.access_token_ttl_secs(),
        "scope": session.scope,
    })))
}

/// Userinfo endpoint — claims for the presented access token.
pub async fn userinfo(State(state): State<AppState>, headers: HeaderMap) -> Result<Json<Value>> {
    let svc = require_oidc(&state)?;

    let bearer = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .ok_or_else(|| AuthError::Unauthorized("Missing Bearer token".to_string()))?;

    // Accept tokens for any registered client.
    let aud = svc
        .clients
        .iter()
        .find(|c| svc.validate_access_token(bearer, &c.client_id).is_ok())
        .map(|c| c.client_id.clone());

    let client_id = aud.ok_or_else(|| {
        AuthError::Unauthorized("Invalid or expired OIDC access token".to_string())
    })?;
    let claims = svc.validate_access_token(bearer, &client_id)?;

    Ok(Json(json!({
        "sub": claims.sub,
        "email": claims.email,
        "email_verified": true,
        "name": claims.name,
        "preferred_username": claims.preferred_username,
    })))
}

// ---------------------------------------------------------------------------
// helpers
// ---------------------------------------------------------------------------

fn require_oidc(state: &AppState) -> Result<&OidcService> {
    state
        .oidc
        .as_deref()
        .ok_or_else(|| AuthError::NotFound("OIDC provider is not enabled".to_string()))
}

fn validate_authorize_request(svc: &OidcService, query: &AuthorizeQuery) -> Result<()> {
    if query.response_type != "code" {
        return Err(AuthError::BadRequest(format!(
            "Unsupported response_type: {} (only 'code' is supported)",
            query.response_type
        )));
    }
    if !svc.is_valid_redirect_uri(&query.client_id, &query.redirect_uri) {
        return Err(AuthError::BadRequest(
            "Unknown client_id or redirect_uri not registered".to_string(),
        ));
    }
    let scope = query.scope.as_deref().unwrap_or("openid");
    if !scope.split_whitespace().any(|s| s == "openid") {
        return Err(AuthError::BadRequest(
            "The 'openid' scope is required".to_string(),
        ));
    }
    Ok(())
}

/// Pull client credentials from the token request: form fields first, then
/// HTTP Basic auth (client_secret_post / client_secret_basic).
fn client_credentials(headers: &HeaderMap, form: &TokenRequest) -> Result<(String, String)> {
    if let (Some(id), Some(secret)) = (&form.client_id, &form.client_secret) {
        return Ok((id.clone(), secret.clone()));
    }

    let basic = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Basic "))
        .ok_or_else(|| AuthError::BadRequest("Client credentials required".to_string()))?;

    let decoded = base64::engine::general_purpose::STANDARD
        .decode(basic)
        .map_err(|_| AuthError::BadRequest("Malformed Basic auth header".to_string()))?;
    let text = String::from_utf8(decoded)
        .map_err(|_| AuthError::BadRequest("Malformed Basic auth header".to_string()))?;
    let (id, secret) = text
        .split_once(':')
        .ok_or_else(|| AuthError::BadRequest("Malformed Basic auth header".to_string()))?;
    Ok((id.to_string(), secret.to_string()))
}

/// Embedded login page (WebAuthn ceremony + register bootstrap).
const LOGIN_PAGE: &str = include_str!("oidc_login.html");

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::oidc::OidcService;
    use crate::config::OidcClientConfig;
    use std::time::Duration;

    fn test_svc() -> OidcService {
        let mut seed = [0u8; 32];
        rand::RngCore::fill_bytes(&mut rand::rngs::OsRng, &mut seed);
        OidcService::new(
            "https://auth.lan".to_string(),
            vec![OidcClientConfig {
                client_id: "astral-key-oidc".to_string(),
                client_secret: "secret".to_string(),
                redirect_uris: vec!["https://auth.lan/oauth2/callback".to_string()],
            }],
            seed,
            Duration::from_secs(3600),
            Duration::from_secs(600),
        )
        .unwrap()
    }

    #[test]
    fn authorize_request_validation() {
        let svc = test_svc();
        let ok = AuthorizeQuery {
            client_id: "astral-key-oidc".to_string(),
            redirect_uri: "https://auth.lan/oauth2/callback".to_string(),
            response_type: "code".to_string(),
            scope: Some("openid profile email".to_string()),
            state: Some("abc".to_string()),
            nonce: Some("n1".to_string()),
            code_challenge: Some("x".to_string()),
            code_challenge_method: Some("S256".to_string()),
        };
        assert!(validate_authorize_request(&svc, &ok).is_ok());

        // Implicit flow must be rejected.
        let implicit = AuthorizeQuery {
            response_type: "id_token".to_string(),
            ..clone_q(&ok)
        };
        assert!(validate_authorize_request(&svc, &implicit).is_err());

        // Unregistered redirect URI must be rejected.
        let bad_uri = AuthorizeQuery {
            redirect_uri: "https://evil.example.com/cb".to_string(),
            ..clone_q(&ok)
        };
        assert!(validate_authorize_request(&svc, &bad_uri).is_err());

        // Missing openid scope must be rejected.
        let no_scope = AuthorizeQuery {
            scope: Some("profile".to_string()),
            ..clone_q(&ok)
        };
        assert!(validate_authorize_request(&svc, &no_scope).is_err());
    }

    fn clone_q(q: &AuthorizeQuery) -> AuthorizeQuery {
        AuthorizeQuery {
            client_id: q.client_id.clone(),
            redirect_uri: q.redirect_uri.clone(),
            response_type: q.response_type.clone(),
            scope: q.scope.clone(),
            state: q.state.clone(),
            nonce: q.nonce.clone(),
            code_challenge: q.code_challenge.clone(),
            code_challenge_method: q.code_challenge_method.clone(),
        }
    }
}
