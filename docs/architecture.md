# Architecture

This document describes Astral Key's architecture, module structure, and
authentication flows.

## Overview

Astral Key is a single-binary auth sidecar written in Rust using the Axum web
framework. It stores all data in **SQLite** (no PostgreSQL, no Redis, no
external cache). Authentication is provided via three mechanisms:

- **FIDO2 / WebAuthn** — passkeys (platform and roaming authenticators)
- **Web3 / SIWE** — Sign-In with Ethereum (EIP-4361)
- **JWT** — signed access and refresh tokens for session management

## Module Layout

```
src/
├── main.rs              # Entry point — Tokio runtime, server bind
├── config.rs            # Env-var configuration (no config file)
├── error.rs             # AuthError enum → HTTP response mapping
├── state.rs             # AppState — shared pool, services, stores
├── api/
│   ├── routes.rs        # Route definitions (public / protected)
│   └── handlers/
│       ├── health.rs    # /health, /ready endpoints
│       ├── web3.rs      # SIWE nonce, verify, chains
│       ├── fido2.rs     # WebAuthn register, authenticate, CRUD
│       └── auth.rs      # Token verification (external services)
├── auth/
│   ├── jwt/             # JWT signing, validation, middleware
│   ├── fido2/           # WebAuthn ceremony logic (webauthn-rs)
│   └── web3/            # SIWE message building + ethers verification
├── db/
│   ├── pool.rs          # SQLx SQLite pool
│   └── models/          # User, Web3Wallet, Fido2Credential, etc.
└── utils/               # Shared utilities
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
     │──────────────────>│                   │
     │                   │ 2. Store challenge│
     │                   │──────────────────>│
     │ 3. {challenge, rp, user, params}     │
     │<──────────────────│                   │
     │                   │                   │
     │ 4. Browser creates passkey           │
     │    (navigator.credentials.create)    │
     │                   │                   │
     │ 5. POST /fido2/register/verify       │
     │──────────────────>│                   │
     │                   │ 6. Verify attest. │
     │                   │ 7. Store cred     │
     │                   │──────────────────>│
     │ 8. {status: "success", id}           │
     │<──────────────────│                   │
     │                   │                   │
     │ 9. POST /fido2/authenticate/options   │
     │──────────────────>│                   │
     │                   │10. Lookup user    │
     │                   │──────────────────>│
     │11. {challenge, allowCredentials}      │
     │<──────────────────│                   │
     │                   │                   │
     │12. User authenticates (biometric/     │
     │    PIN) → navigator.credentials.get   │
     │                   │                   │
     │13. POST /fido2/authenticate/verify    │
     │──────────────────>│                   │
     │                   │14. Verify assert. │
     │                   │15. Update counter │
     │                   │──────────────────>│
     │16. {access_token, refresh_token}      │
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
     │                   │ 2. Generate nonce │
     │ 3. {nonce, message_template, domain} │
     │<──────────────────│                   │
     │                   │                   │
     │ 4. User signs EIP-4361 message       │
     │    in wallet                         │
     │                   │                   │
     │ 5. POST /web3/verify                 │
     │──────────────────>│                   │
     │                   │ 6. Validate nonce │
     │                   │ 7. Verify sig     │
     │                   │    (ethers-rs)    │
     │                   │ 8. Find/create    │
     │                   │    user + wallet  │
     │                   │──────────────────>│
     │ 9. {access_token, refresh_token, user}│
     │<──────────────────│                   │
```

### Token Verification (External Services)

```
┌──────────────┐     ┌──────────────┐
│  Quill MCP   │     │  Astral Key  │
│  (ext svc)   │     │  (API)       │
└──────┬───────┘     └──────┬───────┘
       │                    │
       │ POST /auth/verify  │
       │ {token: "eyJ..."}  │
       │───────────────────>│
       │                    │
       │ {valid: true, sub, │
       │  exp} or           │
       │ {valid: false, err}│
       │<───────────────────│
```

## State Management

Astral Key's `AppState` holds:

- **`Config`** — parsed from environment variables at startup
- **`DbPool`** — SQLx connection pool (SQLite, `sqlx::sqlite::SqlitePool`)
- **`JwtService`** — key material and token TTLs
- **`Fido2Service`** — WebAuthn configuration (rp_id, origins, etc.)
- **`Fido2StateStore`** — in-memory HashMap (with TTL) for challenge state
  during WebAuthn ceremonies (no Redis needed)

## Configuration

All configuration is provided via environment variables. There is no
configuration file. See [`config.example.yaml`](../config.example.yaml) for
the full reference, or [`deployment.md`](deployment.md#environment-variables)
for the list of env vars.
