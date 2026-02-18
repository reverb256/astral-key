# Astral Key Implementation Roadmap

This roadmap outlines the phased implementation plan for Astral Key from prototype to production-ready authentication service.

## Overview

**Current Status:** Prototype Stage (~5% complete)
**Target:** Production-ready Web3/FIDO2 authentication service
**Timeline:** 12 weeks for MVP completion

---

## Phase 1: Foundation (Week 1-2) ⚡ CRITICAL

**Goal:** Make the project buildable and testable with local infrastructure

### Tasks

#### Week 1: Infrastructure Files
- [x] Review project structure
- [ ] Create `flake.nix` with Rust toolchain via fenix
- [ ] Create `docker-compose.yml` with PostgreSQL, Redis, Vaultwarden
- [ ] Create `justfile` task runner (if not exists)
- [ ] Create initial database migration (`migrations/001_initial.sql`)
- [ ] Update README.md with honest status banner
- [ ] Create STATUS.md implementation tracker
- [ ] Create this ROADMAP.md

#### Week 2: Database Foundation
- [ ] Implement `src/db/pool.rs` (SQLx connection pool)
- [ ] Implement `src/db/models/user.rs` (User CRUD)
- [ ] Implement `src/db/models/web3.rs` (Web3Wallet model)
- [ ] Implement `src/db/models/fido2.rs` (Fido2Credential model)
- [ ] Implement `src/db/models/session.rs` (Session model)
- [ ] Implement `src/db/models/nonce.rs` (Nonce model with TTL)
- [ ] Update `src/state.rs` to include DbPool
- [ ] Test database connectivity with `just db-up && just migrate`

**Deliverable:**
```bash
nix develop          # ✅ Works - enters dev shell
just db-up           # ✅ Works - starts PostgreSQL + Redis
just migrate         # ✅ Works - runs migrations
just dev             # ✅ Works - server connects to database
```

---

## Phase 2: Cache & JWT (Week 3)

**Goal:** Fast session storage and token-based authentication

### Tasks

#### Cache Layer
- [ ] Implement `src/cache/pool.rs` (Redis connection manager)
- [ ] Implement `src/cache/operations.rs` (cache operations)
- [ ] Add cache health check to `/ready` endpoint
- [ ] Write cache tests (connection, set/get/delete with TTL)

#### JWT Authentication
- [ ] Implement `src/auth/jwt/mod.rs` (JWT service struct)
- [ ] Implement `src/auth/jwt/claims.rs` (custom claims)
- [ ] Implement `src/auth/jwt/middleware.rs` (Axum JWT validator)
- [ ] Add JWT service to `AppState`
- [ ] Create JWT generation/verification tests
- [ ] Update API docs with JWT format

**Deliverable:**
```bash
cargo test jwt::tests           # ✅ Passes
curl http://localhost:8080/ready # ✅ Shows cache=true
```

---

## Phase 3: FIDO2/WebAuthn (Week 4-6) ⭐ PRIMARY FOCUS

**Goal:** Passwordless authentication (Priority Implementation)

### Week 4: FIDO2 Foundation
- [ ] Implement `src/auth/fido2/mod.rs` (FIDO2 service)
- [ ] Implement `src/auth/fido2/registration.rs` (registration ceremony)
- [ ] Implement `src/auth/fido2/authentication.rs` (authentication ceremony)
- [ ] Implement `src/auth/fido2/webauthn.rs` (WebAuthn wrapper)
- [ ] Update `src/state.rs` to include Fido2Service
- [ ] Write FIDO2 unit tests

### Week 5: FIDO2 Handlers
- [ ] Update `src/api/handlers/fido2.rs::register_options`
- [ ] Update `src/api/handlers/fido2.rs::register_verify`
- [ ] Update `src/api/handlers/fido2.rs::authenticate_options`
- [ ] Update `src/api/handlers/fido2.rs::authenticate_verify`
- [ ] Update `src/api/handlers/fido2.rs::credentials` (list)
- [ ] Update `src/api/handlers/fido2.rs::delete_credential`
- [ ] Add FIDO2 integration tests with mock authenticator

