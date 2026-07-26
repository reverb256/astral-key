//! Outbound federation — ActivityPub delivery with HTTP Signatures.
//!
//! Implements the HTTP Signatures draft (hs2019) required by ActivityPub
//! federation:
//!
//! - **Signing**: outbound POST requests to remote inboxes carry a
//!   `Signature` header signed with the bridge's Ed25519 key
//! - **Verification**: incoming POST requests on `/inbox` are verified
//!   against the sending actor's public key
//!
//! The bridge also provides rate-limited fan-out to followers via
//! [`FederationService::deliver_to_followers`].

use crate::activitypub::{activity_id, activity_types};
use crate::storage::ActivityPubStore;
use anyhow::{Context, Result};
use base64::Engine;
use chrono::Utc;
use ring::signature::{Ed25519KeyPair, KeyPair, UnparsedPublicKey, ED25519};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Semaphore;

// ─── HTTP Signatures constants ───────────────────────────────────────────────

/// The signature algorithm identifier used in HTTP Signatures.
const SIGNATURE_ALGORITHM: &str = "hs2019";

/// The key ID format for our actor's signing key.
fn key_id(domain: &str) -> String {
    format!("https://{domain}/actor#main-key")
}

/// Default headers to sign — the ActivityPub minimum.
const SIGNED_HEADERS: &[&str] = &["(request-target)", "host", "date"];

// ─── Federation service ──────────────────────────────────────────────────────

/// Manages outbound federation — delivering activities to remote inboxes.
///
/// Rate-limited: uses a `tokio::sync::Semaphore` to cap concurrent
/// outbound deliveries.
#[derive(Clone)]
pub struct FederationService {
    /// The bridge's Ed25519 key pair for signing outgoing requests.
    key_pair: Arc<Ed25519KeyPair>,
    /// The public domain used to build actor IRIs.
    domain: Arc<String>,
    /// Reusable reqwest client for outbound HTTP.
    http_client: reqwest::Client,
    /// Concurrency limiter (max parallel deliveries).
    delivery_semaphore: Arc<Semaphore>,
}

impl FederationService {
    /// Create a new federation service.
    ///
    /// `max_concurrent_deliveries` caps how many outbound inbox POSTs
    /// can happen simultaneously (default: 8).
    pub fn new(key_pair: Ed25519KeyPair, domain: &str, max_concurrent_deliveries: usize) -> Self {
        let http_client = reqwest::Client::builder()
            .user_agent("mosaic-activitypub-bridge/0.1.0")
            .timeout(Duration::from_secs(30))
            .build()
            .expect("Failed to create reqwest Client");

        Self {
            key_pair: Arc::new(key_pair),
            domain: Arc::new(domain.to_string()),
            http_client,
            delivery_semaphore: Arc::new(Semaphore::new(max_concurrent_deliveries)),
        }
    }

    /// Get a reference to the Ed25519 key pair.
    pub fn key_pair(&self) -> &Ed25519KeyPair {
        &self.key_pair
    }

    /// Get the public key hex.
    pub fn public_key_hex(&self) -> String {
        hex::encode(self.key_pair.public_key().as_ref())
    }

    // ─── HTTP Signatures: Signing ───────────────────────────────────────────

    /// Build the `Signature` header for an HTTP request.
    ///
    /// Follows the HTTP Signatures IETF draft (`hs2019` algorithm):
    /// 1. Build the signing string from `(request-target)`, `host`, and `date`
    /// 2. Sign with Ed25519
    /// 3. Format as `Signature` header value
    pub fn build_signature_header(
        &self,
        method: &str,
        path: &str,
        host: &str,
        date: &str,
    ) -> String {
        let signing_string = format!(
            "(request-target): {} {}\nhost: {}\ndate: {}",
            method.to_lowercase(),
            path,
            host,
            date
        );

        let signature_bytes = self.key_pair.sign(signing_string.as_bytes());
        let signature_b64 =
            base64::engine::general_purpose::STANDARD.encode(signature_bytes.as_ref());

        let headers = SIGNED_HEADERS.join(" ");

        format!(
            r#"keyId="{}",algorithm="{}",headers="{}",signature="{}""#,
            key_id(&self.domain),
            SIGNATURE_ALGORITHM,
            headers,
            signature_b64,
        )
    }

