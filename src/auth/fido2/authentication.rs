//! Astral Key - FIDO2 authentication flow with webauthn-rs

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use uuid::Uuid;
use webauthn_rs::prelude::*;
use webauthn_rs_core::proto::{AuthenticatorAssertionResponseRaw, AuthenticationExtensionsClientOutputs};

use crate::auth::fido2::Fido2Service;
use crate::auth::fido2::types::{
    AllowCredential, AuthenticationChallenge, AuthenticationRequest, AuthenticationResult,
};
use crate::db::models::Fido2Credential;
use crate::error::{AuthError, Result};
use crate::state::AppState;

/// Start FIDO2 authentication - generate assertion options
pub async fn start_authentication(
    state: &AppState,
    user_id: Uuid,
) -> Result<AuthenticationChallenge> {
    let fido2 = state
        .fido2
        .as_ref()
        .ok_or_else(|| AuthError::Internal("FIDO2 service not initialized".to_string()))?;

    // Get user's credentials from database
    let pool = state.db.inner();
    let db_credentials = Fido2Credential::get_by_user(pool, user_id).await?;

    if db_credentials.is_empty() {
        return Err(AuthError::BadRequest(
            "No credentials registered for this user".to_string(),
        ));
    }

    // Convert database credentials to Passkey format
    // We store the full Passkey as JSON in the public_key field
    let mut passkeys = Vec::new();
    for db_cred in db_credentials {
        // Deserialize the Passkey from stored JSON
        let passkey: Passkey = serde_json::from_str(&db_cred.public_key).map_err(|e| {
            AuthError::Internal(format!("Failed to deserialize stored passkey: {}", e))
        })?;
        passkeys.push(passkey);
    }

    // Generate authentication challenge using webauthn-rs
    let (rcr, auth_state) = fido2
        .webauthn()
        .start_passkey_authentication(&passkeys)
        .map_err(|e| AuthError::Internal(format!("Failed to start authentication: {}", e)))?;

    // Serialize authentication state for storage
    let auth_state_json =
        serde_json::to_string(&auth_state).map_err(|e| {
            AuthError::Internal(format!("Failed to serialize authentication state: {}", e))
        })?;

    // Store state in cache (5 minute TTL)
    fido2.store_state(user_id, "authenticate", auth_state_json).await?;

    // Convert RequestChallengeResponse to our AuthenticationChallenge format
    let challenge = serde_json::to_string(&rcr.public_key.challenge)
        .map_err(|e| AuthError::Internal(format!("Failed to serialize challenge: {}", e)))?;
    let allow_credentials = rcr
        .public_key
        .allow_credentials
        .iter()
        .map(|cred| {
            let id_str = serde_json::to_string(&cred.id)
                .map_err(|e| AuthError::Internal(format!("Failed to serialize credential ID: {}", e)))?;
            Ok::<_, AuthError>(AllowCredential {
                type_: "public-key".to_string(),
                id: id_str,
                transports: None,
            })
        })
        .collect::<Result<Vec<_>>>()?;

    Ok(AuthenticationChallenge {
        challenge,
        allow_credentials,
        timeout: rcr.public_key.timeout.unwrap_or(60000) as u64,
        user_verification: "preferred".to_string(),
    })
}

/// Finish FIDO2 authentication - verify assertion signature
pub async fn finish_authentication(
    state: &AppState,
    user_id: Uuid,
    request: AuthenticationRequest,
) -> Result<AuthenticationResult> {
    let fido2 = state
        .fido2
        .as_ref()
        .ok_or_else(|| AuthError::Internal("FIDO2 service not initialized".to_string()))?;

    // Retrieve authentication state from cache
    let auth_state_json = fido2
        .get_state(user_id, "authenticate")
        .await?
        .ok_or_else(|| AuthError::BadRequest("Authentication challenge expired or not found".to_string()))?;

    // Deserialize authentication state
    let auth_state: PasskeyAuthentication = serde_json::from_str(&auth_state_json).map_err(
        |e| AuthError::Internal(format!("Failed to deserialize authentication state: {}", e)),
    )?;

    // Convert AuthenticationRequest to PublicKeyCredential
    // Helper function to decode base64url
    let decode_b64 = |s: &str| -> Result<Vec<u8>> {
        URL_SAFE_NO_PAD
            .decode(s)
            .map_err(|e| AuthError::BadRequest(format!("Invalid base64url: {}", e)))
    };

    let auth_credential = PublicKeyCredential {
        id: request.id.clone(),
        raw_id: Base64UrlSafeData::from(decode_b64(&request.raw_id)?),
        response: AuthenticatorAssertionResponseRaw {
            client_data_json: Base64UrlSafeData::from(decode_b64(&request.response.client_data_json)?),
            authenticator_data: Base64UrlSafeData::from(decode_b64(&request.response.authenticator_data)?),
            signature: Base64UrlSafeData::from(decode_b64(&request.response.signature)?),
            user_handle: request
                .response
                .user_handle
                .as_ref()
                .map(|h| decode_b64(h))
                .transpose()?
                .map(Base64UrlSafeData::from),
        },
        type_: request.type_,
        extensions: AuthenticationExtensionsClientOutputs {
            appid: None,
            hmac_get_secret: None,
        },
    };

    // Verify authentication using webauthn-rs (full cryptographic verification)
    let auth_result = fido2
        .webauthn()
        .finish_passkey_authentication(&auth_credential, &auth_state)
        .map_err(|e| {
            // Provide detailed error message for debugging
            AuthError::BadRequest(format!("Assertion verification failed: {}", e))
        })?;

    // Consume the challenge (one-time use)
    fido2.consume_state(user_id, "authenticate").await?;

    // Extract credential ID and new counter
    let credential_id = serde_json::to_string(auth_result.cred_id())
        .map_err(|e| AuthError::Internal(format!("Failed to serialize credential ID: {}", e)))?;
    let new_counter = auth_result.counter() as u64;

    Ok(AuthenticationResult {
        credential_id,
        new_counter,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
}
