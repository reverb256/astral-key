# Astral Key — Roadmap

**Last updated:** 2026-07-16

---

## Current Status

> **Last updated:** 2026-07-27. This is a live roadmap — checkmarks reflect
> actual completion against GitHub issues and source code.

Astral Key has completed a major v2 upgrade that added API key management, ZK
JIT capability tokens, session management with refresh token rotation, Ed25519
identity/contacts, rate limiting, audit logging, and an MCP server — all while
keeping the architecture **SQLite-only** (no PostgreSQL, no Redis, no external
cache).

The project is past the "foundation" stage. Most auth primitives are
implemented. The focus now is polish, hardening, and production deployment.

---

## Legend

| Symbol | Meaning |
|--------|---------|
| ✅ | Done |
| 🔄 | In progress |
| ⬜ | Not started |

---

## 1. Build & Compilation (Phase 4 Completion)

| Task | Status |
|------|--------|
| ✅ Build fix: missing functions, env vars, webauthn API | ✅ |
| ✅ Unwrap kill: zero `unwrap()` calls in production code | ✅ |
| ✅ Error codes: `AuthError` enum with specific variants | ✅ |
| ✅ MCP server: rate limit + audit + MCP tools compile check | ✅ |
| ✅ CI runs `cargo clippy` on push/PR (`.github/workflows/ci.yml`) | ✅ |
| ⬜ Build release binary (`cargo build --release`) | ⬜ |

---

## 2. Testing

| Task | Status |
|------|--------|
| ✅ Unit tests: JWT, Web3 SIWE, FIDO2 types | ✅ |
| ✅ Unit tests: rate limiter, audit middleware | ✅ |
| ✅ Unit tests: JIT issuer, verifier, scope, epoch | ✅ |
| ✅ Unit tests: API key hashing, capabilities registry | ✅ |
| ✅ Unit tests: SIWE message parsing, nonce generation | ✅ |
| ⬜ Integration tests: API endpoint suite | ⬜ |
| ⬜ Integration tests: FIDO2 ceremony with mock authenticator | ⬜ |
| ⬜ E2E test script: automate full auth flows | ⬜ |
| ⬜ Coverage analysis (cargo-tarpaulin) | ⬜ |
| ✅ CI pipeline (lint, test, audit, Docker build) | ✅ |

---

## 3. Deployment & Operations

| Task | Status |
|------|--------|
| ✅ Docker Compose (single-service, SQLite volume) | ✅ |
| ✅ Containerfile (multi-stage build) | ✅ |
| ✅ K3s manifests (deployment, service, PVC) | ✅ |
| ✅ sops-encrypted JIT issuer key | ✅ |
| ✅ Audit logging (JSON stdout) | ✅ |
| ✅ Rate limiting middleware | ✅ |
| ⬜ Deploy to K3s cluster | ⬜ |
| ⬜ Set up monitoring (health/ready probes configured) | ⬜ |
| ⬜ Set up structured log shipping (stdout JSON → Vector/Filebeat) | ⬜ |
| ⬜ Database backup procedure (SQLite `.db` file) | ⬜ |
| ⬜ NixOS module (declarative service config) | ⬜ |

---

## 4. Security Hardening

| Task | Status |
|------|--------|
| ✅ SIWE domain spoofing protection | ✅ |
| ✅ Nonce one-time consumption | ✅ |
| ✅ JWT refresh token rotation | ✅ |
| ✅ TokenKind validation (access ≠ refresh) | ✅ |
| ✅ Argon2id API key hashing | ✅ |
| ✅ Rate limiting middleware | ✅ |
| ✅ Audit logging (JSON stdout) | ✅ |
| ✅ Epoch-based batch revocation | ✅ |
| ⬜ JWKS endpoint for public key distribution | ⬜ |
| ⬜ Input sanitization middleware | ⬜ |
| ⬜ Security headers (CSP, X-Frame-Options) | ⬜ |
| ⬜ `cargo audit` integration | ⬜ |
| ⬜ Penetration testing | ⬜ |

---

## 5. Feature Backlog

| Feature | Status | Priority |
|---------|--------|----------|
| OAuth (GitHub) provider | 🟡 Skeleton exists (`src/api/handlers/oauth.rs`) | Medium |
| MCP server (`features = ["mcp"]`) | 🟢 Feature-gated, tools defined | Medium |
| WebSocket API for real-time events | ⬜ | Low |
| OpenTelemetry / Prometheus metrics | ⬜ | Low |
| Casdoor integration | ⬜ | Low |
| Custom PAM module | ⬜ | Low |

---

## 6. Governance & Community

| Task | Status |
|------|--------|
| ✅ MIT License | ✅ |
| ✅ README with quick start | ✅ |
| ✅ CONTRIBUTING.md | ✅ |
| ✅ GitHub issue templates | ✅ |
| ✅ CODEOWNERS | ✅ |
| ⬜ Dependabot auto-merge configuration | ⬜ |
| ⬜ Release workflow (tag → publish container) | ⬜ |
| ⬜ CHANGELOG.md | ⬜ |

---

## Timeline

There is no fixed weekly schedule. Priorities, in order:

1. **Fix the build** — ensure `cargo check && cargo test --lib` passes cleanly
2. **Clippy-clean** — address all warnings
3. **Integration tests** — cover the full API surface
4. **Production deploy** — K3s with sops key, TLS, monitoring
5. **Feature backlog** — OAuth polish, MCP server enablement

---

## Related Documents

- [`IMPLEMENTATION_SUMMARY.md`](IMPLEMENTATION_SUMMARY.md) — detailed module-by-module status
- [`docs/architecture.md`](docs/architecture.md) — module layout and auth flows
- [`docs/deployment.md`](docs/deployment.md) — deployment options and env vars
- [`HEY.md`](HEY.md) — v2 upgrade cross-agent coordination log (archived)
