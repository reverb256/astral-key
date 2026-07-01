//! Astral Key - Web3 SIWE authentication
//!
//! Sign-In with Ethereum implementation.

pub mod nonce;
pub mod siwe;

pub use nonce::{
    consume_nonce, generate_nonce, generate_siwe_message, store_nonce, validate_nonce,
};
pub use siwe::verify_siwe_signature;

use crate::cache::pool::RedisPool;
use crate::error::Result;

/// Web3 service
#[derive(Clone)]
pub struct Web3Service {
    _rpc_url: String,
    cache: RedisPool,
    chain_id: u64,
}

impl Web3Service {
    /// Create a new Web3 service
    pub async fn new(rpc_url: &str, chain_id: u64, cache: RedisPool) -> Result<Self> {
        // TODO: Initialize Ethereum provider when ethers is properly integrated
        Ok(Self {
            _rpc_url: rpc_url.to_string(),
            cache,
            chain_id,
        })
    }

    /// Get cache
    pub fn cache(&self) -> &RedisPool {
        &self.cache
    }

    /// Get chain ID
    pub fn chain_id(&self) -> u64 {
        self.chain_id
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore]
    async fn test_web3_service_creation() {
        // Requires actual RPC endpoint
    }
}
