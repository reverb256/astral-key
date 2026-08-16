//! Astral Key - FIDO2/Passkey authentication handlers

use axum::{extract::State, Json};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::auth::fido2::{
    finish_authentication, finish_registration, start_authentication, start_registration,
};
use crate::auth::jwt::AuthenticatedUser;
use crate::error::{AuthError, Result};
use crate::state::AppState;

/// Get registration options
pub async fn register_options(
    State(state): State<AppState>,
    auth_user: AuthenticatedUser,
    Json(request): Json<RegisterOptionsRequest>,
) -> Result<Json<RegistrationOptions>> {
    // Generate registration challenge
    let challenge = start_registration(
        &state,
        auth_user.user_id,
        &request.username,
        &request.display_name,
    )
    .await?;

    Ok(Json(to_registration_options(challenge)))
}

/// First-user bootstrap — registration options.
///
/// Allowed ONLY while no user has a passkey yet (fresh install). After the
/// first credential exists the normal JWT-protected register flow takes over.
pub async fn bootstrap_options(
    State(state): State<AppState>,
    Json(request): Json<BootstrapOptionsRequest>,
) -> Result<Json<RegistrationOptions>> {
    use crate::db::models::User;

    let pool = state.db.inner();
    if User::credentialed_user_count(pool).await? > 0 {
        return Err(AuthError::Forbidden(
            "First-user registration is closed — sign in with your existing passkey".to_string(),
        ));
    }

    let email = request.email.trim().to_lowercase();
    if !email.contains('@') || email.len() < 3 {
        return Err(AuthError::BadRequest(
            "A valid email address is required".to_string(),
        ));
    }
    let display_name = request.display_name.trim();
    if display_name.is_empty() {
        return Err(AuthError::BadRequest(
            "A display name is required".to_string(),
        ));
    }

    // Reuse a leftover user row from an aborted bootstrap attempt, if any.
    let user = match User::get_by_email(pool, &email).await? {
        Some(u) => u,
        None => User::create_with_email(pool, Uuid::new_v4(), &email, display_name).await?,
    };

    let challenge = start_registration(&state, user.id, &email, display_name).await?;
    Ok(Json(to_registration_options(challenge)))
}

/// First-user bootstrap — complete registration and mint a session token.
pub async fn bootstrap_verify(
    State(state): State<AppState>,
    Json(request): Json<BootstrapVerifyRequest>,
) -> Result<Json<TokenResponse>> {
    use crate::db::models::{Fido2Credential, User};

    let pool = state.db.inner();

    // The registration challenge is keyed by user id; resolve via the email
    // the browser submitted with the ceremony result.
    let user = User::get_by_email(pool, &request.email)
        .await?
        .ok_or_else(|| {
            AuthError::BadRequest(
                "Bootstrap session not found — please start registration again".to_string(),
            )
        })?;

    // Convert response JSON to internal type
    let response: crate::auth::fido2::types::RegistrationResponse =
        serde_json::from_value(request.response.clone())
            .map_err(|e| AuthError::BadRequest(format!("Invalid registration response: {}", e)))?;

    let registration_request = crate::auth::fido2::types::RegistrationRequest {
        id: request.id.clone(),
        raw_id: request.raw_id.clone(),
        response,
        type_: request.type_.clone(),
        transports: vec![],
    };

    // Finish registration (validates challenge, returns credential info)
    let result = finish_registration(&state, user.id, registration_request).await?;

    // Store credential in database
    Fido2Credential::create(pool, user.id, &result.credential_id, &result.public_key).await?;

    // Mint a session so the browser can complete the OIDC flow immediately.
    let jwt = state
        .jwt
        .as_ref()
        .ok_or_else(|| AuthError::Internal("JWT service not initialized".to_string()))?;
    let tokens = jwt.generate_token_pair(user.id)?;

    Ok(Json(TokenResponse {
        access_token: tokens.access_token,
        refresh_token: tokens.refresh_token,
    }))
}

/// Convert a registration challenge into the API response shape.
fn to_registration_options(
    challenge: crate::auth::fido2::types::RegistrationChallenge,
) -> RegistrationOptions {
    RegistrationOptions {
        challenge: challenge.challenge,
        rp: RelyingParty {
            id: challenge.rp.id,
            name: challenge.rp.name,
        },
        user: PublicKeyCredentialUserEntity {
            id: challenge.user.id,
            name: challenge.user.name,
            display_name: challenge.user.display_name,
        },
        pub_key_cred_params: challenge
            .pub_key_cred_params
            .into_iter()
            .map(|p| PublicKeyCredentialParameters {
                type_: p.type_,
                alg: p.alg,
            })
            .collect(),
        timeout: challenge.timeout,
        attestation: "none".to_string(),
        authenticator_selection: AuthenticatorSelection {
            authenticator_attach: "platform".to_string(),
            user_verification: "preferred".to_string(),
        },
    }
}

