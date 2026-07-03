//! Astral Key - Web3 nonce generation and management
//!
//! Uses the nonces table in SQLite (no Redis).

use rand::Rng;

use crate::db::models::Nonce;
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
pub fn generate_siwe_message(domain: &str, address: &str, nonce: &str, chain_id: u64) -> String {
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

/// Store nonce in database
pub async fn store_nonce(state: &AppState, nonce: &str) -> Result<()> {
    let expires_at = chrono::Utc::now() + chrono::Duration::minutes(15);
    Nonce::create(state.db.inner(), nonce, expires_at, None).await?;
    Ok(())
}

/// Validate nonce (check if exists in DB, not used, not expired)
pub async fn validate_nonce(state: &AppState, nonce: &str) -> Result<bool> {
    let pool = state.db.inner();
    let record = Nonce::get_by_nonce(pool, nonce).await?;
    Ok(record.as_ref().map_or(false, |n| n.is_valid()))
}

/// Consume (mark as used) nonce
pub async fn consume_nonce(state: &AppState, nonce: &str) -> Result<bool> {
    let pool = state.db.inner();
    let record = Nonce::get_by_nonce(pool, nonce).await?;
    if let Some(n) = record {
        if n.is_valid() {
            n.mark_as_used(pool).await?;
            return Ok(true);
        }
    }
    Ok(false)
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
