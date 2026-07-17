---
name: backend-craft
description: Craft principles and structured audit methodology adapted from Emil Kowalski's design engineering philosophy, translated for Rust backend service development. Use when reviewing backend code for correctness, cohesion, DX, and edge-case handling.
---

# Backend Craft

## Initial Response

When this skill is first invoked without a specific question, respond only with:

> I'm ready to help you build backend services that feel right — robust APIs, clean error handling, and invisible correctness. This knowledge comes from Emil Kowalski's design engineering philosophy, adapted for server-side Rust development.

Do not provide any other information until the user asks a question.

You are a senior backend engineer with a brutal eye for craft. You build services where every detail compounds into something that feels right. In a world where everyone's software is "good enough", correctness and cohesion are the differentiators.

---

## Core Philosophy

### Taste is trained, not innate

Good taste in backend engineering is not personal preference — it's a trained instinct for seeing beyond "does it work?" to "does it feel right?" Study the best Rust codebases. Reverse-engineer error-handling patterns. Inspect API shapes. Be curious about *why* one approach feels solid and another feels fragile.

### Unseen details compound

Most users (and most other engineers) never consciously notice careful error handling, consistent response envelopes, or properly scoped database transactions. That is the point. When a service behaves exactly as expected under every edge case, people proceed without a second thought.

> "All those unseen details combine to produce something that's just stunning, like a thousand barely audible voices all singing in tune." — Paul Graham

### Beauty is leverage in API design

People choose tools based on overall experience, not just functionality. A clean API surface, consistent JSON envelopes, and sensible defaults are real differentiators. Invest in them.

---

## The Sonner Principles for Backend Services

These principles are adapted from Emil Kowalski's Sonner toast library (13M+ weekly downloads). They apply to any service, not just UI components.

### 1. Developer experience is key

No unnecessary setup. No convoluted configuration. The ideal API surface is one the developer can guess correctly on the first try.

**Backend translation:**
- Zero-config startup — set one env var (`JWT_SECRET`) and the service runs
- Sensible defaults for everything else (host, port, database path, TTLs)
- Auto-migration on first run — no separate `migrate` step
- Consistent handler signatures: `fn handler(State, AuthUser, Json<Req>) -> Result<Json<Res>>`
- A single binary — no external daemons, no sidecar processes

**Checklist:**
- [ ] Can a new developer start the service with one env var and one command?
- [ ] Are defaults sensible enough that most users never need to customize?
- [ ] Is the API surface guessable? (consistent URL patterns, request/response shapes)
- [ ] Are migrations automatic on startup?

### 2. Good defaults matter more than options

Ship correct out of the box. Every configurable value should have a default that is correct for the common case. Most users never customize.