### Week 6: FIDO2 Polish
- [ ] Test with real hardware authenticators
- [ ] Test with platform authenticators (Touch ID, Windows Hello)
- [ ] Add credential backup/restore functionality
- [ ] Update ARCHITECTURE.md with FIDO2 implementation details
- [ ] Create end-to-end FIDO2 example in docs

**Why FIDO2 First:**
- More modern and secure than Web3
- Better user experience (passwordless)
- webauthn-rs library is well-maintained
- Simpler integration (no blockchain RPC needed)

**Deliverable:**
```bash
cargo test fido2::tests           # ✅ Passes
# Manual test with WebAuthn simulator works
```

---

## Phase 4: Web3 Authentication (Week 6-7)

**Goal:** SIWE signature verification for Web3 wallets

### Tasks

#### Week 6: Web3 Foundation
- [ ] Implement `src/auth/web3/siwe.rs` (SIWE message parsing)
- [ ] Implement `src/auth/web3/verifier.rs` (signature verification)
- [ ] Implement `src/auth/web3/nonce.rs` (nonce generation/storage)
- [ ] Implement `src/auth/web3/provider.rs` (Ethereum RPC provider)
- [ ] Add Web3 service to `AppState`
- [ ] Write SIWE verification tests

#### Week 7: Web3 Handlers
- [ ] Update `src/api/handlers/web3.rs::nonce` (real nonce generation)
- [ ] Update `src/api/handlers/web3.rs::verify` (SIWE verification)
- [ ] Update `src/api/handlers/web3.rs::chains` (configured chains)
- [ ] Add Web3 integration tests
- [ ] Test with mainnet and testnet signatures
- [ ] Update docs with Web3 flow diagram

**Deliverable:**
```bash
cargo test web3::tests            # ✅ Passes
# SIWE verification works with real wallet signatures
```

---

## Phase 5: Session Management (Week 7)

**Goal:** Complete authentication lifecycle

### Tasks
- [ ] Update `src/api/handlers/session.rs::refresh` (JWT rotation)
- [ ] Update `src/api/handlers/session.rs::logout` (token invalidation)
- [ ] Update `src/api/handlers/session.rs::list` (user sessions)
- [ ] Update `src/api/handlers/session.rs::delete` (session deletion)
- [ ] Implement refresh token rotation
- [ ] Implement logout/token blacklisting
- [ ] Add session storage in Redis
- [ ] Write session management tests

**Deliverable:**
```bash
cargo test session::tests         # ✅ Passes
# Full auth flow works: nonce → verify → token → refresh → logout
```

---

## Phase 6: Middleware & Security (Week 8)

**Goal:** Protected routes and rate limiting

### Tasks

#### Auth Middleware
- [ ] Implement `src/api/middleware/auth.rs` (JWT validation)
- [ ] Add auth middleware to protected routes
- [ ] Add CORS configuration
- [ ] Add request tracing with OpenTelemetry

#### Rate Limiting
- [ ] Implement `src/api/middleware/rate_limit.rs` (governor)
- [ ] Configure rate limits per endpoint
- [ ] Add rate limit headers to responses
- [ ] Test rate limiting behavior

#### Security
- [ ] Add input validation middleware
- [ ] Add CSRF protection
- [ ] Add security headers (CSP, X-Frame-Options, etc.)
- [ ] Write security tests

**Deliverable:**
```bash
cargo test middleware::tests      # ✅ Passes
curl -H "Authorization: Bearer invalid" http://localhost:8080/api/v1/users/me
# Returns 401 Unauthorized ✅
```

---

## Phase 7: Testing (Week 9-10)

**Goal:** Comprehensive test coverage (>80%)

### Tasks

#### Unit Tests
- [ ] Add unit tests for all auth modules
- [ ] Add unit tests for database models
- [ ] Add unit tests for cache operations
- [ ] Add unit tests for middleware

#### Integration Tests
- [ ] Add API endpoint integration tests
- [ ] Add Web3 signature verification tests
- [ ] Add FIDO2 ceremony tests with virtual authenticator
- [ ] Add database migration rollback tests

#### Security Tests
- [ ] SQL injection tests
- [ ] XSS tests
- [ ] CSRF tests
- [ ] Rate limiting bypass tests
- [ ] JWT manipulation tests

