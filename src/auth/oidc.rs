//! Astral Key — OIDC provider (identity-provider side)
//!
//! Turns Astral Key into an OpenID Connect **provider** so relying parties
//! such as oauth2-proxy can authenticate users with passkeys. Implements the
//! authorization-code flow:
//!
//! ```text
//!   RP ──authorize──▶ Astral Key (login page + WebAuthn) ──code──▶ RP
//!   RP ──token─────▶ Astral Key (id_token + access_token + refresh_token)
//!   RP ──userinfo──▶ Astral Key (claims from access token)
//! ```
//!
//! Tokens are signed **RS256** with the RSA key when `OIDC_RSA_KEY` /
//! `OIDC_RSA_KEY_FILE` is configured, and **EdDSA** (Ed25519) when it is not.
//! The public key is exposed via the JWKS endpoint so relying parties can
//! verify id_tokens out-of-band.
//!
//! Only the authorization-code flow is supported — no implicit flow, no
//! device flow. PKCE (S256) is verified when the relying party sends a
//! challenge, and is MANDATORY for public clients (no client secret).
//!
//! Relying parties are rows in the `oidc_clients` table — see
//! `crate::db::models::oidc_client`. Redirect URIs match exactly.

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use ed25519_dalek::pkcs8::{EncodePrivateKey, EncodePublicKey};
use ed25519_dalek::{SigningKey, VerifyingKey};
use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use rand::RngCore;
use rsa::pkcs8::DecodePrivateKey;
use rsa::traits::PublicKeyParts;
use rsa::{RsaPrivateKey, RsaPublicKey};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::time::Duration;
use uuid::Uuid;

use crate::error::{AuthError, Result};

/// Lowercase hex SHA-256 of a client secret — the storage format for
/// `oidc_clients.client_secret_hash`.
pub fn hash_client_secret(secret: &str) -> String {
    hex::encode(Sha256::digest(secret.as_bytes()))
}

/// Lowercase hex SHA-256 of an opaque refresh token. Only the hash is stored
/// and looked up, so a database leak cannot be replayed.
pub fn hash_refresh_token(token: &str) -> String {
    hex::encode(Sha256::digest(token.as_bytes()))
}

/// A registered OIDC relying party (a row in `oidc_clients`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OidcClient {
    pub client_id: String,
    /// Lowercase hex SHA-256 of the client secret. `None` marks a **public**
    /// client: it must use PKCE (S256) and must not present a secret.
    pub client_secret_hash: Option<String>,
    /// Registered redirect URIs — matched exactly.
    pub redirect_uris: Vec<String>,
    /// Scopes the client may request. `None` allows every advertised scope.
    pub allowed_scopes: Option<Vec<String>>,
}

impl OidcClient {
    /// Build a confidential client. The plaintext secret is hashed here.
    pub fn confidential(
        client_id: String,
        plain_secret: &str,
        redirect_uris: Vec<String>,
        allowed_scopes: Option<Vec<String>>,
    ) -> Self {
        Self {
            client_id,
            client_secret_hash: Some(hash_client_secret(plain_secret)),
            redirect_uris,
            allowed_scopes,
        }
    }

    /// Build a public client (PKCE only, no secret).
    pub fn public(client_id: String, redirect_uris: Vec<String>) -> Self {
        Self {
            client_id,
            client_secret_hash: None,
            redirect_uris,
            allowed_scopes: None,
        }
    }

    /// True when the client holds no secret and therefore must use PKCE.
    pub fn is_public(&self) -> bool {
        self.client_secret_hash.is_none()
    }

    /// Verify a presented secret in constant time.
    ///
    /// Always false for public clients: they authenticate with PKCE instead.
    pub fn verify_secret(&self, presented: &str) -> bool {
        let Some(expected) = self.client_secret_hash.as_deref() else {
            return false;
        };

        use subtle::ConstantTimeEq;
        let candidate = hash_client_secret(presented);
        expected.as_bytes().ct_eq(candidate.as_bytes()).into()
    }

    /// Exact redirect-URI match — no prefixes, no wildcards.
    pub fn allows_redirect_uri(&self, redirect_uri: &str) -> bool {
        self.redirect_uris.iter().any(|uri| uri == redirect_uri)
    }

    /// True when the client may request this scope.
    pub fn allows_scope(&self, scope: &str) -> bool {
        match &self.allowed_scopes {
            None => true,
            Some(allowed) => allowed.iter().any(|s| s == scope),
        }
    }
}

