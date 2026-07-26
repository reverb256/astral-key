//! atproto DID resolution for identity binding claims.
//!
//! Resolves `did:plc:...` and handles via PLC directory.
//! Supports Multikey verification method format (current AT Protocol standard).
//! Returns the DID document and a Mosaic-compatible identity summary.

use serde::{Deserialize, Serialize};

use crate::error::Error;

/// A resolved atproto identity, ready to be stored as a Mosaic binding.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolvedIdentity {
    /// The resolved DID
    pub did: String,

    /// The user's handle (from alsoKnownAs)
    pub handle: Option<String>,

    /// PDS endpoint
    pub pds: Option<String>,

    /// Verification method public key (multibase)
    pub signing_key_multibase: Option<String>,

    /// Verification method type (e.g. "Multikey")
    pub signing_key_type: Option<String>,

    /// Mosaic-compatible summary
    pub mosaic: ResolvedMosaic,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolvedMosaic {
    /// The DID used as external_id
    pub external_id: String,

    /// Suggested display name
    pub display_name: String,
}

/// Resolve an atproto handle or DID to its DID document.
///
/// This calls the PLC directory and optionally the Bsky handle resolution API.
pub async fn resolve(input: &str) -> Result<ResolvedIdentity, Error> {
    let did = resolve_to_did(input).await?;
    let doc = fetch_did_document(&did).await?;

    // Extract handle from alsoKnownAs
    let handle = doc
        .also_known_as
        .as_ref()
        .and_then(|aka| aka.iter().find(|a| a.starts_with("at://")).cloned())
        .map(|a| a.trim_start_matches("at://").to_string());

    // Extract PDS
    let pds = doc.service.as_ref().and_then(|services| {
        services
            .iter()
            .find(|s| s.type_ == "AtprotoPersonalDataServer")
            .map(|s| s.service_endpoint.clone())
    });

    // Extract verification method (Multikey or legacy EcdsaSecp*)
    let vm = doc.verification_method.as_ref().and_then(|vms| vms.first());

    let (signing_key_multibase, signing_key_type) = vm.map_or((None, None), |vm| {
        (vm.public_key_multibase.clone(), Some(vm.type_.clone()))
    });

    Ok(ResolvedIdentity {
        did: did.clone(),
        handle: handle.clone(),
        pds,
        signing_key_multibase,
        signing_key_type,
        mosaic: ResolvedMosaic {
            external_id: did.clone(),
            display_name: handle.unwrap_or(did),
        },
    })
}

/// Resolve a handle or DID to a DID string.
async fn resolve_to_did(input: &str) -> Result<String, Error> {
    let input = input.trim();

    // Already a DID
    if input.starts_with("did:") {
        return Ok(input.to_string());
    }

    // Strip @ prefix and protocol prefixes
    let handle = input
        .trim_start_matches('@')
        .trim_start_matches("https://")
        .trim_start_matches("at://")
        .trim_end_matches('/');

    if handle.contains('.') {
        // Resolve handle via bsky.social
        let url = format!(
            "https://bsky.social/xrpc/com.atproto.identity.resolveHandle?handle={}",
            urlencoding(handle)
        );
        let client = reqwest::Client::new();
        let resp = client
            .get(&url)
            .header("User-Agent", "mosaic-identity-service/1.0")
            .send()
            .await
            .map_err(|e| Error::BadRequest(format!("Handle resolution failed: {}", e)))?;

        if !resp.status().is_success() {
            return Err(Error::NotFound(format!("Handle not found: {}", handle)));
        }

        let body: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| Error::BadRequest(format!("Invalid response: {}", e)))?;

        body["did"]
            .as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| Error::NotFound(format!("No DID returned for handle: {}", handle)))
    } else {
        Err(Error::BadRequest(format!("Not a DID or handle: {}", input)))
    }
}

/// Fetch the DID document from the PLC directory.
async fn fetch_did_document(did: &str) -> Result<DidDocument, Error> {
    let url = format!("https://plc.directory/{}", did);
    let client = reqwest::Client::new();
    let resp = client
        .get(&url)
        .header("User-Agent", "mosaic-identity-service/1.0")
        .send()
        .await
        .map_err(|e| Error::BadRequest(format!("PLC directory unreachable: {}", e)))?;

    if resp.status() == StatusCode::NOT_FOUND {
        return Err(Error::NotFound(format!("DID not registered: {}", did)));
    }

    if !resp.status().is_success() {
        return Err(Error::BadRequest(format!("PLC returned {}", resp.status())));
    }

    let doc: DidDocument = resp
        .json()
        .await
        .map_err(|e| Error::BadRequest(format!("Invalid DID document: {}", e)))?;

    Ok(doc)
}

// ─── DID document types ─────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DidDocument {
    id: String,
    #[serde(default)]
    also_known_as: Option<Vec<String>>,
    #[serde(default)]
    verification_method: Option<Vec<VerificationMethod>>,
    #[serde(default)]
    service: Option<Vec<Service>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct VerificationMethod {
    id: String,
    #[serde(rename = "type")]
    type_: String,
    controller: Option<String>,
    public_key_multibase: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Service {
    id: Option<String>,
    #[serde(rename = "type")]
    type_: String,
    service_endpoint: String,
}

fn urlencoding(s: &str) -> String {
    // Simple URL encoding for handle characters
    s.replace('%', "%25")
        .replace(' ', "%20")
        .replace('@', "%40")
}

use axum::http::StatusCode;
