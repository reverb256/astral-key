# Architecture

This document describes Astral Key's architecture, module structure, and
authentication flows as they actually exist in the codebase.

## Overview

Astral Key is a **single-binary auth sidecar** written in Rust using the Axum
web framework. It stores all data in **SQLite** (no PostgreSQL, no Redis, no
Vaultwarden, no external cache). Authentication is provided via:

- **FIDO2 / WebAuthn** — passkeys (platform and roaming authenticators)
- **Web3 / SIWE** — Sign-In with Ethereum (EIP-4361)
- **JWT** — signed access and refresh tokens for session management
- **Ed25519** — identity keys and signature verification
- **ZK JIT capability tokens** — Ed25519-signed scoped tokens
- **API keys** — Argon2id-hashed, prefix-based (`ak_prod_...`)

The **Mosaic Identity Service (MIS)** crate (`crates/mosaic-identity/`) is a
separate Rust binary with its own 16-endpoint PKI API for Ed25519 key
management, cross-protocol binding, and ML-DSA-65 post-quantum hybrid signing.
It is deployed alongside the auth sidecar in production.

There are also 9 transport bridge crates (`crates/mosaic-bridge-*/`) that act
as sidecar daemons for atproto, buzz (nostr), matrix, irc, activitypub,
telegram, discord, and haven (Socket.IO).

## Module Layout (Auth Sidecar)

```
src/
├── main.rs              # Entry point — Tokio runtime, server bind, health/ready routes
├── lib.rs               # Re-exports modules
├── config.rs            # Env-var configuration (no config file)
├── error.rs             # AuthError enum → HTTP response mapping (string codes + detail)
├── state.rs             # AppState — shared pool, services, stores
├── api/
│   ├── routes.rs        # Route definitions (public / protected / rate-limit / audit)
│   └── handlers/
│       ├── health.rs    # Structured health/ready handlers
│       ├── web3.rs      # SIWE nonce, verify, chains (GET)
│       ├── fido2.rs     # WebAuthn register, authenticate, CRUD
│       ├── auth.rs      # Token verification (external services)
│       ├── session.rs   # Refresh token, list/revoke sessions
│       ├── keys.rs      # API key CRUD + revoke
│       ├── jit.rs       # JIT token mint + verify
│       ├── identity.rs  # Ed25519 identity, contacts, QR, signature verify
│       └── oauth.rs     # GitHub OAuth (unwired — handlers exist but no routes)
├── auth/
│   ├── jwt/             # JWT signing, validation, middleware, claims
│   ├── fido2/           # WebAuthn ceremony logic (webauthn-rs)
│   ├── web3/            # SIWE message building + ethers verification
│   ├── jit/             # JIT issuer, verifier, scope grammar, epoch
│   ├── keys/            # Argon2id hashing + key service
│   ├── capabilities/    # Compile-time scope registry (19 scopes)
│   └── mcp/             # MCP server (feature-gated: features = ["mcp"])
├── db/
│   ├── pool.rs          # SQLx SQLite pool
│   └── models/          # User, Web3Wallet, Fido2Credential, Session, ApiKey, Identity, Contact, OAuthAccount
├── utils/
    └── crypto.rs        # Shared crypto utilities
```

## Authentication Flows

### FIDO2 / WebAuthn

```
┌──────────┐     ┌──────────────┐     ┌──────────┐
│  Client  │     │  Astral Key  │     │  SQLite  │
│ (Browser)│     │  (API)       │     │   DB     │
└────┬─────┘     └──────┬───────┘     └────┬─────┘
     │                   │                   │
     │ 1. POST /fido2/register/options      │
     │    (JWT auth required)               │
     │──────────────────>│                   │
     │                   │ 2. Store challenge│
     │                   │  (in-memory, TTL) │
     │ 3. {challenge, rp, user, params}     │
     │<──────────────────│                   │
     │ 4. Browser creates passkey           │
     │ 5. POST /fido2/register/verify       │
     │──────────────────>│                   │
     │                   │ 6. Verify attest. │
     │                   │ 7. Store cred     │
     │                   │──────────────────>│
     │ 8. {status: "success", id}           │
     │<──────────────────│                   │
     │                   │                   │
     │ 9. POST /fido2/authenticate/options   │
     │    (public — no JWT)                 │
     │──────────────────>│                   │
     │10. {challenge, allowCredentials}      │
     │<──────────────────│                   │
     │11. User authenticates (biometric/PIN) │
     │12. POST /fido2/authenticate/verify    │
     │──────────────────>│                   │
     │                   │13. Verify assert  │
     │14. {access_token, refresh_token}      │
     │<──────────────────│                   │
```

### Web3 / SIWE

```
┌──────────┐     ┌──────────────┐     ┌──────────┐
│  Client  │     │  Astral Key  │     │  SQLite  │
│ (Wallet) │     │  (API)       │     │   DB     │
└────┬─────┘     └──────┬───────┘     └────┬─────┘
     │                   │                   │
     │ 1. POST /web3/nonce                  │
     │──────────────────>│                   │
     │ 2. {nonce, message_template, domain} │
     │<──────────────────│                   │
     │ 3. User signs EIP-4361 in wallet     │
     │ 4. POST /web3/verify                 │
     │──────────────────>│                   │
     │                   │ 5. Validate nonce │
     │                   │ 6. Verify sig     │
     │                   │ 7. Find/create    │
     │                   │    user + wallet  │
     │                   │──────────────────>│
     │ 8. {access_token, refresh_token}     │
     │<──────────────────│                   │
```

## State Management

`AppState` holds:

- **`Config`** — parsed from environment variables at startup
- **`DbPool`** — SQLx connection pool (SQLite only)
- **`Fido2StateStore`** — in-memory HashMap (with TTL) for WebAuthn challenge
  state (no Redis needed)
- **`JwtService`** — key material and token TTLs (optional None if no
  JWT_SECRET set, but the server panics on start if missing)
- **`Fido2Service`** — WebAuthn configuration (rp_id, origins, etc.)
- **`JitIssuer`** — optional (requires `JIT_ISSUER_KEY`)
- **`JitVerifier`** — optional (requires `JIT_ISSUER_KEY`)

## Configuration

All configuration is via environment variables. No config file is read.
See [`config.example.yaml`](../config.example.yaml) for the full reference,
or [`deployment.md`](deployment.md#environment-variables) for the env var
list.

## Error Response Shape

All errors use the following JSON envelope:

```json
{
  "code": "AUTH_BAD_REQUEST",
  "detail": "Human-readable description",
  "docs_url": "https://github.com/reverb256/astral-key/docs/errors.md"
}
```

The `code` field is a machine-readable string (not an integer).
See [`error.rs`](../src/error.rs) for the full enum.

## Mosaic Identity Service (MIS)

A separate Rust binary in `crates/mosaic-identity/` with 16 REST endpoints
on port 8081 for PKI operations. See `crates/mosaic-identity/src/api.rs`
for the full list, or the AGENTS.md file for a summary table.