/// Claims carried by the OIDC **access token** (also used by /userinfo).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OidcAccessClaims {
    pub sub: String,
    pub iss: String,
    pub aud: String,
    pub exp: usize,
    pub iat: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    /// True when `email` was set by the owner or an upstream provider.
    /// Astral Key sends no verification e-mails, so this asserts provenance,
    /// not a completed round-trip.
    #[serde(rename = "email_verified", skip_serializing_if = "Option::is_none")]
    pub email_verified: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(rename = "preferred_username", skip_serializing_if = "Option::is_none")]
    pub preferred_username: Option<String>,
}

/// Claims carried by the OIDC **id_token**.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OidcIdClaims {
    pub sub: String,
    pub iss: String,
    pub aud: String,
    pub exp: usize,
    pub iat: usize,
    pub azp: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nonce: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    /// See `OidcAccessClaims::email_verified`. Omitted when the account has
    /// no email address.
    #[serde(rename = "email_verified", skip_serializing_if = "Option::is_none")]
    pub email_verified: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(rename = "preferred_username", skip_serializing_if = "Option::is_none")]
    pub preferred_username: Option<String>,
    /// Group names. Always serialized — an empty array when the user has no
    /// membership. Defaulted on decode so pre-groups tokens still parse.
    #[serde(default)]
    pub groups: Vec<String>,
}

/// Identity-provider service: key management + token minting/validation.
#[derive(Clone)]
pub struct OidcService {
    pub issuer: String,
    /// Stable Ed25519 key id — derived from the public key so it survives
    /// restarts.
    pub kid: String,
    /// Stable RSA key id — SHA-256 of the modulus, truncated. `None` when no
    /// RSA key is configured.
    pub rsa_kid: Option<String>,
    signing_key: SigningKey,
    encoding_key: EncodingKey,
    decoding_key: DecodingKey,
    /// RSA signing key (optional).
    rsa_encoding_key: Option<EncodingKey>,
    /// RSA verification key (optional).
    rsa_decoding_key: Option<DecodingKey>,
    /// RSA public key, used for the JWKS entry (optional).
    rsa_public_key: Option<RsaPublicKey>,
    access_token_ttl: Duration,
    id_token_ttl: Duration,
}

impl OidcService {
    /// Build the service from config. The Ed25519 seed must be stable across
    /// restarts in production (see `OIDC_SIGNING_KEY`); callers that pass an
    /// ephemeral key are responsible for only using it in dev/test contexts.
    ///
    /// `rsa_pem` is an optional PKCS#8/PKCS#1 RSA private key. When present,
    /// every minted token is signed RS256 with it.
    pub fn new(
        issuer: String,
        seed: [u8; 32],
        access_token_ttl: Duration,
        id_token_ttl: Duration,
        rsa_pem: Option<String>,
    ) -> Result<Self> {
        let signing_key = SigningKey::from_bytes(&seed);
        let verifying_key: VerifyingKey = signing_key.verifying_key();

        // PKCS#8 / SPKI DER from ed25519-dalek → PEM for jsonwebtoken.
        let private_pem = pem_from_der(
            "PRIVATE KEY",
            signing_key
                .to_pkcs8_der()
                .map_err(|e| AuthError::Internal(format!("PKCS8 encode failed: {e}")))?
                .as_bytes(),
        );
        let public_pem = pem_from_der(
            "PUBLIC KEY",
            verifying_key
                .to_public_key_der()
                .map_err(|e| AuthError::Internal(format!("SPKI encode failed: {e}")))?
                .as_bytes(),
        );

        let encoding_key = EncodingKey::from_ed_pem(private_pem.as_bytes())
            .map_err(|e| AuthError::Config(format!("Invalid Ed25519 private key: {e}")))?;
        let decoding_key = DecodingKey::from_ed_pem(public_pem.as_bytes())
            .map_err(|e| AuthError::Config(format!("Invalid Ed25519 public key: {e}")))?;

        let kid = URL_SAFE_NO_PAD.encode(&verifying_key.to_bytes()[..16]);

        // RSA key (optional): signing material for RS256.
        let (rsa_encoding_key, rsa_decoding_key, rsa_kid, rsa_public_key) = match rsa_pem {
            Some(ref pem) => {
                let rsa_private_key = RsaPrivateKey::from_pkcs8_pem(pem)
                    .map_err(|e| AuthError::Config(format!("Invalid RSA private key PEM: {e}")))?;
                let rsa_public_key_inner = rsa_private_key.to_public_key();

                let encoding_key = EncodingKey::from_rsa_pem(pem.as_bytes()).map_err(|e| {
                    AuthError::Config(format!("Invalid RSA private key for encoding: {e}"))
                })?;

                // Derive the verification key from the public components so no
                // PEM variant (PKCS#1 vs PKCS#8, private vs public) can break it.
                let modulus = URL_SAFE_NO_PAD.encode(rsa_public_key_inner.n().to_bytes_be());
                let exponent = URL_SAFE_NO_PAD.encode(rsa_public_key_inner.e().to_bytes_be());
                let decoding_key = DecodingKey::from_rsa_components(&modulus, &exponent)
                    .map_err(|e| AuthError::Config(format!("Invalid RSA public key: {e}")))?;

                // Stable key id: SHA-256 of the modulus, truncated.
                let digest = Sha256::digest(rsa_public_key_inner.n().to_bytes_be());
                let kid = URL_SAFE_NO_PAD.encode(&digest[..16]);

                (
                    Some(encoding_key),
                    Some(decoding_key),
                    Some(kid),
                    Some(rsa_public_key_inner),
                )
            }
            None => (None, None, None, None),
        };

        Ok(Self {
            issuer,
            kid,
            rsa_kid,
            signing_key,
            encoding_key,
            decoding_key,
            rsa_encoding_key,
            rsa_decoding_key,
            rsa_public_key,
            access_token_ttl,
            id_token_ttl,
        })
    }