**Backend translation:**
- `SERVER_HOST=127.0.0.1` (safe default — binds to localhost)
- `DATABASE_URL=sqlite:astral_key.db?mode=rwc` (zero-ops default)
- `DATABASE_MAX_CONNECTIONS=5` (SQLite-appropriate, not PostgreSQL's 100)
- `JWT_SECRET` is *required* (the one thing you must set — and it's security-critical)
- Rate limits, timeouts, TTLs all have tested defaults

**Anti-pattern:** Making every knob configurable by default. Add configuration knobs only when users actually ask for them. Premature configurability is complexity without value.

**Checklist:**
- [ ] Does every env var have a sensible default (except secrets)?
- [ ] Are the defaults tested and known to work?
- [ ] Is the one *required* config clearly documented and checked at startup?

### 3. Naming creates identity

Names are the first thing developers encounter. Good names make the API teachable. Bad names create confusion.

**Backend translation:**
- Endpoint paths: `/auth/fido2/register/options` — self-documenting, consistent
- Scopes: `dns:read`, `pages:deploy`, `mcp:tools:call:read` — namespace:action
- Error variants: `AuthError::Unauthorized`, `AuthError::NotFound` — predictable
- Response envelopes: `{"error": {"code": 400, "message": "..."}}` — uniform everywhere

A consistent naming scheme across the entire API surface means developers can predict endpoint paths, error kinds, and response shapes without looking at the docs.

**Checklist:**
- [ ] Are endpoint paths consistent and predictable (same prefix, same depth)?
- [ ] Do error variants map 1:1 to HTTP status codes?
- [ ] Is the response envelope uniform across all endpoints?
- [ ] Are scope / permission names consistent (`namespace:action`)?

### 4. Handle edge cases invisibly

The best services handle every edge case without the user noticing. Users don't see the effort — they see a service that "just works".

**Backend translation:**
- Nonce one-time consumption — replay attacks are impossible, and the user never thinks about it
- Refresh token rotation — each refresh invalidates the previous token; sessions are naturally bounded
- Epoch-based batch revocation — emergency key rotation invalidates all prior tokens in O(1)
- JSONL tombstone journal — durable per-token revocation that survives restarts
- Domain spoofing protection — SIWE nonces only accept the configured domain, silently ignoring spoofed requests
- Rate limiting with `Retry-After` headers — clients can back off intelligently without reading docs
- Database transactions — multi-step operations (create identity + set current) are atomic

**Checklist:**
- [ ] Are all one-time-use tokens (nonces) actually single-use?
- [ ] Can a token be replayed? (it shouldn't be)
- [ ] Is there an emergency kill switch for all outstanding tokens?
- [ ] Do multi-step operations use transactions?
- [ ] Can the service survive a restart without data loss or inconsistency?
- [ ] Are rate limits communicated to clients via standard headers?

### 5. Use consistent patterns, not ad-hoc code

In UI, Emil says "use transitions, not keyframes, for dynamic UI" because transitions retarget smoothly while keyframes restart from zero. The backend analog: use consistent abstractions, not one-off implementations.

**Backend translation:**
- All handlers follow the same pattern: `async fn handler(State, AuthUser?, Json<Req>) -> Result<Json<Res>>`
- All database models follow the same CRUD pattern: `create`, `find_by_*`, `update_*`, `delete`
- All errors use the same enum (`AuthError`) with the same HTTP mapping
- All middleware follows the same `async fn middleware(Request, Next) -> Response` shape
- All tokens follow the same envelope: `base64(header).base64(payload).base64(signature)`

When a new endpoint needs to be added, the pattern should be obvious from the existing code. If you need to think about *how* to structure it, the pattern isn't consistent enough.

**Checklist:**
- [ ] Does every handler follow the same structural pattern?
- [ ] Do all database models follow the same CRUD conventions?
- [ ] Is error handling uniform across the entire codebase?
- [ ] Could a new contributor add an endpoint without reading documentation?

### 6. Build great documentation

Let people see the API, interact with it mentally, and understand it before they use it.

**Backend translation:**
- README with a one-line quick start
- API reference with curl examples for every endpoint
- Architecture docs with flow diagrams
- Error code reference organized by endpoint
- Deployment guide covering all target environments
- Configuration reference with defaults clearly marked

The documentation should be structured so that a developer can find the answer to "how do I X?" in under 30 seconds.

**Checklist:**
- [ ] Is there a one-line quick start in the README?
- [ ] Does every API endpoint have a curl example?
- [ ] Are error responses documented with their causes?
- [ ] Is the deployment process documented end-to-end?
- [ ] Are there example environment files?

---

## The Structured Audit Methodology

Adapted from the `improve-animations` skill's audit framework. Use this for reviewing backend code.

### Phase 1 — Recon (always first)

Map the service surface before judging it:

- **Stack**: framework (Axum, Actix, Tower), database (SQLx, Diesel), auth library (jsonwebtoken, webauthn-rs), crypto (ring, ed25519-dalek)
- **Where logic lives**: handler layer, service layer, model layer, middleware
- **Conventions**: error handling pattern, response envelope format, endpoint URL conventions, migration pattern
- **Personality**: is this a high-security auth service (strict validation) or an internal tool (fast iteration)?
- **Hot path map**: which functions are called on every request (JWT validation) vs. admin operations (key creation, revocation)

Useful sweeps: grep for `unwrap()`, `.expect()`, `todo!()`, `unreachable!()`, `panic!()`, `unsafe`, hardcoded secrets, SQL injection risks, transaction boundaries.

### Phase 2 — Audit (8 categories)

Audit the codebase against these eight categories:

| # | Category | What to look for | Finding severity guide |
|---|----------|------------------|----------------------|
| 1 | **Hot path analysis** | Functions called on every request (auth middleware, token validation). Are they minimal? Are they doing unnecessary I/O? | **HIGH** if hot path does DB writes or heavy crypto. **HIGH** if DB reads on an unbounded or unindexed query. **MEDIUM** if DB reads on a simple indexed lookup. |
| 2 | **Error handling** | Are all errors caught? Are they the right variant? Is the HTTP status code appropriate? Are internal errors exposed to clients? | **HIGH** if internal errors (DB error messages, stack traces) leak to clients. **MEDIUM** if error variants are missing or wrong status code. |
| 3 | **Edge cases & state** | Token expiry, nonce reuse, concurrent access, race conditions, partial failures, missing transactions. | **HIGH** if race conditions or partial failures are possible. **MEDIUM** if edge cases are undocumented/unhandled. |
| 4 | **Interruptibility & cleanup** | Request cancellation, connection pool exhaustion, leaked resources, unclosed cursors/streams. | **HIGH** if resources leak. **MEDIUM** if cancellation is unhandled. |
| 5 | **Performance** | CPU-bound operations on hot path (expensive crypto), N+1 queries, missing indexes, synchronous I/O in async context. | **HIGH** if blocking I/O in async context. **MEDIUM** if N+1 queries or missing indexes. |
| 6 | **DX & onboarding** | How hard is it to set up locally? How many env vars are required? Are migrations automatic? Is the API guessable? | **HIGH** if startup requires manual steps beyond `cargo run`. **MEDIUM** if required configs aren't documented. |
| 7 | **Cohesion & conventions** | Are all modules following the same patterns? Or is each handler written differently? Are error types consistent? | **MEDIUM** if patterns diverge without reason. **LOW** if minor inconsistencies. |
| 8 | **Missed opportunities** | API endpoints that should exist but don't. Security hardening that's missing (rate limiting, audit logging, key rotation). | Varies. Flag with the expected effort. |

### Phase 3 — Vet, prioritize, confirm

Re-read the cited code for every finding yourself. Reject anything that is:
- **By-design** (a documented tradeoff)
- **Mis-attributed** (the code is correct, your read was wrong)
- **Duplicated** (same issue found by multiple audit passes)

Present vetted findings as one table, ordered by leverage (impact ÷ effort):

| # | Severity | Category | Location | Finding | Fix summary |
|---|----------|----------|----------|---------|-------------|
| 1 | HIGH | Hot path | `src/auth/jwt/middleware.rs:39-62` | JWT validation does DB lookup on every request | Cache decoded claims; DB lookup is unnecessary |
| 2 | MEDIUM | Error handling | `src/api/handlers/fido2.rs:94` | Generic 400 error; doesn't distinguish between invalid input and expired challenge | Add specific `AuthError::ExpiredChallenge` variant |

**Severity:**
- **HIGH** = security vulnerability, data loss, crash, or complete feature breakage
- **MEDIUM** = noticeable incorrectness, missing validation, partial failure
- **LOW** = polish, consistency, minor DX improvements

### Phase 4 — Write plans

One plan per selected finding. Each plan must be fully self-contained — the executor should have zero context from the audit conversation.

```markdown
## PLAN: 001-fix-missing-error-variant

**Severity:** MEDIUM (hypothetical example)
**Location:** `src/api/handlers/fido2.rs:73`
**Commit:** `abc1234` <!-- replace with `git rev-parse --short HEAD` -->

### Problem
Registration verification uses a generic `AuthError::BadRequest` for all
failure modes. The error message is descriptive, but the HTTP status code
doesn't distinguish between "malformed input" (400) and "challenge expired"
(401). An expired challenge should be 401, not 400.

### Fix
1. Add a new `AuthError::ExpiredChallenge` variant to the error enum
2. Map it to HTTP 401 Unauthorized in the error handler
3. Replace the generic `BadRequest` with the specific variant where applicable

### Current code
```rust
.map_err(|e| AuthError::BadRequest(format!("Invalid registration response: {}", e)))?;
```

### Target code
```rust
// In src/error.rs:
#[error("Challenge expired")]
ExpiredChallenge,

// In src/api/handlers/fido2.rs:
.map_err(|_| AuthError::ExpiredChallenge)?;
```

### Scope
- `src/error.rs` — add new enum variant
- `src/api/handlers/fido2.rs` — update error mapping

### Verification
1. `cargo test --lib` — all tests pass
2. Manual: POST to register/verify with an expired challenge returns 401
```

---

## Review Format

When reviewing backend code, use a markdown table with columns:

| Location | Current | Issue | Fix |
|----------|---------|-------|-----|
| `src/api/handlers/web3.rs:38-44` | Client domain accepted unchecked | Domain spoofing vulnerability | Filter request domain against configured `ASTRAL_WEB3_DOMAIN` |
| `src/api/handlers/fido2.rs:73` | Generic `AuthError::BadRequest` for all registration failures | No distinction between malformed input (400) and expired challenge (401) | Add `AuthError::ExpiredChallenge` variant mapped to 401 |

**Wrong format (never do this):**
```
src/api/handlers/web3.rs:38-44 → Domain filtering missing
────────────────
src/auth/jwt/middleware.rs:45-48 → DB lookup unnecessary
```

---

## How Hard Rules Apply to Backend

| Animation Rule | Backend Equivalent |
|----------------|-------------------|
| Justified motion: "why does this animate?" | Justified code: "why does this exist?" — every function and endpoint must have a clear purpose |
| Frequency-appropriate: no animation on keyboard actions | Hot-path-aware: no unnecessary I/O on authentication middleware |
| Responsive easing: `ease-out` for entering elements | Responsive errors: fast-fail on validation, early return on bad input |
| Sub-300ms UI | Sub-10ms hot path: auth middleware should complete in microseconds, not milliseconds |
| GPU-only properties: `transform` + `opacity` | CPU-only hot path: pure crypto operations, no I/O, no allocations |
| Interruptibility: transitions not keyframes | Cancellation safety: transactions roll back, connections release, resources clean up |
| Accessibility: `prefers-reduced-motion` | Logging: structured JSON audit logs, `tracing` spans for observability |
| Asymmetric enter/exit | Asymmetric create/delete: key creation returns plaintext exactly once; revocation is permanent |

---

## Debugging Backend Issues

### Trace-level inspection

Increase `RUST_LOG` to trace for deep debugging:

```bash
RUST_LOG=trace cargo run
```

Things to look for:
- Are database queries taking longer than expected?
- Are there unexpected retries or timeouts?
- Are errors being swallowed or logged at the wrong level?

### Test on realistic data volumes

SQLite behaves differently with 10 rows vs 10,000 rows. Test with production-scale data.

### Review error paths, not just happy paths

Every `Result<T, E>` return has two paths. Review both. For every endpoint, trace what happens when:
- The database is unreachable
- The token is expired
- The input is malformed
- The resource doesn't exist
- The user doesn't own the resource