    // ─── HTTP Signatures: Verification ──────────────────────────────────────

    /// Parse an HTTP Signature header into its components.
    ///
    /// Returns `(key_id, headers_list, signature_base64)`.
    fn parse_signature_header(signature_header: &str) -> Result<(String, Vec<String>, String)> {
        let mut key_id = String::new();
        let mut headers = String::new();
        let mut signature = String::new();
        let mut _algorithm = String::new();

        // Split by comma, handle quoted values
        for part in signature_header.split(',') {
            let part = part.trim();
            if let Some((k, v)) = part.split_once('=') {
                let key = k.trim();
                let value = v.trim().trim_matches('"');
                match key {
                    "keyId" => key_id = value.to_string(),
                    "headers" => headers = value.to_string(),
                    "signature" => signature = value.to_string(),
                    "algorithm" => _algorithm = value.to_string(),
                    _ => {}
                }
            }
        }

        if key_id.is_empty() || signature.is_empty() {
            anyhow::bail!("Invalid Signature header: missing keyId or signature fields");
        }

        let header_list: Vec<String> = if headers.is_empty() {
            // Default: created was signed (hs2019 default)
            vec!["date".to_string()]
        } else {
            headers.split_whitespace().map(String::from).collect()
        };

        Ok((key_id, header_list, signature))
    }

    /// Reconstruct the signing string from actual request headers.
    ///
    /// The `method` and `path` are used for `(request-target)`.
    /// Other headers are looked up by name.
    fn build_verification_string(
        method: &str,
        path: &str,
        headers: &[String],
        header_map: &std::collections::HashMap<String, String>,
    ) -> String {
        let mut parts = Vec::new();
        for h in headers {
            match h.as_str() {
                "(request-target)" => {
                    parts.push(format!(
                        "(request-target): {} {}",
                        method.to_lowercase(),
                        path
                    ));
                }
                other => {
                    if let Some(value) = header_map.get(other) {
                        parts.push(format!("{}: {}", other.to_lowercase(), value));
                    }
                }
            }
        }
        parts.join("\n")
    }