    /// JOSE `alg` value matching the active signing key.
    pub fn signing_alg_name(&self) -> &'static str {
        if self.rsa_encoding_key.is_some() {
            "RS256"
        } else {
            "EdDSA"
        }
    }

    /// (algorithm, key id, encoding key) for the active signing key.
    fn active_signing_key(&self) -> (Algorithm, &str, &EncodingKey) {
        match (&self.rsa_encoding_key, self.rsa_kid.as_deref()) {
            (Some(key), Some(kid)) => (Algorithm::RS256, kid, key),
            _ => (Algorithm::EdDSA, self.kid.as_str(), &self.encoding_key),
        }
    }

    /// Verification key + algorithm matching `active_signing_key()`.
    fn active_decoding_key(&self) -> (&DecodingKey, Algorithm) {
        match &self.rsa_decoding_key {
            Some(key) => (key, Algorithm::RS256),
            None => (&self.decoding_key, Algorithm::EdDSA),
        }
    }

    /// Validation pre-configured with the active algorithm + our issuer.
    fn validation(&self) -> Validation {
        let (_, alg) = self.active_decoding_key();
        let mut validation = Validation::new(alg);
        validation.set_issuer(&[&self.issuer]);
        validation
    }

    fn encode_claims<T: Serialize>(&self, claims: &T) -> Result<String> {
        let (alg, kid, key) = self.active_signing_key();
        let header = Header {
            alg,
            kid: Some(kid.to_string()),
            ..Header::default()
        };

        encode(&header, claims, key)
            .map_err(|e| AuthError::Internal(format!("Failed to sign token: {e}")))
    }

    /// Public base64url-encoded Ed25519 key (x coordinate for the JWKS).
    pub fn public_key_b64(&self) -> String {
        URL_SAFE_NO_PAD.encode(self.signing_key.verifying_key().to_bytes())
    }

    /// OIDC discovery document (`/.well-known/openid-configuration`).
    pub fn discovery_document(&self) -> Value {
        json!({
            "issuer": self.issuer,
            "authorization_endpoint": format!("{}/oidc/authorize", self.issuer),
            "token_endpoint": format!("{}/oidc/token", self.issuer),
            "userinfo_endpoint": format!("{}/oidc/userinfo", self.issuer),
            "jwks_uri": format!("{}/.well-known/jwks.json", self.issuer),
            "response_types_supported": ["code"],
            "response_modes_supported": ["query"],
            "subject_types_supported": ["public"],
            // Only what this deployment actually signs with.
            "id_token_signing_alg_values_supported": [self.signing_alg_name()],
            "token_endpoint_auth_methods_supported": [
                "client_secret_post",
                "client_secret_basic",
                "none"
            ],
            "code_challenge_methods_supported": ["S256"],
            "scopes_supported": ["openid", "profile", "email"],
            "claims_supported": [
                "sub", "iss", "aud", "exp", "iat", "azp", "nonce",
                "email", "email_verified", "name", "preferred_username", "groups"
            ],
            "grant_types_supported": ["authorization_code", "refresh_token"]
        })
    }

    /// JSON Web Key Set (`/.well-known/jwks.json`).
    pub fn jwks(&self) -> Value {
        let mut keys = vec![json!({
            "kty": "OKP",
            "crv": "Ed25519",
            "alg": "EdDSA",
            "use": "sig",
            "kid": self.kid,
            "x": self.public_key_b64()
        })];

        if let Some(rsa_public_key) = &self.rsa_public_key {
            let n = URL_SAFE_NO_PAD.encode(rsa_public_key.n().to_bytes_be());
            // Usually 65537, but read it from the key rather than assuming.
            let e = URL_SAFE_NO_PAD.encode(rsa_public_key.e().to_bytes_be());
            keys.push(json!({
                "kty": "RSA",
                "alg": "RS256",
                "use": "sig",
                "kid": self.rsa_kid,
                "n": n,
                "e": e
            }));
        }

        json!({ "keys": keys })
    }

    /// Mint an id_token for the authorization-code exchange.
    pub fn sign_id_token(
        &self,
        user_id: Uuid,
        client_id: &str,
        nonce: Option<String>,
        email: Option<String>,
        display_name: Option<String>,
        groups: &[String],
    ) -> Result<String> {
        let now = chrono::Utc::now().timestamp() as usize;
        let claims = OidcIdClaims {
            sub: user_id.to_string(),
            iss: self.issuer.clone(),
            aud: client_id.to_string(),
            exp: now + self.id_token_ttl.as_secs() as usize,
            iat: now,
            azp: client_id.to_string(),
            nonce,
            // No verification round-trip exists: an address present on the
            // account is asserted verified, an absent one is omitted entirely.
            email_verified: email.as_ref().map(|_| true),
            email,
            name: display_name.clone(),
            preferred_username: display_name,
            groups: groups.to_vec(),
        };

        self.encode_claims(&claims)
    }

    /// Mint an OIDC access token (validated by /oidc/userinfo and passed to
    /// backends when the RP requests `pass-access-token`).
    pub fn sign_access_token(
        &self,
        user_id: Uuid,
        client_id: &str,
        email: Option<String>,
        display_name: Option<String>,
    ) -> Result<String> {
        let now = chrono::Utc::now().timestamp() as usize;
        let claims = OidcAccessClaims {
            sub: user_id.to_string(),
            iss: self.issuer.clone(),
            aud: client_id.to_string(),
            exp: now + self.access_token_ttl.as_secs() as usize,
            iat: now,
            email_verified: email.as_ref().map(|_| true),
            email,
            name: display_name.clone(),
            preferred_username: display_name,
        };

        self.encode_claims(&claims)
    }

    /// Validate an access token while accepting any audience.
    ///
    /// /oidc/userinfo does not know the client up front; the caller checks
    /// the returned `aud` against the client registry.
    pub fn validate_access_token_any_aud(&self, token: &str) -> Result<OidcAccessClaims> {
        let (key, _) = self.active_decoding_key();
        let mut validation = self.validation();
        validation.validate_aud = false;

        let claims = decode::<OidcAccessClaims>(token, key, &validation)
            .map_err(|e| AuthError::Unauthorized(format!("Invalid OIDC access token: {e}")))?
            .claims;

        Ok(claims)
    }

    /// Access token lifetime in seconds (advertised as `expires_in`).
    pub fn access_token_ttl_secs(&self) -> u64 {
        self.access_token_ttl.as_secs()
    }

    /// Mint an opaque one-time authorization code.
    pub fn new_authorization_code(&self) -> String {
        let mut bytes = [0u8; 32];
        rand::rngs::OsRng.fill_bytes(&mut bytes);
        hex::encode(bytes)
    }

    /// Mint an opaque refresh token. Only its hash is persisted.
    pub fn new_refresh_token(&self) -> String {
        let mut bytes = [0u8; 32];
        rand::rngs::OsRng.fill_bytes(&mut bytes);
        hex::encode(bytes)
    }

    /// Verify a PKCE verifier against the stored challenge (S256 only).
    pub fn verify_pkce(
        &self,
        code_challenge: Option<&str>,
        code_challenge_method: Option<&str>,
        code_verifier: Option<&str>,
    ) -> bool {
        let (Some(challenge), Some(method), Some(verifier)) =
            (code_challenge, code_challenge_method, code_verifier)
        else {
            // No PKCE on either side — allowed for confidential clients
            // (oauth2-proxy always sends it, but other RPs may not).
            // Public clients are rejected earlier, where PKCE is mandatory.
            return code_challenge.is_none() && code_verifier.is_none();
        };

        if !method.eq_ignore_ascii_case("S256") {
            return false;
        }

        let digest = Sha256::digest(verifier.as_bytes());
        let computed = URL_SAFE_NO_PAD.encode(digest);
        // Constant-time comparison: the verifier is attacker-controlled.
        use subtle::ConstantTimeEq;
        computed.as_bytes().ct_eq(challenge.as_bytes()).into()
    }
}

