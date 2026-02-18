//! Astral Key - FIDO2 registration flow

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use std::time::Duration;
use uuid::Uuid;

use crate::auth::fido2::types::{
    AllowCredential, AuthenticationChallenge, AuthenticationRequest, AuthenticationResponse,
    PublicKeyCredentialParameters, RegistrationChallenge, RegistrationRequest, RegistrationResult,
    RelyingParty, WebauthnUser,
};
use crate::cache::pool::RedisPool;
use crate::error::{AuthError, Result};
use crate::state::AppState;

/// Challenge storage key prefix
const CHALLENGE_PREFIX: &str = "fido2_challenge:";

/// Start FIDO2 registration
pub async fn start_registration(
    state: &AppState,
    user_id: Uuid,
    username: &str,
    display_name: &str,
) -> Result<RegistrationChallenge> {
    // Generate challenge
    let challenge_value = Uuid::new_v4().as_bytes().to_vec();
    let challenge_b64 = URL_SAFE_NO_PAD.encode(&challenge_value);

    // Store challenge in cache (5 minute TTL)
    let challenge_key = format!("{}{}:{}", CHALLENGE_PREFIX, user_id, "register");
    state
        .cache
        .set_with_expiry(&challenge_key, &challenge_b64, 300)
        .await?;

    // Create user entity
    let user = WebauthnUser {
        id: URL_SAFE_NO_PAD.encode(user_id.as_bytes()),
        name: username.to_string(),
        display_name: display_name.to_string(),
    };

    // Create relying party
    let rp = RelyingParty {
        id: state.config.fido2.rp_id.clone(),
        name: state.config.fido2.rp_name.clone(),
    };

    // Public key credential parameters
    let pub_key_cred_params = vec![
        PublicKeyCredentialParameters {
            type_: "public-key".to_string(),
            alg: -7, // ES256
        },
        PublicKeyCredentialParameters {
            type_: "public-key".to_string(),
            alg: -257, // RS256
        },
    ];

    Ok(RegistrationChallenge {
        challenge: challenge_b64,
        user,
        rp,
        pub_key_cred_params,
        timeout: 60000, // 60 seconds
    })
}

/// Finish FIDO2 registration
pub async fn finish_registration(
    state: &AppState,
    user_id: Uuid,
    request: RegistrationRequest,
) -> Result<RegistrationResult> {
    // Validate request
    if request.type_ != "public-key" {
        return Err(AuthError::BadRequest("Invalid credential type".to_string()));
    }

    // Extract credential ID and public key from attestation object
    // For now, we'll store base64-encoded data
    // In production, use webauthn-rs to properly verify attestation

    let credential_id = request.raw_id.clone();

    // Decode the attestation object to extract public key
    // For simplicity, we'll store the entire attestation object as the public key
    // This should be replaced with proper WebAuthn verification
    let attestation_object = &request.response.attestation_object;
    let public_key = attestation_object.clone(); // Store attestation object for now

    Ok(RegistrationResult {
        credential_id,
        public_key,
        counter: 0,
        transports: None, // TODO: Extract from response
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore]
    async fn test_start_registration() {
        // Requires database and cache connection
    }
}
