//! API handlers for the Mosaic Identity Service.

use std::sync::Arc;

use axum::{
    extract::{Path, Query, State},
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};

use crate::bindings;
use crate::crypto;
use crate::error::Error;
use crate::storage::Storage;

// ─── State ──────────────────────────────────────────────────────────────────

pub struct AppState {
    pub storage: Storage,
}

type SharedState = Arc<AppState>;

// ─── Request/Response types ────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct KeyGenerateRequest {
    pub rotated_from: Option<String>,
    /// Optional BIP-39 mnemonic phrase (24 English words). When provided,
    /// keys are derived deterministically from the mnemonic + passphrase.
    /// When omitted, random keys are generated (legacy behavior).
    pub mnemonic: Option<String>,
    /// BIP-39 passphrase (optional "25th word"), only meaningful when
    /// `mnemonic` is set.
    #[serde(default)]
    pub passphrase: String,
}

#[derive(Serialize)]
pub struct KeyResponse {
    pub key_id: String,
    pub pubkey_hex: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub privkey_pkcs8_hex: Option<String>,
    pub algorithm: String,
    pub created_at: String,
    pub rotated_from: Option<String>,
}

#[derive(Deserialize)]
pub struct KeyImportRequest {
    pub privkey_hex: String,
}

#[derive(Deserialize)]
pub struct SignRequest {
    pub key_id: String,
    pub message_hex: String,
}

#[derive(Serialize)]
pub struct SignResponse {
    pub signature_hex: String,
    pub key_id: String,
}

#[derive(Deserialize)]
pub struct VerifyRequest {
    pub pubkey_hex: String,
    pub message_hex: String,
    pub signature_hex: String,
}

#[derive(Serialize)]
pub struct VerifyResponse {
    pub valid: bool,
}

#[derive(Deserialize)]
pub struct BindResolveRequest {
    pub did_or_handle: String,
}

#[derive(Deserialize)]
pub struct BindClaimRequest {
    pub key_id: String,
    pub protocol: String,
    pub external_id: String,
    pub proof: Option<String>,
}

#[derive(Deserialize)]
pub struct ResolveQuery {
    pub protocol: String,
    pub id: String,
}

// ─── Routes ─────────────────────────────────────────────────────────────────

pub fn router(storage: Storage) -> Router {
    let state = Arc::new(AppState { storage });

    Router::new()
        .route("/health", get(health))
        .route("/keys/generate", post(key_generate))
        .route("/keys/import", post(key_import))
        .route("/keys", get(key_list))
        .route("/keys/{key_id}", get(key_get))
        .route("/keys/{key_id}/history", get(key_history))
        .route("/sign", post(sign_handler))
        .route("/verify", post(verify_handler))
        // PQ hybrid
        .route("/sign/hybrid", post(sign_hybrid_handler))
        .route("/verify/hybrid", post(verify_hybrid_handler))
        .route("/bindings/resolve", post(bind_resolve))
        .route("/bindings/claim", post(bind_claim))
        .route("/keys/{key_id}/bindings", get(bind_list))
        .route("/resolve", get(resolve_external))
        // Nostr
        .route("/nostr/resolve", post(nostr_resolve))
        // Agent certs
        .route("/agent/cert", post(agent_cert_handler))
        .with_state(state)
}

// ─── Handlers ───────────────────────────────────────────────────────────────

async fn health() -> Json<serde_json::Value> {
    Json(serde_json::json!({ "status": "ok", "service": "mosaic-identity" }))
}

// ─── Keys ───────────────────────────────────────────────────────────────────

