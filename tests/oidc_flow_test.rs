//! Astral Key — OIDC authorization-code flow integration tests
//!
//! Drives the complete headless flow against an in-memory database, with no
//! WebAuthn ceremony (the authorize POST accepts a session access token):
//!
//! ```text
//!   seed user + client + groups
//!     → GET  /oidc/authorize   (session id from the login page)
//!     → POST /oidc/authorize   (session access token)      → authorization code
//!     → POST /oidc/token       (code + PKCE verifier)      → id/access/refresh
//!     →      /.well-known/jwks.json → verify the RS256 id_token signature
//!     → GET  /oidc/userinfo    (access token)
//!     → POST /oidc/token       (grant_type=refresh_token)
//! ```
//!
//! Key material is generated in-process — no key is ever committed.

use std::sync::OnceLock;

use astral_key::api::routes;
use astral_key::config::{Config, OidcClientConfig, OidcConfig};
use astral_key::db::models::User;
use astral_key::state::AppState;

use axum::{
    body::Body,
    http::{Request, StatusCode},
    Router,
};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use jsonwebtoken::{decode, Algorithm, DecodingKey, Validation};
use rsa::pkcs8::{EncodePrivateKey, LineEnding};
use rsa::RsaPrivateKey;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use tower::ServiceExt;

const ISSUER: &str = "https://auth.test";
const CLIENT_ID: &str = "test-rp";
const CLIENT_SECRET: &str = "test-rp-secret-8f21c4";
const REDIRECT_URI: &str = "https://rp.example/callback";
const PUBLIC_CLIENT_ID: &str = "public-spa";
const PUBLIC_REDIRECT_URI: &str = "https://spa.example/cb";
const PKCE_VERIFIER: &str = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";

/// PKCS#8 RSA private key generated once per test binary (~4s in debug).
fn rsa_pem() -> &'static str {
    static PEM: OnceLock<String> = OnceLock::new();
    PEM.get_or_init(|| {
        let key = RsaPrivateKey::new(&mut rand::thread_rng(), 2048).expect("RSA keygen");
        key.to_pkcs8_pem(LineEnding::LF)
            .expect("PKCS#8 PEM encode")
            .to_string()
    })
}

/// A booted app plus the seeding facts the flow needs.
struct TestApp {
    app: Router,
    state: AppState,
    user_id: uuid::Uuid,
}

/// Boot the app with OIDC enabled, seed a user, a public client and groups.
async fn setup(use_rsa: bool) -> TestApp {
    // JWT_SECRET is required by AppState::new() (minimum 32 bytes).
    std::env::set_var(
        "JWT_SECRET",
        "abcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890",
    );

    let mut config = Config::from_env().expect("config from env");
    config.database.url = "sqlite::memory:?mode=rwc".to_string();
    config.database.max_connections = 1;
    config.oidc = OidcConfig {
        enabled: true,
        issuer: ISSUER.to_string(),
        signing_key_hex: Some(hex::encode([9u8; 32])),
        clients: vec![OidcClientConfig {
            client_id: CLIENT_ID.to_string(),
            client_secret: CLIENT_SECRET.to_string(),
            redirect_uris: vec![REDIRECT_URI.to_string()],
        }],
        access_token_ttl: 3600,
        id_token_ttl: 600,
        refresh_token_ttl: 3600,
        rsa_key_pem: use_rsa.then(|| rsa_pem().to_string()),
    };

    let state = AppState::new(config).await.expect("AppState init");
    let pool = state.db.inner();

    // Seed a user with an email address (the OIDC subject).
    let user = User::create_with_email(pool, uuid::Uuid::new_v4(), "jane@example.com", "Jane Doe")
        .await
        .expect("seed user");

    // Seed a PUBLIC client (no secret ⇒ PKCE S256 is mandatory).
    sqlx::query(
        "INSERT INTO oidc_clients (client_id, client_secret_hash, redirect_uris, allowed_scopes) \
         VALUES (?1, NULL, ?2, NULL)",
    )
    .bind(PUBLIC_CLIENT_ID)
    .bind(serde_json::to_string(&vec![PUBLIC_REDIRECT_URI]).expect("json"))
    .execute(pool)
    .await
    .expect("seed public client");

    // Seed groups with SQL — the documented management path.
    sqlx::query("INSERT INTO oidc_groups (name) VALUES ('admins'), ('devs')")
        .execute(pool)
        .await
        .expect("seed groups");
    sqlx::query(
        "INSERT INTO oidc_group_members (user_id, group_name) VALUES (?1, 'admins'), (?1, 'devs')",
    )
    .bind(user.id.to_string())
    .execute(pool)
    .await
    .expect("seed membership");

    let app = routes(Router::new(), state.clone());

    TestApp {
        app,
        state,
        user_id: user.id,
    }
}

