//! Astral Key - OIDC provider handlers
//!
//! Implements the OIDC discovery, JWKS, authorization-code, token, and
//! userinfo endpoints that oauth2-proxy (and any other RP) talks to.
//!
//! Relying parties are resolved from the `oidc_clients` table on every
//! request, so a client inserted with SQL is live without a restart.

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

use crate::auth::oidc::{hash_refresh_token, OidcClient, OidcService};
use crate::db::models::{oidc_client, oidc_group, oidc_refresh_token, User};
use crate::error::{AuthError, Result};
use crate::state::AppState;

/// Prefixes used for the in-memory OIDC state store.
const SESSION_PREFIX: &str = "oidc:session:";
const CODE_PREFIX: &str = "oidc:code:";
/// Authorize sessions + authorization codes live for 10 minutes.
const OIDC_TTL_SECS: u64 = 600;
/// Upper bound for `OIDC_REFRESH_TOKEN_TTL` (10 years) — keeps the chrono
/// arithmetic in range instead of panicking on a bad value.
const MAX_REFRESH_TOKEN_TTL_SECS: u64 = 10 * 365 * 24 * 3600;

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
    #[serde(rename = "refresh_token")]
    pub refresh_token: Option<String>,
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
    require_oidc(&state)?;
    let client = require_client(&state, &query.client_id).await?;
    validate_authorize_request(&client, &query)?;

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

/// Token endpoint — exchanges the authorization code (or a refresh token)
/// for tokens.
pub async fn token(
    State(state): State<AppState>,
    headers: HeaderMap,
    Form(form): Form<TokenRequest>,
) -> Result<Json<Value>> {
    let svc = require_oidc(&state)?;

    match form.grant_type.as_str() {
        "authorization_code" => exchange_authorization_code(&state, svc, &headers, &form).await,
        "refresh_token" => exchange_refresh_token(&state, svc, &headers, &form).await,
        other => Err(AuthError::BadRequest(format!(
            "Unsupported grant_type: {other}"
        ))),
    }
}

/// `grant_type=authorization_code`.
async fn exchange_authorization_code(
    state: &AppState,
    svc: &OidcService,
    headers: &HeaderMap,
    form: &TokenRequest,
) -> Result<Json<Value>> {
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

    // Client authentication: the client must be registered, and a confidential
    // client must prove possession of its secret.
    let (client_id, client_secret) = presented_client_credentials(headers, form)?;
    let client = require_client(state, &client_id).await?;
    authenticate_client(&client, client_secret.as_deref())?;
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

    // PKCE verification (S256). Mandatory for public clients — enforced when
    // the authorization request was accepted.
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

    let groups = oidc_group::groups_for_user(state.db.inner(), user.id).await?;
    let id_token = svc.sign_id_token(
        user.id,
        &client_id,
        session.nonce.clone(),
        user.email.clone(),
        user.display_name.clone(),
        &groups,
    )?;
    let access_token = svc.sign_access_token(
        user.id,
        &client_id,
        user.email.clone(),
        user.display_name.clone(),
    )?;
    let refresh_token =
        issue_refresh_token(state, svc, &client_id, user.id, &session.scope).await?;

    // One-time use: consume both the code and its session.
    state
        .oauth_state
        .delete(&format!("{CODE_PREFIX}{code}"))
        .await;
    state.oauth_state.delete(&session_key).await;

    Ok(Json(json!({
        "access_token": access_token,
        "id_token": id_token,
        "refresh_token": refresh_token,
        "token_type": "Bearer",
        "expires_in": svc.access_token_ttl_secs(),
        "scope": session.scope,
    })))
}

