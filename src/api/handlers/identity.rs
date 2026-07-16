//! Astral Key - Identity and contact handlers
//!
//! Replaces mosiac-identity's Ed25519 identity, contacts, QR, and signing APIs.
//! Private keys are intentionally held by clients; the server only stores public keys
//! and verifies signatures.

use axum::{extract::Path, extract::Query, extract::State, Json};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::auth::jwt::AuthenticatedUser;
use crate::db::models::{Contact, Identity};
use crate::error::{AuthError, Result};
use crate::state::AppState;

/// Create a new Ed25519 identity record.
/// The client generates the keypair and sends only the public key.
pub async fn create_identity(
    State(_state): State<AppState>,
    auth_user: AuthenticatedUser,
    Json(request): Json<CreateIdentityRequest>,
) -> Result<Json<IdentityResponse>> {
    if !is_valid_pubkey(&request.pubkey) {
        return Err(AuthError::BadRequest(
            "Invalid Ed25519 public key".to_string(),
        ));
    }

    let pool = _state.db.inner();

    let identity = Identity::create(
        pool,
        auth_user.user_id,
        &request.pubkey,
        request.label.as_deref(),
    )
    .await?;

    Ok(Json(IdentityResponse {
        id: identity.id,
        pubkey: identity.pubkey,
        label: identity.label,
        is_current: identity.is_current,
        created_at: identity.created_at,
    }))
}

/// List all identities for the authenticated user.
pub async fn list_identities(
    State(_state): State<AppState>,
    auth_user: AuthenticatedUser,
) -> Result<Json<Vec<IdentityResponse>>> {
    let pool = _state.db.inner();
    let identities = Identity::get_by_user(pool, auth_user.user_id).await?;

    Ok(Json(
        identities
            .into_iter()
            .map(|i| IdentityResponse {
                id: i.id,
                pubkey: i.pubkey,
                label: i.label,
                is_current: i.is_current,
                created_at: i.created_at,
            })
            .collect(),
    ))
}

/// Get the current identity for the authenticated user.
pub async fn current_identity(
    State(_state): State<AppState>,
    auth_user: AuthenticatedUser,
) -> Result<Json<IdentityResponse>> {
    let pool = _state.db.inner();
    let identity = Identity::get_current(pool, auth_user.user_id)
        .await?
        .ok_or_else(|| AuthError::NotFound("No current identity".to_string()))?;

    Ok(Json(IdentityResponse {
        id: identity.id,
        pubkey: identity.pubkey,
        label: identity.label,
        is_current: identity.is_current,
        created_at: identity.created_at,
    }))
}