// ---------------------------------------------------------------------------
// HTTP helpers
// ---------------------------------------------------------------------------

async fn send(app: &Router, request: Request<Body>) -> (StatusCode, String) {
    let response = app.clone().oneshot(request).await.expect("request");
    let status = response.status();
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("body");
    (status, String::from_utf8_lossy(&body).to_string())
}

async fn get(app: &Router, uri: &str) -> (StatusCode, String) {
    send(
        app,
        Request::builder()
            .uri(uri)
            .body(Body::empty())
            .expect("request"),
    )
    .await
}

async fn post_json(app: &Router, uri: &str, payload: &Value) -> (StatusCode, Value) {
    let (status, body) = send(
        app,
        Request::builder()
            .method("POST")
            .uri(uri)
            .header("content-type", "application/json")
            .body(Body::from(payload.to_string()))
            .expect("request"),
    )
    .await;

    (status, serde_json::from_str(&body).unwrap_or(Value::Null))
}

async fn post_form(app: &Router, uri: &str, form: &str) -> (StatusCode, Value) {
    let (status, body) = send(
        app,
        Request::builder()
            .method("POST")
            .uri(uri)
            .header("content-type", "application/x-www-form-urlencoded")
            .body(Body::from(form.to_string()))
            .expect("request"),
    )
    .await;

    (status, serde_json::from_str(&body).unwrap_or(Value::Null))
}

/// Percent-encode a value for use in a query string.
fn encode(value: &str) -> String {
    url::form_urlencoded::byte_serialize(value.as_bytes()).collect()
}

/// S256 PKCE challenge for a verifier.
fn challenge_for(verifier: &str) -> String {
    URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()))
}

/// Start the authorization request and return the login-page session id.
async fn start_authorize(app: &Router, query: &str) -> (StatusCode, String) {
    let uri = format!("/oidc/authorize?{query}");
    let (status, body) = get(app, &uri).await;
    if status != StatusCode::OK {
        return (status, body);
    }

    let session_id = body
        .split("const SESSION_ID = \"")
        .nth(1)
        .and_then(|rest| rest.split('"').next())
        .expect("login page embeds the session id")
        .to_string();

    (status, session_id)
}

/// Standard authorize query for the confidential client.
fn authorize_query(client_id: &str, redirect_uri: &str, challenge: &str) -> String {
    format!(
        "client_id={}&redirect_uri={}&response_type=code&scope=openid%20profile%20email\
         &state=xyz&nonce=n-1&code_challenge={challenge}&code_challenge_method=S256",
        encode(client_id),
        encode(redirect_uri),
    )
}

/// Complete the authorize POST with a session access token, returning the code.
async fn complete_authorize(t: &TestApp, session_id: &str) -> (StatusCode, Value) {
    let session_token = t
        .state
        .jwt
        .as_ref()
        .expect("jwt service")
        .generate_access_token(t.user_id)
        .expect("session access token");

    post_json(
        &t.app,
        "/oidc/authorize",
        &json!({ "session_id": session_id, "access_token": session_token }),
    )
    .await
}

/// Extract a query parameter from an absolute URL.
fn query_param(url: &str, name: &str) -> Option<String> {
    url::Url::parse(url)
        .ok()?
        .query_pairs()
        .find(|(key, _)| key == name)
        .map(|(_, value)| value.into_owned())
}

