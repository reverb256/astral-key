# Astral Key API Documentation

## Overview

Astral Key provides a RESTful API for Web3, FIDO2, and Passkey authentication.

**⚠️ API Status: Most endpoints are NOT YET IMPLEMENTED**

This documentation describes the **planned** API. Currently, most endpoints return "not implemented" errors. See [STATUS.md](../STATUS.md) for implementation progress.

## Base URL

```
https://api.astral-key.local/api/v1
```

### Current Working Endpoints

Only the following endpoints currently work:
- `GET /health` - Returns service health status ✅
- `GET /ready` - Returns readiness status (partial) ✅

All authentication endpoints are **not yet implemented**.

## Authentication

Most endpoints require a valid JWT token in the Authorization header:

```
Authorization: Bearer <access_token>
```

## Endpoints

### Health & Discovery

#### GET /health
Returns service health status.

**Response:**
```json
{
  "status": "healthy",
  "version": "0.1.0"
}
```

#### GET /ready
Returns service readiness status.

**Response:**
```json
{
  "status": "ready",
  "checks": {
    "database": true,
    "redis": true,
    "vaultwarden": true
  }
}
```

### Web3 Authentication

**⚠️ Status: NOT YET IMPLEMENTED**

All Web3 authentication endpoints currently return "not implemented" errors. Implementation is planned for Phase 4 (Week 6-7). See [ROADMAP.md](../ROADMAP.md).

#### POST /auth/web3/nonce

**Status:** Returns placeholder nonce (not yet stored in database)
Request a nonce for SIWE (Sign-In with Ethereum).

**Response:**
```json
{
  "nonce": "abc123def456",
  "message_template": "Sign in to Astral Key",
  "domain": "app.astral-key.local"
}
```

#### POST /auth/web3/verify

**Status:** Returns "not implemented" error
Verify Web3 signature and authenticate.

**Request:**
```json
{
  "message": "app.astral-key.local wants you to sign in...",
  "signature": "0x...",
  "chain_id": 1
}
```

**Response:**
```json
{
  "access_token": "eyJ...",
  "refresh_token": "eyJ...",
  "user": {
    "id": "uuid",
    "address": "0x...",
    "chain_id": 1
  }
}
```

### FIDO2/Passkey Authentication

**⚠️ Status: NOT YET IMPLEMENTED**

All FIDO2/Passkey authentication endpoints currently return "not implemented" errors. Implementation is planned for Phase 3 (Week 4-6) as the **PRIMARY FOCUS**. See [ROADMAP.md](../ROADMAP.md).

#### POST /auth/fido2/register/options

**Status:** Returns "not implemented" error
Get registration options for a new passkey.

**Response:**
```json
{
  "challenge": "base64_challenge",
  "rp": {
    "name": "Astral Key",
    "id": "app.astral-key.local"
  },
  "user": {
    "id": "base64_user_id",
    "name": "user@example.com",
    "display_name": "User Name"
  },
  "pub_key_cred_params": [
    { "type": "public-key", "alg": -7 }
  ]
}
```

#### POST /auth/fido2/register/verify
Verify and complete passkey registration.

#### POST /auth/fido2/authenticate/options
Get authentication options.

#### POST /auth/fido2/authenticate/verify
Verify passkey authentication.

## Error Responses

All errors follow this format:

```json
{
  "error": {
    "code": 400,
    "message": "Description of the error"
  }
}
```

### Common Error Codes

| Code | Description |
|------|-------------|
| 400 | Bad Request |
| 401 | Unauthorized |
| 403 | Forbidden |
| 404 | Not Found |
| 429 | Too Many Requests |
| 500 | Internal Server Error |

## Rate Limiting

API requests are rate-limited to 60 requests per minute per IP address.

## WebSocket API

Real-time authentication events are available via WebSocket:

```
wss://api.astral-key.local/v1/ws
```

See full documentation for event types and message formats.
