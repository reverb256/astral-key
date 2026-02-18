//! Astral Key - FIDO2 types

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Registration challenge response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistrationChallenge {
    pub challenge: String,
    pub user: WebauthnUser,
    pub rp: RelyingParty,
    pub pub_key_cred_params: Vec<PublicKeyCredentialParameters>,
    pub timeout: u64,
}

/// Authentication challenge response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthenticationChallenge {
    pub challenge: String,
    pub allow_credentials: Vec<AllowCredential>,
    pub timeout: u64,
    pub user_verification: String,
}

/// WebAuthn user
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebauthnUser {
    pub id: String,
    pub name: String,
    pub display_name: String,
}

/// Relying party
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelyingParty {
    pub id: String,
    pub name: String,
}

/// Public key credential parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublicKeyCredentialParameters {
    #[serde(rename = "type")]
    pub type_: String,
    pub alg: i64,
}

/// Allow credential (for authentication)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AllowCredential {
    #[serde(rename = "type")]
    pub type_: String,
    pub id: String,
    pub transports: Option<Vec<String>>,
}

/// Registration request
#[derive(Debug, Deserialize)]
pub struct RegistrationRequest {
    pub id: String,
    pub raw_id: String,
    pub response: RegistrationResponse,
    #[serde(rename = "type")]
    pub type_: String,
}

/// Registration response
#[derive(Debug, Deserialize)]
pub struct RegistrationResponse {
    pub client_data_json: String,
    pub attestation_object: String,
}

/// Authentication request
#[derive(Debug, Deserialize)]
pub struct AuthenticationRequest {
    pub id: String,
    pub raw_id: String,
    pub response: AuthenticationResponse,
    #[serde(rename = "type")]
    pub type_: String,
}

/// Authentication response
#[derive(Debug, Deserialize)]
pub struct AuthenticationResponse {
    pub client_data_json: String,
    pub authenticator_data: String,
    pub signature: String,
    pub user_handle: Option<String>,
}

/// Registration result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistrationResult {
    pub credential_id: String,
    pub public_key: String,
    pub counter: u64,
    pub transports: Option<Vec<String>>,
}

/// Authentication result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthenticationResult {
    pub credential_id: String,
    pub new_counter: u64,
}