async fn key_generate(
    State(state): State<SharedState>,
    Json(req): Json<KeyGenerateRequest>,
) -> Result<Json<KeyResponse>, Error> {
    // Determine key material: mnemonic-based (deterministic) or random
    let (pubkey, privkey, key_id) = if let Some(ref phrase) = req.mnemonic {
        // Deterministic: BIP-39 mnemonic → seed → sub-keys
        let seed = crate::hd::mnemonic_to_seed(phrase, &req.passphrase)?;
        let (privkey_hex, pubkey_hex, kid) = crate::hd::derive_ed25519_from_seed(&seed);
        (pubkey_hex, Some(privkey_hex), kid)
    } else {
        // Random (legacy path)
        let (pubkey, privkey, kid) = crate::crypto::generate_key();
        (pubkey, Some(privkey), kid)
    };

    // Mint a companion ML-DSA-65 keypair when the `pq` feature is enabled.
    let (ml_pk, ml_sk) = if cfg!(feature = "pq") {
        if let Some(ref phrase) = req.mnemonic {
            let seed = crate::hd::mnemonic_to_seed(phrase, &req.passphrase)?;
            crate::hd::derive_mldsa_from_seed(&seed)
        } else {
            crate::crypto::generate_mldsa_keypair()
        }
    } else {
        (String::new(), String::new())
    };
    let has_pq = !ml_pk.is_empty();

    state
        .storage
        .insert_key(
            &pubkey,
            privkey.as_deref(),
            &key_id,
            req.rotated_from.as_deref(),
            if has_pq { Some(&ml_pk) } else { None },
            if has_pq { Some(&ml_sk) } else { None },
        )
        .await?;

    let created_at = state.storage.get_key(&key_id).await?.created_at;
    Ok(Json(KeyResponse {
        pubkey_hex: pubkey,
        key_id,
        privkey_pkcs8_hex: privkey,
        algorithm: if has_pq { "Ed25519+ML-DSA-65".into() } else { "Ed25519".into() },
        created_at,
        rotated_from: req.rotated_from,
    }))
}

async fn key_import(
    State(state): State<SharedState>,
    Json(req): Json<KeyImportRequest>,
) -> Result<Json<KeyResponse>, Error> {
    let (pubkey, key_id) = crypto::derive_public_key(&req.privkey_hex)?;
    state
        .storage
        .insert_key(&pubkey, Some(&req.privkey_hex), &key_id, None, None, None)
        .await?;

    let created_at = state.storage.get_key(&key_id).await?.created_at;
    Ok(Json(KeyResponse {
        pubkey_hex: pubkey,
        key_id,
        privkey_pkcs8_hex: Some(req.privkey_hex),
        algorithm: "Ed25519".into(),
        created_at,
        rotated_from: None,
    }))
}

async fn key_list(State(state): State<SharedState>) -> Result<Json<Vec<KeyResponse>>, Error> {
    let keys = state.storage.list_keys().await?;
    Ok(Json(
        keys.into_iter()
            .map(|k| KeyResponse {
                pubkey_hex: k.pubkey_hex,
                key_id: k.key_id,
                privkey_pkcs8_hex: None,
                algorithm: k.algorithm,
                created_at: k.created_at,
                rotated_from: k.rotated_from,
            })
            .collect(),
    ))
}

async fn key_get(
    State(state): State<SharedState>,
    Path(key_id): Path<String>,
) -> Result<Json<KeyResponse>, Error> {
    let k = state.storage.get_key(&key_id).await?;
    Ok(Json(KeyResponse {
        pubkey_hex: k.pubkey_hex,
        key_id: k.key_id,
        privkey_pkcs8_hex: None,
        algorithm: k.algorithm,
        created_at: k.created_at,
        rotated_from: k.rotated_from,
    }))
}

async fn key_history(
    State(state): State<SharedState>,
    Path(key_id): Path<String>,
) -> Result<Json<serde_json::Value>, Error> {
    let rotations = state.storage.get_rotation_history(&key_id).await?;
    Ok(Json(serde_json::json!({
        "key_id": key_id,
        "rotations": rotations.into_iter().map(|(old, new, at)| {
            serde_json::json!({ "old_key_id": old, "new_key_id": new, "rotated_at": at })
        }).collect::<Vec<_>>()
    })))
}

// ─── Signing ────────────────────────────────────────────────────────────────

async fn sign_handler(
    State(state): State<SharedState>,
    Json(req): Json<SignRequest>,
) -> Result<Json<SignResponse>, Error> {
    let key = state.storage.get_key(&req.key_id).await?;
    let privkey = key.privkey_pkcs8_hex.as_deref().ok_or_else(|| {
        Error::BadRequest("Cannot sign with this key: no private key stored".into())
    })?;

    let msg = hex::decode(&req.message_hex)
        .map_err(|_| Error::BadRequest("message_hex is not valid hex".into()))?;

    let signature_hex = crypto::sign(privkey, &msg)?;

    Ok(Json(SignResponse {
        signature_hex,
        key_id: req.key_id,
    }))
}

