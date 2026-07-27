//! Astral Key - OAuth authentication handlers
//!
//! Supports GitHub OAuth and is designed to be extensible for other providers.

use axum::{extract::Query, extract::State, Json};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::auth::jwt::AuthenticatedUser;
use crate::config::OAuthProviderConfig;
use crate::db::models::{OAuthAccount, User};
use crate::error::{AuthError, Result};
use crate::state::AppState;

/// Query parameters for OAuth callback
#[derive(Debug, Deserialize)]
pub struct OAuthCallbackQuery {
    pub code: String,
    pub state: Option<String>,
}

/// Query parameters for OAuth login initiation
#[derive(Debug, Deserialize)]
pub struct OAuthLoginQuery {
    pub redirect_uri: Option<String>,
}

/// OAuth provider authorization URL response
#[derive(Debug, Serialize)]
pub struct OAuthAuthorizeResponse {
    pub authorization_url: String,
}

/// OAuth token exchange response
#[derive(Debug, Serialize)]
pub struct OAuthTokenResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub user: OAuthUserInfo,
}

/// OAuth user info
#[derive(Debug, Serialize)]
pub struct OAuthUserInfo {
    pub id: String,
    pub provider: String,
    pub provider_user_id: String,
    pub email: Option<String>,
    pub name: Option<String>,
    pub avatar_url: Option<String>,
}

/// Initiate GitHub OAuth login
pub async fn github_login(
    State(state): State<AppState>,
    Query(query): Query<OAuthLoginQuery>,
) -> Result<Json<OAuthAuthorizeResponse>> {
    let config = state
        .config
        .oauth
        .github
        .as_ref()
        .ok_or_else(|| AuthError::Config("GitHub OAuth is not configured".to_string()))?;

    let redirect_uri = query
        .redirect_uri
        .unwrap_or_else(|| config.redirect_uri.clone());

    let state_param = generate_oauth_state();
    // Store state in cache for validation during callback
    state
        .oauth_state
        .store(
            &format!("oauth:state:{}", state_param),
            redirect_uri.clone(),
            600,
        )
        .await;

    let authorization_url = format!(
        "{}?client_id={}&redirect_uri={}&scope={}&state={}",
        config.authorize_url,
        url::form_urlencoded::byte_serialize(config.client_id.as_bytes()).collect::<String>(),
        url::form_urlencoded::byte_serialize(redirect_uri.as_bytes()).collect::<String>(),
        url::form_urlencoded::byte_serialize(config.scopes.as_bytes()).collect::<String>(),
        url::form_urlencoded::byte_serialize(state_param.as_bytes()).collect::<String>()
    );

    Ok(Json(OAuthAuthorizeResponse { authorization_url }))
}

/// Handle GitHub OAuth callback
pub async fn github_callback(
    State(state): State<AppState>,
    Query(query): Query<OAuthCallbackQuery>,
) -> Result<Json<OAuthTokenResponse>> {
    let config = state
        .config
        .oauth
        .github
        .as_ref()
        .ok_or_else(|| AuthError::Config("GitHub OAuth is not configured".to_string()))?;

    let redirect_uri = validate_oauth_state(&state, query.state.as_deref()).await?;

    // Exchange code for access token
    let token_response = exchange_code_for_token(config, &query.code, &redirect_uri).await?;

    // Fetch GitHub user profile and primary email
    let (github_user, primary_email) = fetch_github_profile(&token_response.access_token).await?;

    let pool = state.db.inner();

    // Find or create user
    let user = match OAuthAccount::get_by_provider_and_user_id(
        pool,
        "github",
        &github_user.id.to_string(),
    )
    .await?
    {
        Some(account) => User::get_by_id(pool, account.user_id)
            .await?
            .ok_or_else(|| AuthError::Internal("Linked user not found".to_string()))?,
        None => {
            let user = User::create(pool).await?;
            OAuthAccount::create(
                pool,
                user.id,
                "github",
                &github_user.id.to_string(),
                primary_email.as_deref(),
                github_user.name.as_deref().or(github_user.login.as_deref()),
                github_user.avatar_url.as_deref(),
                Some(&token_response.access_token),
                token_response.refresh_token.as_deref(),
                None,
            )
            .await?;
            user
        }
    };

    // Generate JWT tokens
    let jwt = state
        .jwt
        .as_ref()
        .ok_or_else(|| AuthError::Internal("JWT service not initialized".to_string()))?;

    let tokens = jwt.generate_token_pair(user.id)?;

    Ok(Json(OAuthTokenResponse {
        access_token: tokens.access_token,
        refresh_token: tokens.refresh_token,
        user: OAuthUserInfo {
            id: user.id.to_string(),
            provider: "github".to_string(),
            provider_user_id: github_user.id.to_string(),
            email: primary_email,
            name: github_user.name.clone().or(github_user.login.clone()),
            avatar_url: github_user.avatar_url.clone(),
        },
    }))
}