/// `grant_type=refresh_token` — rotates the presented token.
async fn exchange_refresh_token(
    state: &AppState,
    svc: &OidcService,
    headers: &HeaderMap,
    form: &TokenRequest,
) -> Result<Json<Value>> {
    let presented = form
        .refresh_token
        .as_deref()
        .ok_or_else(|| AuthError::BadRequest("Missing refresh_token".to_string()))?;

    let (client_id, client_secret) = presented_client_credentials(headers, form)?;
    let client = require_client(state, &client_id).await?;
    authenticate_client(&client, client_secret.as_deref())?;

    let token_hash = hash_refresh_token(presented);
    let record = oidc_refresh_token::find(state.db.inner(), &token_hash)
        .await?
        .ok_or_else(|| AuthError::Unauthorized("Invalid or expired refresh token".to_string()))?;

    if record.client_id != client_id {
        return Err(AuthError::Unauthorized(
            "Refresh token was not issued to this client".to_string(),
        ));
    }

    if record.is_expired(chrono::Utc::now()) {
        oidc_refresh_token::delete(state.db.inner(), &token_hash).await?;
        return Err(AuthError::Unauthorized("Refresh token expired".to_string()));
    }

    let user = User::get_by_id(state.db.inner(), record.user_id)
        .await?
        .ok_or_else(|| AuthError::Unauthorized("User no longer exists".to_string()))?;

    let groups = oidc_group::groups_for_user(state.db.inner(), user.id).await?;
    // A refreshed id_token carries no nonce: the RP already validated one.
    let id_token = svc.sign_id_token(
        user.id,
        &client_id,
        None,
        user.email.clone(),
        user.display_name.clone(),
        &groups,
    )?;
    let access_token = svc.sign_access_token(
        user.id,
        &client_id,
        user.email.clone(),
        user.display_name.clone(),
    )?;

    // Rotation: the presented token is single-use.
    oidc_refresh_token::delete(state.db.inner(), &token_hash).await?;
    let refresh_token = issue_refresh_token(state, svc, &client_id, user.id, &record.scope).await?;

    Ok(Json(json!({
        "access_token": access_token,
        "id_token": id_token,
        "refresh_token": refresh_token,
        "token_type": "Bearer",
        "expires_in": svc.access_token_ttl_secs(),
        "scope": record.scope,
    })))
}

/// Mint + persist a refresh token, returning the raw value (hash stored).
async fn issue_refresh_token(
    state: &AppState,
    svc: &OidcService,
    client_id: &str,
    user_id: Uuid,
    scope: &str,
) -> Result<String> {
    let ttl = state.config.oidc.refresh_token_ttl;
    if ttl > MAX_REFRESH_TOKEN_TTL_SECS {
        return Err(AuthError::Config(format!(
            "OIDC_REFRESH_TOKEN_TTL must be at most {MAX_REFRESH_TOKEN_TTL_SECS} seconds"
        )));
    }

    let token = svc.new_refresh_token();
    let expires_at = chrono::Utc::now() + chrono::Duration::seconds(ttl as i64);
    oidc_refresh_token::insert(
        state.db.inner(),
        &hash_refresh_token(&token),
        client_id,
        user_id,
        scope,
        expires_at,
    )
    .await?;

    Ok(token)
}

