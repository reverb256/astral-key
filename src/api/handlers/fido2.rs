//! Astral Key - FIDO2/Passkey authentication handlers

use axum::{extract::State, Json};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::auth::fido2::{start_registration, finish_registration, start_authentication, finish_authentication};
use crate::auth::jwt::{TokenPair, AuthenticatedUser};
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

    Ok(Json(RegistrationOptions {
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
        pub_key_cred_params: challenge.pub_key_cred_params.into_iter().map(|p| PublicKeyCredentialParameters {
            type_: p.type_,
            alg: p.alg,
        }).collect(),
        timeout: challenge.timeout,
        attestation: "none".to_string(),
        authenticator_selection: AuthenticatorSelection {
            authenticator_attach: "platform".to_string(),
            user_verification: "preferred".to_string(),
        },
    }))
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
    use crate::db::models::{User, Fido2Credential};

    // Get user by username from database
    let pool = state.db.inner();

    // For FIDO2 authentication, we need to look up the user
    // Since we don't have username-based user lookup yet, we'll use a different approach:
    // The authenticate_options should be called with a user identifier
    // For now, we'll require the client to know their user ID or we'll create a session-based lookup

    // Try to parse as UUID first
    let user_id = if let Ok(uuid) = Uuid::parse_str(&request.username) {
        uuid
    } else {
        // If not a UUID, look for users by some other means
        // For now, we'll return an error if no valid user ID is provided
        return Err(AuthError::BadRequest(
            "Username must be a valid user UUID".to_string(),
        ));
    };

    // Verify user exists
    let _user = User::get_by_id(pool, user_id)
        .await?
        .ok_or_else(|| AuthError::NotFound("User not found".to_string()))?;

    let challenge = start_authentication(&state, user_id).await?;

    Ok(Json(AuthenticationOptions {
        challenge: challenge.challenge,
        allow_credentials: challenge.allow_credentials.into_iter().map(|c| AllowedCredential {
            type_: c.type_,
            id: c.id,
        }).collect(),
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
        serde_json::from_value(request.response.clone())
            .map_err(|e| AuthError::BadRequest(format!("Invalid authentication response: {}", e)))?;

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
    credential.update_usage(pool, result.new_counter as i64).await?;

    // Generate JWT tokens
    let jwt = state.jwt.as_ref().ok_or_else(|| {
        AuthError::Internal("JWT service not initialized".to_string())
    })?;

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
    // TODO: Get user credentials from database
    let pool = state.db.inner();

    // For now, return empty list
    let credentials = crate::db::models::Fido2Credential::get_by_user(pool, auth_user.user_id)
        .await
        .unwrap_or_default();

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
