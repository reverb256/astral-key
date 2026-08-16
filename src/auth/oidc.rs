//! Astral Key — OIDC provider (identity-provider side)
//!
//! Turns Astral Key into an OpenID Connect **provider** so relying parties
//! such as oauth2-proxy can authenticate users with passkeys. Implements the
//! authorization-code flow:
//!
//! ```text
//!   RP ──authorize──▶ Astral Key (login page + WebAuthn) ──code──▶ RP
//!   RP ──token─────▶ Astral Key (id_token + access_token)
//!   RP ──userinfo──▶ Astral Key (claims from access token)
//! ```
//!
//! Tokens are signed with Ed25519 (EdDSA); the public key is exposed via a
//! JWKS endpoint so relying parties can verify id_tokens out-of-band.
//!
//! Only the authorization-code flow with PKCE (S256) is supported — no
//! implicit flow, no device flow. That covers oauth2-proxy and every other
//! well-behaved RP.

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use ed25519_dalek::pkcs8::{EncodePrivateKey, EncodePublicKey};
use ed25519_dalek::{SigningKey, VerifyingKey};
use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::time::Duration;
use uuid::Uuid;

use crate::config::OidcClientConfig;
use crate::error::{AuthError, Result};

/// A registered OIDC relying party.
#[derive(Debug, Clone)]
pub struct OidcClient {
    pub client_id: String,
    pub client_secret: String,
    pub redirect_uris: Vec<String>,
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
    #[serde(rename = "email_verified", skip_serializing_if = "Option::is_none")]
    pub email_verified: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(rename = "preferred_username", skip_serializing_if = "Option::is_none")]
    pub preferred_username: Option<String>,
}

/// Identity-provider service: key management + token minting/validation.
#[derive(Clone)]
pub struct OidcService {
    pub issuer: String,
    pub clients: Vec<OidcClient>,
    /// Stable key id — derived from the public key so it survives restarts.
    pub kid: String,
    signing_key: SigningKey,
    encoding_key: EncodingKey,
    decoding_key: DecodingKey,
    access_token_ttl: Duration,
    id_token_ttl: Duration,
}

impl OidcService {
    /// Build the service from config. The Ed25519 seed must be stable across
    /// restarts in production (see `OIDC_SIGNING_KEY`); callers that pass an
    /// ephemeral key are responsible for only using it in dev/test contexts.
    pub fn new(
        issuer: String,
        client_configs: Vec<OidcClientConfig>,
        seed: [u8; 32],
        access_token_ttl: Duration,
        id_token_ttl: Duration,
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

        Ok(Self {
            issuer,
            clients: client_configs
                .into_iter()
                .map(|c| OidcClient {
                    client_id: c.client_id,
                    client_secret: c.client_secret,
                    redirect_uris: c.redirect_uris,
                })
                .collect(),
            kid,
            signing_key,
            encoding_key,
            decoding_key,
            access_token_ttl,
            id_token_ttl,
        })
    }

    /// Look up a registered client by id.
    pub fn find_client(&self, client_id: &str) -> Option<&OidcClient> {
        self.clients.iter().find(|c| c.client_id == client_id)
    }

    /// Validate client credentials presented at the token endpoint.
    pub fn verify_client_secret(&self, client_id: &str, client_secret: &str) -> bool {
        self.find_client(client_id)
            .map(|c| {
                // Constant-time comparison to avoid leaking the secret.
                use subtle::ConstantTimeEq;
                let a = c.client_secret.as_bytes();
                let b = client_secret.as_bytes();
                a.ct_eq(b).into()
            })
            .unwrap_or(false)
    }

    /// Validate a registered redirect URI for the client.
    pub fn is_valid_redirect_uri(&self, client_id: &str, redirect_uri: &str) -> bool {
        self.find_client(client_id)
            .map(|c| c.redirect_uris.iter().any(|u| u == redirect_uri))
            .unwrap_or(false)
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
            "id_token_signing_alg_values_supported": ["EdDSA"],
            "token_endpoint_auth_methods_supported": [
                "client_secret_post",
                "client_secret_basic"
            ],
            "code_challenge_methods_supported": ["S256"],
            "scopes_supported": ["openid", "profile", "email"],
            "claims_supported": [
                "sub", "iss", "aud", "exp", "iat", "azp", "nonce",
                "email", "email_verified", "name", "preferred_username"
            ],
            "grant_types_supported": ["authorization_code"]
        })
    }

    /// JSON Web Key Set (`/.well-known/jwks.json`).
    pub fn jwks(&self) -> Value {
        json!({
            "keys": [{
                "kty": "OKP",
                "crv": "Ed25519",
                "alg": "EdDSA",
                "use": "sig",
                "kid": self.kid,
                "x": self.public_key_b64()
            }]
        })
    }

    /// Mint an id_token for the authorization-code exchange.
    pub fn sign_id_token(
        &self,
        user_id: Uuid,
        client_id: &str,
        nonce: Option<String>,
        email: Option<String>,
        display_name: Option<String>,
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
            email,
            email_verified: Some(true),
            name: display_name.clone(),
            preferred_username: display_name,
        };

        let header = Header {
            alg: Algorithm::EdDSA,
            kid: Some(self.kid.clone()),
            ..Header::default()
        };
        encode(&header, &claims, &self.encoding_key)
            .map_err(|e| AuthError::Internal(format!("Failed to sign id_token: {e}")))
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
            email,
            name: display_name.clone(),
            preferred_username: display_name,
        };

        let header = Header {
            alg: Algorithm::EdDSA,
            kid: Some(self.kid.clone()),
            ..Header::default()
        };
        encode(&header, &claims, &self.encoding_key)
            .map_err(|e| AuthError::Internal(format!("Failed to sign access token: {e}")))
    }

    /// Validate an OIDC access token (for /oidc/userinfo).
    pub fn validate_access_token(
        &self,
        token: &str,
        expected_aud: &str,
    ) -> Result<OidcAccessClaims> {
        let mut validation = Validation::new(Algorithm::EdDSA);
        validation.set_audience(&[expected_aud]);
        validation.set_issuer(&[&self.issuer]);

        let claims = decode::<OidcAccessClaims>(token, &self.decoding_key, &validation)
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
            // No PKCE on either side — allowed (oauth2-proxy always sends it,
            // but other RPs may not).
            return code_challenge.is_none() && code_verifier.is_none();
        };

        if !method.eq_ignore_ascii_case("S256") {
            return false;
        }

        let digest = Sha256::digest(verifier.as_bytes());
        let computed = URL_SAFE_NO_PAD.encode(digest);
        computed == challenge
    }
}