/// Link a GitHub OAuth account to the currently authenticated user
pub async fn github_link(
    State(state): State<AppState>,
    auth_user: AuthenticatedUser,
    Query(query): Query<OAuthCallbackQuery>,
) -> Result<Json<OAuthUserInfo>> {
    let config = state
        .config
        .oauth
        .github
        .as_ref()
        .ok_or_else(|| AuthError::Config("GitHub OAuth is not configured".to_string()))?;

    let redirect_uri = validate_oauth_state(&state, query.state.as_deref()).await?;

    let token_response = exchange_code_for_token(config, &query.code, &redirect_uri).await?;
    let (github_user, primary_email) = fetch_github_profile(&token_response.access_token).await?;

    let pool = state.db.inner();

    // Check if this GitHub account is already linked to another user
    if let Some(existing) = OAuthAccount::get_by_provider_and_user_id(
        pool,
        "github",
        &github_user.id.to_string(),
    )
    .await?
    {
        if existing.user_id != auth_user.user_id {
            return Err(AuthError::Conflict(
                "GitHub account is already linked to another user".to_string(),
            ));
        }
    } else {
        OAuthAccount::create(
            pool,
            auth_user.user_id,
            "github",
            &github_user.id.to_string(),
            primary_email.as_deref(),
            github_user.name.as_deref().or(github_user.login.as_deref()),
            github_user.avatar_url.as_deref(),
            Some(&token_response.access_token),
            token_response.refresh_token.as_deref(),
            None,
        )
        .await?;
    }

    Ok(Json(OAuthUserInfo {
        id: auth_user.user_id.to_string(),
        provider: "github".to_string(),
        provider_user_id: github_user.id.to_string(),
        email: primary_email,
        name: github_user.name.clone().or(github_user.login.clone()),
        avatar_url: github_user.avatar_url.clone(),
    }))
}

fn generate_oauth_state() -> String {
    Uuid::new_v4().to_string()
}

/// Validate the OAuth state parameter and return the cached redirect URI.
async fn validate_oauth_state(state: &AppState, query_state: Option<&str>) -> Result<String> {
    let state_key = query_state
        .map(|s| format!("oauth:state:{}", s))
        .ok_or_else(|| AuthError::BadRequest("Missing OAuth state parameter".to_string()))?;

    let redirect_uri = state
        .oauth_state
        .get(&state_key)
        .await
        .ok_or_else(|| AuthError::BadRequest("Invalid or expired OAuth state".to_string()))?;

    state.oauth_state.delete(&state_key).await;

    Ok(redirect_uri)
}

#[derive(Debug, Deserialize)]
struct GitHubTokenResponse {
    access_token: String,
    refresh_token: Option<String>,
    token_type: String,
}

#[derive(Debug, Deserialize)]
struct GitHubUser {
    id: u64,
    login: Option<String>,
    name: Option<String>,
    email: Option<String>,
    avatar_url: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct GitHubEmail {
    email: String,
    primary: bool,
    verified: bool,
}

async fn exchange_code_for_token(
    config: &OAuthProviderConfig,
    code: &str,
    redirect_uri: &str,
) -> Result<GitHubTokenResponse> {
    let client = reqwest::Client::new();
    let params = [
        ("client_id", config.client_id.as_str()),
        ("client_secret", config.client_secret.as_str()),
        ("code", code),
        ("redirect_uri", redirect_uri),
    ];

    let response = client
        .post(&config.token_url)
        .header("Accept", "application/json")
        .form(&params)
        .send()
        .await
        .map_err(|e| AuthError::Internal(format!("Failed to exchange code for token: {}", e)))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(AuthError::Internal(format!(
            "GitHub token exchange failed: {} - {}",
            status, body
        )));
    }

    let token_response: GitHubTokenResponse = response.json().await.map_err(|e| {
        AuthError::Internal(format!("Failed to parse GitHub token response: {}", e))
    })?;

    if !token_response.token_type.eq_ignore_ascii_case("bearer") {
        return Err(AuthError::Internal(
            "GitHub token type is not Bearer".to_string(),
        ));
    }

    Ok(token_response)
}

async fn fetch_github_profile(access_token: &str) -> Result<(GitHubUser, Option<String>)> {
    let user = fetch_github_user(access_token).await?;

    let emails = match fetch_github_emails(access_token).await {
        Ok(emails) => Some(emails),
        Err(e) => {
            tracing::warn!("Failed to fetch GitHub emails, continuing without email: {}", e);
            None
        }
    };

    let primary_email = emails.and_then(|emails| {
        emails
            .iter()
            .find(|e| e.primary && e.verified)
            .cloned()
            .or_else(|| emails.first().cloned())
            .map(|e| e.email)
    });

    Ok((user, primary_email))
}

async fn fetch_github_user(access_token: &str) -> Result<GitHubUser> {
    let client = reqwest::Client::new();
    let response = client
        .get("https://api.github.com/user")
        .header("Authorization", format!("Bearer {}", access_token))
        .header("Accept", "application/json")
        .header("User-Agent", "astral-key")
        .send()
        .await
        .map_err(|e| AuthError::Internal(format!("Failed to fetch GitHub user: {}", e)))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(AuthError::Internal(format!(
            "GitHub user fetch failed: {} - {}",
            status, body
        )));
    }

    let user: GitHubUser = response.json().await.map_err(|e| {
        AuthError::Internal(format!("Failed to parse GitHub user response: {}", e))
    })?;

    Ok(user)
}

async fn fetch_github_emails(access_token: &str) -> Result<Vec<GitHubEmail>> {
    let client = reqwest::Client::new();
    let response = client
        .get("https://api.github.com/user/emails")
        .header("Authorization", format!("Bearer {}", access_token))
        .header("Accept", "application/json")
        .header("User-Agent", "astral-key")
        .send()
        .await
        .map_err(|e| AuthError::Internal(format!("Failed to fetch GitHub emails: {}", e)))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(AuthError::Internal(format!(
            "GitHub emails fetch failed: {} - {}",
            status, body
        )));
    }

    let emails: Vec<GitHubEmail> = response.json().await.map_err(|e| {
        AuthError::Internal(format!("Failed to parse GitHub emails response: {}", e))
    })?;

    Ok(emails)
}