/// Userinfo endpoint — claims for the presented access token.
pub async fn userinfo(State(state): State<AppState>, headers: HeaderMap) -> Result<Json<Value>> {
    let svc = require_oidc(&state)?;

    let bearer = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .ok_or_else(|| AuthError::Unauthorized("Missing Bearer token".to_string()))?;

    // The token carries its own audience; validate the signature first, then
    // require the audience to be a client we issued for.
    let claims = svc.validate_access_token_any_aud(bearer)?;
    if oidc_client::get(state.db.inner(), &claims.aud)
        .await?
        .is_none()
    {
        return Err(AuthError::Unauthorized(
            "OIDC access token was issued to an unknown client".to_string(),
        ));
    }

    let user_id = Uuid::parse_str(&claims.sub)
        .map_err(|_| AuthError::Unauthorized("Invalid subject in access token".to_string()))?;
    let groups = oidc_group::groups_for_user(state.db.inner(), user_id).await?;

    let mut body = json!({
        "sub": claims.sub,
        "groups": groups,
    });

    // email_verified is only asserted alongside an email address.
    if let Some(email) = claims.email {
        body["email"] = json!(email);
        body["email_verified"] = json!(claims.email_verified.unwrap_or(false));
    }
    if let Some(name) = claims.name {
        body["name"] = json!(name);
    }
    if let Some(username) = claims.preferred_username {
        body["preferred_username"] = json!(username);
    }

    Ok(Json(body))
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

/// Resolve a relying party from the registry, or reject the request.
async fn require_client(state: &AppState, client_id: &str) -> Result<OidcClient> {
    oidc_client::get(state.db.inner(), client_id)
        .await?
        .ok_or_else(|| AuthError::BadRequest("Unknown client_id".to_string()))
}

/// Validate an authorization request against its registered client.
fn validate_authorize_request(client: &OidcClient, query: &AuthorizeQuery) -> Result<()> {
    if query.response_type != "code" {
        return Err(AuthError::BadRequest(format!(
            "Unsupported response_type: {} (only 'code' is supported)",
            query.response_type
        )));
    }

    // EXACT match — an unregistered callback must never be honoured.
    if !client.allows_redirect_uri(&query.redirect_uri) {
        return Err(AuthError::BadRequest(
            "Unknown client_id or redirect_uri not registered".to_string(),
        ));
    }

    let scope = query.scope.as_deref().unwrap_or("openid");
    let requested: Vec<&str> = scope.split_whitespace().collect();
    if !requested.contains(&"openid") {
        return Err(AuthError::BadRequest(
            "The 'openid' scope is required".to_string(),
        ));
    }
    for requested_scope in requested {
        if !client.allows_scope(requested_scope) {
            return Err(AuthError::BadRequest(format!(
                "Scope '{requested_scope}' is not allowed for this client"
            )));
        }
    }

    let uses_s256 = query
        .code_challenge_method
        .as_deref()
        .map(|m| m.eq_ignore_ascii_case("S256"))
        .unwrap_or(false);

    // Public clients (no secret) MUST use PKCE S256.
    if client.is_public() && (query.code_challenge.is_none() || !uses_s256) {
        return Err(AuthError::BadRequest(
            "Public clients must send code_challenge with code_challenge_method=S256".to_string(),
        ));
    }

    // A challenge that is present must be S256 — 'plain' is never accepted.
    if query.code_challenge.is_some() && !uses_s256 {
        return Err(AuthError::BadRequest(
            "Unsupported code_challenge_method (only S256 is supported)".to_string(),
        ));
    }

    Ok(())
}

/// Check client authentication at the token endpoint.
///
/// Public clients hold no secret and authenticate with their PKCE verifier;
/// confidential clients must present a matching secret.
fn authenticate_client(client: &OidcClient, presented_secret: Option<&str>) -> Result<()> {
    if client.is_public() {
        return Ok(());
    }

    match presented_secret {
        Some(secret) if client.verify_secret(secret) => Ok(()),
        _ => Err(AuthError::Unauthorized(
            "Invalid client credentials".to_string(),
        )),
    }
}

/// Pull the presented client id + optional secret from the token request:
/// form fields, HTTP Basic auth, or a mix of both
/// (client_secret_post / client_secret_basic).
fn presented_client_credentials(
    headers: &HeaderMap,
    form: &TokenRequest,
) -> Result<(String, Option<String>)> {
    let basic = basic_credentials(headers)?;

    let client_id = form
        .client_id
        .clone()
        .or_else(|| basic.as_ref().map(|(id, _)| id.clone()))
        .ok_or_else(|| AuthError::BadRequest("Client credentials required".to_string()))?;

    let client_secret = form
        .client_secret
        .clone()
        .or_else(|| basic.map(|(_, secret)| secret));

    Ok((client_id, client_secret))
}

/// Decode an HTTP Basic authorization header, when one is present.
fn basic_credentials(headers: &HeaderMap) -> Result<Option<(String, String)>> {
    let Some(basic) = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Basic "))
    else {
        return Ok(None);
    };

    let decoded = base64::engine::general_purpose::STANDARD
        .decode(basic)
        .map_err(|_| AuthError::BadRequest("Malformed Basic auth header".to_string()))?;
    let text = String::from_utf8(decoded)
        .map_err(|_| AuthError::BadRequest("Malformed Basic auth header".to_string()))?;
    let (id, secret) = text
        .split_once(':')
        .ok_or_else(|| AuthError::BadRequest("Malformed Basic auth header".to_string()))?;

    Ok(Some((id.to_string(), secret.to_string())))
}

/// Embedded login page (WebAuthn ceremony + register bootstrap).
const LOGIN_PAGE: &str = include_str!("oidc_login.html");

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::oidc::hash_client_secret;

    const REDIRECT: &str = "https://auth.lan/oauth2/callback";

    fn confidential_client() -> OidcClient {
        OidcClient::confidential(
            "astral-key-oidc".to_string(),
            "super-secret",
            vec![REDIRECT.to_string()],
            None,
        )
    }

    fn public_client() -> OidcClient {
        OidcClient::public(
            "public-spa".to_string(),
            vec!["https://spa.example/cb".to_string()],
        )
    }

    fn authorize_query() -> AuthorizeQuery {
        AuthorizeQuery {
            client_id: "astral-key-oidc".to_string(),
            redirect_uri: REDIRECT.to_string(),
            response_type: "code".to_string(),
            scope: Some("openid profile email".to_string()),
            state: Some("abc".to_string()),
            nonce: Some("n1".to_string()),
            code_challenge: Some("challenge".to_string()),
            code_challenge_method: Some("S256".to_string()),
        }
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

    #[test]
    fn authorize_request_validation() {
        let client = confidential_client();
        assert!(validate_authorize_request(&client, &authorize_query()).is_ok());

        // Implicit flow must be rejected.
        let implicit = AuthorizeQuery {
            response_type: "id_token".to_string(),
            ..clone_q(&authorize_query())
        };
        assert!(validate_authorize_request(&client, &implicit).is_err());

        // Missing openid scope must be rejected.
        let no_scope = AuthorizeQuery {
            scope: Some("profile".to_string()),
            ..clone_q(&authorize_query())
        };
        assert!(validate_authorize_request(&client, &no_scope).is_err());

        // PKCE 'plain' must be rejected.
        let plain = AuthorizeQuery {
            code_challenge_method: Some("plain".to_string()),
            ..clone_q(&authorize_query())
        };
        assert!(validate_authorize_request(&client, &plain).is_err());
    }

    #[test]
    fn redirect_uri_mismatch_is_rejected() {
        let client = confidential_client();

        let bad_uri = AuthorizeQuery {
            redirect_uri: "https://evil.example.com/cb".to_string(),
            ..clone_q(&authorize_query())
        };
        assert!(validate_authorize_request(&client, &bad_uri).is_err());

        // Prefix of a registered URI is not a match.
        let prefix = AuthorizeQuery {
            redirect_uri: "https://auth.lan/oauth2".to_string(),
            ..clone_q(&authorize_query())
        };
        assert!(validate_authorize_request(&client, &prefix).is_err());
    }

    #[test]
    fn public_client_requires_pkce_s256() {
        let client = public_client();

        let mut without_pkce = authorize_query();
        without_pkce.client_id = "public-spa".to_string();
        without_pkce.redirect_uri = "https://spa.example/cb".to_string();
        without_pkce.code_challenge = None;
        without_pkce.code_challenge_method = None;
        assert!(validate_authorize_request(&client, &without_pkce).is_err());

        // A challenge without the S256 method is still rejected.
        let mut wrong_method = clone_q(&without_pkce);
        wrong_method.code_challenge = Some("challenge".to_string());
        wrong_method.code_challenge_method = Some("plain".to_string());
        assert!(validate_authorize_request(&client, &wrong_method).is_err());

        // S256 PKCE passes.
        let mut with_pkce = clone_q(&without_pkce);
        with_pkce.code_challenge = Some("challenge".to_string());
        with_pkce.code_challenge_method = Some("S256".to_string());
        assert!(validate_authorize_request(&client, &with_pkce).is_ok());
    }

    #[test]
    fn scopes_outside_the_registration_are_rejected() {
        let client = OidcClient {
            client_id: "restricted".to_string(),
            client_secret_hash: Some(hash_client_secret("s3cret")),
            redirect_uris: vec![REDIRECT.to_string()],
            allowed_scopes: Some(vec!["openid".to_string()]),
        };

        let allowed = AuthorizeQuery {
            client_id: "restricted".to_string(),
            scope: Some("openid".to_string()),
            ..clone_q(&authorize_query())
        };
        assert!(validate_authorize_request(&client, &allowed).is_ok());

        let denied = AuthorizeQuery {
            client_id: "restricted".to_string(),
            scope: Some("openid profile".to_string()),
            ..clone_q(&authorize_query())
        };
        assert!(validate_authorize_request(&client, &denied).is_err());
    }

    #[test]
    fn confidential_client_needs_its_secret() {
        let client = confidential_client();
        assert!(authenticate_client(&client, Some("super-secret")).is_ok());
        assert!(authenticate_client(&client, Some("wrong")).is_err());
        assert!(authenticate_client(&client, None).is_err());

        // Public clients present no secret at all (PKCE carries the proof).
        let public = public_client();
        assert!(authenticate_client(&public, None).is_ok());
        assert!(authenticate_client(&public, Some("ignored")).is_ok());
    }

    #[test]
    fn credentials_come_from_form_or_basic_or_both() {
        let headers = HeaderMap::new();

        let form = TokenRequest {
            grant_type: "authorization_code".to_string(),
            code: None,
            redirect_uri: None,
            client_id: Some("form-client".to_string()),
            client_secret: Some("form-secret".to_string()),
            code_verifier: None,
            refresh_token: None,
        };
        let (id, secret) = presented_client_credentials(&headers, &form).expect("form creds");
        assert_eq!(id, "form-client");
        assert_eq!(secret.as_deref(), Some("form-secret"));

        // Nothing anywhere → rejected.
        let empty = TokenRequest {
            grant_type: "authorization_code".to_string(),
            code: None,
            redirect_uri: None,
            client_id: None,
            client_secret: None,
            code_verifier: None,
            refresh_token: None,
        };
        assert!(presented_client_credentials(&headers, &empty).is_err());

        // HTTP Basic.
        let basic = base64::engine::general_purpose::STANDARD.encode("basic-client:basic-secret");
        let mut basic_headers = HeaderMap::new();
        basic_headers.insert(
            header::AUTHORIZATION,
            format!("Basic {basic}").parse().expect("header value"),
        );
        let (id, secret) =
            presented_client_credentials(&basic_headers, &empty).expect("basic creds");
        assert_eq!(id, "basic-client");
        assert_eq!(secret.as_deref(), Some("basic-secret"));

        // client_id in the form, secret in the header (mixed) still works.
        let mixed = TokenRequest {
            client_id: Some("basic-client".to_string()),
            ..empty
        };
        let (id, secret) =
            presented_client_credentials(&basic_headers, &mixed).expect("mixed creds");
        assert_eq!(id, "basic-client");
        assert_eq!(secret.as_deref(), Some("basic-secret"));
    }

    #[test]
    fn malformed_basic_header_is_rejected() {
        let mut headers = HeaderMap::new();
        headers.insert(
            header::AUTHORIZATION,
            "Basic !!!not-base64!!!".parse().expect("header value"),
        );
        let form = TokenRequest {
            grant_type: "authorization_code".to_string(),
            code: None,
            redirect_uri: None,
            client_id: None,
            client_secret: None,
            code_verifier: None,
            refresh_token: None,
        };
        assert!(presented_client_credentials(&headers, &form).is_err());
        assert!(basic_credentials(&headers).is_err());
    }
}
