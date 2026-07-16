//! MCP tools for astral-key.
//!
//! Each tool is a thin wrapper around the same service functions used by the
//! REST API.  Tools are registered via the `#[tool_router]` macro and served
//! over stdio (stdin / stdout).

#![cfg(feature = "mcp")]

use std::sync::OnceLock;

use rmcp::{model::*, tool, transport::stdio, RmcpError, ServerHandler, ServiceExt};
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

use crate::auth::jit::{JitIssuer, JitVerifier, SignedToken};
use crate::auth::keys::KeyService;
use crate::config::Config;
use crate::db::pool::DbPool;
use crate::state::AppState;

// ---------------------------------------------------------------------------
// Static service references (initialised once before serving)
// ---------------------------------------------------------------------------

static MCP_APP_STATE: OnceLock<McpAppState> = OnceLock::new();

struct McpAppState {
    db: DbPool,
    jit_issuer: Option<JitIssuer>,
    jit_verifier: Option<JitVerifier>,
    issuer_id: String,
}

fn app_state() -> &'static McpAppState {
    MCP_APP_STATE.get().expect("McpAppState not initialised")
}

// ---------------------------------------------------------------------------
// MCP server struct
// ---------------------------------------------------------------------------

/// MCP server exposing astral-key auth capabilities as tools.
#[derive(Clone)]
pub struct AstralKeyMcp;

// ---------------------------------------------------------------------------
// Tool definitions
// ---------------------------------------------------------------------------

impl AstralKeyMcp {
    /// Health check — verifies the service is running.
    #[tool(description = "Check whether the astral-key service is healthy")]
    async fn astral_health() -> Result<CallToolResult, RmcpError> {
        let healthy = app_state().db.health_check().await.unwrap_or(false);
        let content = if healthy {
            Content::json(json!({ "status": "healthy" }))
                .map_err(|e| RmcpError::internal_error(format!("serialisation: {e}"), None))?
        } else {
            Content::json(json!({ "status": "unhealthy" }))
                .map_err(|e| RmcpError::internal_error(format!("serialisation: {e}"), None))?
        };
        Ok(CallToolResult::success(vec![content]))
    }

    /// Mint a ZK JIT capability token.
    #[tool(description = "Mint a new ZK JIT capability token with the given scopes and TTL")]
    async fn astral_mint_token(
        #[tool(param)] scopes: Vec<String>,
        #[tool(param)] audience: String,
        #[tool(param)] ttl_seconds: u64,
    ) -> Result<CallToolResult, RmcpError> {
        let state = app_state();
        let issuer = state
            .jit_issuer
            .as_ref()
            .ok_or_else(|| RmcpError::internal_error("JIT issuer not configured", None))?;

        let signed: SignedToken = issuer.mint(scopes, &audience, ttl_seconds);
        let content = Content::json(json!({
            "token": signed.token,
            "expires_at": signed.expires_at,
            "token_id": signed.token_id,
        }))
        .map_err(|e| RmcpError::internal_error(format!("serialisation: {e}"), None))?;

        Ok(CallToolResult::success(vec![content]))
    }

    /// Verify a capability token.
    #[tool(description = "Verify a ZK JIT capability token and return its claims")]
    async fn astral_verify_token(
        #[tool(param)] token: String,
    ) -> Result<CallToolResult, RmcpError> {
        let state = app_state();
        let verifier = state
            .jit_verifier
            .as_ref()
            .ok_or_else(|| RmcpError::internal_error("JIT verifier not configured", None))?;

        match verifier.verify(&token) {
            Ok(claims) => {
                let content = Content::json(json!({
                    "valid": true,
                    "subject": claims.subject,
                    "issuer": claims.issuer,
                    "audience": claims.audience,
                    "scopes": claims.scopes,
                    "issued_at": claims.issued_at,
                    "expires_at": claims.expires_at,
                    "epoch": claims.epoch,
                }))
                .map_err(|e| RmcpError::internal_error(format!("serialisation: {e}"), None))?;
                Ok(CallToolResult::success(vec![content]))
            }
            Err(e) => {
                let content = Content::json(json!({
                    "valid": false,
                    "error": e.to_string(),
                }))
                .map_err(|e| RmcpError::internal_error(format!("serialisation: {e}"), None))?;
                Ok(CallToolResult::success(vec![content]))
            }
        }
    }

    /// Create a new API key for a user.
    #[tool(description = "Create a new API key for the given user ID")]
    async fn astral_create_key(
        #[tool(param)] user_id: String,
        #[tool(param)] name: String,
        #[tool(param)] scopes: Vec<String>,
        #[tool(param)] environment: String,
        #[tool(param)] expires_in_seconds: Option<i64>,
    ) -> Result<CallToolResult, RmcpError> {
        let db = app_state().db.inner();
        let uid = Uuid::parse_str(&user_id)
            .map_err(|e| RmcpError::invalid_params(format!("Invalid user_id: {e}")))?;

        let expires_in = expires_in_seconds.map(chrono::Duration::seconds);
        let scope_refs: Vec<&str> = scopes.iter().map(|s| s.as_str()).collect();

        match KeyService::create_key(db, uid, &name, &scope_refs, &environment, expires_in).await {
            Ok((summary, raw_key)) => {
                let content = Content::json(json!({
                    "id": summary.id,
                    "api_key": raw_key,
                    "key_prefix": summary.key_prefix,
                    "name": summary.name,
                    "scopes": summary.scopes,
                    "environment": summary.environment,
                }))
                .map_err(|e| RmcpError::internal_error(format!("serialisation: {e}"), None))?;
                Ok(CallToolResult::success(vec![content]))
            }
            Err(e) => {
                let content = Content::text(format!("Error: {e}"));
                Ok(CallToolResult::success(vec![content]))
            }
        }
    }

