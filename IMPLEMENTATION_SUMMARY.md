# Astral Key — Implementation Summary

**Date:** 2026-07-27 (refresh)
**Status:** Core auth complete, MIS shipped, bridges deployed

---

## Executive Summary

Astral Key is a single-binary authentication sidecar for FIDO2/WebAuthn passkey
and Web3/SIWE authentication. Built with Rust (Axum 0.7) and backed entirely by
**SQLite** — no PostgreSQL, no Redis, no external cache.

A v2 upgrade added API key management, ZK JIT capability tokens, session
management with refresh token rotation, Ed25519 identity/contacts, rate
limiting, audit logging, and an MCP server. The project retains its zero-ops
character: a single binary + a single SQLite file.

The repo also contains the **Mosaic Identity Service (MIS)** crate — a
standalone PKI service for Ed25519 key management, cross-protocol identity
binding, ML-DSA-65 PQ hybrid signing, BIP-39 HD derivation, and agent
ephemeral certs (16 REST endpoints on port 8081).

And **9 transport bridge crates** (atproto, buzz, matrix, irc, activitypub,
telegram, discord, haven, + shared mosaic-client lib).

---

## Current Architecture

| Layer | Technology |
|-------|-----------|
| **Runtime** | Tokio (async) |
| **Web framework** | Axum 0.7 |
| **Database** | SQLite 3 (sqlx 0.8) |
| **FIDO2 / WebAuthn** | webauthn-rs 0.5 |
| **Web3 / SIWE** | ethers-rs 2.0, siwe 0.6 |
| **JWT** | jsonwebtoken 10.x (HMAC HS256) |
| **API key hashing** | Argon2id (argon2 0.5) |
| **JIT tokens** | Ed25519 (ed25519-dalek 2.x) |
| **Post-quantum** | ML-DSA-65 (FIPS 204, feature-gated `pq`) |
| **Deploy** | Docker Compose, K3s, Nix dev shell |

---

## Completed Modules

### 1. Web3 / SIWE Authentication

- SIWE message parsing (EIP-4361) with field validation
- ECDSA signature recovery via ethers-rs
- Domain spoofing protection — only the configured `ASTRAL_WEB3_DOMAIN` is
  accepted; client-supplied domains are filtered
- Cryptographically secure nonce generation (32 bytes, hex-encoded)
- SQLite-backed nonce store with 15-minute TTL and one-time consumption
- Auto-creation of users and wallets on first successful authentication
- Multi-chain support: Ethereum, Polygon, Arbitrum, Optimism, Goerli, Sepolia

**Files:** `src/auth/web3/siwe.rs`, `src/auth/web3/nonce.rs`,
`src/api/handlers/web3.rs`

### 2. FIDO2 / WebAuthn

- Full WebAuthn ceremony — registration and authentication challenges
- Challenge verification via webauthn-rs (attestation + assertion)
- In-memory challenge state store (no Redis needed)
- Passkey credential CRUD — create, list, delete with ownership checks
- Counter tracking for replay-attack prevention
- Cross-platform passkey sync with resident key requirement

**Files:** `src/auth/fido2/registration.rs`, `src/auth/fido2/authentication.rs`,
`src/auth/fido2/types.rs`, `src/api/handlers/fido2.rs`

### 3. JWT Sessions

- Access tokens (15-minute TTL) and refresh tokens (7-day TTL)
- TokenKind validation — refresh tokens cannot be used as access tokens
- Refresh token rotation — each refresh invalidates the previous token
- Session CRUD — list, revoke by ID
- Token verification endpoint for external services (e.g., Quill MCP)
- SHA-256 hashed refresh tokens for session lookup

**Files:** `src/auth/jwt/mod.rs`, `src/auth/jwt/claims.rs`,
`src/auth/jwt/middleware.rs`, `src/api/handlers/session.rs`,
`src/db/models/session.rs`

### 4. API Key Management