#### Performance
- [ ] Benchmark auth operations
- [ ] Benchmark database queries
- [ ] Load test API endpoints
- [ ] Optimize bottlenecks

**Deliverable:**
```bash
just test-coverage               # ✅ >80% coverage
just audit                       # ✅ No vulnerabilities
cargo bench                      # ✅ Performance baselines established
```

---

## Phase 8: Production Readiness (Week 11-12)

**Goal:** Deploy to production

### Tasks

#### Deployment
- [ ] Create NixOS module (`nix/nixos-module.nix`)
- [ ] Create container image Dockerfile
- [ ] Add SSL/TLS configuration
- [ ] Configure systemd service
- [ ] Setup monitoring and alerting

#### Operations
- [ ] Configure proper logging (JSON format)
- [ ] Setup Prometheus metrics
- [ ] Setup OpenTelemetry tracing
- [ ] Create backup/restore procedures
- [ ] Create runbook for common issues

#### Documentation
- [ ] Update README.md with production deployment
- [ ] Update deployment guide
- [ ] Create troubleshooting guide
- [ ] Update API documentation
- [ ] Create migration guide

#### Security Audit
- [ ] Run `cargo audit`
- [ ] Run dependency review
- [ ] Review security configurations
- [ ] Penetration testing
- [ ] Fix any security issues

**Deliverable:**
```bash
# Production deployment works
nix build .#container              # ✅ Container image builds
nixos-rebuild switch               # ✅ Service starts successfully
curl https://auth.example.com/health  # ✅ Returns healthy
```

---

## Risk Assessment

### High Risk Areas

| Area | Risk | Mitigation |
|------|------|------------|
| **Web3 Signatures** | Complex crypto, edge cases | Extensive unit tests, integration tests with real providers |
| **FIDO2** | WebAuthn is complex | webauthn-rs library, virtual authenticator tests |
| **Session Security** | JWT handling must be secure | Security audit, constant-time comparisons |
| **Race Conditions** | Concurrent nonce/session usage | Database transactions, Redis transactions |
| **Performance** | High load scenarios | Load testing, optimization, caching |

### Dependencies

- **ethers** - Web3 provider (well-maintained)
- **siwe** - SIWE message parsing (active)
- **webauthn-rs** - WebAuthn implementation (active)
- **jsonwebtoken** - JWT handling (mature)
- **sqlx** - Database (compile-time checked)

---

## Success Criteria

### MVP (Minimum Viable Product)
- [x] Server starts and responds to health checks
- [ ] FIDO2 registration and authentication works
- [ ] Web3 SIWE authentication works
- [ ] JWT token generation and validation works
- [ ] Session management works
- [ ] Database persistence works
- [ ] Redis caching works
- [ ] Test coverage > 80%

### Production Ready
- [ ] All MVP criteria
- [ ] Security audit passed
- [ ] Performance benchmarks met
- [ ] Documentation complete
- [ ] Deployment automation
- [ ] Monitoring and alerting
- [ ] Backup/restore procedures

---

## Getting Involved

We welcome contributors! See [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

### Priority Areas for Contribution

1. **FIDO2/WebAuthn Implementation** (Week 4-6) - Primary focus
2. **Web3 SIWE Implementation** (Week 6-7)
3. **Testing** (Week 9-10)
4. **Documentation** (Ongoing)

### Ways to Help

- Pick up any task marked with `[ ]` in this roadmap
- Report bugs via GitHub issues
- Suggest improvements via pull requests
- Write documentation
- Add tests

---

## Timeline Summary

| Phase | Duration | Focus |
|-------|----------|-------|
| Phase 1 | Week 1-2 | Infrastructure (flake.nix, docker-compose, migrations) |
| Phase 2 | Week 3 | Cache & JWT |
| Phase 3 | Week 4-6 | FIDO2/WebAuthn ⭐ |
| Phase 4 | Week 6-7 | Web3 SIWE |
| Phase 5 | Week 7 | Session Management |
| Phase 6 | Week 8 | Middleware & Security |
| Phase 7 | Week 9-10 | Testing |
| Phase 8 | Week 11-12 | Production |

**Total Timeline:** 12 weeks to production-ready MVP

---

*Last Updated: 2026-02-18*
*For current status, see [STATUS.md](STATUS.md)*