/// Delete an identity by ID.
pub async fn delete_identity(
    State(_state): State<AppState>,
    auth_user: AuthenticatedUser,
    Path(identity_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>> {
    let pool = _state.db.inner();
    let identity = Identity::get_by_id(pool, identity_id)
        .await?
        .ok_or_else(|| AuthError::NotFound("Identity not found".to_string()))?;

    if identity.user_id != auth_user.user_id {
        return Err(AuthError::Forbidden("Not your identity".to_string()));
    }

    identity.delete(pool).await?;

    Ok(Json(serde_json::json!({
        "message": "Identity deleted",
    })))
}

/// Set an identity as the current one for the user.
pub async fn set_current_identity(
    State(_state): State<AppState>,
    auth_user: AuthenticatedUser,
    Path(identity_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>> {
    let pool = _state.db.inner();
    let identity = Identity::get_by_id(pool, identity_id)
        .await?
        .ok_or_else(|| AuthError::NotFound("Identity not found".to_string()))?;

    if identity.user_id != auth_user.user_id {
        return Err(AuthError::Forbidden("Not your identity".to_string()));
    }

    identity.set_current(pool).await?;

    Ok(Json(serde_json::json!({
        "message": "Current identity updated",
    })))
}

/// Verify a signed JSON envelope.
/// The client signs data locally and sends { data, signature, pubkey } for verification.
/// This is a public endpoint; no authentication required.
///
/// IMPORTANT: The client must sign the exact canonical JSON string produced by
/// `JSON.stringify(data)` (no extra whitespace, stable key order). The server
/// re-serializes `request.data` with `serde_json::to_string`; any mismatch in
/// whitespace or key order will cause verification to fail.
pub async fn verify_signature(Json(request): Json<VerifyRequest>) -> Result<Json<VerifyResponse>> {
    use base64::engine::{general_purpose::URL_SAFE_NO_PAD, Engine};
    use ed25519_dalek::{Signature, Verifier, VerifyingKey};

    let pubkey_bytes = URL_SAFE_NO_PAD
        .decode(&request.pubkey)
        .map_err(|e| AuthError::BadRequest(format!("Invalid pubkey encoding: {}", e)))?;
    let pubkey_array: [u8; 32] = pubkey_bytes
        .try_into()
        .map_err(|_| AuthError::BadRequest("Invalid pubkey length".to_string()))?;
    let verifying_key = VerifyingKey::from_bytes(&pubkey_array)
        .map_err(|e| AuthError::BadRequest(format!("Invalid Ed25519 pubkey: {}", e)))?;

    let sig_bytes = URL_SAFE_NO_PAD
        .decode(&request.signature)
        .map_err(|e| AuthError::BadRequest(format!("Invalid signature encoding: {}", e)))?;
    let signature = Signature::from_slice(&sig_bytes)
        .map_err(|e| AuthError::BadRequest(format!("Invalid signature: {}", e)))?;

    let message = serde_json::to_string(&request.data)
        .map_err(|e| AuthError::BadRequest(format!("Invalid data: {}", e)))?;

    let valid = verifying_key.verify(message.as_bytes(), &signature).is_ok();

    Ok(Json(VerifyResponse { valid }))
}

/// List contacts for the authenticated user.
pub async fn list_contacts(
    State(_state): State<AppState>,
    auth_user: AuthenticatedUser,
) -> Result<Json<Vec<ContactResponse>>> {
    let pool = _state.db.inner();
    let contacts = Contact::get_by_owner(pool, auth_user.user_id).await?;

    Ok(Json(
        contacts
            .into_iter()
            .map(|c| ContactResponse {
                id: c.id,
                pubkey: c.pubkey,
                label: c.label,
                discovered_via: c.discovered_via,
                first_seen_at: c.first_seen_at,
                last_seen_at: c.last_seen_at,
            })
            .collect(),
    ))
}

/// Add or update a contact.
pub async fn upsert_contact(
    State(_state): State<AppState>,
    auth_user: AuthenticatedUser,
    Json(request): Json<UpsertContactRequest>,
) -> Result<Json<ContactResponse>> {
    if !is_valid_pubkey(&request.pubkey) {
        return Err(AuthError::BadRequest(
            "Invalid Ed25519 public key".to_string(),
        ));
    }

    let pool = _state.db.inner();
    let contact = Contact::upsert(
        pool,
        auth_user.user_id,
        &request.pubkey,
        request.label.as_deref(),
        request.discovered_via.as_deref(),
    )
    .await?;

    Ok(Json(ContactResponse {
        id: contact.id,
        pubkey: contact.pubkey,
        label: contact.label,
        discovered_via: contact.discovered_via,
        first_seen_at: contact.first_seen_at,
        last_seen_at: contact.last_seen_at,
    }))
}

/// Delete a contact by public key.
pub async fn delete_contact(
    State(_state): State<AppState>,
    auth_user: AuthenticatedUser,
    Path(pubkey): Path<String>,
) -> Result<Json<serde_json::Value>> {
    let pool = _state.db.inner();
    Contact::delete(pool, auth_user.user_id, &pubkey).await?;

    Ok(Json(serde_json::json!({
        "message": "Contact deleted",
    })))
}

/// Request body for creating an identity
#[derive(Deserialize)]
pub struct CreateIdentityRequest {
    pub pubkey: String,
    pub label: Option<String>,
}

/// Request body for verifying a signature
#[derive(Deserialize)]
pub struct VerifyRequest {
    pub data: serde_json::Value,
    pub signature: String,
    pub pubkey: String,
}

/// Response for signature verification
#[derive(Serialize)]
pub struct VerifyResponse {
    pub valid: bool,
}

/// Request body for upserting a contact
#[derive(Deserialize)]
pub struct UpsertContactRequest {
    pub pubkey: String,
    pub label: Option<String>,
    pub discovered_via: Option<String>,
}

/// Identity response
#[derive(Serialize)]
pub struct IdentityResponse {
    pub id: Uuid,
    pub pubkey: String,
    pub label: Option<String>,
    pub is_current: bool,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// Contact response
#[derive(Serialize)]
pub struct ContactResponse {
    pub id: Uuid,
    pub pubkey: String,
    pub label: Option<String>,
    pub discovered_via: String,
    pub first_seen_at: chrono::DateTime<chrono::Utc>,
    pub last_seen_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// QR code generation parameters
#[derive(Deserialize)]
pub struct QrParams {
    pub format: Option<String>,
    pub width: Option<u32>,
}

/// QR code generation response
#[derive(Serialize)]
pub struct QrResponse {
    pub pubkey: String,
    pub fingerprint: String,
    pub format: String,
    pub data: String,
}

/// QR scan request
#[derive(Deserialize)]
pub struct ScanQrRequest {
    pub qr: String,
    pub label: Option<String>,
}

/// Generate a QR code for a public key.
/// Supports `svg` (default) and `png` formats.
pub async fn generate_qr(
    Path(pubkey): Path<String>,
    Query(params): Query<QrParams>,
) -> Result<Json<QrResponse>> {
    if !is_valid_pubkey(&pubkey) {
        return Err(AuthError::BadRequest(
            "Invalid Ed25519 public key".to_string(),
        ));
    }

    let format_type = params
        .format
        .unwrap_or_else(|| "svg".to_string())
        .to_lowercase();
    let width = params.width.unwrap_or(300).clamp(50, 2000);
    let uri = pubkey_uri(&pubkey);

    let data = match format_type.as_str() {
        "png" => generate_png_qr(&uri, width)?,
        "svg" => generate_svg_qr(&uri, width)?,
        _ => {
            return Err(AuthError::BadRequest(
                "Unsupported format; use 'svg' or 'png'".to_string(),
            ))
        }
    };

    Ok(Json(QrResponse {
        fingerprint: fingerprint(&pubkey),
        pubkey,
        format: format_type,
        data,
    }))
}

/// Parse a QR scan and save the discovered public key as a contact.
pub async fn scan_qr(
    State(_state): State<AppState>,
    auth_user: AuthenticatedUser,
    Json(request): Json<ScanQrRequest>,
) -> Result<Json<ContactResponse>> {
    let (pubkey, _fingerprint) = parse_qr(&request.qr)
        .ok_or_else(|| AuthError::BadRequest("Invalid QR content".to_string()))?;

    let pool = _state.db.inner();
    let contact = Contact::upsert(
        pool,
        auth_user.user_id,
        &pubkey,
        request.label.as_deref(),
        Some("qr"),
    )
    .await?;

    Ok(Json(ContactResponse {
        id: contact.id,
        pubkey: contact.pubkey,
        label: contact.label,
        discovered_via: contact.discovered_via,
        first_seen_at: contact.first_seen_at,
        last_seen_at: contact.last_seen_at,
    }))
}

fn generate_svg_qr(uri: &str, width: u32) -> Result<String> {
    use qrcode::render::svg;

    let svg = qrcode::QrCode::new(uri.as_bytes())
        .map_err(|e| AuthError::BadRequest(format!("QR generation failed: {}", e)))?
        .render::<svg::Color>()
        .min_dimensions(width, width)
        .dark_color(svg::Color("#000000"))
        .light_color(svg::Color("#ffffff"))
        .build();

    Ok(svg)
}

fn generate_png_qr(uri: &str, width: u32) -> Result<String> {
    use base64::engine::{general_purpose::STANDARD, Engine};

    let img = qrcode::QrCode::new(uri.as_bytes())
        .map_err(|e| AuthError::BadRequest(format!("QR generation failed: {}", e)))?
        .render::<image::Rgba<u8>>()
        .min_dimensions(width, width)
        .build();

    let mut png_bytes: Vec<u8> = Vec::new();
    image::DynamicImage::ImageRgba8(img)
        .write_to(
            &mut std::io::Cursor::new(&mut png_bytes),
            image::ImageFormat::Png,
        )
        .map_err(|e| AuthError::Internal(format!("PNG encoding failed: {}", e)))?;

    let b64 = STANDARD.encode(&png_bytes);
    Ok(format!("data:image/png;base64,{}", b64))
}

/// Validate that a string is a Base64URL-encoded 32-byte Ed25519 public key.
fn is_valid_pubkey(pubkey: &str) -> bool {
    use base64::engine::{general_purpose::URL_SAFE_NO_PAD, Engine};

    if pubkey.is_empty() || pubkey.len() < 43 || pubkey.len() > 44 {
        return false;
    }

    URL_SAFE_NO_PAD
        .decode(pubkey)
        .map(|b| b.len() == 32)
        .unwrap_or(false)
}

/// Generate a short human-readable fingerprint for a public key.
fn fingerprint(pubkey: &str) -> String {
    use sha2::{Digest, Sha256};

    let hash = Sha256::digest(pubkey.as_bytes());
    hex::encode(hash)[..8].to_string()
}

/// Build a QR-friendly URI for sharing a public key.
fn pubkey_uri(pubkey: &str) -> String {
    format!("mosaic://{}?fn={}", pubkey, fingerprint(pubkey))
}

/// Parse QR content into (pubkey, fingerprint).
/// Accepts mosaic://, mosiac://, or raw Base64URL pubkeys.
fn parse_qr(scanned: &str) -> Option<(String, String)> {
    let trimmed = scanned.trim();

    // URI schemes
    for scheme in ["mosaic://", "mosiac://", "astral://"] {
        if let Some(rest) = trimmed.strip_prefix(scheme) {
            // For astral://, expect optional path segment "identity/"
            let rest = rest.strip_prefix("identity/").unwrap_or(rest);
            let (pubkey, fp) = parse_uri_host(rest)?;
            return Some((pubkey, fp));
        }
    }

    // Raw Base64URL pubkey
    if is_valid_pubkey(trimmed) {
        return Some((trimmed.to_string(), fingerprint(trimmed)));
    }

    None
}

fn parse_uri_host(rest: &str) -> Option<(String, String)> {
    // Split on ? to separate pubkey from query
    let (host, query) = rest.split_once('?').unwrap_or((rest, ""));
    let pubkey = host.split('/').next()?.to_string();
    if !is_valid_pubkey(&pubkey) {
        return None;
    }
    let fp = if query.is_empty() {
        fingerprint(&pubkey)
    } else {
        let fp = query
            .split('&')
            .find_map(|part| {
                let (k, v) = part.split_once('=')?;
                if k == "fn" {
                    Some(v.to_string())
                } else {
                    None
                }
            })
            .unwrap_or_else(|| fingerprint(&pubkey));
        fp
    };
    Some((pubkey, fp))
}
