//! Astral Key - Web3 SIWE authentication
//!
//! Sign-In with Ethereum implementation.

pub mod nonce;
pub mod siwe;

pub use nonce::{
    consume_nonce, generate_nonce, generate_siwe_message, store_nonce, validate_nonce,
};

use crate::error::Result;

/// Web3 service
#[derive(Clone)]
pub struct Web3Service {
    _rpc_url: String,
    chain_id: u64,
}

impl Web3Service {
    /// Create a new Web3 service
    pub async fn new(rpc_url: &str, chain_id: u64) -> Result<Self> {
        Ok(Self {
            _rpc_url: rpc_url.to_string(),
            chain_id,
        })
    }

    /// Get chain ID
    pub fn chain_id(&self) -> u64 {
        self.chain_id
    }
}
