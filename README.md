<div align="center">

# Astral Key

**Single-binary auth sidecar — FIDO2/WebAuthn passkeys, Web3/SIWE, and JWT sessions**

[![Rust](https://img.shields.io/badge/Rust-000000?logo=rust&logoColor=white)](https://rust-lang.org)
[![License](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![NixOS](https://img.shields.io/badge/NixOS-5277C3?logo=nixos&logoColor=white)](https://nixos.org)

</div>

Astral Key is an open-source authentication middleware written in Rust
([Axum](https://github.com/tokio-rs/axum)). It is designed as a
**single-binary sidecar** — embed it next to your application and let it
handle passkey and Web3 authentication. No external database, cache, or
credential store is required: everything is backed by **SQLite**.

---

## Quick Start

### With Cargo

```bash
# Clone and build
git clone https://github.com/reverb256/astral-key.git
cd astral-key
JWT_SECRET=$(openssl rand -hex 32) cargo run

# Test it
curl http://localhost:8080/health
```

### With Docker Compose

```bash
git clone https://github.com/reverb256/astral-key.git
cd astral-key
JWT_SECRET=$(openssl rand -hex 32) docker compose up -d
curl http://localhost:8080/health
```

### With Nix

```bash
nix develop
cargo build --release
JWT_SECRET=$(openssl rand -hex 32) ./target/release/astral-key
```

---

## Features

### ✅ Implemented

| Feature | Description |
|---------|-------------|
| **FIDO2 / WebAuthn Passkeys** | Register and authenticate with platform or roaming authenticators (Touch ID, Windows Hello, YubiKey, etc.). Full WebAuthn ceremony with challenge verification. |
| **Web3 / SIWE** | Sign-In with Ethereum (EIP-4361). Generate nonces, verify signatures via `ethers-rs`, auto-create users and wallets. |
| **JWT Sessions** | Access tokens (15 min) and refresh tokens (7 days). Token verification endpoint for external services. |
| **SQLite Storage** | All state in a single SQLite database — users, Web3 wallets, FIDO2 credentials, session nonces. |
| **Passkey CRUD** | List and delete registered passkeys. |
| **Multi-chain Support** | Ethereum, Polygon, Arbitrum, Optimism, Goerli, Sepolia. |
| **GitHub OAuth** | Optional GitHub OAuth provider for additional login methods. |

### 🔜 Coming Soon

- **API Key Management** — create, rotate, and revoke API keys for programmatic access
- **ZK JIT Capability Tokens** — zero-knowledge just-in-time capability tokens for fine-grained authorization
- **MCP Server** — Model Context Protocol server for AI agent integration
- **Rate Limiting** — configurable request throttling per endpoint
- **NixOS Module** — declarative NixOS service configuration

---

## Architecture

```
┌──────────────┐     ┌──────────────────┐     ┌────────┐
│   Client     │────>│   Astral Key     │────>│ SQLite │
│ (Browser /   │     │   (Axum API)     │     │  (DB)  │
│  Wallet)     │<────│   Port 8080      │<────│        │
└──────────────┘     └──────────────────┘     └────────┘
                          │
                     ┌────┴────┐
                     │ In-Memory│
                     │ FIDO2    │
                     │ State    │
                     └─────────┘
```

There is no PostgreSQL, no Redis, no Vaultwarden, and no external cache.
The server runs as a single process with a single SQLite file.

See [`docs/architecture.md`](docs/architecture.md) for detailed flow diagrams
and module descriptions.

---

## API Overview

| Endpoint | Auth | Description |
|----------|------|-------------|
| `GET /health` | — | Liveness check |
| `GET /ready` | — | Readiness check (database) |
| `POST /api/v1/auth/web3/chains` | — | List supported chains |
| `POST /api/v1/auth/web3/nonce` | — | Request SIWE nonce |
| `POST /api/v1/auth/web3/verify` | — | Verify SIWE signature → JWT |
| `POST /api/v1/auth/fido2/register/options` | JWT | Start passkey registration |
| `POST /api/v1/auth/fido2/register/verify` | JWT | Complete passkey registration |
| `POST /api/v1/auth/fido2/authenticate/options` | — | Start passkey authentication |
| `POST /api/v1/auth/fido2/authenticate/verify` | — | Complete passkey authentication → JWT |
| `GET /api/v1/auth/fido2/credentials` | JWT | List registered passkeys |
| `DELETE /api/v1/auth/fido2/credentials/:id` | JWT | Delete a passkey |
| `POST /api/v1/auth/verify` | — | Verify a JWT token |

Full API reference with curl examples: [`docs/api.md`](docs/api.md)

---

## Configuration

All configuration is via environment variables. There is no configuration
file.

```bash
# Required: JWT signing key (at least 32 bytes)
export JWT_SECRET=$(openssl rand -hex 32)

# Optional overrides
export SERVER_HOST=0.0.0.0
export SERVER_PORT=8080
export DATABASE_URL="sqlite:/data/astral-key.db?mode=rwc"
export FIDO2_RP_ID=localhost
export FIDO2_RP_NAME="My App"
export FIDO2_ORIGINS=http://localhost:8080
export RUST_LOG=info,astral_key=debug
```

Full environment variable reference: [`docs/deployment.md`](docs/deployment.md#environment-variables)
Example files: [`config.example.yaml`](config.example.yaml), [`.env.example`](.env.example)

---

## Documentation

| Document | Description |
|----------|-------------|
| [`docs/architecture.md`](docs/architecture.md) | Module layout and authentication flows |
| [`docs/api.md`](docs/api.md) | Full API reference with curl examples |
| [`docs/deployment.md`](docs/deployment.md) | Docker Compose, Nix, K8s, env reference |
| [`docs/errors.md`](docs/errors.md) | Error code reference |
| [`CONTRIBUTING.md`](CONTRIBUTING.md) | How to build, test, and submit PRs |

---

## License

MIT License — see [LICENSE](LICENSE) for details.

Copyright © Jeremy Kroeker ([reverb256](https://github.com/reverb256))