/// Wrap DER bytes in a standard PEM frame (64-column base64).
fn pem_from_der(label: &str, der: &[u8]) -> String {
    let b64 = base64::engine::general_purpose::STANDARD.encode(der);
    let mut pem = format!("-----BEGIN {label}-----\n");
    for chunk in b64.as_bytes().chunks(64) {
        pem.push_str(std::str::from_utf8(chunk).unwrap());
        pem.push('\n');
    }
    pem.push_str(&format!("-----END {label}-----\n"));
    pem
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_service() -> OidcService {
        let mut seed = [0u8; 32];
        rand::rngs::OsRng.fill_bytes(&mut seed);
        OidcService::new(
            "https://auth.lan".to_string(),
            vec![OidcClientConfig {
                client_id: "astral-key-oidc".to_string(),
                client_secret: "super-secret".to_string(),
                redirect_uris: vec!["https://auth.lan/oauth2/callback".to_string()],
            }],
            seed,
            Duration::from_secs(3600),
            Duration::from_secs(600),
        )
        .unwrap()
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
        assert_eq!(doc["jwks_uri"], "https://auth.lan/.well-known/jwks.json");
        assert!(doc["code_challenge_methods_supported"]
            .as_array()
            .unwrap()
            .contains(&json!("S256")));
        assert!(doc["response_types_supported"]
            .as_array()
            .unwrap()
            .contains(&json!("code")));
    }

    #[test]
    fn jwks_exposes_ed25519_key() {
        let svc = test_service();
        let jwks = svc.jwks();
        let key = &jwks["keys"][0];
        assert_eq!(key["kty"], "OKP");
        assert_eq!(key["crv"], "Ed25519");
        assert_eq!(key["alg"], "EdDSA");
        assert_eq!(key["kid"], svc.kid);
        // 32-byte base64url public key.
        let x = URL_SAFE_NO_PAD.decode(key["x"].as_str().unwrap()).unwrap();
        assert_eq!(x.len(), 32);
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
            )
            .unwrap();

        let mut validation = Validation::new(Algorithm::EdDSA);
        validation.set_audience(&["astral-key-oidc"]);
        validation.set_issuer(&["https://auth.lan"]);
        let claims = decode::<OidcIdClaims>(&token, &svc.decoding_key, &validation)
            .unwrap()
            .claims;
        assert_eq!(claims.sub, user_id.to_string());
        assert_eq!(claims.nonce.as_deref(), Some("nonce-123"));
        assert_eq!(claims.email.as_deref(), Some("j@example.com"));
        assert_eq!(claims.preferred_username.as_deref(), Some("Jane Doe"));
        assert_eq!(claims.azp, "astral-key-oidc");
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
            .unwrap();
        let claims = svc
            .validate_access_token(&token, "astral-key-oidc")
            .unwrap();
        assert_eq!(claims.sub, user_id.to_string());
        assert_eq!(claims.aud, "astral-key-oidc");
        assert_eq!(claims.email.as_deref(), Some("j@example.com"));
        // Wrong audience must fail.
        assert!(svc.validate_access_token(&token, "other-client").is_err());
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
    }

    #[test]
    fn client_secret_validation() {
        let svc = test_service();
        assert!(svc.verify_client_secret("astral-key-oidc", "super-secret"));
        assert!(!svc.verify_client_secret("astral-key-oidc", "wrong"));
        assert!(!svc.verify_client_secret("unknown-client", "super-secret"));
        assert!(svc.is_valid_redirect_uri("astral-key-oidc", "https://auth.lan/oauth2/callback"));
        assert!(!svc.is_valid_redirect_uri("astral-key-oidc", "https://evil.example.com/cb"));
    }

    #[test]
    fn authorization_codes_are_unique() {
        let svc = test_service();
        let a = svc.new_authorization_code();
        let b = svc.new_authorization_code();
        assert_eq!(a.len(), 64);
        assert_ne!(a, b);
    }
}