- Argon2id-hashed API keys — only the hash is stored; plaintext shown once
- Key format: `ak_{environment}_{base58}` (e.g., `ak_prod_...`)
- Prefix-based lookup — extract `ak_prod_` to find candidates, verify hash
- Create, list, revoke (soft delete), and hard-delete operations
- Expiration support and ownership scoping

**Files:** `src/auth/keys/hashing.rs`, `src/auth/keys/service.rs`,
`src/api/handlers/keys.rs`, `src/db/models/api_key.rs`

### 5. ZK JIT Capability Tokens

- Ed25519-signed capability tokens with `base64(header).base64(payload).base64(sig)` format
- Zero-database-write minting — pure CPU operation
- Three-party model: issuer (holds signing key), verifier (holds verifying key),
  consumer (presents token)
- Scope grammar (`namespace:action`) with `admin` wildcard
- Compile-time scope registry with 19 known scopes
- Epoch-based batch revocation (O(1) emergency kill switch)
- JSONL tombstone journal for durable per-token revocation
- Comprehensive test suite for issuer, verifier, scope validation, epoch,
  and tombstone persistence

**Files:** `src/auth/jit/issuer.rs`, `src/auth/jit/verifier.rs`,
`src/auth/jit/scope.rs`, `src/auth/jit/epoch.rs`,
`src/auth/capabilities/registry.rs`

### 6. Ed25519 Identity & Contacts

- Ed25519 public-key identity management (create, list, set current, delete)
- Signature verification endpoint — clients sign locally, server verifies
  Ed25519 signatures over canonical JSON
- Contact graph — add/update contacts by public key, from QR scan
- QR code generation (SVG + PNG) for public-key sharing
- URI schemes: `mosaic://`, `mosiac://`, `astral://identity/`

**Files:** `src/api/handlers/identity.rs`, `src/db/models/identity.rs`,
`src/db/models/contact.rs`

### 7. Middleware

- **JWT auth middleware** — validates `Authorization: Bearer` header on
  protected routes
- **Rate limiting** — per-key token-bucket limiter (API key prefix + client IP),
  returns `429 Too Many Requests` with `Retry-After` header
- **Audit logging** — structured JSON audit events written to stdout, one
  per request, with request ID, client IP, resource, and outcome
- **CORS** — permissive CORS layer (narrow for production)

**Files:** `src/api/middleware/rate_limit.rs`, `src/api/middleware/audit.rs`,
`src/api/middleware/cors.rs`, `src/auth/jwt/middleware.rs`

### 8. MCP Server (feature-gated)

- Model Context Protocol server behind `features = ["mcp"]`
- Tools: health check, mint token, verify token, create API key
- Static state initialised once before serving

**Files:** `src/auth/mcp/tools.rs`, `src/auth/mcp/mod.rs`

### 9. Documentation (fresh as of 2026-07-27)

- `README.md` — project overview, quick start (cargo, Docker, Nix)
- `docs/api.md` — full API reference with curl examples (all 25+ endpoints)
- `docs/architecture.md` — truthful SQLite-only module layout
- `docs/deployment.md` — Docker Compose, Nix dev shell, K3s deployment guide
- `docs/errors.md` — error code reference by endpoint (string codes)
- `CONTRIBUTING.md` — build, test, and PR guidelines
- `config.example.yaml` — environment variable reference
- `knowledge.md` — AI project knowledge for automated tooling
- `AGENTS.md` — AI agent knowledge base (MIS, bridges, cluster topology)

---

## Endpoints

### Public (no auth)