/// Verify registration
pub async fn register_verify(
    State(state): State<AppState>,
    auth_user: AuthenticatedUser,
    Json(request): Json<RegisterVerifyRequest>,
) -> Result<Json<CredentialResponse>> {
    use crate::db::models::Fido2Credential;

    // Convert response JSON to internal type
    let response: crate::auth::fido2::types::RegistrationResponse =
        serde_json::from_value(request.response.clone())
            .map_err(|e| AuthError::BadRequest(format!("Invalid registration response: {}", e)))?;

    // Convert request to internal type
    let registration_request = crate::auth::fido2::types::RegistrationRequest {
        id: request.id.clone(),
        raw_id: request.raw_id.clone(),
        response,
        type_: request.type_.clone(),
        transports: vec![],
    };

    // Finish registration (validates challenge, returns credential info)
    let result = finish_registration(&state, auth_user.user_id, registration_request).await?;

    // Store credential in database
    let pool = state.db.inner();
    let credential = Fido2Credential::create(
        pool,
        auth_user.user_id,
        &result.credential_id,
        &result.public_key,
    )
    .await?;

    Ok(Json(CredentialResponse {
        id: credential.id.to_string(),
        status: "success".to_string(),
    }))
}

/// Get authentication options
pub async fn authenticate_options(
    State(state): State<AppState>,
    Json(request): Json<AuthenticateOptionsRequest>,
) -> Result<Json<AuthenticationOptions>> {
    use crate::db::models::User;

    // Get user by username from database
    let pool = state.db.inner();

    // For FIDO2 authentication, we need to look up the user
    // Since we don't have username-based user lookup yet, we'll use a different approach:
    // The authenticate_options should be called with a user identifier
    // For now, we'll require the client to know their user ID or we'll create a session-based lookup

    // Accept either a user UUID (API clients) or an email (OIDC login page).
    let user_id = if let Ok(uuid) = Uuid::parse_str(&request.username) {
        uuid
    } else {
        User::get_by_email(pool, &request.username)
            .await?
            .ok_or_else(|| AuthError::NotFound("User not found".to_string()))?
            .id
    };

    // Verify user exists
    let _user = User::get_by_id(pool, user_id)
        .await?
        .ok_or_else(|| AuthError::NotFound("User not found".to_string()))?;

    let challenge = start_authentication(&state, user_id).await?;

    Ok(Json(AuthenticationOptions {
        challenge: challenge.challenge,
        allow_credentials: challenge
            .allow_credentials
            .into_iter()
            .map(|c| AllowedCredential {
                type_: c.type_,
                id: c.id,
            })
            .collect(),
        user_verification: "preferred".to_string(),
        timeout: challenge.timeout,
    }))
}

/// Verify authentication
pub async fn authenticate_verify(
    State(state): State<AppState>,
    Json(request): Json<AuthenticateVerifyRequest>,
) -> Result<Json<TokenResponse>> {
    use crate::db::models::Fido2Credential;

    // Convert response JSON to internal type
    let response: crate::auth::fido2::types::AuthenticationResponse =
        serde_json::from_value(request.response.clone()).map_err(|e| {
            AuthError::BadRequest(format!("Invalid authentication response: {}", e))
        })?;

    // Convert request to internal type
    let auth_request = crate::auth::fido2::types::AuthenticationRequest {
        id: request.id.clone(),
        raw_id: request.raw_id.clone(),
        response,
        type_: request.type_.clone(),
    };

    // Look up credential from database
    let pool = state.db.inner();
    let credential = Fido2Credential::get_by_credential_id(pool, &auth_request.raw_id)
        .await?
        .ok_or_else(|| AuthError::Unauthorized("Credential not found".to_string()))?;

    // Finish authentication (validates assertion, returns new counter)
    let user_id = credential.user_id;
    let result = finish_authentication(&state, user_id, auth_request).await?;

    // Update credential usage counter
    credential
        .update_usage(pool, result.new_counter as i64)
        .await?;

    // Generate JWT tokens
    let jwt = state
        .jwt
        .as_ref()
        .ok_or_else(|| AuthError::Internal("JWT service not initialized".to_string()))?;

    let tokens = jwt.generate_token_pair(user_id)?;

    Ok(Json(TokenResponse {
        access_token: tokens.access_token,
        refresh_token: tokens.refresh_token,
    }))
}

