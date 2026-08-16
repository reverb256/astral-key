//! Astral Key - Authentication modules
//!
//! Web3, FIDO2, and JWT authentication implementations.

pub mod capabilities;
pub mod fido2;
pub mod jit;
pub mod jwt;
pub mod keys;
pub mod oidc;
pub mod web3;

/// MCP server (feature-gated).
///
/// Enabled with: `cargo build --features mcp`
#[cfg(feature = "mcp")]
pub mod mcp;
