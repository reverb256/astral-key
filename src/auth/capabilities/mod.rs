//! Capability registry — defines all known scope namespaces for astral-key.
//!
//! This module provides the compile-time registry of valid capability scopes.
//! Scopes not listed here are rejected at mint time by the issuer.
//!
//! ## Scope namespaces
//!
//! | Namespace | Purpose |
//! |-----------|---------|
//! | `auth`    | Authentication operations (passkey, web3, token) |
//! | `key`     | API key management (CRUD) |
//! | `jit`     | JIT token operations (mint, verify) |
//! | `mcp`     | MCP tool and resource access |
//! | `dns`     | DNS record management (homelab) |
//! | `pages`   | Pages deployment (homelab) |

pub mod registry;
