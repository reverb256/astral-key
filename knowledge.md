# Astral Key — Project Knowledge

## What This Is

**Rust workspace** with two main subsystems:

1. **Astral Key** (`src/`) — Authentication sidecar for **FIDO2/WebAuthn passkeys** + **Web3/SIWE** (Ethereum wallet signatures). Issues JWT sessions. Single binary + SQLite (no Redis/Postgres needed).
   - Port **8080** by default
   - `src/api/handlers/` — Request handlers (web3, fido2, session, keys, identity, jit, oauth)
   - `src/auth/` — Auth logic (web3, fido2, jwt, jit, keys, capabilities, mcp)
   - `src/db/` — SQLx SQLite models + pool
   - `migrations/` — SQLite migrations (001–003)

2. **Mosaic Identity Service (MIS)** (`crates/mosaic-identity/`) — Standalone PKI service for **Ed25519 key management**, cross-protocol identity binding (atproto, nostr, matrix, irc), **ML-DSA-65** post-quantum hybrid signing (feature-gated), BIP-39 → SLIP-10 HD derivation, and agent ephemeral cert delegation.
   - Port **8081** by default
   - 16 REST endpoints (`api.rs`), Ed25519 + PQ crypto (`crypto.rs`), SQLite storage (`storage.rs`)

3. **Transport bridges** (`crates/mosaic-bridge-*/`) — Sidecar containers for atproto, buzz (nostr), matrix, irc, haven (Socket.IO), activitypub, telegram, discord. Selected via `BRIDGE_TYPE` env var.

## Where Key Code Lives

| Area | Path | Notes |
|------|------|-------|
| API routes | `src/api/routes.rs` | All endpoint definitions |
| Auth handlers | `src/api/handlers/*.rs` | web3, fido2, session, keys, identity, jit, oauth, health |
| Middleware | `src/api/middleware/` | rate_limit, audit, cors, jwt middleware |
| FIDO2/WebAuthn | `src/auth/fido2/` | registration, authentication, types |
| Web3/SIWE | `src/auth/web3/` | siwe, nonce modules |
| JWT | `src/auth/jwt/` | claims, middleware, mod (tests) |
| JIT capability tokens | `src/auth/jit/` | issuer, verifier, scope, epoch |
| API keys | `src/auth/keys/` | hashing (Argon2id), service |
| Capabilities registry | `src/auth/capabilities/registry.rs` | 19 known scopes, compile-time |
| MCP server | `src/auth/mcp/` | Feature-gated (`features = ["mcp"]`) |
| MIS (PKI) | `crates/mosaic-identity/src/` | api, crypto, storage, hd, bindings, nostr |
| Config | `src/config.rs` | Environment-variable driven |
| State | `src/state.rs` | AppState shared across handlers |
| Docker | `Containerfile`, `Dockerfile`, `docker-compose.yml` | |
| K8s | `k8s/` | Deployment manifests |
| Claude agents/skills | `.claude/agents/`, `.claude/skills/` | crypto-reviewer, api-tester, rust-conventions, etc. |

## Commands

```bash
# Build & check
cargo build                          # Build everything
cargo build -p mosaic-identity       # Build just MIS
cargo check                          # Type-check (fast)

# Run
JWT_SECRET=$(openssl rand -hex 32) cargo run                    # Astral Key dev server (:8080)
cargo run -p mosaic-identity -- \
  --database "sqlite:///tmp/mis.db?mode=rwc"                    # MIS standalone (:8081)
docker compose up -d                                            # Docker deployment

# Test
cargo test --lib                       # All unit tests (in-memory SQLite, no deps)
cargo test --lib -- --nocapture        # With stdout
cargo test --lib jwt::                 # Specific module
cargo test -p mosaic-identity          # MIS tests

# Lint & format
cargo clippy                           # Lint
cargo fmt                              # Format
cargo clippy --all-features -- -D warnings  # CI-level strictness

# MIS with post-quantum
cargo build -p mosaic-identity --features pq --release

# Nix
nix develop                            # Dev shell with pinned Rust
nix flake check                        # Checks (test + clippy + fmt)
```

## Notable Conventions & Constraints

- **SQLite only** — No PostgreSQL, Redis, or external daemons. Single binary + one `.db` file. Zero ops.
- **All config via environment variables** — `JWT_SECRET` is required (min 32 bytes hex). See `src/config.rs` or `docs/deployment.md` for the full list.
- **Ed25519 for JIT capability tokens** — Fast verify, deterministic. Used in `src/auth/jit/`.
- **API key format**: `ak_{env}_{base58}` (e.g., `ak_prod_...` or `ak_dev_...`). Hashed with **Argon2id**.
- **Scope model**: Flat set intersection (`namespace:action`), with `admin` wildcard. 19 known scopes in `src/auth/capabilities/registry.rs`.
- **Revocation model**: Epoch-based batch revocation (O(1) fast path) + JSONL tombstone journal for per-token revocation.
- **MCP server** is feature-gated (`--features mcp`). Not in default build.
- **MIS PQ signing** is feature-gated (`--features pq`). ML-DSA-65 (FIPS 204) via `pqcrypto-mldsa`.
- **MSRV**: Rust 1.75
- **Test pattern**: Unit tests live `#[cfg(test)] mod tests` alongside source code. Integration tests not yet implemented.
- **Cluster deploy**: K3s on local nodes (nexus, forge, sentry, zephyr). Registry at `nexus:5000`. Images loaded via `docker save | ctr import`.
- **Casdoor** configurations live in `casdoor/` directory (OIDC alternative provider).
- **Workspace members**: root crate + 10 sub-crates under `crates/`.
- **CI**: `.github/workflows/ci.yml` — runs on push/PR to main/develop.
