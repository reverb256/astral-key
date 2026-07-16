# Error Codes

All API errors follow a uniform JSON envelope:

```json
{
  "error": {
    "code": 400,
    "message": "Human-readable description of the error"
  }
}
```

## HTTP Status Codes

| Code | Meaning | When It Happens |
|------|---------|-----------------|
| `400` | Bad Request | Invalid JSON payload, missing required fields, nonce not found in SIWE message, FIDO2 response parsing failure |
| `401` | Unauthorized | Missing `Authorization: Bearer` header, expired or malformed JWT token, invalid/expired Web3 nonce, FIDO2 credential not found |
| `403` | Forbidden | Attempting to delete a credential owned by another user |
| `404` | Not Found | User not found during FIDO2 authentication, credential not found by ID |
| `429` | Too Many Requests | Rate limit exceeded |
| `500` | Internal Server Error | Database error, JWT service initialization failure, unexpected server errors |
| `501` | Not Implemented | Requested feature has not been implemented yet |

## Error Messages by Endpoint

### Health

| Endpoint | Code | Message | Cause |
|----------|------|---------|-------|
| `GET /ready` | `503` | `database_unavailable` | SQLite connection pool health check failed |

### Web3 / SIWE

| Endpoint | Code | Message | Cause |
|----------|------|---------|-------|
| `POST /auth/web3/verify` | `400` | `Nonce not found in message` | SIWE message does not contain a `Nonce:` line |
| `POST /auth/web3/verify` | `401` | `Invalid or expired nonce` | Nonce was not found in the store or has expired (15 min TTL) |
| `POST /auth/web3/verify` | `500` | `JWT service not initialized` | Server started without a valid JWT_SECRET |

### FIDO2 / WebAuthn

| Endpoint | Code | Message | Cause |
|----------|------|---------|-------|
| `POST /auth/fido2/register/verify` | `400` | `Invalid registration response: ...` | Client response could not be deserialized |
| `POST /auth/fido2/authenticate/options` | `400` | `Username must be a valid user UUID` | The `username` field is not a valid UUID |
| `POST /auth/fido2/authenticate/options` | `404` | `User not found` | No user exists with the given UUID |
| `POST /auth/fido2/authenticate/verify` | `400` | `Invalid authentication response: ...` | Client response could not be deserialized |
| `POST /auth/fido2/authenticate/verify` | `401` | `Credential not found` | No credential matches the given `raw_id` |
| `DELETE /auth/fido2/credentials/:id` | `404` | `Credential not found` | Credential UUID does not exist |
| `DELETE /auth/fido2/credentials/:id` | `403` | `Not your credential` | Authenticated user does not own the credential |

### Token Verification

| Endpoint | Code | Message | Cause |
|----------|------|---------|-------|
| `POST /auth/verify` | `200` | `JWT service not initialized` | Returned as `{valid: false, error: ...}` — server has no JWT_SECRET |
| `POST /auth/verify` | `200` | `JWT token expired` | Returned as `{valid: false, error: ...}` |
| `POST /auth/verify` | `200` | `Invalid token` | Returned as `{valid: false, error: ...}` |

## Internal Errors

These are server-side errors that should never be exposed to end users. If
you see one in production, check the server logs.

| Message | Cause |
|---------|-------|
| `Internal database error` | SQLx query failure (connection lost, disk full, etc.) |
| `Failed to initialize JWT service: ...` | JWT secret is malformed or too short |
| `Database error: ...` | SQLx error propagated from model operations |
| `Migration failed: ...` | SQLx migration could not be applied |
