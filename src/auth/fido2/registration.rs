//! Astral Key - FIDO2 registration flow with webauthn-rs

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use uuid::Uuid;
use webauthn_rs::prelude::*;
use webauthn_rs_core::proto::{
    AuthenticatorAttestationResponseRaw, RegistrationExtensionsClientOutputs,
};

use crate::auth::fido2::types::{
    PublicKeyCredentialParameters, RegistrationChallenge, RegistrationRequest, RegistrationResult,
    RelyingParty, WebauthnUser,
};
use crate::db::models::Fido2Credential;
use crate::error::{AuthError, Result};
use crate::state::AppState;

/// Start FIDO2 registration - generate credential creation options
pub async fn start_registration(
    state: &AppState,
    user_id: Uuid,
    username: &str,
    display_name: &str,
) -> Result<RegistrationChallenge> {
    let fido2 = state
        .fido2
        .as_ref()
        .ok_or_else(|| AuthError::Internal("FIDO2 service not initialized".to_string()))?;

    // Get any existing credentials for this user (for exclusion)
    let pool = state.db.inner();
    let existing_credentials = Fido2Credential::get_by_user(pool, user_id).await?;

    // Convert existing credentials to CredentialID format for exclusion
    let exclude_credentials: Option<Vec<CredentialID>> = if existing_credentials.is_empty() {
        None
    } else {
        Some(
            existing_credentials
                .iter()
                .filter_map(|cred| {
                    // Deserialize the Passkey to get the credential ID
                    let passkey: Passkey = serde_json::from_str(&cred.public_key).ok()?;
                    Some(passkey.cred_id().clone())
                })
                .collect(),
        )
    };

    // Generate registration challenge using webauthn-rs
    let (ccr, reg_state) = fido2
        .webauthn()
        .start_passkey_registration(user_id, username, display_name, exclude_credentials)
        .map_err(|e| AuthError::Internal(format!("Failed to start registration: {}", e)))?;

    // Serialize registration state for storage
    let reg_state_json = serde_json::to_string(&reg_state).map_err(|e| {
        AuthError::Internal(format!("Failed to serialize registration state: {}", e))
    })?;

    // Store state in cache (5 minute TTL)
    fido2
        .store_state(user_id, "register", reg_state_json)
        .await?;

    // Convert CreationChallengeResponse to our RegistrationChallenge format
    let challenge = serde_json::to_string(&ccr.public_key.challenge)
        .map_err(|e| AuthError::Internal(format!("Failed to serialize challenge: {}", e)))?;
    let user = WebauthnUser {
        id: serde_json::to_string(&ccr.public_key.user.id)
            .map_err(|e| AuthError::Internal(format!("Failed to serialize user ID: {}", e)))?,
        name: ccr.public_key.user.name,
        display_name: ccr.public_key.user.display_name,
    };
    let rp = RelyingParty {
        id: ccr.public_key.rp.id,
        name: ccr.public_key.rp.name,
    };

    // Convert pub_key_cred_params
    let pub_key_cred_params = ccr
        .public_key
        .pub_key_cred_params
        .iter()
        .map(|alg| PublicKeyCredentialParameters {
            type_: "public-key".to_string(),
            alg: alg.alg,
        })
        .collect();

    Ok(RegistrationChallenge {
        challenge,
        user,
        rp,
        pub_key_cred_params,
        timeout: ccr.public_key.timeout.unwrap_or(60000) as u64,
    })
}

/// Finish FIDO2 registration - verify attestation and extract credential
pub async fn finish_registration(
    state: &AppState,
    user_id: Uuid,
    request: RegistrationRequest,
) -> Result<RegistrationResult> {
    let fido2 = state
        .fido2
        .as_ref()
        .ok_or_else(|| AuthError::Internal("FIDO2 service not initialized".to_string()))?;

    // Retrieve registration state from cache
    let reg_state_json = fido2.get_state(user_id, "register").await?.ok_or_else(|| {
        AuthError::BadRequest("Registration challenge expired or not found".to_string())
    })?;

    // Deserialize registration state
    let reg_state: PasskeyRegistration = serde_json::from_str(&reg_state_json).map_err(|e| {
        AuthError::Internal(format!("Failed to deserialize registration state: {}", e))
    })?;

    // Convert RegistrationRequest to RegisterPublicKeyCredential
    // Helper function to decode base64url
    let decode_b64 = |s: &str| -> Result<Vec<u8>> {
        URL_SAFE_NO_PAD
            .decode(s)
            .map_err(|e| AuthError::BadRequest(format!("Invalid base64url: {}", e)))
    };

    let reg_credential = RegisterPublicKeyCredential {
        id: request.id.clone(),
        raw_id: Base64UrlSafeData::from(decode_b64(&request.raw_id)?),
        response: AuthenticatorAttestationResponseRaw {
            client_data_json: Base64UrlSafeData::from(decode_b64(
                &request.response.client_data_json,
            )?),
            attestation_object: Base64UrlSafeData::from(decode_b64(
                &request.response.attestation_object,
            )?),
            transports: None,
        },
        type_: request.type_,
        extensions: RegistrationExtensionsClientOutputs {
            appid: None,
            cred_props: None,
            hmac_secret: None,
            cred_protect: None,
            min_pin_length: None,
        },
    };

    // Verify registration using webauthn-rs (full cryptographic verification)
    let passkey = fido2
        .webauthn()
        .finish_passkey_registration(&reg_credential, &reg_state)
        .map_err(|e| {
            // Provide detailed error message for debugging
            AuthError::BadRequest(format!("Attestation verification failed: {}", e))
        })?;

    // Consume the challenge (one-time use)
    fido2.consume_state(user_id, "register").await?;

    // Extract credential data for storage
    let credential_id = serde_json::to_string(passkey.cred_id())
        .map_err(|e| AuthError::Internal(format!("Failed to serialize credential ID: {}", e)))?;

    // Store the full Passkey as JSON for later use
    let public_key = serde_json::to_string(&passkey)
        .map_err(|e| AuthError::Internal(format!("Failed to serialize passkey: {}", e)))?;

    // For new passkeys, counter starts at 0
    let counter = 0;

    // Extract transports if available (from credential)
    let transports = None; // Can be extracted from passkey if needed

    Ok(RegistrationResult {
        credential_id,
        public_key,
        counter,
        transports,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
}
