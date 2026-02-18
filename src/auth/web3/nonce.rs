//! Astral Key - Web3 nonce generation and management

use rand::Rng;
use uuid::Uuid;

use crate::error::Result;
use crate::state::AppState;

/// Nonce length in bytes
const NONCE_LENGTH: usize = 32;

/// Generate a new random nonce
pub fn generate_nonce() -> String {
    let mut rng = rand::thread_rng();
    let bytes: [u8; NONCE_LENGTH] = rng.gen();
    hex::encode(bytes)
}

/// Generate a SIWE message
pub fn generate_siwe_message(
    domain: &str,
    address: &str,
    nonce: &str,
    chain_id: u64,
) -> String {
    format!(
        "{domain} wants you to sign in with your Ethereum account:\n{address}\n\nSign in to Astral Key\n\nURI: https://{domain}\nVersion: 1\nChain ID: {chain_id}\nNonce: {nonce}\nIssued At: {timestamp}\nExpiration Time: {expiration}",
        domain = domain,
        address = address,
        nonce = nonce,
        chain_id = chain_id,
        timestamp = chrono::Utc::now().to_rfc3339(),
        expiration = (chrono::Utc::now() + chrono::Duration::minutes(15)).to_rfc3339()
    )
}

/// Store nonce in cache
pub async fn store_nonce(state: &AppState, nonce: &str) -> Result<()> {
    state.cache.set_with_expiry(nonce, "1", 900).await // 15 minutes
}

/// Validate nonce (check if exists and not used)
pub async fn validate_nonce(state: &AppState, nonce: &str) -> Result<bool> {
    state.cache.exists(nonce).await
}

/// Consume (delete) nonce
pub async fn consume_nonce(state: &AppState, nonce: &str) -> Result<bool> {
    state.cache.delete(nonce).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_nonce() {
        let nonce = generate_nonce();
        assert_eq!(nonce.len(), NONCE_LENGTH * 2); // hex encoded
    }

    #[test]
    fn test_generate_siwe_message() {
        let domain = "app.example.com";
        let address = "0x742d35Cc6634C0532925a3b844Bc9e7595f0bEb";
        let nonce = "abc123";
        let chain_id = 1;

        let message = generate_siwe_message(domain, address, nonce, chain_id);

        assert!(message.contains(domain));
        assert!(message.contains(address));
        assert!(message.contains(nonce));
        assert!(message.contains(&chain_id.to_string()));
    }
}
