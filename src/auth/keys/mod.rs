//! Astral Key - API Key service
//!
//! Service layer for API key management with Argon2id hashing.

pub mod hashing;
pub mod service;

pub use service::KeyService;
