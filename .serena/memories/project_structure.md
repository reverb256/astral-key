# Astral Key — Project Structure

## Root Directory Layout

```
astral-key/
├── src/                      # Auth sidecar source
├── crates/                   # Workspace crates (MIS + bridges)
├── migrations/               # SQLite migrations
├── docs/                     # Documentation
├── k8s/                      # K3s manifests
├── casdoor/                  # Casdoor OIDC configs
├── .github/                  # GitHub workflows
├── .serena/                  # Serena workspace files
├── .claude/                  # Claude AI agents + skills
├── Cargo.toml                # Workspace manifest
├── Cargo.lock                # Dependency lock
├── flake.nix                 # Nix flake (dev shell)
├── Containerfile             # Docker multi-stage build
├── Dockerfile                # MIS container build
├── docker-compose.yml        # Single-service Docker Compose
├── Dockerfile.mosaic-identity # MIS container
├── Dockerfile.bridges        # Bridge container
├── bridges-entrypoint.sh     # Bridge type dispatcher
├── README.md
├── knowledge.md              # AI project knowledge
├── ARCHITECTURE.md           # Brief arch pointer
├── ROADMAP.md                # Roadmap
├── TESTING.md                # Testing guide
├── CONTRIBUTING.md           # Contribution guidelines
├── IMPLEMENTATION_SUMMARY.md # Module-by-module status
├── AGENTS.md                 # AI agent knowledge base
├── HEY.md                    # Historical coordination log (stale)
└── LICENSE                   # MIT license
```

## Source Structure (`src/`)

```
src/
├── lib.rs                 # Library root
├── main.rs                # Binary entry point + health/ready routes
├── config.rs              # Env-var config
├── error.rs               # AuthError (string error codes)
├── state.rs               # AppState (DbPool, services, stores)
│
├── api/                   # HTTP Layer
│   ├── routes.rs          # Route definitions (public vs protected)
│   ├── handlers/          # 9 handler modules
│   │   ├── health.rs      # Health/ready handlers
│   │   ├── web3.rs        # SIWE nonce, verify, chains
│   │   ├── fido2.rs       # WebAuthn register, authenticate, CRUD
│   │   ├── auth.rs        # Token verification
│   │   ├── session.rs     # Token refresh, list/revoke sessions
│   │   ├── keys.rs        # API key CRUD + revoke
│   │   ├── jit.rs         # JIT token mint + verify
│   │   ├── identity.rs    # Ed25519 identity, contacts, QR, verify
│   │   └── oauth.rs       # GitHub OAuth (unwired)
│   └── middleware/
│       ├── rate_limit.rs  # Token-bucket rate limiter
│       ├── audit.rs       # JSON audit logging
│       └── cors.rs        # CORS layer
│
├── auth/                  # Authentication modules
│   ├── jwt/               # JWT service + middleware + claims
│   ├── fido2/             # WebAuthn registration + authentication + types
│   ├── web3/              # SIWE signing + nonce
│   ├── jit/               # JIT issuer, verifier, scope, epoch
│   ├── keys/              # Argon2id hashing + key service
│   ├── capabilities/      # Compile-time scope registry (19 scopes)
│   └── mcp/               # MCP server (feature-gated)
│
├── db/                    # Database Layer
│   ├── pool.rs            # SQLite pool
│   └── models/            # User, Web3Wallet, Fido2Credential,
│                          # Session, ApiKey, Identity, Contact, OAuthAccount
│
└── utils/
    └── crypto.rs          # Crypto helpers
```

## Crate Structure (`crates/`)

```
crates/
├── mosaic-identity/          # MIS — standalone PKI service (port 8081)
│   ├── src/api.rs            # 16 REST endpoints
│   ├── src/crypto.rs         # Ed25519 + ML-DSA-65
│   ├── src/storage.rs        # SQLite key store
│   ├── src/hd.rs             # BIP-39 → SLIP-10 HD derivation
│   ├── src/bindings.rs       # atproto DID resolver
│   ├── src/nostr.rs          # npub decoder
│   └── ...
├── mosaic-client/            # Shared bridge client lib
├── mosaic-bridge-atproto/    # atproto adapter
├── mosaic-bridge-buzz/       # Nostr adapter
├── mosaic-bridge-matrix/     # Matrix AS adapter
├── mosaic-bridge-irc/        # IRC adapter
├── mosaic-bridge-activitypub/ # ActivityPub adapter
├── mosaic-bridge-telegram/   # Telegram adapter
├── mosaic-bridge-discord/    # Discord adapter
├── mosaic-bridge-haven/      # Haven Socket.IO adapter
```

## Database Schema (`migrations/`)

3 SQLite migrations:
- `001_initial.sql` — Core tables (users, wallets, credentials)
- `002_api_keys_and_sessions.sql` — API keys + session management
- `003_identity_contacts.sql` — Ed25519 identities + contact graph

## API Routes

**Public Routes (on `/api/v1/`):**
- `POST /auth/web3/nonce` — Request SIWE nonce
- `POST /auth/web3/verify` — Verify SIWE signature → JWT
- `GET /auth/web3/chains` — List supported chains
- `POST /auth/fido2/authenticate/options` — FIDO2 auth challenge
- `POST /auth/fido2/authenticate/verify` — FIDO2 auth → JWT
- `POST /auth/verify` — Validate a JWT
- `POST /auth/token/refresh` — Refresh token pair
- `POST /auth/jit/verify` — Verify capability token
- `POST /identity/verify` — Ed25519 signature verify
- `GET /identity/qr/:pubkey` — Generate QR code

**Protected Routes (JWT required, on `/api/v1/`):**
- `POST /auth/fido2/register/options` — Register challenge
- `POST /auth/fido2/register/verify` — Complete registration
- `GET /auth/fido2/credentials` — List passkeys
- `DELETE /auth/fido2/credentials/:id` — Delete passkey
- `POST /auth/keys` — Create API key
- `GET /auth/keys` — List API keys
- `DELETE /auth/keys/:id` — Hard-delete API key
- `POST /auth/keys/:id/revoke` — Revoke API key
- `GET /auth/sessions` — List sessions
- `DELETE /auth/sessions/:id` — Revoke session
- `POST /auth/jit/mint` — Mint capability token
- `POST /identity` — Create identity
- `GET /identity` — List identities
- `GET /identity/current` — Current identity
- `POST /identity/:id/set-current` — Set active identity
- `DELETE /identity/:id` — Delete identity
- `GET /contacts` — List contacts
- `POST /contacts` — Add/update contact
- `POST /contacts/scan` — Parse QR → contact
- `DELETE /contacts/:pubkey` — Delete contact

**OAuth (unwired — handlers exist but no routes):**
- `GET /auth/oauth/github/login` — Initiate OAuth
- `GET /auth/oauth/github/callback` — OAuth callback

## Key Dependencies

**Core:** tokio, axum, tower, tower-http
**Database:** sqlx (SQLite)
**Auth:** ethers, siwe, webauthn-rs, jsonwebtoken, ed25519-dalek, argon2
**Crypto:** ring, ed25519-dalek, pqcrypto-mldsa (optional)
**Errors:** thiserror, anyhow
**Other:** serde, serde_json, tracing, chrono, uuid, qrcode, image
