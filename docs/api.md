# API Reference

Base URL: `http://localhost:8080`

All authentication endpoints are prefixed with `/api/v1`.

- [Health & Readiness](#health--readiness)
- [Web3 / SIWE Authentication](#web3--siwe-authentication)
- [FIDO2 / WebAuthn](#fido2--webauthn)
- [JWT Token Verification](#jwt-token-verification)
- [Token Refresh & Session Management](#token-refresh--session-management)
- [API Key Management](#api-key-management)
- [ZK JIT Capability Tokens](#zk-jit-capability-tokens)
- [Ed25519 Identity](#ed25519-identity)
- [Contacts](#contacts)
- [QR Codes](#qr-codes)
- [OAuth (GitHub)](#oauth-github)
- [Error Responses](#error-responses)

---

## Health & Readiness

### `GET /health`

Returns service liveness — always `200 OK` if the process is running.

**Response `200` (plain text):**

```
OK
```

```bash
curl http://localhost:8080/health
```

**Note:** A structured JSON variant exists at `/health` under the handler module
but the top-level `/health` route returns plain text `"OK"`. Use this for
liveness probes.

---

### `GET /ready`

Returns readiness (checks database connectivity).

**Response `200`:**

```json
{
  "status": "ready",
  "checks": { "database": true }
}
```

**Response `503`** (database unavailable):

```json
{
  "status": "not_ready",
  "error": "database_unavailable"
}
```

```bash
curl http://localhost:8080/ready
```

---

## Web3 / SIWE Authentication

### `GET /api/v1/auth/web3/chains`

Returns the list of supported blockchain networks. **Method is GET.**

**Response `200`:**

```json
{
  "chains": [
    { "id": 1, "name": "ethereum", "display_name": "Ethereum", "type": "mainnet" },
    { "id": 137, "name": "polygon", "display_name": "Polygon", "type": "mainnet" },
    { "id": 42161, "name": "arbitrum", "display_name": "Arbitrum", "type": "mainnet" },
    { "id": 10, "name": "optimism", "display_name": "Optimism", "type": "mainnet" },
    { "id": 5, "name": "goerli", "display_name": "Goerli", "type": "testnet" },
    { "id": 11155111, "name": "sepolia", "display_name": "Sepolia", "type": "testnet" }
  ]
}
```

```bash
curl http://localhost:8080/api/v1/auth/web3/chains
```

---

### `POST /api/v1/auth/web3/nonce`

Request a cryptographic nonce for SIWE (Sign-In with Ethereum).

**Request:**

```json
{
  "domain": "maplespike.ca",
  "address": "0x742d35Cc6634C0532925a3b844Bc9e7595f2bD18",
  "chain_id": 1
}
```

All fields are optional. The `domain` is validated against the configured
`ASTRAL_WEB3_DOMAIN` — spoofed domains are rejected.

**Response `200`:**

```json
{
  "nonce": "a1b2c3d4e5f6...",
  "message_template": "maplespike.ca wants you to sign in with your Ethereum account:\n0x742d35Cc6634C0532925a3b844Bc9e7595f2bD18\n\nSign in to Astral Key\n\nURI: http://localhost:8080\nVersion: 1\nChain ID: 1\nNonce: a1b2c3d4e5f6...\nIssued At: 2026-07-16T00:00:00Z",
  "domain": "maplespike.ca",
  "chain_id": 1
}
```

```bash
curl -X POST http://localhost:8080/api/v1/auth/web3/nonce \
  -H "Content-Type: application/json" \
  -d '{"domain": "maplespike.ca", "address": "0x742d35Cc6634C0532925a3b844Bc9e7595f2bD18", "chain_id": 1}'
```

---

### `POST /api/v1/auth/web3/verify`

Verify an EIP-4361 SIWE signature. On success, creates or looks up the user
and wallet, then returns JWT tokens.

**Request:**

```json
{
  "message": "maplespike.ca wants you to sign in with your Ethereum account:\n0x742d35Cc6634C0532925a3b844Bc9e7595f2bD18\n\nSign in to Astral Key\n\nURI: http://localhost:8080\nVersion: 1\nChain ID: 1\nNonce: a1b2c3d4e5f6...\nIssued At: 2026-07-16T00:00:00Z",
  "signature": "0x...",
  "chain_id": 1
}
```

**Response `200`:**

```json
{
  "access_token": "eyJhbGciOiJIUzI1NiIs...",
  "refresh_token": "eyJhbGciOiJIUzI1NiIs...",
  "user": {
    "id": "550e8400-e29b-41d4-a716-446655440000",
    "address": "0x742d35cc6634c0532925a3b844bc9e7595f2bd18",
    "chain_id": 1
  }
}
```

**Error `401`:**

```json
{
  "code": "AUTH_UNAUTHORIZED",
  "detail": "Invalid or expired nonce",
  "docs_url": "https://github.com/reverb256/astral-key/docs/errors.md"
}
```

```bash
curl -X POST http://localhost:8080/api/v1/auth/web3/verify \
  -H "Content-Type: application/json" \
  -d '{"message": "...", "signature": "0x...", "chain_id": 1}'
```

---

## FIDO2 / WebAuthn

### `POST /api/v1/auth/fido2/register/options`

Start passkey registration. **Requires JWT authentication.**

**Request:**

```json
{
  "username": "user@example.com",
  "display_name": "User Name"
}
```

**Response `200`:**

```json
{
  "challenge": "base64url-encoded-challenge",
  "rp": { "name": "Astral Key", "id": "localhost" },
  "user": {
    "id": "base64url-user-id",
    "name": "user@example.com",
    "display_name": "User Name"
  },
  "pub_key_cred_params": [
    { "type": "public-key", "alg": -7 },
    { "type": "public-key", "alg": -257 }
  ],
  "timeout": 60000,
  "attestation": "none",
  "authenticator_selection": {
    "authenticator_attach": "platform",
    "user_verification": "preferred"
  }
}
```

```bash
curl -X POST http://localhost:8080/api/v1/auth/fido2/register/options \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer eyJ..." \
  -d '{"username": "user@example.com", "display_name": "User Name"}'
```

---

### `POST /api/v1/auth/fido2/register/verify`

Complete passkey registration. **Requires JWT authentication.**

**Request:**

```json
{
  "id": "base64url-credential-id",
  "raw_id": "base64url-raw-id",
  "type": "public-key",
  "response": { "...": "..." }
}
```

**Response `200`:**

```json
{
  "id": "550e8400-e29b-41d4-a716-446655440000",
  "status": "success"
}
```

---

### `POST /api/v1/auth/fido2/authenticate/options`

Start passkey authentication (public — no JWT required).

**Request:**

```json
{
  "username": "550e8400-e29b-41d4-a716-446655440000"
}
```

The `username` must be a valid UUID.

**Response `200`:**

```json
{
  "challenge": "base64url-encoded-challenge",
  "allow_credentials": [
    { "type": "public-key", "id": "base64url-credential-id" }
  ],
  "user_verification": "preferred",
  "timeout": 60000
}
```

---

### `POST /api/v1/auth/fido2/authenticate/verify`

Complete passkey authentication (public). Returns JWT tokens on success.

**Request:**

```json
{
  "id": "base64url-credential-id",
  "raw_id": "base64url-raw-id",
  "type": "public-key",
  "response": { "...": "..." }
}
```

**Response `200`:**

```json
{
  "access_token": "eyJhbGciOiJIUzI1NiIs...",
  "refresh_token": "eyJhbGciOiJIUzI1NiIs..."
}
```

---

### `GET /api/v1/auth/fido2/credentials`

List the authenticated user's registered passkeys. **Requires JWT.**

**Response `200`:**

```json
[
  {
    "id": "550e8400-e29b-41d4-a716-446655440000",
    "name": "Unnamed Credential",
    "created_at": "2026-07-16T00:00:00Z",
    "last_used_at": "2026-07-16T01:00:00Z"
  }
]
```

`last_used_at` is `null` if the credential has never been used for
authentication since the counter was last recorded.

---

### `DELETE /api/v1/auth/fido2/credentials/:id`

Delete a passkey credential. **Requires JWT.** Only the credential owner can
delete it.

**Response `200`:**

```json
{
  "message": "Credential deleted successfully"
}
```

**Error `403`** (not the owner):

```json
{
  "code": "AUTH_FORBIDDEN",
  "detail": "Not your credential",
  "docs_url": "https://github.com/reverb256/astral-key/docs/errors.md"
}
```

---

## JWT Token Verification

### `POST /api/v1/auth/verify`

Validate a JWT token. Used by external services to verify session validity.

**Request:**

```json
{
  "token": "eyJhbGciOiJIUzI1NiIs..."
}
```

**Response `200`** (valid):

```json
{
  "valid": true,
  "sub": "550e8400-e29b-41d4-a716-446655440000",
  "exp": 1768468800
}
```

**Response `200`** (invalid — note this always returns 200 with the error
in the body; it does not return 401):

```json
{
  "valid": false,
  "error": "JWT token expired"
}
```

```bash
curl -X POST http://localhost:8080/api/v1/auth/verify \
  -H "Content-Type: application/json" \
  -d '{"token": "eyJ..."}'
```

---

## Token Refresh & Session Management

### `POST /api/v1/auth/token/refresh`

Exchange a refresh token for a new token pair. Implements refresh token
rotation — each refresh invalidates the previous token.

**Request:**

```json
{
  "refresh_token": "eyJ...",
  "device_info": "",
  "ip_address": ""
}
```

`device_info` and `ip_address` are reserved for future use.

**Response `200`:**

```json
{
  "access_token": "eyJ...",
  "refresh_token": "eyJ...",
  "session_id": "550e8400-e29b-41d4-a716-446655440000"
}
```

```bash
curl -X POST http://localhost:8080/api/v1/auth/token/refresh \
  -H "Content-Type: application/json" \
  -d '{"refresh_token": "eyJ..."}'
```

---

### `GET /api/v1/auth/sessions`

List active sessions for the authenticated user. **Requires JWT.**

**Response `200`:**

```json
{
  "sessions": [
    {
      "id": "550e8400-e29b-41d4-a716-446655440000",
      "user_id": "550e8400-e29b-41d4-a716-446655440001",
      "created_at": "2026-07-16T00:00:00Z",
      "expires_at": "2026-07-23T00:00:00Z",
      "revoked": false
    }
  ]
}
```

```bash
curl http://localhost:8080/api/v1/auth/sessions \
  -H "Authorization: Bearer eyJ..."
```

---

### `DELETE /api/v1/auth/sessions/:id`

Revoke a session. **Requires JWT.** Only the session owner can revoke.

**Response `200`:**

```json
{
  "revoked": true
}
```

```bash
curl -X DELETE http://localhost:8080/api/v1/auth/sessions/550e8400-e29b-41d4-a716-446655440000 \
  -H "Authorization: Bearer eyJ..."
```

---

## API Key Management

### `POST /api/v1/auth/keys`

Create a new API key. **Requires JWT.** The plaintext key is returned exactly
once and is not stored.

**Request:**

```json
{
  "name": "My Key",
  "scopes": ["dns:read", "pages:deploy"],
  "environment": "prod",
  "expires_in_seconds": 2592000
}
```

- `environment` defaults to `"prod"`.
- `scopes` defaults to `[]`.
- `expires_in_seconds` is optional; omit for no expiration.

**Response `200`:**

```json
{
  "id": "550e8400-e29b-41d4-a716-446655440000",
  "api_key": "ak_prod_AbCdEfGhIjKlMnOpQrStUvWxYz...",
  "key_prefix": "ak_prod_",
  "name": "My Key",
  "scopes": "dns:read,pages:deploy",
  "environment": "prod"
}
```

**Important:** Save the plaintext `api_key` — it will not be shown again.

```bash
curl -X POST http://localhost:8080/api/v1/auth/keys \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer eyJ..." \
  -d '{"name": "My Key", "scopes": ["dns:read", "pages:deploy"], "environment": "prod"}'
```

---

### `GET /api/v1/auth/keys`

List all API keys for the authenticated user. **Requires JWT.**
Returns summaries only — no plaintext keys, no hashes.

**Response `200`:**

```json
{
  "keys": [
    {
      "id": "550e8400-e29b-41d4-a716-446655440000",
      "key_prefix": "ak_prod_",
      "name": "My Key",
      "scopes": "dns:read,pages:deploy",
      "environment": "prod",
      "created_at": "2026-07-16T00:00:00Z",
      "expires_at": "2026-08-15T00:00:00Z",
      "revoked_at": null
    }
  ]
}
```

---

### `DELETE /api/v1/auth/keys/:id`

Hard-delete an API key. **Requires JWT.**

**Response `200`:**

```json
{
  "deleted": true
}
```

---

### `POST /api/v1/auth/keys/:id/revoke`

Revoke an API key (soft delete — sets `revoked_at`). **Requires JWT.**

**Response `200`:**

```json
{
  "revoked": true
}
```

---

## ZK JIT Capability Tokens

These endpoints require `JIT_ISSUER_KEY` to be configured. If not set, they
return 500 with "JIT issuer not configured".

### `POST /api/v1/auth/jit/mint`

Mint a new capability token. **Requires JWT.**

**Request:**

```json
{
  "scopes": ["dns:read", "pages:deploy"],
  "audience": "my-service",
  "ttl_seconds": 3600
}
```

`ttl_seconds` defaults to the server's configured default (typically 3600).

**Response `200`:**

```json
{
  "token": "base64(header).base64(payload).base64(signature)",
  "expires_at": 1768468800,
  "token_id": "550e8400-e29b-41d4-a716-446655440000"
}
```

```bash
curl -X POST http://localhost:8080/api/v1/auth/jit/mint \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer eyJ..." \
  -d '{"scopes": ["dns:read"], "audience": "my-service", "ttl_seconds": 3600}'
```

---

### `POST /api/v1/auth/jit/verify`

Verify a capability token. **Public** — no JWT required, so delegated services
can validate tokens without authenticating first.

**Request:**

```json
{
  "token": "base64(header).base64(payload).base64(signature)"
}
```

**Response `200`:**

```json
{
  "subject": "ak:issuer:01",
  "issuer": "ak:issuer:01",
  "audience": "my-service",
  "scopes": ["dns:read"],
  "issued_at": 1768465200,
  "expires_at": 1768468800,
  "epoch": 1
}
```

```bash
curl -X POST http://localhost:8080/api/v1/auth/jit/verify \
  -H "Content-Type: application/json" \
  -d '{"token": "base64.header.payload.signature"}'
```

---

## Ed25519 Identity

All identity endpoints except `/verify` and `/qr/:pubkey` require JWT
authentication. Public keys are stored as Base64URL-encoded 32-byte Ed25519
public keys. Private keys are never sent to the server.

### `POST /api/v1/identity`

Create a new Ed25519 identity record. **Requires JWT.**

**Request:**

```json
{
  "pubkey": "base64url-encoded-32-byte-ed25519-public-key",
  "label": "My Laptop"
}
```

`label` is optional.

**Response `200`:**

```json
{
  "id": "550e8400-e29b-41d4-a716-446655440000",
  "pubkey": "base64url...",
  "label": "My Laptop",
  "is_current": false,
  "created_at": "2026-07-16T00:00:00Z"
}
```

---

### `GET /api/v1/identity`

List all identities for the authenticated user. **Requires JWT.**

---

### `GET /api/v1/identity/current`

Get the current active identity. **Requires JWT.**
Returns 404 if no identity is set as current.

---

### `POST /api/v1/identity/:id/set-current`

Set an identity as the active one. **Requires JWT.**

**Response `200`:**

```json
{
  "message": "Current identity updated"
}
```

---

### `DELETE /api/v1/identity/:id`

Delete an identity. **Requires JWT.** Only the owner can delete.

**Response `200`:**

```json
{
  "message": "Identity deleted"
}
```

---

### `POST /api/v1/identity/verify`

Verify an Ed25519 signature over canonical JSON. **Public** — no JWT required.

The client signs the canonical JSON string (`serde_json::to_string` output)
locally and sends the data, signature, and public key for verification.
Key order and whitespace must match exactly.

**Request:**

```json
{
  "data": {"msg": "hello"},
  "signature": "base64url-ed25519-signature",
  "pubkey": "base64url-ed25519-public-key"
}
```

**Response `200`:**

```json
{
  "valid": true
}
```

---

## Contacts

### `GET /api/v1/contacts`

List contacts for the authenticated user. **Requires JWT.**

**Response `200`:**

```json
[
  {
    "id": "550e8400-e29b-41d4-a716-446655440000",
    "pubkey": "base64url...",
    "label": "Alice",
    "discovered_via": "qr",
    "first_seen_at": "2026-07-16T00:00:00Z",
    "last_seen_at": "2026-07-16T01:00:00Z"
  }
]
```

---

### `POST /api/v1/contacts`

Add or update a contact by public key. **Requires JWT.**

**Request:**

```json
{
  "pubkey": "base64url...",
  "label": "Alice",
  "discovered_via": "manual"
}
```

`label` and `discovered_via` are optional.

---

### `POST /api/v1/contacts/scan`

Parse a QR scan result and save the discovered public key as a contact.
**Requires JWT.**

**Request:**

```json
{
  "qr": "mosaic://base64urlpubkey?fn=abcd1234",
  "label": "Alice"
}
```

Accepts `mosaic://`, `mosiac://`, `astral://identity/`, or raw Base64URL
public keys.

---

### `DELETE /api/v1/contacts/:pubkey`

Delete a contact by public key. **Requires JWT.**

**Response `200`:**

```json
{
  "message": "Contact deleted"
}
```

---

## QR Codes

### `GET /api/v1/identity/qr/:pubkey`

Generate a QR code for sharing a public key. **Public.** Supports SVG
(default) and PNG formats.

**Query parameters:**
- `format`: `"svg"` or `"png"` (default: `"svg"`)
- `width`: pixel dimension, 50–2000 (default: 300)

**Response `200`:**

```json
{
  "pubkey": "base64url...",
  "fingerprint": "abcd1234",
  "format": "svg",
  "data": "<svg>...</svg>"
}
```

For PNG format, `data` is a `data:image/png;base64,...` URI.

```bash
curl "http://localhost:8080/api/v1/identity/qr/base64urlpubkey?format=svg&width=300"
```

---

## OAuth (GitHub)

OAuth requires `OAUTH_GITHUB_CLIENT_ID` and `OAUTH_GITHUB_CLIENT_SECRET` to
be configured. **Note:** The OAuth handlers exist but are not yet wired into
the route tree. These endpoints are currently unreachable.

---

## Error Responses

All API errors (except `/health` and `/auth/verify`) follow a uniform JSON
envelope:

```json
{
  "code": "AUTH_UNAUTHORIZED",
  "detail": "Human-readable description of the error",
  "docs_url": "https://github.com/reverb256/astral-key/docs/errors.md"
}
```

| HTTP Code | Meaning | Machine Code |
|-----------|---------|-------------|
| 400 | Bad request / validation error | `AUTH_BAD_REQUEST` |
| 401 | Unauthorized (missing or invalid JWT) | `AUTH_UNAUTHORIZED` |
| 403 | Forbidden (wrong ownership) | `AUTH_FORBIDDEN` |
| 404 | Resource not found | `AUTH_NOT_FOUND` |
| 429 | Rate limited | — |
| 500 | Internal server error | `AUTH_INTERNAL_ERROR` |
| 501 | Not implemented | `AUTH_NOT_IMPLEMENTED` |

See [`docs/errors.md`](errors.md) for the full error code reference.
