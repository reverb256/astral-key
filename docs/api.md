# API Reference

Base URL: `http://localhost:8080`

All authentication endpoints are prefixed with `/api/v1`.

- [Health & Readiness](#health--readiness)
- [Web3 / SIWE Authentication](#web3--siwe-authentication)
- [FIDO2 / WebAuthn](#fido2--webauthn)
- [Token Verification](#token-verification)
- [Error Responses](#error-responses)

---

## Health & Readiness

### `GET /health`

Returns service liveness status.

**Response `200`:**

```json
{
  "status": "healthy",
  "version": "0.1.0"
}
```

```bash
curl http://localhost:8080/health
```

---

### `GET /ready`

Returns service readiness status (checks database connectivity).

**Response `200`:**

```json
{
  "status": "ready",
  "checks": {
    "database": true
  }
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

### `POST /api/v1/auth/web3/chains`

Returns the list of supported blockchain networks.

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

Request a cryptographic nonce for SIWE (Sign-In with Ethereum). The returned
`message_template` is a partially filled EIP-4361 message; the client should
complete it and have the user sign it in their wallet.

**Request:**

```json
{
  "domain": "maplespike.ca",
  "address": "0x742d35Cc6634C0532925a3b844Bc9e7595f2bD18",
  "chain_id": 1
}
```

All fields are optional.

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
  "error": {
    "code": 401,
    "message": "Invalid or expired nonce"
  }
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
  "rp": {
    "name": "Astral Key",
    "id": "localhost"
  },
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

```bash
curl -X POST http://localhost:8080/api/v1/auth/fido2/register/verify \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer eyJ..." \
  -d '{"id": "...", "raw_id": "...", "type": "public-key", "response": {...}}'
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

```bash
curl -X POST http://localhost:8080/api/v1/auth/fido2/authenticate/options \
  -H "Content-Type: application/json" \
  -d '{"username": "550e8400-e29b-41d4-a716-446655440000"}'
```

---

### `POST /api/v1/auth/fido2/authenticate/verify`

Complete passkey authentication (public — no JWT required). Returns JWT tokens
on success.

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

```bash
curl -X POST http://localhost:8080/api/v1/auth/fido2/authenticate/verify \
  -H "Content-Type: application/json" \
  -d '{"id": "...", "raw_id": "...", "type": "public-key", "response": {...}}'
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

```bash
curl http://localhost:8080/api/v1/auth/fido2/credentials \
  -H "Authorization: Bearer eyJ..."
```

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
  "error": {
    "code": 403,
    "message": "Not your credential"
  }
}
```

```bash
curl -X DELETE http://localhost:8080/api/v1/auth/fido2/credentials/550e8400-e29b-41d4-a716-446655440000 \
  -H "Authorization: Bearer eyJ..."
```

---

## Token Verification

### `POST /api/v1/auth/verify`

Validate a JWT token. Used by external services (e.g., Quill MCP) to verify
session validity without full authentication logic.

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

**Response `200`** (invalid):

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

## Error Responses

All errors follow a uniform JSON envelope:

```json
{
  "error": {
    "code": 400,
    "message": "Description of the error"
  }
}
```

| HTTP Code | Meaning                 |
|-----------|-------------------------|
| 400       | Bad request / validation error |
| 401       | Unauthorized (missing or invalid JWT) |
| 403       | Forbidden (wrong ownership) |
| 404       | Resource not found       |
| 429       | Rate limited             |
| 500       | Internal server error    |
| 501       | Not implemented          |

See [`docs/errors.md`](errors.md) for the full error code reference.
