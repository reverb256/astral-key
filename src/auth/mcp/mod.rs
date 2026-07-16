//! MCP server for astral-key (feature-gated).
//!
//! Enabled with: `cargo build --features mcp`
//!
//! Provides MCP tools that wrap the auth API:
//!
//! | Tool                   | Description                           |
//! |------------------------|---------------------------------------|
//! | `astral_health`        | Health check                          |
//! | `astral_mint_token`    | Mint a ZK JIT capability token        |
//! | `astral_verify_token`  | Verify a capability token             |
//! | `astral_create_key`    | Create an API key                     |
//! | `astral_list_keys`     | List API keys                         |
//! | `astral_revoke_key`    | Revoke an API key                     |
//!
//! # Transport
//!
//! Uses **stdio** (stdin / stdout) for Hermes / Claude Code integration.
//!
//! # Usage
//!
//! ```ignore
//! cargo run --features mcp
//! ```
//!
//! The server listens on stdio and responds to JSON-RPC requests from
//! any MCP client (Claude Code, VS Code, etc.).

#![cfg(feature = "mcp")]

pub mod tools;

pub use tools::run_mcp_server;