| Method | Path | Description |
|--------|------|-------------|
| GET | `/health` | Liveness check (returns plain text `"OK"`) |
| GET | `/ready` | Readiness check (DB) |
| **GET** | `/api/v1/auth/web3/chains` | List supported chains |
| POST | `/api/v1/auth/web3/nonce` | SIWE nonce |
| POST | `/api/v1/auth/web3/verify` | SIWE → JWT |
| POST | `/api/v1/auth/fido2/authenticate/options` | Auth challenge |
| POST | `/api/v1/auth/fido2/authenticate/verify` | Auth → JWT |
| POST | `/api/v1/auth/verify` | Validate a JWT |
| POST | `/api/v1/auth/token/refresh` | Refresh token pair |
| POST | `/api/v1/auth/jit/verify` | Verify capability token |
| POST | `/api/v1/identity/verify` | Verify Ed25519 signature |
| GET | `/api/v1/identity/qr/:pubkey` | Generate QR code |

### Protected (JWT required)

| Method | Path | Description |
|--------|------|-------------|
| POST | `/api/v1/auth/fido2/register/options` | Register options |
| POST | `/api/v1/auth/fido2/register/verify` | Register verify |
| GET | `/api/v1/auth/fido2/credentials` | List passkeys |
| DELETE | `/api/v1/auth/fido2/credentials/:id` | Delete passkey |
| POST | `/api/v1/auth/keys` | Create API key |
| GET | `/api/v1/auth/keys` | List API keys |
| DELETE | `/api/v1/auth/keys/:id` | Hard-delete API key |
| POST | `/api/v1/auth/keys/:id/revoke` | Revoke API key |
| GET | `/api/v1/auth/sessions` | List sessions |
| DELETE | `/api/v1/auth/sessions/:id` | Revoke session |
| POST | `/api/v1/auth/jit/mint` | Mint capability token |
| POST | `/api/v1/identity` | Create identity |
| GET | `/api/v1/identity` | List identities |
| GET | `/api/v1/identity/current` | Current identity |
| POST | `/api/v1/identity/:id/set-current` | Set current identity |
| DELETE | `/api/v1/identity/:id` | Delete identity |
| GET | `/api/v1/contacts` | List contacts |
| POST | `/api/v1/contacts` | Add / update contact |
| POST | `/api/v1/contacts/scan` | Scan QR → contact |
| DELETE | `/api/v1/contacts/:pubkey` | Delete contact |

### OAuth (handlers exist, not wired into routes)

| Method | Path | Description |
|--------|------|-------------|
| GET | `/auth/oauth/github/login` | Initiate GitHub OAuth |
| GET | `/auth/oauth/github/callback` | GitHub OAuth callback |

---

## Key Design Decisions

| Decision | Rationale |
|----------|-----------|
| **SQLite only** | Zero ops — no Redis/Postgres daemons to manage |
| **Ed25519 for JIT signing** | Fastest verify, deterministic, safest 2026 default |
| **API key prefix `ak_prod_`** | Industry standard (GitHub `ghp_`, Stripe `sk_live_`) |
| **Argon2id for API key hashing** | GPU-resistant memory-hard function |
| **Scope = flat set intersection** | No Zanzibar graph needed; set membership sufficient |
| **MCP server feature-gated** | Optional dependency, minimal base-binary overhead |
| **Revocation = epoch + tombstones** | Epoch fast path for batch, JSONL for individual |
| **ML-DSA-65 for PQ** | FIPS 204 compliant, replaces previous FALCON-512 stub |

---

## Testing Strategy

- Unit tests live alongside code (`#[cfg(test)] mod tests`)
- Modules with tests: JWT, Web3 SIWE, FIDO2 types, rate limiter, audit,
  JIT issuer/verifier/scope/epoch, API key hashing, capabilities registry,
  SIWE message parsing, nonce generation, CORS
- Integration tests: `tests/` directory (planned, not yet implemented)
- No external services required — SQLite runs in-memory for tests

---

## Remaining Work

- `cargo check` and `cargo test --lib` — verify current state compiles cleanly
- Integration tests for the full API surface (keys, sessions, JIT, identity)
- Wire OAuth (GitHub) handlers into route tree (requires adding `oauth_state` field to AppState)
- Build + push container images to registry
- CI/CD: release workflow (tag → OCI publish)
