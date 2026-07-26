use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::Deserialize;
use std::sync::Arc;
use tracing::{info, warn};

use crate::matrix_client::MatrixClient;
use crate::room_mapper::RoomMapper;
use mosaic_client::MosaicClient;

/// Shared application state available to AS handlers.
pub struct AsState {
    pub mis: MosaicClient,
    pub matrix: MatrixClient,
    pub room_mapper: RoomMapper,
    pub hs_token: String,
    pub domain: String,
}

/// Query parameters carrying the AS auth token.
#[derive(Deserialize)]
pub struct AccessTokenParams {
    pub access_token: Option<String>,
}

/// Transaction ID path parameter.
#[derive(Deserialize)]
pub struct TxnIdPath {
    pub txn_id: String,
}

/// User ID path parameter.
#[derive(Deserialize)]
pub struct UserIdPath {
    pub user_id: String,
}

/// Inbound event from the Matrix homeserver (transaction content).
#[derive(Deserialize)]
pub struct TransactionBody {
    #[serde(default)]
    pub events: Vec<MatrixEvent>,
}

/// A single Matrix event from the homeserver.
#[derive(Debug, Deserialize)]
pub struct MatrixEvent {
    #[serde(default)]
    pub event_id: Option<String>,
    #[serde(default)]
    pub sender: Option<String>,
    #[serde(default)]
    pub room_id: Option<String>,
    #[serde(rename = "type")]
    pub event_type: Option<String>,
    #[serde(default)]
    pub content: Option<serde_json::Value>,
    #[serde(default)]
    pub origin_server_ts: Option<u64>,
}

// ─── Auth helpers ────────────────────────────────────────────────────────────

/// Verify that the homeserver's access_token matches our configured hs_token.
fn verify_hs_token(state: &AsState, token: Option<&str>) -> Result<(), AsError> {
    match token {
        Some(t) if t == state.hs_token => Ok(()),
        _ => Err(AsError::Unauthorized),
    }
}

/// Error responses for AS API calls.
pub(crate) enum AsError {
    Unauthorized,
    NotFound,
    Internal(String),
}

impl IntoResponse for AsError {
    fn into_response(self) -> axum::response::Response {
        let (status, errcode, error) = match self {
            AsError::Unauthorized => (
                StatusCode::UNAUTHORIZED,
                "M_UNKNOWN_TOKEN",
                "Invalid access_token",
            ),
            AsError::NotFound => (StatusCode::NOT_FOUND, "M_NOT_FOUND", "Not found"),
            AsError::Internal(ref msg) => {
                warn!("AS internal error: {}", msg);
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({
                        "errcode": "M_UNKNOWN",
                        "error": msg,
                    })),
                )
                    .into_response();
            }
        };
        (
            status,
            Json(serde_json::json!({
                "errcode": errcode,
                "error": error,
            })),
        )
            .into_response()
    }
}

// ─── Handlers ────────────────────────────────────────────────────────────────

/// `POST /matrix/transactions/{txn_id}`
///
/// Receives batches of events pushed by the Matrix homeserver.
/// The homeserver includes `?access_token=<hs_token>` for authentication.
pub async fn handle_transaction(
    state: State<Arc<AsState>>,
    Path(_txn_path): Path<TxnIdPath>,
    Query(params): Query<AccessTokenParams>,
    Json(body): Json<TransactionBody>,
) -> Result<Json<serde_json::Value>, AsError> {
    verify_hs_token(&state, params.access_token.as_deref())?;

    let txn_id = &_txn_path.txn_id;
    let event_count = body.events.len();
    info!(
        "[MATRIX AS] Transaction {}: {} event(s)",
        txn_id, event_count
    );

    for event in &body.events {
        // Only process room message events
        if event.event_type.as_deref() == Some("m.room.message") {
            if let Some(content) = &event.content {
                let body_text = content
                    .get("body")
                    .and_then(|v| v.as_str())
                    .unwrap_or("(no body)");
                let sender = event.sender.as_deref().unwrap_or("(unknown)");
                let room_id = event.room_id.as_deref().unwrap_or("(unknown)");

                info!(
                    "[MATRIX ←] {} in {}: {}",
                    sender,
                    room_id.chars().take(20).collect::<String>(),
                    body_text.chars().take(80).collect::<String>()
                );

                // In production: publish to Mosaic event bus.
                // For now we just log the event and let the caller
                // (main.rs) wire up the on_matrix_message callback.
            }
        }
    }

    // Always respond 200 — the HS will retry if we don't ack.
    Ok(Json(serde_json::json!({})))
}

/// `GET /matrix/users/{user_id}`
///
/// Called by the homeserver to check if a virtual Mosaic user exists.
/// The homeserver includes `?access_token=<hs_token>` for authentication.
///
/// Expected user ID format: `@mosaic_<pubkey_hex>:<domain>`
pub async fn handle_user_query(
    state: State<Arc<AsState>>,
    Path(path): Path<UserIdPath>,
    Query(params): Query<AccessTokenParams>,
) -> Result<Json<serde_json::Value>, AsError> {
    verify_hs_token(&state, params.access_token.as_deref())?;

    let user_id = &path.user_id;
    // Expected format: @mosaic_<pubkey_hex>:<domain>
    let domain = &state.domain;

    if !user_id.starts_with("@mosaic_") || !user_id.ends_with(&format!(":{}", domain)) {
        info!(
            "[MATRIX AS] User lookup rejected (wrong namespace): {}",
            user_id
        );
        return Ok(Json(serde_json::json!({})));
    }

    // Extract pubkey: @mosaic_<pubkey_hex>:<domain>
    let prefix = "@mosaic_";
    let suffix = format!(":{}", domain);
    let pubkey_hex = &user_id[prefix.len()..(user_id.len() - suffix.len())];

    if pubkey_hex.is_empty() || !pubkey_hex.chars().all(|c| c.is_ascii_hexdigit()) {
        info!(
            "[MATRIX AS] User lookup rejected (invalid pubkey): {}",
            user_id
        );
        return Ok(Json(serde_json::json!({})));
    }

    info!(
        "[MATRIX AS] User lookup: {} (pubkey: {}...)",
        user_id,
        &pubkey_hex[..pubkey_hex.len().min(12)]
    );

    // Verify this pubkey has a binding in MIS
    match state.mis.resolve_binding("ed25519", pubkey_hex).await {
        Ok(_binding) => {
            let short_pk = &pubkey_hex[..pubkey_hex.len().min(8)];
            let display_name = format!("Mosaic User {}", short_pk);
            Ok(Json(serde_json::json!({
                "user_id": user_id,
                "display_name": display_name,
            })))
        }
        Err(e) => {
            info!(
                "[MATRIX AS] No binding for {}...: {}",
                &pubkey_hex[..pubkey_hex.len().min(12)],
                e
            );
            // Return empty — homeserver will reject the user
            Ok(Json(serde_json::json!({})))
        }
    }
}
