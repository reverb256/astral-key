# Astral Key — Error Codes

All API errors (except `/health` and `/auth/verify`) follow a uniform JSON
envelope:

```json
{
  "code": "AUTH_BAD_REQUEST",
  "detail": "Human-readable description of the error",
  "docs_url": "https://github.com/reverb256/astral-key/docs/errors.md"
}
```

- `code` is a **machine-readable string** (not an integer).
- `detail` is a human-readable description.
- `docs_url` is an optional link to this error reference.

---

## HTTP Status Codes

| Code | Meaning | Machine Code | When It Happens |
|------|---------|--------------|-----------------|
| `400` | Bad Request | `AUTH_BAD_REQUEST` | Invalid JSON payload, missing required fields, nonce not found in SIWE message, FIDO2 response parsing failure, invalid Ed25519 pubkey |
| `401` | Unauthorized | `AUTH_UNAUTHORIZED` | Missing `Authorization: Bearer` header, expired or malformed JWT token, invalid/expired Web3 nonce, FIDO2 credential not found, JIT token verification failed |
| `403` | Forbidden | `AUTH_FORBIDDEN` | Attempting to delete a credential owned by another user, revoke another user's session |
| `404` | Not Found | `AUTH_NOT_FOUND` | User not found during FIDO2 authentication, credential/session/identity not found by ID |
| `429` | Too Many Requests | (none) | Rate limit exceeded — includes `Retry-After` header |
| `500` | Internal Server Error | `AUTH_INTERNAL_ERROR` | Database error, JWT service initialization failure, JIT issuer not configured |
| `501` | Not Implemented | `AUTH_NOT_IMPLEMENTED` | Requested feature has not been implemented yet |

---

## Error Messages by Endpoint

### Health

| Endpoint | Code | Detail | Cause |
|----------|------|--------|-------|
| `GET /ready` | `503` | `database_unavailable` | SQLite connection pool health check failed |

### Web3 / SIWE

| Endpoint | Code | Detail | Cause |
|----------|------|--------|-------|
| `POST /auth/web3/verify` | `400` | `Nonce not found in message` | SIWE message does not contain a `Nonce:` line |
| `POST /auth/web3/verify` | `401` | `Invalid or expired nonce` | Nonce was not found in the store or has expired (15 min TTL) |
| `POST /auth/web3/verify` | `500` | `JWT service not initialized` | Server started without a valid JWT_SECRET |

### FIDO2 / WebAuthn

| Endpoint | Code | Detail | Cause |
|----------|------|--------|-------|
| `POST /auth/fido2/register/verify` | `400` | `Invalid registration response: ...` | Client response could not be deserialized |
| `POST /auth/fido2/authenticate/options` | `400` | `Username must be a valid user UUID` | The `username` field is not a valid UUID |
| `POST /auth/fido2/authenticate/options` | `404` | `User not found` | No user exists with the given UUID |
| `POST /auth/fido2/authenticate/verify` | `400` | `Invalid authentication response: ...` | Client response could not be deserialized |
| `POST /auth/fido2/authenticate/verify` | `401` | `Credential not found` | No credential matches the given `raw_id` |
| `DELETE /auth/fido2/credentials/:id` | `404` | `Credential not found` | Credential UUID does not exist |
| `DELETE /auth/fido2/credentials/:id` | `403` | `Not your credential` | Authenticated user does not own the credential |

### Token Verification

| Endpoint | Code | Detail | Cause |
|----------|------|--------|-------|
| `POST /auth/verify` | `200` | `JWT service not initialized` | Returned as `{valid: false, error: ...}` — server has no JWT_SECRET |
| `POST /auth/verify` | `200` | `JWT token expired` | Returned as `{valid: false, error: ...}` |
| `POST /auth/verify` | `200` | `Invalid token` | Returned as `{valid: false, error: ...}` |

### Session Management

| Endpoint | Code | Detail | Cause |
|----------|------|--------|-------|
| `POST /auth/token/refresh` | `401` | `Session not found or revoked` | Refresh token hash not found in the sessions table |
| `DELETE /auth/sessions/:id` | `403` | `Cannot revoke another user's session` | Authenticated user does not own the session |

### Ed25519 Identity

| Endpoint | Code | Detail | Cause |
|----------|------|--------|-------|
| `POST /identity` | `400` | `Invalid Ed25519 public key` | Pubkey is not valid Base64URL or wrong length |
| `POST /identity/verify` | `400` | `Invalid pubkey encoding` | Pubkey is not valid Base64URL |
| `GET /identity/qr/:pubkey` | `400` | `Invalid Ed25519 public key` | Pubkey is not valid Base64URL or wrong length |
| `GET /identity/qr/:pubkey` | `400` | `Unsupported format; use 'svg' or 'png'` | `format` query param is not `svg` or `png` |

### JIT Tokens

| Endpoint | Code | Detail | Cause |
|----------|------|--------|-------|
| `POST /auth/jit/mint` | `500` | `JIT issuer not configured` | JIT_ISSUER_KEY not set |
| `POST /auth/jit/verify` | `500` | `JIT verifier not configured` | JIT_ISSUER_KEY not set |
| `POST /auth/jit/verify` | `401` | `Token verification failed: ...` | Expired, revoked, unknown issuer, or malformed token |

---

## Internal Errors

These are server-side errors that should never be exposed to end users. If
you see one in production, check the server logs.

| Detail | Cause |
|--------|-------|
| `Internal database error` | SQLx query failure (connection lost, disk full, etc.) |
| `Failed to initialize JWT service: ...` | JWT secret is malformed or too short |
| `Database error: ...` | SQLx error propagated from model operations |
| `Migration failed: ...` | SQLx migration could not be applied |