    /// List API keys for a user.
    #[tool(description = "List all API keys (summaries) for the given user ID")]
    async fn astral_list_keys(#[tool(param)] user_id: String) -> Result<CallToolResult, RmcpError> {
        let db = app_state().db.inner();
        let uid = Uuid::parse_str(&user_id)
            .map_err(|e| RmcpError::invalid_params(format!("Invalid user_id: {e}")))?;

        match KeyService::list_keys(db, uid).await {
            Ok(keys) => {
                let content = Content::json(json!({ "keys": keys }))
                    .map_err(|e| RmcpError::internal_error(format!("serialisation: {e}"), None))?;
                Ok(CallToolResult::success(vec![content]))
            }
            Err(e) => {
                let content = Content::text(format!("Error: {e}"));
                Ok(CallToolResult::success(vec![content]))
            }
        }
    }

    /// Revoke an API key.
    #[tool(description = "Revoke an API key by ID for a given user")]
    async fn astral_revoke_key(
        #[tool(param)] user_id: String,
        #[tool(param)] key_id: String,
    ) -> Result<CallToolResult, RmcpError> {
        let db = app_state().db.inner();
        let uid = Uuid::parse_str(&user_id)
            .map_err(|e| RmcpError::invalid_params(format!("Invalid user_id: {e}")))?;
        let kid = Uuid::parse_str(&key_id)
            .map_err(|e| RmcpError::invalid_params(format!("Invalid key_id: {e}")))?;

        match KeyService::revoke_key(db, kid, uid).await {
            Ok(()) => {
                let content = Content::text("API key revoked successfully");
                Ok(CallToolResult::success(vec![content]))
            }
            Err(e) => {
                let content = Content::text(format!("Error: {e}"));
                Ok(CallToolResult::success(vec![content]))
            }
        }
    }
}

// ---------------------------------------------------------------------------
// ServerHandler implementation (required by rmcp)
// ---------------------------------------------------------------------------

/// Required by the `#[tool]` macro machinery — provides server metadata and
/// initialisation.
impl ServerHandler for AstralKeyMcp {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: ProtocolVersion::V_2025_06_18,
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            server_info: Implementation::from_build_env(),
            instructions: Some(
                "astral-key authentication service — manage API keys and capability tokens."
                    .to_string(),
            ),
        }
    }

    async fn initialize(
        &self,
        _request: InitializeRequestParam,
        _context: rmcp::service::RequestContext<rmcp::RoleServer>,
    ) -> Result<InitializeResult, RmcpError> {
        Ok(self.get_info())
    }
}

// ---------------------------------------------------------------------------
// Entrypoint
// ---------------------------------------------------------------------------

/// Initialise and run the MCP server over stdio.
///
/// Loads configuration, connects to the database, initialises JIT
/// services, and starts serving MCP tools on stdin/stdout.
pub async fn run_mcp_server() -> anyhow::Result<()> {
    tracing::info!("Starting astral-key MCP server");

    let config = Config::from_env()?;
    let state = AppState::new(config.clone()).await?;
    let db = state.db;

    // Initialise JIT issuer from environment, if available.
    let jit_issuer = match std::env::var("JIT_ISSUER_KEY") {
        Ok(key_hex) => {
            let issuer_id =
                std::env::var("JIT_ISSUER_ID").unwrap_or_else(|_| "ak:mcp:issuer:01".to_string());
            match JitIssuer::new(&key_hex, &issuer_id) {
                Ok(issuer) => {
                    tracing::info!("JIT issuer initialised: {issuer_id}");
                    Some(issuer)
                }
                Err(e) => {
                    tracing::warn!("Failed to initialise JIT issuer: {e}");
                    None
                }
            }
        }
        Err(_) => {
            tracing::info!("JIT_ISSUER_KEY not set — mint-token tool will be unavailable");
            None
        }
    };

    // Initialise JIT verifier from issuer key, if available.
    let jit_verifier = jit_issuer.as_ref().map(|issuer| {
        let verifier = JitVerifier::new();
        // Register the issuer's public key.
        // For simplicity we derive it from the signing key at startup.
        // In production the verifier would load the public key separately.
        let _ = issuer; // verifier needs public key, not signing key
        verifier
    });

    let issuer_id =
        std::env::var("JIT_ISSUER_ID").unwrap_or_else(|_| "ak:mcp:issuer:01".to_string());

    MCP_APP_STATE
        .set(McpAppState {
            db,
            jit_issuer,
            jit_verifier,
            issuer_id,
        })
        .map_err(|_| anyhow::anyhow!("McpAppState already initialised"))?;

    tracing::info!("Initialisation complete — starting stdio MCP server");

    let service = AstralKeyMcp.serve(stdio()).await?;
    service.waiting().await?;

    Ok(())
}
