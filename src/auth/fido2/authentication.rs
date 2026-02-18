//! Astral Key - FIDO2 authentication flow

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use uuid::Uuid;

use crate::auth::fido2::types::{
    AllowCredential, AuthenticationChallenge, AuthenticationRequest, AuthenticationResult,
};
use crate::cache::pool::RedisPool;
use crate::db::models::Fido2Credential;
use crate::error::{AuthError, Result};
use crate::state::AppState;

/// Challenge storage key prefix
const CHALLENGE_PREFIX: &str = "fido2_challenge:";

/// Start FIDO2 authentication
pub async fn start_authentication(
    state: &AppState,
    user_id: Uuid,
) -> Result<AuthenticationChallenge> {
    // Get user's credentials
    let pool = state.db.inner();
    let credentials = Fido2Credential::get_by_user(pool, user_id).await?;

    if credentials.is_empty() {
        return Err(AuthError::BadRequest(
            "No credentials registered for this user".to_string(),
        ));
    }

    // Generate challenge
    let challenge_value = Uuid::new_v4().as_bytes().to_vec();
    let challenge_b64 = URL_SAFE_NO_PAD.encode(&challenge_value);

    // Store challenge in cache (5 minute TTL)
    let challenge_key = format!("{}{}:{}", CHALLENGE_PREFIX, user_id, "authenticate");
    state
        .cache
        .set_with_expiry(&challenge_key, &challenge_b64, 300)
        .await?;

    // Create allow credentials list
    let allow_credentials = credentials
        .iter()
        .map(|cred| AllowCredential {
            type_: "public-key".to_string(),
            id: cred.credential_id.clone(),
            transports: cred.transport.clone().map(|t| vec![t]),
        })
        .collect();

    Ok(AuthenticationChallenge {
        challenge: challenge_b64,
        allow_credentials,
        timeout: 60000,
        user_verification: "preferred".to_string(),
    })
}

/// Finish FIDO2 authentication
pub async fn finish_authentication(
    state: &AppState,
    user_id: Uuid,
    request: AuthenticationRequest,
) -> Result<AuthenticationResult> {
    // Validate request
    if request.type_ != "public-key" {
        return Err(AuthError::BadRequest("Invalid credential type".to_string()));
    }

    // TODO: Implement actual WebAuthn assertion verification
    // This requires the webauthn-rs library to verify the assertion object
    // For now, return a placeholder result

    Ok(AuthenticationResult {
        credential_id: request.raw_id.clone(),
        new_counter: 0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore]
    async fn test_start_authentication() {
        // Requires database and cache connection
    }
}
