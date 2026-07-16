//! ZK JIT capability token module.
//!
//! Ed25519-signed capability tokens with zero-knowledge just-in-time issuance.
//! No database writes on mint — pure crypto operations.
//!
//! Token format: `base64(header).base64(payload).base64(signature)`
//! - Header: `{"typ":"CAP","alg":"EdDSA"}` (constant)
//! - Payload: JSON-serialized [`Capability`] struct
//! - Signature: Ed25519 signature over `header_b64 + "." + payload_b64`

pub mod epoch;
pub mod issuer;
pub mod scope;
pub mod verifier;

pub use epoch::{EpochManager, TombstoneJournal};
pub use issuer::JitIssuer;
pub use scope::{is_valid_scope, satisfies};
pub use verifier::JitVerifier;

use serde::{Deserialize, Serialize};

/// Capability token payload (signed content).
///
/// Carries the full authorization context that is cryptographically signed
/// by the issuing key.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Capability {
    /// Token ID (UUID v4 — unique per minted token)
    pub sub: String,
    /// Issuer key ID, e.g. "ak:issuer:01"
    pub iss: String,
    /// Target audience — identifies the service this token is intended for
    pub aud: String,
    /// Issued at (Unix seconds)
    pub iat: i64,
    /// Expires at (Unix seconds)
    pub exp: i64,
    /// Permission scopes granted to this token
    pub scopes: Vec<String>,
    /// Issuance epoch — used for batch revocation
    pub epoch: u64,
}

/// Full signed token returned to the caller after minting.
///
/// The `token` field is the complete signed token string in
/// `base64(header).base64(payload).base64(signature)` format.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignedToken {
    /// The full signed token string
    pub token: String,
    /// Unix timestamp when the token expires
    pub expires_at: i64,
    /// Token ID (UUID v4) — correlates with `Capability.sub`
    pub token_id: String,
}

/// Verified and decoded claims returned by the verifier.
///
/// These are the **trusted** contents of a token after all validation
/// checks (signature, expiry, epoch, revocation) have passed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerifiedClaims {
    pub subject: String,
    pub issuer: String,
    pub audience: String,
    pub scopes: Vec<String>,
    pub issued_at: i64,
    pub expires_at: i64,
    pub epoch: u64,
}

/// Verification errors returned during token validation.
///
/// **Note:** This is a temporary standalone enum using `thiserror`.
/// It will be replaced by Phase 1's unified `Error` enum during compilation.
#[derive(Debug, thiserror::Error)]
pub enum VerificationError {
    #[error("Invalid signature")]
    InvalidSignature,

    #[error("Token expired at {0}")]
    Expired(i64),

    #[error("Token revoked: {0}")]
    Revoked(String),

    #[error("Stale epoch: token={0}, current={1}")]
    StaleEpoch(u64, u64),

    #[error("Unknown issuer: {0}")]
    UnknownIssuer(String),

    #[error("Decode error: {0}")]
    Decode(String),
}
