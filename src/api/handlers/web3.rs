//! Astral Key - Web3 authentication handlers

use axum::{extract::State, Json};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::auth::web3::siwe::verify_siwe_signature;
use crate::auth::web3::{
    consume_nonce, generate_nonce, generate_siwe_message, store_nonce, validate_nonce,
};
use crate::db::models::Web3Wallet;
use crate::error::{AuthError, Result};
use crate::state::AppState;

/// Request a nonce for SIWE
pub async fn nonce(
    State(state): State<AppState>,
    Json(request): Json<NonceRequest>,
) -> Result<Json<NonceResponse>> {
    // Generate cryptographically secure nonce
    let nonce = generate_nonce();

    // Store nonce in cache with 15 minute expiration
    store_nonce(&state, &nonce).await?;

    // Generate SIWE message template
    let domain = request.domain.unwrap_or_else(|| "localhost".to_string());
    let address = request.address.unwrap_or_else(|| "0x0".to_string());
    let chain_id = request.chain_id.unwrap_or(1);

    let message_template = generate_siwe_message(&domain, &address, &nonce, chain_id);

    Ok(Json(NonceResponse {
        nonce,
        message_template,
        domain,
        chain_id,
    }))
}

/// Verify Web3 signature
pub async fn verify(
    State(state): State<AppState>,
    Json(request): Json<VerifyRequest>,
) -> Result<Json<AuthResponse>> {
    // Extract nonce from message for validation
    let nonce = extract_nonce_from_message(&request.message)?;

    // Validate nonce exists in cache
    if !validate_nonce(&state, &nonce).await? {
        return Err(AuthError::Unauthorized(
            "Invalid or expired nonce".to_string(),
        ));
    }

    // Verify SIWE signature and recover address
    let address =
        verify_siwe_signature(&request.message, &request.signature, request.chain_id).await?;

    // Consume nonce (one-time use)
    consume_nonce(&state, &nonce).await?;

    // Convert address to string
    let address_string = format!("{:#x}", address);

    // Check if wallet exists in database, create user if not
    let pool = state.db.inner();

    // Try to find existing wallet
    let wallet =
        Web3Wallet::get_by_address_and_chain(pool, &address_string, request.chain_id as i32)
            .await?;

    let user_id = if let Some(wallet) = wallet {
        // Existing user
        wallet.user_id
    } else {
        // Create new user and wallet
        let user = crate::db::models::User::create(pool).await?;
        Web3Wallet::create(pool, user.id, &address_string, request.chain_id as i32).await?;
        user.id
    };

    // Update last used timestamp
    if let Some(wallet) =
        Web3Wallet::get_by_address_and_chain(pool, &address_string, request.chain_id as i32).await?
    {
        let _ = wallet.update_last_used(pool).await;
    }

    // Generate JWT tokens
    let jwt = state
        .jwt
        .as_ref()
        .ok_or_else(|| AuthError::Internal("JWT service not initialized".to_string()))?;

    let tokens = jwt.generate_token_pair(user_id)?;

    Ok(Json(AuthResponse {
        access_token: tokens.access_token,
        refresh_token: tokens.refresh_token,
        user: UserInfo {
            id: user_id.to_string(),
            address: address_string,
            chain_id: request.chain_id,
        },
    }))
}

/// Get supported chains
pub async fn chains(State(_state): State<AppState>) -> Result<Json<serde_json::Value>> {
    Ok(Json(json!({
        "chains": [
            { "id": 1, "name": "ethereum", "display_name": "Ethereum", "type": "mainnet" },
            { "id": 137, "name": "polygon", "display_name": "Polygon", "type": "mainnet" },
            { "id": 42161, "name": "arbitrum", "display_name": "Arbitrum", "type": "mainnet" },
            { "id": 10, "name": "optimism", "display_name": "Optimism", "type": "mainnet" },
            { "id": 5, "name": "goerli", "display_name": "Goerli", "type": "testnet" },
            { "id": 11155111, "name": "sepolia", "display_name": "Sepolia", "type": "testnet" },
        ]
    })))
}

/// Helper: Extract nonce from SIWE message
fn extract_nonce_from_message(message: &str) -> Result<String> {
    message
        .lines()
        .find(|line| line.trim().starts_with("Nonce:"))
        .and_then(|line| line.split(':').nth(1))
        .map(|s| s.trim().to_string())
        .ok_or_else(|| AuthError::BadRequest("Nonce not found in message".to_string()))
}

// Request/Response types

/// Nonce request
#[derive(Deserialize)]
pub struct NonceRequest {
    pub domain: Option<String>,
    pub address: Option<String>,
    pub chain_id: Option<u64>,
}

/// Nonce response
#[derive(Serialize)]
pub struct NonceResponse {
    pub nonce: String,
    pub message_template: String,
    pub domain: String,
    pub chain_id: u64,
}

/// Verify request
#[derive(Deserialize)]
pub struct VerifyRequest {
    pub message: String,
    pub signature: String,
    pub chain_id: u64,
}

/// Authentication response
#[derive(Serialize)]
pub struct AuthResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub user: UserInfo,
}

/// User info
#[derive(Serialize)]
pub struct UserInfo {
    pub id: String,
    pub address: String,
    pub chain_id: u64,
}