    /// Fetch a remote actor's public key from their actor profile.
    ///
    /// Given a `keyId` URL like `https://instance.social/users/alice#main-key`,
    /// fetches the actor document and extracts `publicKey.publicKeyPem`.
    pub async fn fetch_actor_public_key(key_id_url: &str) -> Result<String> {
        // Strip the fragment to get the actor URL
        let actor_url = key_id_url.split('#').next().unwrap_or(key_id_url);

        let client = reqwest::Client::builder()
            .user_agent("mosaic-activitypub-bridge/0.1.0")
            .timeout(Duration::from_secs(10))
            .build()
            .context("Failed to create reqwest client")?;

        let resp = client
            .get(actor_url)
            .header("Accept", "application/activity+json, application/ld+json")
            .send()
            .await
            .context(format!("Failed to fetch actor: {actor_url}"))?;

        if !resp.status().is_success() {
            anyhow::bail!("Actor fetch returned {} for {}", resp.status(), actor_url);
        }

        let actor: Value = resp.json().await.context("Failed to parse actor JSON")?;

        let pubkey = actor
            .get("publicKey")
            .and_then(|pk| pk.get("publicKeyPem"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| anyhow::anyhow!("No publicKeyPem found in actor: {actor_url}"))?;

        Ok(pubkey)
    }

    /// Verify an HTTP Signature against a request.
    ///
    /// # Arguments
    /// * `method` — HTTP method (e.g. "POST")
    /// * `path` — request path (e.g. "/inbox")
    /// * `signature_header` — value of the `Signature` header
    /// * `headers` — map of header names to values (at minimum must include Host, Date)
    ///
    /// Returns the verified actor ID on success.
    pub async fn verify_signature(
        &self,
        method: &str,
        path: &str,
        signature_header: &str,
        headers: &std::collections::HashMap<String, String>,
    ) -> Result<String> {
        let (key_id, header_list, signature_b64) = Self::parse_signature_header(signature_header)?;

        // Reconstruct the signing string
        let signing_string = Self::build_verification_string(method, path, &header_list, headers);

        // Decode the signature
        let signature_bytes = base64::engine::general_purpose::STANDARD
            .decode(&signature_b64)
            .context("Failed to decode signature base64")?;

        // Fetch the actor's public key from keyId URL
        let pem = Self::fetch_actor_public_key(&key_id).await?;

        // Parse the PEM to extract raw Ed25519 public key bytes
        let pubkey_bytes = pem_to_ed25519_pubkey(&pem).context("Failed to parse PEM public key")?;

        // Verify using ring
        let public_key: UnparsedPublicKey<&[u8]> = UnparsedPublicKey::new(&ED25519, &pubkey_bytes);
        public_key
            .verify(signing_string.as_bytes(), &signature_bytes)
            .map_err(|_| anyhow::anyhow!("HTTP Signature verification failed"))?;

        tracing::debug!("HTTP Signature verified for keyId: {}", key_id);
        Ok(key_id)
    }

    // ─── Outbound delivery ──────────────────────────────────────────────────

    /// Deliver an activity JSON payload to a remote inbox.
    ///
    /// Signs the HTTP request with the bridge's Ed25519 key
    /// per the HTTP Signatures spec.
    async fn deliver_to_inbox(
        &self,
        inbox_url: &str,
        activity_body: &Value,
        activity_id_str: &str,
    ) -> Result<()> {
        let _permit = self
            .delivery_semaphore
            .acquire()
            .await
            .context("Failed to acquire delivery permit")?;

        let url = url::Url::parse(inbox_url).context("Invalid inbox URL")?;
        let host = url
            .host_str()
            .context("Inbox URL missing host")?
            .to_string();
        let path = url.path().to_string();
        let date = Utc::now().format("%a, %d %b %Y %H:%M:%S GMT").to_string();

        // Build the body as JSON
        let body_json =
            serde_json::to_string(activity_body).context("Failed to serialize activity")?;

        // Compute digest header
        let digest = {
            let mut hasher = Sha256::new();
            hasher.update(body_json.as_bytes());
            let hash = hasher.finalize();
            format!(
                "SHA-256={}",
                base64::engine::general_purpose::STANDARD.encode(hash)
            )
        };

        // Build the Signature header
        let signature = self.build_signature_header("POST", &path, &host, &date);

        tracing::debug!(
            "Delivering {} to {} (host: {}, path: {})",
            activity_id_str,
            inbox_url,
            host,
            path
        );

        let resp = self
            .http_client
            .post(inbox_url)
            .header("Host", &host)
            .header("Date", &date)
            .header("Digest", &digest)
            .header("Signature", &signature)
            .header("Content-Type", "application/activity+json")
            .header("Accept", "application/activity+json, application/ld+json")
            .body(body_json)
            .send()
            .await
            .context(format!("Failed to deliver to {inbox_url}"))?;

        let status = resp.status();
        let body_text = resp.text().await.unwrap_or_default();

        if status.is_success() || status.as_u16() == 202 {
            tracing::info!(
                "Delivered {} to {} (status: {})",
                activity_id_str,
                inbox_url,
                status
            );
            Ok(())
        } else {
            anyhow::bail!("Delivery to {inbox_url} returned {status}: {body_text:.200}",)
        }
    }

    /// Fan-out an activity to all followers.
    ///
    /// For each follower, delivers to their `shared_inbox_url` (preferred)
    /// or `inbox_url`. Errors are logged but do not abort the fan-out.
    ///
    /// Returns `(success_count, error_count)`.
    pub async fn deliver_to_followers(
        &self,
        store: &ActivityPubStore,
        activity: &Value,
    ) -> (usize, usize) {
        let activity_id_str = activity
            .get("id")
            .and_then(|v| v.as_str())
            .unwrap_or("<unknown>");

        let followers = store.get_followers().await;
        let mut success = 0usize;
        let mut errors = 0usize;

        // De-duplicate inboxes by URL (prefer shared_inbox)
        let mut delivered = std::collections::HashSet::new();

        for follower in &followers {
            let target_url = follower
                .shared_inbox_url
                .as_deref()
                .unwrap_or(&follower.inbox_url)
                .to_string();

            if !delivered.insert(target_url.clone()) {
                // Already delivered to this URL (shared inbox)
                continue;
            }

            if let Err(e) = self
                .deliver_to_inbox(&target_url, activity, activity_id_str)
                .await
            {
                tracing::warn!(
                    "Failed to deliver {} to {}: {:#}",
                    activity_id_str,
                    target_url,
                    e
                );
                errors += 1;
            } else {
                success += 1;
            }
        }

        tracing::info!(
            "Fan-out complete: {success} delivered, {errors} failed for {activity_id_str}"
        );
        (success, errors)
    }

    /// Build and deliver a Follow-Accept activity back to the follower's inbox.
    pub async fn send_accept(
        &self,
        follower_actor_id: &str,
        follow_activity_id: &str,
    ) -> Result<()> {
        // Fetch the follower's inbox from their actor profile
        let (inbox_url, shared_inbox_url) = fetch_actor_inbox(follower_actor_id).await?;
        let target_url = shared_inbox_url.as_deref().unwrap_or(&inbox_url);
        let actor_id = format!("https://{}/actor", self.domain);
        let accept_id = activity_id(&self.domain, &uuid::Uuid::new_v4().to_string());

        let accept = serde_json::json!({
            "@context": crate::activitypub::AS_CONTEXT,
            "id": accept_id,
            "type": activity_types::ACCEPT,
            "actor": actor_id,
            "object": {
                "id": follow_activity_id,
                "type": activity_types::FOLLOW,
                "actor": follower_actor_id,
                "object": actor_id,
            },
        });

        self.deliver_to_inbox(target_url, &accept, &accept_id)
            .await?;
        Ok(())
    }

    /// Send a Remove activity for a follower that was removed.
    pub async fn send_undo_follow(&self, follower_actor_id: &str) -> Result<()> {
        let (inbox_url, shared_inbox_url) = fetch_actor_inbox(follower_actor_id).await?;
        let target_url = shared_inbox_url.as_deref().unwrap_or(&inbox_url);
        let actor_id = format!("https://{}/actor", self.domain);
        let undo_id = activity_id(&self.domain, &uuid::Uuid::new_v4().to_string());

        let undo = serde_json::json!({
            "@context": crate::activitypub::AS_CONTEXT,
            "id": undo_id,
            "type": activity_types::UNDO,
            "actor": actor_id,
            "object": {
                "id": format!("{}/follows/{}", actor_id, follower_actor_id),
                "type": activity_types::FOLLOW,
                "actor": follower_actor_id,
                "object": actor_id,
            },
        });

        self.deliver_to_inbox(target_url, &undo, &undo_id).await?;
        Ok(())
    }
}

// ─── PEM parsing helpers ─────────────────────────────────────────────────────

/// Extract raw Ed25519 public key bytes from a PEM-encoded SPKI.
///
/// Supports both PKIX/SPKI PEM (`-----BEGIN PUBLIC KEY-----`) and
/// the bare `BEGIN RSA PUBLIC KEY` formats. Returns the 32-byte
/// raw Ed25519 public key.
fn pem_to_ed25519_pubkey(pem: &str) -> Result<Vec<u8>> {
    let pem = pem.trim();

    // Find the base64 body between PEM headers
    let b64_body = pem
        .lines()
        .filter(|line| !line.starts_with("-----"))
        .collect::<Vec<_>>()
        .join("");

    if b64_body.is_empty() {
        anyhow::bail!("Empty PEM body");
    }

    let der_bytes = base64::engine::general_purpose::STANDARD
        .decode(&b64_body)
        .context("Failed to base64-decode PEM body")?;

    // Parse DER-encoded SubjectPublicKeyInfo
    // Ed25519 SPKI structure:
    //   SEQUENCE {
    //     SEQUENCE { OID 1.3.101.112 (Ed25519) }
    //     BIT STRING { <32 bytes pubkey> }
    //   }
    //
    // The public key bytes start after the algorithm identifier.

    // Simple DER parser — find the BIT STRING tag (0x03)
    // and extract the following 33 bytes (1 unused bits + 32 pubkey)
    for i in 0..der_bytes.len().saturating_sub(34) {
        if der_bytes[i] == 0x03 && der_bytes[i + 1] == 0x21 {
            // BIT STRING, length 33 (unused bits + 32 bytes key)
            return Ok(der_bytes[i + 3..i + 35].to_vec());
        }
    }

    anyhow::bail!("Could not find Ed25519 public key in PEM (expected 32-byte BIT STRING)");
}

/// PEM-encode an Ed25519 public key in SPKI format.
///
/// Constructs the DER SubjectPublicKeyInfo and wraps it in PEM.
pub fn ed25519_pubkey_to_pem(pubkey_bytes: &[u8]) -> String {
    // Ed25519 SPKI DER:
    // 30 2a        SEQUENCE (42 bytes)
    //   30 05      SEQUENCE (5 bytes) — AlgorithmIdentifier
    //     06 03 2b 65 70   OID 1.3.101.112
    //   03 21      BIT STRING (33 bytes)
    //     00       unused bits = 0
    //     <32 bytes public key>
    let mut der = Vec::with_capacity(44);
    der.push(0x30); // SEQUENCE
    der.push(0x2a); // length (42)
    der.push(0x30); // SEQUENCE (AlgorithmIdentifier)
    der.push(0x05); // length (5)
    der.push(0x06); // OID
    der.push(0x03); // length (3)
    der.push(0x2b); // 1.3.101.112 (Ed25519)
    der.push(0x65);
    der.push(0x70);
    der.push(0x03); // BIT STRING
    der.push(0x21); // length (33)
    der.push(0x00); // unused bits
    der.extend_from_slice(pubkey_bytes);

    let b64 = base64::engine::general_purpose::STANDARD.encode(&der);
    let mut pem = String::from("-----BEGIN PUBLIC KEY-----\n");
    // RFC 7468: 64 chars per line
    for chunk in b64.as_bytes().chunks(64) {
        pem.push_str(&String::from_utf8_lossy(chunk));
        pem.push('\n');
    }
    pem.push_str("-----END PUBLIC KEY-----");
    pem
}

/// Fetch a remote actor's inbox(es) by fetching their Actor profile.
async fn fetch_actor_inbox(actor_id: &str) -> Result<(String, Option<String>)> {
    let client = reqwest::Client::builder()
        .user_agent("mosaic-activitypub-bridge/0.1.0")
        .timeout(Duration::from_secs(10))
        .build()
        .context("Failed to create reqwest client")?;

    let resp = client
        .get(actor_id)
        .header("Accept", "application/activity+json, application/ld+json")
        .send()
        .await
        .context(format!("Failed to fetch actor: {actor_id}"))?;

    if !resp.status().is_success() {
        anyhow::bail!("Actor fetch returned {} for {}", resp.status(), actor_id);
    }

    let actor: Value = resp.json().await.context("Failed to parse actor JSON")?;

    let inbox = actor
        .get("inbox")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| anyhow::anyhow!("No inbox found in actor: {actor_id}"))?;

