# Astral Key — Testing Guide

This document describes how to test the Astral Key authentication service.

---

## Prerequisites

- **Rust 1.75+** — [Install via rustup](https://rustup.rs)
- **Nix** (optional, for `nix develop` with pinned toolchain)
- No external services required (SQLite runs in-memory for tests)

---

## Running Tests

### Unit Tests (all modules)

```bash
# Run all unit tests
cargo test --lib

# With output
cargo test --lib -- --nocapture

# Run tests for a specific module
cargo test --lib jwt::
cargo test --lib web3::
cargo test --lib fido2::
cargo test --lib jit::
cargo test --lib keys::
cargo test --lib rate_limit::
cargo test --lib audit::
cargo test --lib capabilities::
```

### Integration Tests

Integration tests are planned but not yet implemented. Once added, they will
live in `tests/` and run with:

```bash
cargo test --test <test_file_name>
```

---

## Module Test Coverage

| Module | What's tested | File |
|--------|---------------|------|
| **JWT** | Token generation (access, refresh, pair), validation (access, refresh), token-kind cross-check, invalid token rejection, user ID extraction, short-secret rejection | `src/auth/jwt/mod.rs`, `src/auth/jwt/tests.rs` |
| **Web3 SIWE** | SIWE message parsing (domain, address, version, chain_id, nonce), chain ID mismatch validation | `src/auth/web3/siwe.rs` |
| **Web3 Nonce** | Nonce length (64 hex chars), SIWE message template generation | `src/auth/web3/nonce.rs` |
| **JIT Issuer** | Valid/invalid key hex, mint signature format (3-part token), self-verification of minted tokens, epoch increment, epoch embedded in token | `src/auth/jit/issuer.rs` |
| **JIT Verifier** | Valid token verification, expired token rejection, unknown issuer rejection, stale epoch rejection, revoked token rejection, malformed token rejection, dynamic key registration | `src/auth/jit/verifier.rs` |
| **JIT Scope** | Exact-match satisfaction, missing-scope rejection, admin wildcard, empty-required pass, empty-granted fail, valid scope grammar, invalid scope rejection | `src/auth/jit/scope.rs` |
| **JIT Epoch** | Initial value, increment, set, tombstone create/revoke, tombstone persistence across sessions, tombstone reload | `src/auth/jit/epoch.rs` |
| **Rate Limiter** | Burst allowance, refill over time, build_key with/without bearer token, build_key fallback to X-Real-IP, unknown IP, Retry-After header format | `src/api/middleware/rate_limit.rs` |
| **Audit** | AuditEvent JSON serialisation, client_ip from X-Forwarded-For, X-Real-IP, unknown, Forwarded-For preference over Real-IP | `src/api/middleware/audit.rs` |
| **API Key Hashing** | Key format (prefix `ak_prod_`), env prefix `ak_dev_`, valid hash verification, invalid key rejection, prefix extraction | `src/auth/keys/hashing.rs` |
| **Capabilities Registry** | Known scope validation (19 scopes), unknown scope rejection, round-trip completeness, namespace parse/display, namespace_of helper | `src/auth/capabilities/registry.rs` |
| **CORS** | Layer creation test | `src/api/middleware/cors.rs` |

---

## Manual Testing

### Health & Readiness

```bash
# Start the server (SQLite database created automatically)
JWT_SECRET=$(openssl rand -hex 32) cargo run

# Health check
curl http://localhost:8080/health

# Readiness check (checks database connectivity)
curl http://localhost:8080/ready
```

### Web3 / SIWE Flow

```bash
# 1. Request a nonce
curl -X POST http://localhost:8080/api/v1/auth/web3/nonce \
  -H "Content-Type: application/json" \
  -d '{"domain": "maplespike.ca", "address": "0x742d35Cc6634C0532925a3b844Bc9e7595f2bD18", "chain_id": 1}'

# 2. Sign the SIWE message with your Ethereum wallet
# (Use the nonce and message_template from step 1)

# 3. Verify signature and get JWT tokens
curl -X POST http://localhost:8080/api/v1/auth/web3/verify \
  -H "Content-Type: application/json" \
  -d '{"message": "<signed message>", "signature": "<signature>", "chain_id": 1}'

# 4. Verify the JWT token
curl -X POST http://localhost:8080/api/v1/auth/verify \
  -H "Content-Type: application/json" \
  -d '{"token": "<access_token>"}'
```

### FIDO2 / Passkey Flow

```bash
# 1. Get authentication options
curl -X POST http://localhost:8080/api/v1/auth/fido2/authenticate/options \
  -H "Content-Type: application/json" \
  -d '{"username": "<user_uuid>"}'

# 2. Use the challenge with WebAuthn API (navigator.credentials.get())

# 3. Verify assertion and get JWT tokens
curl -X POST http://localhost:8080/api/v1/auth/fido2/authenticate/verify \
  -H "Content-Type: application/json" \
  -d '{"id": "<credential_id>", "raw_id": "<raw_credential_id>", "response": {...}, "type": "public-key"}'
```

### API Key Management

```bash
# Create an API key (requires JWT)
curl -X POST http://localhost:8080/api/v1/auth/keys \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer <access_token>" \
  -d '{"name": "My Key", "scopes": ["dns:read", "pages:deploy"], "environment": "prod"}'

# List keys
curl http://localhost:8080/api/v1/auth/keys \
  -H "Authorization: Bearer <access_token>"

# Revoke a key
curl -X POST http://localhost:8080/api/v1/auth/keys/:id/revoke \
  -H "Authorization: Bearer <access_token>"
```

### Session Management

```bash
# Refresh tokens
curl -X POST http://localhost:8080/api/v1/auth/token/refresh \
  -H "Content-Type: application/json" \
  -d '{"refresh_token": "<refresh_token>"}'

# List active sessions
curl http://localhost:8080/api/v1/auth/sessions \
  -H "Authorization: Bearer <access_token>"

# Revoke a session
curl -X DELETE http://localhost:8080/api/v1/auth/sessions/:id \
  -H "Authorization: Bearer <access_token>"
```

### Ed25519 Identity & Signatures

```bash
# Create an identity (stores public key only)
curl -X POST http://localhost:8080/api/v1/identity \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer <access_token>" \
  -d '{"pubkey": "<base64url-32-byte-ed25519-pubkey>", "label": "My Key"}'

# Verify a signature (public endpoint)
curl -X POST http://localhost:8080/api/v1/identity/verify \
  -H "Content-Type: application/json" \
  -d '{"data": {"msg": "hello"}, "signature": "<base64url-sig>", "pubkey": "<base64url-pubkey>"}'

# Generate a QR code for a public key
curl http://localhost:8080/api/v1/identity/qr/<base64url-pubkey>?format=svg
```

---

## CI/CD

Tests run automatically via GitHub Actions on:

- Every pull request
- Push to `main` or `master`

See `.github/workflows/ci.yml` for the CI configuration.

---

## Troubleshooting

### Tests fail with build errors

Make sure your Rust toolchain is up to date:

```bash
rustup update stable
```

### Tests hang or time out

Some asynchronous tests may time out under heavy load. Increase the test
timeout:

```bash
cargo test -- --test-threads=1
```

### Database-related test issues

SQLite tests run in-memory (no file I/O). If you see SQLite errors, check
that the `sqlx` crate feature `sqlite` is enabled in `Cargo.toml`.

---

## Continuous Testing

```bash
# Install cargo-watch
cargo install cargo-watch

# Watch mode — re-run tests on every save
cargo watch -x 'test --lib'

# Watch a specific module
cargo watch -x 'test --lib jwt::'
```
