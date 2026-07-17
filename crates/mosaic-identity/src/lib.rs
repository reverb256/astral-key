//! Mosaic Identity Service — PKI layer for the Mosaic ecosystem.
//!
//! Provides key generation, signing, verification, and cross-protocol
//! identity binding resolution. Consumed by:
//! - Mosaic server (Node.js chat/UI) for event signing
//! - Astral Key (Rust auth) for signature verification
//! - Transport plugins (atproto, buzz, matrix, irc) for identity resolution

pub mod api;
pub mod bindings;
pub mod config;
pub mod crypto;
pub mod error;
pub mod nostr;
pub mod storage;