/// Wrap DER bytes in a standard PEM frame (64-column base64).
fn pem_from_der(label: &str, der: &[u8]) -> String {
    let b64 = base64::engine::general_purpose::STANDARD.encode(der);
    let mut pem = format!("-----BEGIN {label}-----\n");
    for chunk in b64.as_bytes().chunks(64) {
        pem.push_str(std::str::from_utf8(chunk).unwrap_or_default());
        pem.push('\n');
    }
    pem.push_str(&format!("-----END {label}-----\n"));
    pem
}

#[cfg(test)]
mod tests {
    use super::*;
    use rsa::pkcs8::{EncodePrivateKey, LineEnding};
    use std::sync::OnceLock;

    /// Ed25519-only service (no RSA key configured).
    fn test_service() -> OidcService {
        let mut seed = [0u8; 32];
        rand::rngs::OsRng.fill_bytes(&mut seed);
        OidcService::new(
            "https://auth.lan".to_string(),
            seed,
            Duration::from_secs(3600),
            Duration::from_secs(600),
            None,
        )
        .expect("Ed25519 service")
    }

    /// PKCS#8 RSA private key generated in-process (never committed).
    /// Keygen costs ~4s in the debug profile, so it runs once per test binary.
    fn test_rsa_pem() -> &'static str {
        static PEM: OnceLock<String> = OnceLock::new();
        PEM.get_or_init(|| {
            let key = RsaPrivateKey::new(&mut rand::thread_rng(), 2048).expect("RSA keygen");
            key.to_pkcs8_pem(LineEnding::LF)
                .expect("PKCS#8 PEM encode")
                .to_string()
        })
    }

    /// RS256 service built from a freshly generated RSA key.
    fn rsa_service() -> OidcService {
        static SVC: OnceLock<OidcService> = OnceLock::new();
        SVC.get_or_init(|| {
            let mut seed = [0u8; 32];
            rand::rngs::OsRng.fill_bytes(&mut seed);
            OidcService::new(
                "https://auth.lan".to_string(),
                seed,
                Duration::from_secs(3600),
                Duration::from_secs(600),
                Some(test_rsa_pem().to_string()),
            )
            .expect("RSA service")
        })
        .clone()
    }

    /// Build a verification key from the JWKS entry with this kid — the same
    /// path a relying party takes, independent of the in-process key material.
    fn jwks_decoding_key(jwks: &Value, kid: &str) -> DecodingKey {
        let entry = jwks["keys"]
            .as_array()
            .expect("keys array")
            .iter()
            .find(|k| k["kid"] == kid && k["kty"] == "RSA")
            .expect("RSA JWKS entry with matching kid");
        DecodingKey::from_rsa_components(
            entry["n"].as_str().expect("n"),
            entry["e"].as_str().expect("e"),
        )
        .expect("decoding key from JWKS components")
    }

    #[test]
    fn discovery_document_is_complete() {
        let svc = test_service();
        let doc = svc.discovery_document();
        assert_eq!(doc["issuer"], "https://auth.lan");
        assert_eq!(
            doc["authorization_endpoint"],
            "https://auth.lan/oidc/authorize"
        );
        assert_eq!(doc["token_endpoint"], "https://auth.lan/oidc/token");
        assert_eq!(doc["userinfo_endpoint"], "https://auth.lan/oidc/userinfo");
        assert_eq!(doc["jwks_uri"], "https://auth.lan/.well-known/jwks.json");
        assert!(doc["code_challenge_methods_supported"]
            .as_array()
            .expect("array")
            .contains(&json!("S256")));
        assert!(doc["response_types_supported"]
            .as_array()
            .expect("array")
            .contains(&json!("code")));
        assert!(doc["claims_supported"]
            .as_array()
            .expect("array")
            .contains(&json!("groups")));
        assert!(doc["grant_types_supported"]
            .as_array()
            .expect("array")
            .contains(&json!("refresh_token")));
    }

    #[test]
    fn jwks_exposes_ed25519_key() {
        let svc = test_service();
        let jwks = svc.jwks();
        let keys = jwks["keys"].as_array().expect("keys array");
        assert_eq!(keys.len(), 1, "Ed25519-only config exposes one key");
        let key = &keys[0];
        assert_eq!(key["kty"], "OKP");
        assert_eq!(key["crv"], "Ed25519");
        assert_eq!(key["alg"], "EdDSA");
        assert_eq!(key["kid"], svc.kid);
        // 32-byte base64url public key.
        let x = URL_SAFE_NO_PAD
            .decode(key["x"].as_str().expect("x"))
            .expect("base64url x");
        assert_eq!(x.len(), 32);
    }

    #[test]
    fn ed25519_only_service_signs_eddsa() {
        let svc = test_service();
        assert_eq!(svc.signing_alg_name(), "EdDSA");
        let doc = svc.discovery_document();
        assert_eq!(
            doc["id_token_signing_alg_values_supported"],
            json!(["EdDSA"])
        );

        let token = svc
            .sign_id_token(
                Uuid::new_v4(),
                "astral-key-oidc",
                None,
                Some("j@example.com".to_string()),
                Some("Jane Doe".to_string()),
                &[],
            )
            .expect("id_token");
        let header = jsonwebtoken::decode_header(&token).expect("header");
        assert_eq!(header.alg, Algorithm::EdDSA);
        assert_eq!(header.kid.as_deref(), Some(svc.kid.as_str()));
    }

    #[test]
    fn jwks_rsa_entry_shape() {
        let svc = rsa_service();
        let jwks = svc.jwks();
        let keys = jwks["keys"].as_array().expect("keys array");
        assert_eq!(keys.len(), 2, "RSA config adds a second key");

        let rsa_entry = keys
            .iter()
            .find(|k| k["kty"] == "RSA")
            .expect("RSA entry present");
        let kid = svc.rsa_kid.as_deref().expect("rsa kid");
        assert_eq!(rsa_entry["alg"], "RS256");
        assert_eq!(rsa_entry["use"], "sig");
        assert_eq!(rsa_entry["kid"], kid);

        // n and e must be base64url and decode to sane lengths.
        let n = URL_SAFE_NO_PAD
            .decode(rsa_entry["n"].as_str().expect("n"))
            .expect("base64url n");
        let e = URL_SAFE_NO_PAD
            .decode(rsa_entry["e"].as_str().expect("e"))
            .expect("base64url e");
        assert_eq!(n.len(), 256, "2048-bit modulus");
        assert_eq!(e, vec![0x01, 0x00, 0x01], "exponent 65537");
    }

    #[test]
    fn rsa_service_advertises_rs256_only() {
        let svc = rsa_service();
        assert_eq!(svc.signing_alg_name(), "RS256");
        assert_eq!(
            svc.discovery_document()["id_token_signing_alg_values_supported"],
            json!(["RS256"])
        );
    }

    #[test]
    fn rsa_id_token_verifies_against_jwks_entry() {
        let svc = rsa_service();
        let kid = svc.rsa_kid.clone().expect("rsa kid");
        let user_id = Uuid::new_v4();

        let token = svc
            .sign_id_token(
                user_id,
                "astral-key-oidc",
                Some("nonce-123".to_string()),
                Some("j@example.com".to_string()),
                Some("Jane Doe".to_string()),
                &["admins".to_string(), "devs".to_string()],
            )
            .expect("id_token");

        // JOSE header must name RS256 + the RSA kid.
        let header = jsonwebtoken::decode_header(&token).expect("header");
        assert_eq!(header.alg, Algorithm::RS256);
        assert_eq!(header.kid.as_deref(), Some(kid.as_str()));

        // Independent verification: key material parsed from the JWKS output.
        let key = jwks_decoding_key(&svc.jwks(), &kid);
        let mut validation = Validation::new(Algorithm::RS256);
        validation.set_audience(&["astral-key-oidc"]);
        validation.set_issuer(&["https://auth.lan"]);
        let claims = decode::<OidcIdClaims>(&token, &key, &validation)
            .expect("RS256 verification against JWKS")
            .claims;

        assert_eq!(claims.sub, user_id.to_string());
        assert_eq!(claims.aud, "astral-key-oidc");
        assert_eq!(claims.nonce.as_deref(), Some("nonce-123"));
        assert_eq!(claims.email.as_deref(), Some("j@example.com"));
        assert_eq!(claims.email_verified, Some(true));
        assert_eq!(claims.groups, vec!["admins", "devs"]);
    }

    #[test]
    fn rsa_access_token_round_trips_and_rejects_wrong_audience() {
        let svc = rsa_service();
        let user_id = Uuid::new_v4();
        let token = svc
            .sign_access_token(
                user_id,
                "astral-key-oidc",
                Some("j@example.com".to_string()),
                None,
            )
            .expect("access token");

        let header = jsonwebtoken::decode_header(&token).expect("header");
        assert_eq!(header.alg, Algorithm::RS256);

        // Signature + issuer verify with the RSA decoding key.
        let claims = svc
            .validate_access_token_any_aud(&token)
            .expect("validates with the RSA decoding key");
        assert_eq!(claims.sub, user_id.to_string());
        assert_eq!(claims.aud, "astral-key-oidc");
        assert_eq!(claims.email_verified, Some(true));

        // An explicit audience check still rejects a foreign client.
        let (key, alg) = svc.active_decoding_key();
        let mut validation = Validation::new(alg);
        validation.set_audience(&["other-client"]);
        validation.set_issuer(&["https://auth.lan"]);
        assert!(decode::<OidcAccessClaims>(&token, key, &validation).is_err());
    }

    #[test]
    fn id_token_round_trips_with_pkcs8_key() {
        let svc = test_service();
        let user_id = Uuid::new_v4();
        let token = svc
            .sign_id_token(
                user_id,
                "astral-key-oidc",
                Some("nonce-123".to_string()),
                Some("j@example.com".to_string()),
                Some("Jane Doe".to_string()),
                &[],
            )
            .expect("id_token");

        let mut validation = Validation::new(Algorithm::EdDSA);
        validation.set_audience(&["astral-key-oidc"]);
        validation.set_issuer(&["https://auth.lan"]);
        let claims = decode::<OidcIdClaims>(&token, &svc.decoding_key, &validation)
            .expect("decode")
            .claims;
        assert_eq!(claims.sub, user_id.to_string());
        assert_eq!(claims.nonce.as_deref(), Some("nonce-123"));
        assert_eq!(claims.email.as_deref(), Some("j@example.com"));
        assert_eq!(claims.preferred_username.as_deref(), Some("Jane Doe"));
        assert_eq!(claims.azp, "astral-key-oidc");
        assert!(claims.groups.is_empty(), "no groups -> empty array");
    }

    #[test]
    fn id_token_omits_email_verified_without_email() {
        let svc = test_service();
        let token = svc
            .sign_id_token(Uuid::new_v4(), "astral-key-oidc", None, None, None, &[])
            .expect("id_token");

        // Inspect the payload: email/email_verified must be absent, groups present.
        let payload = token.split('.').nth(1).expect("payload segment");
        let raw = URL_SAFE_NO_PAD.decode(payload).expect("base64url payload");
        let value: Value = serde_json::from_slice(&raw).expect("payload json");

        assert!(value.get("email").is_none());
        assert!(value.get("email_verified").is_none());
        assert_eq!(value["groups"], json!([]));
    }

    #[test]
    fn access_token_round_trips_and_userinfo_claims() {
        let svc = test_service();
        let user_id = Uuid::new_v4();
        let token = svc
            .sign_access_token(
                user_id,
                "astral-key-oidc",
                Some("j@example.com".to_string()),
                None,
            )
            .expect("access token");
        let claims = svc.validate_access_token_any_aud(&token).expect("decode");
        assert_eq!(claims.sub, user_id.to_string());
        assert_eq!(claims.aud, "astral-key-oidc");
        assert_eq!(claims.email.as_deref(), Some("j@example.com"));
        assert_eq!(claims.email_verified, Some(true));

        // Audience is enforced when the caller names one.
        let (key, alg) = svc.active_decoding_key();
        let mut validation = Validation::new(alg);
        validation.set_audience(&["other-client"]);
        validation.set_issuer(&["https://auth.lan"]);
        assert!(decode::<OidcAccessClaims>(&token, key, &validation).is_err());
    }

    #[test]
    fn pkce_verification() {
        let svc = test_service();
        let verifier = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
        let digest = Sha256::digest(verifier.as_bytes());
        let challenge = URL_SAFE_NO_PAD.encode(digest);

        assert!(svc.verify_pkce(Some(&challenge), Some("S256"), Some(verifier)));
        assert!(!svc.verify_pkce(Some(&challenge), Some("S256"), Some("wrong-verifier")));
        assert!(!svc.verify_pkce(Some(&challenge), Some("plain"), Some(verifier)));
        assert!(svc.verify_pkce(None, None, None));
        // Challenge without verifier, or verifier without challenge, fails.
        assert!(!svc.verify_pkce(Some(&challenge), Some("S256"), None));
        assert!(!svc.verify_pkce(None, None, Some(verifier)));
    }

    #[test]
    fn authorization_codes_are_unique() {
        let svc = test_service();
        let a = svc.new_authorization_code();
        let b = svc.new_authorization_code();
        assert_eq!(a.len(), 64);
        assert_ne!(a, b);
    }

    #[test]
    fn refresh_tokens_are_unique_and_hash_to_64_hex() {
        let svc = test_service();
        let a = svc.new_refresh_token();
        let b = svc.new_refresh_token();
        assert_eq!(a.len(), 64);
        assert_ne!(a, b);

        let hash = hash_refresh_token(&a);
        assert_eq!(hash.len(), 64);
        assert_ne!(hash, a, "the stored value must not be the token");
        assert_eq!(hash, hash_refresh_token(&a), "hashing is deterministic");
    }

    #[test]
    fn client_secret_is_hashed_and_compared_constant_time() {
        let client = OidcClient::confidential(
            "astral-key-oidc".to_string(),
            "super-secret",
            vec!["https://auth.lan/oauth2/callback".to_string()],
            None,
        );

        // SHA-256 hex, never the plaintext.
        let expected = hash_client_secret("super-secret");
        assert_eq!(
            client.client_secret_hash.as_deref(),
            Some(expected.as_str())
        );
        assert_eq!(expected.len(), 64);
        assert_ne!(client.client_secret_hash.as_deref(), Some("super-secret"));

        assert!(client.verify_secret("super-secret"));
        assert!(!client.verify_secret("wrong"));
        assert!(!client.verify_secret(""));
        assert!(!client.is_public());
    }

    #[test]
    fn public_client_never_verifies_a_secret() {
        let client = OidcClient::public(
            "public-spa".to_string(),
            vec!["https://spa.example.com/cb".to_string()],
        );
        assert!(client.is_public());
        assert!(!client.verify_secret(""));
        assert!(!client.verify_secret("anything"));
    }

    #[test]
    fn redirect_uri_matching_is_exact() {
        let client = OidcClient::confidential(
            "astral-key-oidc".to_string(),
            "s3cret",
            vec!["https://auth.lan/oauth2/callback".to_string()],
            None,
        );

        assert!(client.allows_redirect_uri("https://auth.lan/oauth2/callback"));
        // No prefixes, no trailing-slash tolerance, no host suffix matching.
        assert!(!client.allows_redirect_uri("https://auth.lan/oauth2/callback/extra"));
        assert!(!client.allows_redirect_uri("https://auth.lan/oauth2/callback?x=1"));
        assert!(!client.allows_redirect_uri("https://auth.lan.evil.com/oauth2/callback"));
        assert!(!client.allows_redirect_uri("http://auth.lan/oauth2/callback"));
        assert!(!client.allows_redirect_uri("HTTPS://auth.lan/oauth2/callback"));
    }

    #[test]
    fn allowed_scopes_are_enforced_when_set() {
        let restricted = OidcClient {
            client_id: "restricted".to_string(),
            client_secret_hash: Some(hash_client_secret("s3cret")),
            redirect_uris: vec!["https://rp.example.com/cb".to_string()],
            allowed_scopes: Some(vec!["openid".to_string(), "email".to_string()]),
        };

        assert!(restricted.allows_scope("openid"));
        assert!(restricted.allows_scope("email"));
        assert!(!restricted.allows_scope("profile"));

        let unrestricted = OidcClient::public("any".to_string(), vec![]);
        assert!(unrestricted.allows_scope("anything"));
    }
}
