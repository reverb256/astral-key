//! Astral Key - FIDO2/Passkey authentication handlers

use axum::{extract::State, Json};
use serde::{Deserialize, Serialize};

use crate::error::{AuthError, Result};
use crate::state::AppState;

/// Get registration options
pub async fn register_options(State(_state): State<AppState>) -> Result<Json<RegistrationOptions>> {
    // TODO: Generate WebAuthn registration options
    Err(AuthError::Internal(
        "FIDO2 registration not yet implemented".to_string(),
    ))
}

/// Verify registration
pub async fn register_verify(
    State(_state): State<AppState>,
    Json(_request): Json<RegisterVerifyRequest>,
) -> Result<Json<serde_json::Value>> {
    // TODO: Verify WebAuthn registration
    Err(AuthError::Internal(
        "FIDO2 registration verification not yet implemented".to_string(),
    ))
}

/// Get authentication options
pub async fn authenticate_options(
    State(_state): State<AppState>,
) -> Result<Json<AuthenticationOptions>> {
    // TODO: Generate WebAuthn authentication options
    Err(AuthError::Internal(
        "FIDO2 authentication not yet implemented".to_string(),
    ))
}

/// Verify authentication
pub async fn authenticate_verify(
    State(_state): State<AppState>,
    Json(_request): Json<AuthenticateVerifyRequest>,
) -> Result<Json<serde_json::Value>> {
    // TODO: Verify WebAuthn authentication
    Err(AuthError::Internal(
        "FIDO2 authentication verification not yet implemented".to_string(),
    ))
}

/// List credentials
pub async fn credentials(State(_state): State<AppState>) -> Result<Json<serde_json::Value>> {
    // TODO: List user credentials
    Err(AuthError::Internal(
        "Credential listing not yet implemented".to_string(),
    ))
}

/// Delete credential
pub async fn delete_credential(State(_state): State<AppState>) -> Result<Json<serde_json::Value>> {
    // TODO: Delete credential
    Err(AuthError::Internal(
        "Credential deletion not yet implemented".to_string(),
    ))
}

/// Registration options response
#[derive(Serialize)]
pub struct RegistrationOptions {
    pub challenge: String,
    pub rp: RelyingParty,
    pub user: PublicKeyCredentialUserEntity,
    pub pub_key_cred_params: Vec<PublicKeyCredentialParameters>,
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