async fn verify_handler(
    State(_state): State<SharedState>,
    Json(req): Json<VerifyRequest>,
) -> Result<Json<VerifyResponse>, Error> {
    let msg = hex::decode(&req.message_hex)
        .map_err(|_| Error::BadRequest("message_hex is not valid hex".into()))?;

    let valid = crypto::verify(&req.pubkey_hex, &msg, &req.signature_hex)?;

    Ok(Json(VerifyResponse { valid }))
}

// ─── PQ Hybrid ──────────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct SignHybridRequest {
    pub key_id: String,
    pub message_hex: String,
}

#[derive(Serialize)]
pub struct SignHybridResponse {
    pub ed25519_sig: String,
    pub ml_dsa_sig: String,
    pub algorithm: String,
    pub pubkey_hex: String,
    pub key_id: String,
}

async fn sign_hybrid_handler(
    State(state): State<SharedState>,
    Json(req): Json<SignHybridRequest>,
) -> Result<Json<SignHybridResponse>, Error> {
    let key = state.storage.get_key(&req.key_id).await?;
    let privkey = key
        .privkey_pkcs8_hex
        .as_deref()
        .ok_or_else(|| Error::BadRequest("Cannot sign: no private key stored".into()))?;
    let ml_dsa_privkey = key
        .ml_dsa_privkey_hex
        .clone()
        .unwrap_or_default();

    let msg = hex::decode(&req.message_hex)
        .map_err(|_| Error::BadRequest("message_hex is not valid hex".into()))?;

    let sig = crypto::sign_hybrid(privkey, &ml_dsa_privkey, &msg)?;

    Ok(Json(SignHybridResponse {
        ed25519_sig: sig.ed25519_sig,
        ml_dsa_sig: sig.ml_dsa_sig,
        algorithm: sig.algorithm,
        pubkey_hex: sig.pubkey_hex,
        key_id: req.key_id,
    }))
}

#[derive(Deserialize)]
pub struct VerifyHybridRequest {
    pub pubkey_hex: String,
    pub message_hex: String,
    pub ed25519_sig: String,
    /// ML-DSA-65 signature (hex). Optional for backward-compat with classical-only sigs.
    #[serde(default)]
    pub ml_dsa_sig: String,
    /// ML-DSA-65 public key (hex). Optional; required when `ml_dsa_sig` is present.
    #[serde(default)]
    pub ml_dsa_pubkey_hex: String,
}

async fn verify_hybrid_handler(
    State(_state): State<SharedState>,
    Json(req): Json<VerifyHybridRequest>,
) -> Result<Json<VerifyResponse>, Error> {
    let msg = hex::decode(&req.message_hex)
        .map_err(|_| Error::BadRequest("message_hex not valid hex".into()))?;

    let sig = crypto::HybridSignature {
        ed25519_sig: req.ed25519_sig,
        ml_dsa_sig: req.ml_dsa_sig,
        algorithm: "ed25519+ml-dsa-65".to_string(),
        pubkey_hex: req.pubkey_hex,
        ml_dsa_pubkey_hex: req.ml_dsa_pubkey_hex,
    };
    let valid = crypto::verify_hybrid(&sig, &msg)?;

    Ok(Json(VerifyResponse { valid }))
}

// ─── Nostr ─────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct NostrResolveRequest {
    pub npub: String,
}

async fn nostr_resolve(
    State(_state): State<SharedState>,
    Json(req): Json<NostrResolveRequest>,
) -> Result<Json<crate::nostr::ResolvedNostrIdentity>, Error> {
    let resolved = crate::nostr::resolve_npub(&req.npub)?;
    Ok(Json(resolved))
}

// ─── Agent certs ────────────────────────────────────────────────────────────

/// Issue a time-bound, scope-limited certificate for an ephemeral agent key.
///
/// The owner's long-lived key signs a certificate that delegates authority
/// to an agent's ephemeral key. The agent can then sign events on behalf of
/// the owner within the specified scope until the certificate expires.

