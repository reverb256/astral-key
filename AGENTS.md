# PROJECT KNOWLEDGE BASE

**Generated:** 2026-07-16
**Type:** Rust microservice (auth sidecar)
**Status:** Implementing — core auth modules complete

## OVERVIEW

Astral Key is a single-binary authentication sidecar for FIDO2/WebAuthn
passkey and Web3/SIWE authentication. Built with Rust (Axum) and SQLite.

## STRUCTURE

```
astral-key/
├── src/
│   ├── main.rs              # Entry point
│   ├── config.rs            # Env var configuration
│   ├── error.rs             # AuthError enum → HTTP responses
│   ├── state.rs             # AppState (pool, services)
│   ├── api/
│   │   ├── routes.rs        # Route definitions
│   │   └── handlers/        # Request handlers
│   │       ├── health.rs    # /health, /ready
│   │       ├── web3.rs      # SIWE nonce, verify, chains
│   │       ├── fido2.rs     # WebAuthn ceremony handlers
│   │       └── auth.rs      # Token verification
│   ├── auth/
│   │   ├── jwt/             # JWT signing, validation, middleware
│   │   ├── fido2/           # WebAuthn ceremony logic
│   │   └── web3/            # SIWE message + ethers verification
│   ├── db/
│   │   ├── pool.rs          # SQLx SQLite pool
│   │   └── models/          # User, Web3Wallet, Fido2Credential, etc.
│   └── utils/               # Utilities
├── migrations/               # SQLx migrations
├── docs/                     # Documentation
├── nix/                      # NixOS module (WIP)
├── k8s/                      # K3s manifests (WIP)
├── config.example.yaml       # Env var reference
├── docker-compose.yml        # Single-service Docker Compose
├── Cargo.toml                # Rust dependencies
├── flake.nix                 # Nix flake
└── README.md                 # Project README
```

## KEY FACTS

- **Language:** Rust 2021 edition
- **Framework:** Axum 0.7
- **Storage:** SQLite only (sqlx)
- **Auth:** FIDO2/WebAuthn, Web3/SIWE, JWT (access + refresh tokens)
- **Config:** Environment variables (`std::env::var`) — no config file
- **Build:** `cargo build`, `nix develop`
- **Deploy:** Docker Compose, K3s
- **License:** MIT
- **Copyright:** Jeremy Kroeker (reverb256)

## ENVIRONMENT VARIABLES

| Variable | Required | Default |
|----------|----------|---------|
| `JWT_SECRET` | **Yes** | — |
| `SERVER_HOST` | No | `127.0.0.1` |
| `SERVER_PORT` | No | `8080` |
| `DATABASE_URL` | No | `sqlite:astral_key.db?mode=rwc` |
| `DATABASE_MAX_CONNECTIONS` | No | `5` |
| `FIDO2_RP_ID` | No | `localhost` |
| `FIDO2_RP_NAME` | No | `Astral Key` |
| `FIDO2_ORIGINS` | No | `http://localhost:8080` |
| `FIDO2_ATTESTATION` | No | `indirect` |
| `ASTRAL_WEB3_DOMAIN` | No | `maplespike.ca` |
| `OAUTH_BASE_URL` | No | `http://localhost:8080` |
| `OAUTH_GITHUB_CLIENT_ID` | No | — |
| `OAUTH_GITHUB_CLIENT_SECRET` | No | — |
| `OAUTH_GITHUB_REDIRECT_URI` | No | `{OAUTH_BASE_URL}/auth/oauth/github/callback` |
| `RUST_LOG` | No | `info,astral_key=debug` |

See [`config.example.yaml`](config.example.yaml) and
[`docs/deployment.md`](docs/deployment.md) for details.

## API ENDPOINTS

| Endpoint | Auth | Description |
|----------|------|-------------|
| `GET /health` | — | Liveness |
| `GET /ready` | — | Readiness (DB check) |
| `POST /api/v1/auth/web3/chains` | — | Supported chains |
| `POST /api/v1/auth/web3/nonce` | — | SIWE nonce |
| `POST /api/v1/auth/web3/verify` | — | Verify SIWE → JWT |
| `POST /api/v1/auth/fido2/register/options` | JWT | Register options |
| `POST /api/v1/auth/fido2/register/verify` | JWT | Register verify |
| `POST /api/v1/auth/fido2/authenticate/options` | — | Auth options |
| `POST /api/v1/auth/fido2/authenticate/verify` | — | Auth verify → JWT |
| `GET /api/v1/auth/fido2/credentials` | JWT | List passkeys |
| `DELETE /api/v1/auth/fido2/credentials/:id` | JWT | Delete passkey |
| `POST /api/v1/auth/verify` | — | Validate JWT |

## COMMANDS

```bash
cargo build              # Build binary
cargo run                # Start server
cargo test               # Run tests
cargo fmt                # Format code
cargo clippy             # Lint
cargo audit              # Security audit
nix develop              # Enter dev shell
docker compose up -d     # Start Docker service
```

## CONVENTIONS

- **Async-first**: All I/O via Tokio
- **Error handling**: `thiserror` + `anyhow`, never `unwrap()` in production
- **Config**: Only env vars, no config files
- **Testing**: Unit tests in source files, integration tests in `tests/`
- **SQLite**: All migrations must be SQLite-compatible

## UPCOMING FEATURES

- API key management (create, rotate, revoke)
- ZK JIT capability tokens
- MCP server (Model Context Protocol)
- Rate limiting middleware
- NixOS module (production-ready)

## LINKS

- Repo: https://github.com/reverb256/astral-key
- README: https://github.com/reverb256/astral-key#readme
- Docs: ./docs/

## LAST UPDATED

2026-07-16: FOSS documentation phase — added config.example.yaml, docker-compose.yml,
CONTRIBUTING.md, docs/architecture.md, docs/api.md, docs/deployment.md, docs/errors.md,
.env.example. Rewrote README.md and AGENTS.md. Removed STATUS.md.
