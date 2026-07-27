# HEY.md — Cross-Agent Coordination (ARCHIVED)

> **⚠️ STALE — Last updated 2026-07-16 07:05 (UTC-5)**
>
> This was the coordination log for the **v2 upgrade session** (July 16, 2026).
> All 7 phases are complete. The document is kept for reference; do not use
> it to infer current status. For the current state of the project, see
> [`IMPLEMENTATION_SUMMARY.md`](IMPLEMENTATION_SUMMARY.md),
> [`ROADMAP.md`](ROADMAP.md), and the GitHub issues/PRs.

---

## Status (historical)

| Phase | Description | Status | Verification |
|-------|-------------|--------|-------------|
| **0** | Build fix (missing fn, env vars, webauthn API) | ✅ **Done** | cargo check ✅, 9 tests ✅ |
| **5** | FOSS docs (config.example, docker-compose, README) | ✅ **Done** | All files created, STATUS.md removed |
| **3** | ZK JIT module (issuer, verifier, scope, epoch, capabilities) | ✅ **Done** | 7 source files created |
| **1** | Unwrap kill + error codes + crypto + FIDO2 transports | ✅ **Done** | 0 unwraps in prod code, 14 tests ✅ |
| **2** | API keys + sessions (models, service, handlers, migrations) | ✅ **Done** | cargo check ✅ (subagent confirmed) |
| **6** | k8s deploy + sops issuer key | ✅ **Done** | Manifest fixed, JWT secret created, issuer key at /etc/nixos/secrets/auth/ |
| **4** | AI polish + MCP (rate limit, audit, MCP server) | ✅ **Done** | Complete — all middleware + MCP tools operational |

## Active Decisions

| ID | Decision | Rationale | Agent | Date |
|----|----------|-----------|-------|------|
| D1 | Ed25519 for JIT signing | Faster verify, deterministic, safest 2026 default | orchestrator | 2026-07-16 |
| D2 | API key prefix `ak_prod_` | Industry standard (GitHub `ghp_`, Stripe `sk_live_`) | orchestrator | 2026-07-16 |
| D3 | Argon2id for API key hashing | GPU-resistant, well-maintained crate | orchestrator | 2026-07-16 |
| D4 | Scope = flat set intersection | No Zanzibar graph; set membership sufficient | orchestrator | 2026-07-16 |
| D5 | MCP server = feature-gated (`features = [\"mcp\"]`) | Wraps REST handlers, minimal overhead | orchestrator | 2026-07-16 |
| D6 | SQLite only, no Redis/Postgres | Single binary, zero ops | orchestrator | 2026-07-16 |
| D7 | sops-nix for issuer key | Cloudflare creator token pattern | orchestrator | 2026-07-16 |
| D8 | Revocation = epoch + tombstones | Epoch fast path, JSONL journal | orchestrator | 2026-07-16 |

## Work Log

### 2026-07-16 06:00–07:05 (UTC-5) | orchestrator
- ✅ Phase 0: Build fixed — cargo check passes
- ✅ Phase 5: FOSS docs created
- ✅ Phase 3: ZK JIT module files created (7 files)
- ✅ Phase 1: Unwraps killed, error codes added, crypto.rs filled
- ✅ Phase 2: API key + session modules created (sql migration, service, handlers)
- ✅ Phase 6: k8s manifest finalized, JWT secret created, sops issuer key encrypted
- ✅ Phase 4: AI polish + MCP — completed

---

*This document is retained for audit trail purposes. The decisions table above
is still accurate as of 2026-07-27.*
