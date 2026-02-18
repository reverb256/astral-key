# PROJECT KNOWLEDGE BASE

**Generated:** 2026-02-18
**Updated:** 2026-02-18
**Type:** Rust microservice
**Status:** ~60% Complete - Foundation implemented, testing in progress

## OVERVIEW
Astral Key - Microservice for Web3, FIDO2, and Passkey authentication with Vaultwarden backend. Built with Axum and NixOS.

## STRUCTURE
```
astral-key/
├── src/
│   ├── api/              # HTTP layer (handlers, middleware, routes)
│   │   ├── handlers/     # Request handlers (health, web3, fido2, session, user)
│   │   └── middleware/   # Axum middleware (auth, cors, rate_limit, tracing)
│   ├── auth/             # Auth implementations (web3, fido2, jwt)
│   │   ├── jwt/          # JWT token generation/validation/middleware ✅
│   │   ├── fido2/        # WebAuthn registration/authentication ✅
│   │   └── web3/         # SIWE signature verification ✅
│   ├── db/               # PostgreSQL connection & models
│   │   ├── pool.rs       # SQLx connection pool ✅
│   │   └── models/       # Database models ✅
│   ├── cache/            # Redis caching layer
│   │   ├── pool.rs       # Redis connection manager ✅
│   │   └── operations.rs # Cache operations ✅
│   ├── config.rs         # Configuration from env/TOML ✅
│   ├── error.rs          # Error types ✅
│   └── state.rs          # Application state with db/cache/jwt ✅
├── migrations/           # SQLx database migrations ✅
│   └── 001_initial.sql   # Initial schema ✅
├── nix/                 # NixOS modules & flake config
├── docker-compose.yml   # Local development environment ✅
├── flake.nix            # Nix dev shell ✅
├── justfile             # Just command runner
├── docs/                # Documentation
├── STATUS.md            # Implementation status ✅
├── ROADMAP.md           # Implementation roadmap ✅
└── CONTRIBUTING.md      # Contribution guidelines ✅
```

## WHERE TO LOOK
| Task | Location | Status |
|------|----------|--------|
| **Entry point** | src/main.rs | ✅ Complete |
| **Auth flow** | src/auth/ | ✅ Implemented |
| **API handlers** | src/api/handlers/ | 🔨 Needs wiring to auth modules |
| **Config** | src/config.rs | ✅ Complete |
| **Database** | src/db/ | ✅ Complete |
| **Migrations** | migrations/001_initial.sql | ✅ Complete |
| **State** | src/state.rs | ✅ Complete |

## CONVENTIONS
- **Async-first**: All code async with Tokio runtime
- **Type safety**: Rust 2021 edition, strict types
- **Error handling**: `thiserror` + `anyhow` pattern
- **Config**: Environment variables via `config` crate
- **Testing**: Unit tests in source, integration in `tests/`
- **Nix native**: Dev environment via `nix develop`

## ANTI-PATTERNS (THIS PROJECT)
- **NO** blocking operations in async context
- **NEVER** use `unwrap()` outside tests (use `?` operator)
- **NO** hardcoded secrets (use env vars)
- **NEVER** commit SQLx queries without migration
- **NO** direct database access without pool
- **NEVER** skip JWT validation in auth routes

## UNIQUE STYLES
- **Multi-chain Web3**: Ethereum, Polygon, Arbitrum, Optimism, Solana
- **FIDO2/WebAuthn**: Platform + roaming authenticator support
- **Vaultwarden backend**: Secure credential storage (stub)
- **Redis caching**: Session and token caching
- **NixOS modules**: Declarative deployment (planned)

## COMMANDS
```bash
just dev              # Development with hot reload (cargo watch)
just test             # Run all tests
just test-coverage    # Coverage report (tarpaulin)
just db-up            # Start PostgreSQL + Redis (docker-compose)
just migrate          # Run SQLx migrations
just migrate-new <name>  # Create migration
just fmt              # Format code (cargo fmt + nixpkgs-fmt)
just lint             # Run clippy and other linters
just build            # Production binary
just container        # Build Nix container image
just shell            # Enter Nix dev environment
just audit            # Security audit (cargo deny)
```

## NOTES
- Rust 1.75+ required
- Uses `sqlx` for compile-time checked queries
- `just` is the command runner (alternative to make)
- Nix flake for reproducible builds
- Postgres + Redis required for development
- OpenTelemetry + Prometheus metrics (planned)
- Vaultwarden runs as separate service

## CURRENT IMPLEMENTATION STATUS

### ✅ Completed (~60%)
- Database connection pool with SQLx
- Redis connection pool
- JWT token generation and validation
- FIDO2/WebAuthn ceremony logic
- Web3 SIWE nonce generation
- Session management with token rotation
- All middleware (auth, CORS, rate limiting, tracing)
- Nix flake configuration
- Docker Compose for local services
- Initial database migration
- All documentation updates

### 🔨 In Progress (~25%)
- Wiring auth modules to API handlers
- Unit tests and integration tests
- FIDO2 real attestation verification
- Web3 real signature verification

### ❌ Not Started (~15%)
- NixOS module for production
- Container image
- CI/CD pipelines
- Vaultwarden integration
- Metrics/Tracing
- WebSocket API

## LINKS
- README: /data/@projects/astral-key/README.md
- ARCHITECTURE: /data/@projects/astral-key/ARCHITECTURE.md
- STATUS: /data/@projects/astral-key/STATUS.md
- ROADMAP: /data/@projects/astral-key/ROADMAP.md
- Repo: https://github.com/reverb256/astral-key

## LAST UPDATED
2026-02-18: Major implementation progress - database, cache, JWT, FIDO2, Web3 all implemented
