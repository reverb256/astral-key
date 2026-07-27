# Astral Key — Architecture

> **This document has been superseded.** The authoritative, code-matched
> architecture document is [`docs/architecture.md`](docs/architecture.md).
>
> What follows is a brief summary for orientation. For full detail including
> module layout, authentication flow diagrams, state management, and the
> Mosaic Identity Service, see `docs/architecture.md`.

## What this project actually is

Astral Key is a **single-binary auth sidecar** (Rust, Axum, **SQLite only**).
No PostgreSQL, no Redis, no Vaultwarden. The process listens on port 8080.

The repo also contains:

- **Mosaic Identity Service (MIS)** — a separate Rust binary (`crates/mosaic-identity/`)
  with 16 REST endpoints for Ed25519 PKI operations, cross-protocol identity
  binding, and ML-DSA-65 post-quantum hybrid signing.
- **9 transport bridge crates** (`crates/mosaic-bridge-*/`) — sidecar daemons
  for atproto, buzz (nostr), matrix, irc, activitypub, telegram, discord,
  and haven (Socket.IO).
- **`crates/mosaic-client/`** — shared client library used by the bridges.

## Quick module map

```
src/
├── main.rs              # Entrypoint + health/ready routes
├── config.rs            # Env-var config (no TOML/YAML/INI file read)
├── error.rs             # AuthError with string error codes
├── state.rs             # AppState (DbPool, services, in-memory FIDO2 store)
├── api/handlers/        # 9 handler modules (web3, fido2, session, keys, jit, identity, auth, health, oauth)
├── api/routes.rs        # All route definitions (public vs protected)
├── api/middleware/       # rate_limit, audit, cors
├── auth/                # jwt, fido2, web3, jit, keys, capabilities, mcp
├── db/                  # SQLite pool + models
└── utils/crypto.rs

crates/mosaic-identity/  # Separate binary on port 8081 (16 endpoints)
crates/mosaic-bridge-*/  # 9 sidecar daemons
```

## Key constraints

- **All config via environment variables** — `JWT_SECRET` required on startup
- **SQLite only** — no PostgreSQL, Redis, or Vaultwarden
- **No NixOS module exists** — deployment is via Docker Compose or K3s
- **No TOML/JSON config file** — the binary reads env vars only
- **OAuth handlers exist but are not wired into routes** — endpoints unreachable

See [`docs/architecture.md`](docs/architecture.md) for the full treatment.