    let shared_inbox = actor
        .get("endpoints")
        .and_then(|e| e.get("sharedInbox"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    Ok((inbox, shared_inbox))
}

/// Generate a new Ed25519 key pair from a random seed.
///
/// The seed is 32 cryptographically random bytes. Returns the
/// key pair and the hex-encoded seed.
pub fn generate_key_pair() -> Result<(Ed25519KeyPair, String)> {
    let seed: [u8; 32] = rand::random();
    let seed_hex = hex::encode(&seed);

    let key_pair = Ed25519KeyPair::from_seed_unchecked(&seed)
        .map_err(|e| anyhow::anyhow!("Failed to create Ed25519 key pair: {e}"))?;

    Ok((key_pair, seed_hex))
}

/// Reconstruct an Ed25519 key pair from a hex-encoded seed.
pub fn key_pair_from_seed(seed_hex: &str) -> Result<Ed25519KeyPair> {
    let seed = hex::decode(seed_hex).context("Invalid hex seed")?;
    if seed.len() != 32 {
        anyhow::bail!("Ed25519 seed must be exactly 32 bytes (got {})", seed.len());
    }
    let key_pair = Ed25519KeyPair::from_seed_unchecked(&seed)
        .map_err(|e| anyhow::anyhow!("Failed to create Ed25519 key pair from seed: {e}"))?;
    Ok(key_pair)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_key_pair() {
        let (kp, seed_hex) = generate_key_pair().unwrap();
        assert_eq!(seed_hex.len(), 64); // 32 bytes = 64 hex chars
        assert_eq!(kp.public_key().as_ref().len(), 32);
    }

    #[test]
    fn test_reconstruct_from_seed() {
        let (kp1, seed_hex) = generate_key_pair().unwrap();
        let kp2 = key_pair_from_seed(&seed_hex).unwrap();
        assert_eq!(kp1.public_key().as_ref(), kp2.public_key().as_ref());
    }

    #[test]
    fn test_pem_roundtrip() {
        let (kp, _) = generate_key_pair().unwrap();
        let pubkey = kp.public_key().as_ref();
        let pem = ed25519_pubkey_to_pem(pubkey);
        assert!(pem.starts_with("-----BEGIN PUBLIC KEY-----"));
        assert!(pem.ends_with("-----END PUBLIC KEY-----"));
        let decoded = pem_to_ed25519_pubkey(&pem).unwrap();
        assert_eq!(decoded, pubkey);
    }

    #[test]
    fn test_sign_and_verify() {
        let (kp, _) = generate_key_pair().unwrap();
        let message = b"test message for signing";
        let signature = kp.sign(message);

        let pubkey_bytes = kp.public_key().as_ref();
        let public_key = UnparsedPublicKey::new(&ED25519, pubkey_bytes);
        assert!(public_key.verify(message, signature.as_ref()).is_ok());

        // Wrong message should fail
        let wrong_message = b"wrong message";
        assert!(public_key
            .verify(wrong_message, signature.as_ref())
            .is_err());
    }

    #[test]
    fn test_parse_signature_header() {
        let header = r#"keyId="https://example.com/actor#main-key",algorithm="hs2019",headers="(request-target) host date",signature="YmFzZTY0c2lnbmF0dXJl""#;
        let (key_id, headers, sig) = parse_signature_header_inner(header).unwrap();
        assert_eq!(key_id, "https://example.com/actor#main-key");
        assert_eq!(headers, vec!["(request-target)", "host", "date"]);
        assert_eq!(sig, "YmFzZTY0c2lnbmF0dXJl");
    }

    // Test wrapper to call the private function
    fn parse_signature_header_inner(header: &str) -> Result<(String, Vec<String>, String)> {
        FederationService::parse_signature_header(header)
    }
}