/// Verification key built from the JWKS entry with this kid — the relying
/// party's path, independent of the server's key material.
fn jwks_rsa_key(jwks: &Value, kid: &str) -> DecodingKey {
    let entry = jwks["keys"]
        .as_array()
        .expect("keys array")
        .iter()
        .find(|k| k["kid"] == kid && k["kty"] == "RSA")
        .expect("RSA JWKS entry for the id_token kid");
    DecodingKey::from_rsa_components(
        entry["n"].as_str().expect("n"),
        entry["e"].as_str().expect("e"),
    )
    .expect("decoding key from JWKS components")
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn authorization_code_flow_issues_rs256_id_token_verifiable_from_jwks() {
    let t = setup(true).await;
    let challenge = challenge_for(PKCE_VERIFIER);

    // Backward compatibility: the legacy env client was upserted at boot with
    // a SHA-256 secret hash (never the plaintext).
    let stored: (Option<String>,) =
        sqlx::query_as("SELECT client_secret_hash FROM oidc_clients WHERE client_id = ?1")
            .bind(CLIENT_ID)
            .fetch_one(t.state.db.inner())
            .await
            .expect("legacy client row");
    let expected_hash = hex::encode(Sha256::digest(CLIENT_SECRET.as_bytes()));
    assert_eq!(stored.0.as_deref(), Some(expected_hash.as_str()));
    assert_ne!(stored.0.as_deref(), Some(CLIENT_SECRET));

    // 1. Authorization request → login page carrying the session id.
    let (status, session_id) = start_authorize(
        &t.app,
        &authorize_query(CLIENT_ID, REDIRECT_URI, &challenge),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    // 2. Authorize POST with the (already verified) Astral Key session token.
    let (status, body) = complete_authorize(&t, &session_id).await;
    assert_eq!(status, StatusCode::OK, "authorize POST: {body}");
    let redirect_url = body["redirect_url"].as_str().expect("redirect_url");
    assert!(redirect_url.starts_with(REDIRECT_URI));
    let code = query_param(redirect_url, "code").expect("code in redirect");
    assert_eq!(query_param(redirect_url, "state").as_deref(), Some("xyz"));

    // 3. Token exchange with the PKCE verifier.
    let form = format!(
        "grant_type=authorization_code&code={}&redirect_uri={}&client_id={}&client_secret={}&code_verifier={}",
        encode(&code),
        encode(REDIRECT_URI),
        encode(CLIENT_ID),
        encode(CLIENT_SECRET),
        encode(PKCE_VERIFIER),
    );
    let (status, tokens) = post_form(&t.app, "/oidc/token", &form).await;
    assert_eq!(status, StatusCode::OK, "token endpoint: {tokens}");

    let id_token = tokens["id_token"].as_str().expect("id_token");
    let access_token = tokens["access_token"].as_str().expect("access_token");
    let refresh_token = tokens["refresh_token"].as_str().expect("refresh_token");
    assert_eq!(tokens["token_type"], "Bearer");
    assert_eq!(tokens["scope"], "openid profile email");
    assert_eq!(tokens["expires_in"], 3600);

    // 4. The id_token must be RS256 with the RSA kid...
    let header = jsonwebtoken::decode_header(id_token).expect("id_token header");
    assert_eq!(header.alg, Algorithm::RS256);
    let kid = header.kid.expect("kid present");

    // ...and verify against the PUBLIC key parsed out of the JWKS output.
    let (status, jwks_body) = get(&t.app, "/.well-known/jwks.json").await;
    assert_eq!(status, StatusCode::OK);
    let jwks: Value = serde_json::from_str(&jwks_body).expect("jwks json");
    assert!(jwks["keys"]
        .as_array()
        .expect("keys")
        .iter()
        .any(|k| k["alg"] == "RS256"));

    let key = jwks_rsa_key(&jwks, &kid);
    let mut validation = Validation::new(Algorithm::RS256);
    validation.set_audience(&[CLIENT_ID]);
    validation.set_issuer(&[ISSUER]);
    let claims = decode::<Value>(id_token, &key, &validation)
        .expect("RS256 id_token verifies against the JWKS")
        .claims;

    assert_eq!(claims["iss"], ISSUER);
    assert_eq!(claims["aud"], CLIENT_ID);
    assert_eq!(claims["azp"], CLIENT_ID);
    assert_eq!(claims["sub"], t.user_id.to_string());
    assert_eq!(claims["email"], "jane@example.com");
    assert_eq!(claims["email_verified"], true);
    assert_eq!(claims["nonce"], "n-1");
    assert_eq!(claims["groups"], json!(["admins", "devs"]));
    assert!(claims["exp"].as_u64().expect("exp") > claims["iat"].as_u64().expect("iat"));

    // 5. userinfo mirrors the identity + groups.
    let (status, userinfo_body) = send(
        &t.app,
        Request::builder()
            .uri("/oidc/userinfo")
            .header("authorization", format!("Bearer {access_token}"))
            .body(Body::empty())
            .expect("request"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let userinfo: Value = serde_json::from_str(&userinfo_body).expect("userinfo json");
    assert_eq!(userinfo["sub"], t.user_id.to_string());
    assert_eq!(userinfo["email"], "jane@example.com");
    assert_eq!(userinfo["email_verified"], true);
    assert_eq!(userinfo["groups"], json!(["admins", "devs"]));

    // 6. Refresh grant rotates the token.
    let form = format!(
        "grant_type=refresh_token&refresh_token={}&client_id={}&client_secret={}",
        encode(refresh_token),
        encode(CLIENT_ID),
        encode(CLIENT_SECRET),
    );
    let (status, refreshed) = post_form(&t.app, "/oidc/token", &form).await;
    assert_eq!(status, StatusCode::OK, "refresh grant: {refreshed}");
    let rotated = refreshed["refresh_token"]
        .as_str()
        .expect("rotated refresh token");
    assert_ne!(rotated, refresh_token);

    let refreshed_header =
        jsonwebtoken::decode_header(refreshed["id_token"].as_str().expect("refreshed id_token"))
            .expect("header");
    assert_eq!(refreshed_header.alg, Algorithm::RS256);

    // The presented token is single-use.
    let (status, body) = post_form(&t.app, "/oidc/token", &form).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "replay: {body}");

    // 7. The authorization code is single-use too.
    let (status, _) = post_form(
        &t.app,
        "/oidc/token",
        &format!(
            "grant_type=authorization_code&code={}&redirect_uri={}&client_id={}&client_secret={}&code_verifier={}",
            encode(&code),
            encode(REDIRECT_URI),
            encode(CLIENT_ID),
            encode(CLIENT_SECRET),
            encode(PKCE_VERIFIER),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn ed25519_only_config_still_signs_eddsa_and_verifies() {
    let t = setup(false).await;
    let challenge = challenge_for(PKCE_VERIFIER);

    let (status, session_id) = start_authorize(
        &t.app,
        &authorize_query(CLIENT_ID, REDIRECT_URI, &challenge),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, body) = complete_authorize(&t, &session_id).await;
    assert_eq!(status, StatusCode::OK, "authorize POST: {body}");
    let code = query_param(body["redirect_url"].as_str().expect("redirect"), "code")
        .expect("code in redirect");

    let form = format!(
        "grant_type=authorization_code&code={}&redirect_uri={}&client_id={}&client_secret={}&code_verifier={}",
        encode(&code),
        encode(REDIRECT_URI),
        encode(CLIENT_ID),
        encode(CLIENT_SECRET),
        encode(PKCE_VERIFIER),
    );
    let (status, tokens) = post_form(&t.app, "/oidc/token", &form).await;
    assert_eq!(status, StatusCode::OK, "token endpoint: {tokens}");
    let id_token = tokens["id_token"].as_str().expect("id_token");

    // EdDSA, and no RSA key in the JWKS.
    let header = jsonwebtoken::decode_header(id_token).expect("header");
    assert_eq!(header.alg, Algorithm::EdDSA);
    let kid = header.kid.expect("kid");

    let (_, jwks_body) = get(&t.app, "/.well-known/jwks.json").await;
    let jwks: Value = serde_json::from_str(&jwks_body).expect("jwks json");
    let keys = jwks["keys"].as_array().expect("keys");
    assert_eq!(keys.len(), 1, "Ed25519-only config has no RSA key");
    assert!(keys.iter().all(|k| k["alg"] != "RS256"));

    let x = keys
        .iter()
        .find(|k| k["kid"] == kid)
        .expect("Ed25519 entry")["x"]
        .as_str()
        .expect("x")
        .to_string();
    let key = DecodingKey::from_ed_components(&x).expect("ed decoding key");

    let mut validation = Validation::new(Algorithm::EdDSA);
    validation.set_audience(&[CLIENT_ID]);
    validation.set_issuer(&[ISSUER]);
    let claims = decode::<Value>(id_token, &key, &validation)
        .expect("EdDSA id_token verifies against the JWKS")
        .claims;
    assert_eq!(claims["sub"], t.user_id.to_string());
    assert_eq!(claims["groups"], json!(["admins", "devs"]));
}

#[tokio::test]
async fn wrong_pkce_verifier_is_rejected() {
    let t = setup(true).await;
    let challenge = challenge_for(PKCE_VERIFIER);

    let (status, session_id) = start_authorize(
        &t.app,
        &authorize_query(CLIENT_ID, REDIRECT_URI, &challenge),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (_, body) = complete_authorize(&t, &session_id).await;
    let code = query_param(body["redirect_url"].as_str().expect("redirect"), "code")
        .expect("code in redirect");

    let form = format!(
        "grant_type=authorization_code&code={}&redirect_uri={}&client_id={}&client_secret={}&code_verifier={}",
        encode(&code),
        encode(REDIRECT_URI),
        encode(CLIENT_ID),
        encode(CLIENT_SECRET),
        encode("not-the-verifier"),
    );
    let (status, body) = post_form(&t.app, "/oidc/token", &form).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert_eq!(body["code"], "AUTH_BAD_REQUEST");
}

#[tokio::test]
async fn unknown_client_and_unregistered_redirect_are_rejected() {
    let t = setup(true).await;
    let challenge = challenge_for(PKCE_VERIFIER);

    // Unknown client at the authorization endpoint.
    let (status, _) = start_authorize(
        &t.app,
        &authorize_query("no-such-client", REDIRECT_URI, &challenge),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    // Unknown client at the token endpoint.
    let (status, body) = post_form(
        &t.app,
        "/oidc/token",
        &format!(
            "grant_type=authorization_code&code=deadbeef&client_id={}&client_secret=x&code_verifier={PKCE_VERIFIER}",
            encode("no-such-client"),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");

    // Registered client, unregistered redirect URI.
    let (status, _) = start_authorize(
        &t.app,
        &authorize_query(CLIENT_ID, "https://evil.example/cb", &challenge),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    // Registered client, redirect URI that is a prefix of the registered one.
    let (status, _) = start_authorize(
        &t.app,
        &authorize_query(CLIENT_ID, "https://rp.example/call", &challenge),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn redirect_uri_mismatch_at_token_endpoint_is_rejected() {
    let t = setup(true).await;
    let challenge = challenge_for(PKCE_VERIFIER);

    let (status, session_id) = start_authorize(
        &t.app,
        &authorize_query(CLIENT_ID, REDIRECT_URI, &challenge),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (_, body) = complete_authorize(&t, &session_id).await;
    let code = query_param(body["redirect_url"].as_str().expect("redirect"), "code")
        .expect("code in redirect");

    let form = format!(
        "grant_type=authorization_code&code={}&redirect_uri={}&client_id={}&client_secret={}&code_verifier={}",
        encode(&code),
        encode("https://rp.example/other"),
        encode(CLIENT_ID),
        encode(CLIENT_SECRET),
        encode(PKCE_VERIFIER),
    );
    let (status, body) = post_form(&t.app, "/oidc/token", &form).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
}

#[tokio::test]
async fn unknown_or_replayed_authorization_code_is_rejected() {
    let t = setup(true).await;
    let challenge = challenge_for(PKCE_VERIFIER);
    let exchange = |code: &str| {
        format!(
            "grant_type=authorization_code&code={}&redirect_uri={}&client_id={}&client_secret={}&code_verifier={}",
            encode(code),
            encode(REDIRECT_URI),
            encode(CLIENT_ID),
            encode(CLIENT_SECRET),
            encode(PKCE_VERIFIER),
        )
    };

    // A code that was never issued.
    let (status, body) = post_form(&t.app, "/oidc/token", &exchange("never-issued")).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");

    // A real code, replayed after a successful exchange.
    let (status, session_id) = start_authorize(
        &t.app,
        &authorize_query(CLIENT_ID, REDIRECT_URI, &challenge),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (_, body) = complete_authorize(&t, &session_id).await;
    let code = query_param(body["redirect_url"].as_str().expect("redirect"), "code")
        .expect("code in redirect");

    let (status, _) = post_form(&t.app, "/oidc/token", &exchange(&code)).await;
    assert_eq!(status, StatusCode::OK);

    let (status, body) = post_form(&t.app, "/oidc/token", &exchange(&code)).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "replay: {body}");
}

#[tokio::test]
async fn wrong_client_secret_is_rejected() {
    let t = setup(true).await;
    let challenge = challenge_for(PKCE_VERIFIER);

    let (status, session_id) = start_authorize(
        &t.app,
        &authorize_query(CLIENT_ID, REDIRECT_URI, &challenge),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (_, body) = complete_authorize(&t, &session_id).await;
    let code = query_param(body["redirect_url"].as_str().expect("redirect"), "code")
        .expect("code in redirect");

    let form = format!(
        "grant_type=authorization_code&code={}&redirect_uri={}&client_id={}&client_secret={}&code_verifier={}",
        encode(&code),
        encode(REDIRECT_URI),
        encode(CLIENT_ID),
        encode("guessed-secret"),
        encode(PKCE_VERIFIER),
    );
    let (status, body) = post_form(&t.app, "/oidc/token", &form).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "{body}");
}

#[tokio::test]
async fn public_client_requires_pkce_and_needs_no_secret() {
    let t = setup(true).await;
    let challenge = challenge_for(PKCE_VERIFIER);

    // A public client without a challenge must be refused at authorize.
    let query_no_pkce = format!(
        "client_id={}&redirect_uri={}&response_type=code&scope=openid",
        encode(PUBLIC_CLIENT_ID),
        encode(PUBLIC_REDIRECT_URI),
    );
    let (status, body) = get(&t.app, &format!("/oidc/authorize?{query_no_pkce}")).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");

    // With S256 PKCE it works end-to-end and the token endpoint needs no secret.
    let (status, session_id) = start_authorize(
        &t.app,
        &authorize_query(PUBLIC_CLIENT_ID, PUBLIC_REDIRECT_URI, &challenge),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, body) = complete_authorize(&t, &session_id).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let code = query_param(body["redirect_url"].as_str().expect("redirect"), "code")
        .expect("code in redirect");

    let form = format!(
        "grant_type=authorization_code&code={}&redirect_uri={}&client_id={}&code_verifier={}",
        encode(&code),
        encode(PUBLIC_REDIRECT_URI),
        encode(PUBLIC_CLIENT_ID),
        encode(PKCE_VERIFIER),
    );
    let (status, tokens) = post_form(&t.app, "/oidc/token", &form).await;
    assert_eq!(status, StatusCode::OK, "{tokens}");

    let header = jsonwebtoken::decode_header(tokens["id_token"].as_str().expect("id_token"))
        .expect("header");
    assert_eq!(header.alg, Algorithm::RS256);
    assert_eq!(
        tokens["refresh_token"].as_str().map(str::len),
        Some(64),
        "opaque refresh token"
    );

    // The same code cannot be replayed with a wrong verifier afterwards, and a
    // fresh grant needs the correct verifier.
    let (status, session_id) = start_authorize(
        &t.app,
        &authorize_query(PUBLIC_CLIENT_ID, PUBLIC_REDIRECT_URI, &challenge),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (_, body) = complete_authorize(&t, &session_id).await;
    let code = query_param(body["redirect_url"].as_str().expect("redirect"), "code")
        .expect("code in redirect");

    let (status, body) = post_form(
        &t.app,
        "/oidc/token",
        &format!(
            "grant_type=authorization_code&code={}&redirect_uri={}&client_id={}&code_verifier=wrong",
            encode(&code),
            encode(PUBLIC_REDIRECT_URI),
            encode(PUBLIC_CLIENT_ID),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
}

#[tokio::test]
async fn userinfo_rejects_missing_and_forged_tokens() {
    let t = setup(true).await;

    // No Authorization header.
    let (status, _) = get(&t.app, "/oidc/userinfo").await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    // A structurally valid but unsigned/forged token.
    let (status, _) = send(
        &t.app,
        Request::builder()
            .uri("/oidc/userinfo")
            .header("authorization", "Bearer not.a.jwt")
            .body(Body::empty())
            .expect("request"),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn oidc_discovery_advertises_rs256_when_configured() {
    let t = setup(true).await;
    let (status, body) = get(&t.app, "/.well-known/openid-configuration").await;
    assert_eq!(status, StatusCode::OK);
    let doc: Value = serde_json::from_str(&body).expect("discovery json");
    assert_eq!(doc["issuer"], ISSUER);
    assert_eq!(
        doc["id_token_signing_alg_values_supported"],
        json!(["RS256"])
    );
    assert!(doc["token_endpoint_auth_methods_supported"]
        .as_array()
        .expect("methods")
        .contains(&json!("none")));
}