#[derive(Deserialize)]
pub struct AgentCertRequest {
    pub owner_key_id: String,
    pub agent_pubkey_hex: String,
    pub scope: Vec<String>,
    pub ttl_seconds: Option<u64>,
}

#[derive(Serialize)]
pub struct AgentCertResponse {
    pub cert_b64: String,
    pub owner_key_id: String,
    pub agent_pubkey_hex: String,
    pub expires_at: String,
    pub scope: Vec<String>,
}

async fn agent_cert_handler(
    State(state): State<SharedState>,
    Json(req): Json<AgentCertRequest>,
) -> Result<Json<AgentCertResponse>, Error> {
    let key = state.storage.get_key(&req.owner_key_id).await?;
    let privkey = key
        .privkey_pkcs8_hex
        .as_deref()
        .ok_or_else(|| Error::BadRequest("Owner has no private key stored".into()))?;

    let ttl = req.ttl_seconds.unwrap_or(3600).min(86400);
    let expires = chrono::Utc::now() + chrono::Duration::seconds(ttl as i64);
    let expires_str = expires.to_rfc3339();
    let scope_str = if req.scope.is_empty() {
        "*".to_string()
    } else {
        req.scope.join(",")
    };
    let payload = format!(
        "agent-cert:{}:{}:{}:{}",
        key.pubkey_hex, req.agent_pubkey_hex, expires_str, scope_str
    );
    let sig = crate::crypto::sign(privkey, payload.as_bytes())?;

    let cert_data = serde_json::json!({
        "owner_pubkey": key.pubkey_hex,
        "agent_pubkey": req.agent_pubkey_hex,
        "expires_at": expires_str,
        "scope": req.scope,
        "signature": sig,
    });
    // Encode as JSON (verifier checks the plain JSON + signature field)
    let cert_b64 = hex::encode(cert_data.to_string().as_bytes());

    Ok(Json(AgentCertResponse {
        cert_b64,
        owner_key_id: req.owner_key_id,
        agent_pubkey_hex: req.agent_pubkey_hex,
        expires_at: expires_str,
        scope: req.scope,
    }))
}

// ─── Bindings ───────────────────────────────────────────────────────────────

async fn bind_resolve(
    State(_state): State<SharedState>,
    Json(req): Json<BindResolveRequest>,
) -> Result<Json<bindings::ResolvedIdentity>, Error> {
    let resolved = bindings::resolve(&req.did_or_handle).await?;
    Ok(Json(resolved))
}

async fn bind_claim(
    State(state): State<SharedState>,
    Json(req): Json<BindClaimRequest>,
) -> Result<Json<serde_json::Value>, Error> {
    state.storage.get_key(&req.key_id).await?; // verify key exists

    state
        .storage
        .upsert_binding(
            &req.key_id,
            &req.protocol,
            &req.external_id,
            req.proof.as_deref(),
        )
        .await?;

    Ok(Json(serde_json::json!({
        "status": "claimed",
        "key_id": req.key_id,
        "protocol": req.protocol,
        "external_id": req.external_id
    })))
}

async fn bind_list(
    State(state): State<SharedState>,
    Path(key_id): Path<String>,
) -> Result<Json<serde_json::Value>, Error> {
    let bindings = state.storage.get_bindings(&key_id).await?;
    Ok(Json(serde_json::json!({
        "key_id": key_id,
        "bindings": bindings.into_iter().map(|b| {
            serde_json::json!({
                "protocol": b.protocol,
                "external_id": b.external_id,
                "proof": b.proof,
                "claimed_at": b.claimed_at
            })
        }).collect::<Vec<_>>()
    })))
}

async fn resolve_external(
    State(state): State<SharedState>,
    Query(query): Query<ResolveQuery>,
) -> Result<Json<serde_json::Value>, Error> {
    let binding = state
        .storage
        .resolve_external(&query.protocol, &query.id)
        .await?;
    let key = state.storage.get_key(&binding.key_id).await?;

    Ok(Json(serde_json::json!({
        "key_id": binding.key_id,
        "pubkey_hex": key.pubkey_hex,
        "algorithm": key.algorithm,
        "protocol": binding.protocol,
        "external_id": binding.external_id
    })))
}