/// List credentials
pub async fn credentials(
    State(state): State<AppState>,
    auth_user: AuthenticatedUser,
) -> Result<Json<Vec<CredentialInfo>>> {
    let pool = state.db.inner();

    let credentials =
        crate::db::models::Fido2Credential::get_by_user(pool, auth_user.user_id).await?;

    let credential_infos: Vec<CredentialInfo> = credentials
        .into_iter()
        .map(|c| CredentialInfo {
            id: c.id,
            name: c.name.unwrap_or_else(|| "Unnamed Credential".to_string()),
            created_at: c.created_at,
            last_used_at: c.last_used_at,
        })
        .collect();

    Ok(Json(credential_infos))
}

/// Delete credential
pub async fn delete_credential(
    State(state): State<AppState>,
    auth_user: AuthenticatedUser,
    axum::extract::Path(credential_id): axum::extract::Path<Uuid>,
) -> Result<Json<serde_json::Value>> {
    let pool = state.db.inner();

    // Get credential
    let credential = crate::db::models::Fido2Credential::get_by_id(pool, credential_id)
        .await?
        .ok_or_else(|| AuthError::NotFound("Credential not found".to_string()))?;

    // Check ownership
    if credential.user_id != auth_user.user_id {
        return Err(AuthError::Forbidden("Not your credential".to_string()));
    }

    // Delete credential
    credential.delete(pool).await?;

    Ok(Json(serde_json::json!({
        "message": "Credential deleted successfully",
    })))
}

/// Registration options response
#[derive(Serialize)]
pub struct RegistrationOptions {
    pub challenge: String,
    pub rp: RelyingParty,
    pub user: PublicKeyCredentialUserEntity,
    pub pub_key_cred_params: Vec<PublicKeyCredentialParameters>,
    pub timeout: u64,
    pub attestation: String,
    pub authenticator_selection: AuthenticatorSelection,
}

/// Relying party info
#[derive(Serialize)]
pub struct RelyingParty {
    pub name: String,
    pub id: String,
}

/// User entity
#[derive(Serialize)]
pub struct PublicKeyCredentialUserEntity {
    pub id: String,
    pub name: String,
    pub display_name: String,
}

/// Credential parameters
#[derive(Serialize)]
pub struct PublicKeyCredentialParameters {
    #[serde(rename = "type")]
    pub type_: String,
    pub alg: i64,
}

/// Authenticator selection
#[derive(Serialize)]
pub struct AuthenticatorSelection {
    pub authenticator_attach: String,
    pub user_verification: String,
}

/// Registration verify request
#[derive(Deserialize)]
pub struct RegisterVerifyRequest {
    pub id: String,
    pub raw_id: String,
    pub response: serde_json::Value,
    #[serde(rename = "type")]
    pub type_: String,
}

/// Authentication options response
#[derive(Serialize)]
pub struct AuthenticationOptions {
    pub challenge: String,
    pub allow_credentials: Vec<AllowedCredential>,
    pub user_verification: String,
    pub timeout: u64,
}

/// Allowed credential
#[derive(Serialize)]
pub struct AllowedCredential {
    #[serde(rename = "type")]
    pub type_: String,
    pub id: String,
}

/// Authentication verify request
#[derive(Deserialize)]
pub struct AuthenticateVerifyRequest {
    pub id: String,
    pub raw_id: String,
    pub response: serde_json::Value,
    #[serde(rename = "type")]
    pub type_: String,
}

/// Register options request
#[derive(Deserialize)]
pub struct RegisterOptionsRequest {
    pub username: String,
    pub display_name: String,
}

/// First-user bootstrap: registration options request
#[derive(Deserialize)]
pub struct BootstrapOptionsRequest {
    pub email: String,
    pub display_name: String,
}

/// First-user bootstrap: registration verify request
#[derive(Deserialize)]
pub struct BootstrapVerifyRequest {
    pub id: String,
    pub raw_id: String,
    pub response: serde_json::Value,
    #[serde(rename = "type")]
    pub type_: String,
    pub email: String,
}

/// Authenticate options request
#[derive(Deserialize)]
pub struct AuthenticateOptionsRequest {
    pub username: String,
}

/// Credential response
#[derive(Serialize)]
pub struct CredentialResponse {
    pub id: String,
    pub status: String,
}

/// Token response
#[derive(Serialize)]
pub struct TokenResponse {
    pub access_token: String,
    pub refresh_token: String,
}

/// Credential info
#[derive(Serialize)]
pub struct CredentialInfo {
    pub id: Uuid,
    pub name: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub last_used_at: Option<chrono::DateTime<chrono::Utc>>,
}
