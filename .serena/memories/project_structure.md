# Astral Key - Project Structure

## Root Directory Layout

```
astral-key/
├── src/                      # Source code
├── tests/                    # Integration tests
├── benches/                  # Benchmarks (planned)
├── migrations/               # SQL database migrations
├── docs/                     # Documentation
├── static/                   # Static assets
├── scripts/                  # Utility scripts
├── nix/                      # Nix configurations
├── .github/                  # GitHub workflows
├── .serena/                  # Serena workspace files
├── Cargo.toml                # Rust package manifest
├── Cargo.lock                # Dependency lock file
├── flake.nix                 # Nix flake configuration
├── shell.nix                 # Nix development shell
├── justfile                  # Just command recipes
├── docker-compose.yml        # Dev infrastructure
├── docker-compose.prod.yml   # Production infrastructure
├── Dockerfile                # Container image
├── nixos-module.nix          # NixOS service module
├── README.md                 # Project overview
├── STATUS.md                 # Implementation status
├── ROADMAP.md                # Implementation plan
├── TESTING.md                # Testing guide
├── CONTRIBUTING.md           # Contribution guidelines
├── ARCHITECTURE.md           # System design
└── LICENSE                   # MIT license
```

## Source Structure (`src/`)

```
src/
├── lib.rs                 # Library root, exports public API
├── main.rs                # Binary entry point, server setup
├── error.rs               # Error types (thiserror-based)
├── config.rs              # Configuration management
├── state.rs               # Application state (App struct)
│
├── api/                   # HTTP Layer
│   ├── mod.rs             # API module exports
│   ├── routes.rs          # Route definitions
│   ├── handlers/          # Request handlers
│   │   ├── mod.rs
│   │   ├── health.rs      # Health check endpoints
│   │   ├── web3.rs        # Web3/SIWE handlers
│   │   ├── fido2.rs       # FIDO2/WebAuthn handlers
│   │   ├── session.rs     # Session management handlers
│   │   └── user.rs        # User management handlers
│   └── middleware.rs      # Custom middleware (CORS, rate limiting)
│
├── auth/                  # Authentication Layer
│   ├── mod.rs             # Auth module exports
│   ├── jwt/               # JWT Authentication
│   │   ├── mod.rs
│   │   ├── claims.rs      # Custom JWT claims
│   │   ├── middleware.rs  # JWT validator extractor
│   │   └── service.rs     # Token generation/validation
│   ├── web3/              # Web3 Authentication
│   │   ├── mod.rs
│   │   ├── nonce.rs       # SIWE nonce generation/storage
│   │   └── siwe.rs        # Signature verification (ethers-rs)
│   └── fido2/             # FIDO2/WebAuthn
│       ├── mod.rs
│       ├── registration.rs  # Registration ceremony
│       ├── authentication.rs # Authentication ceremony
│       └── types.rs        # WebAuthn types
│
├── db/                    # Database Layer
│   ├── mod.rs
│   ├── pool.rs            # PostgreSQL connection pool
│   └── models/            # Database models
│       ├── mod.rs
│       ├── user.rs        # User CRUD operations
│       ├── web3.rs        # Web3 wallet operations
│       ├── fido2.rs       # FIDO2 credential operations
│       ├── session.rs     # JWT session operations
│       └── nonce.rs       # SIWE nonce operations
│
├── cache/                 # Redis Cache Layer
│   ├── mod.rs
│   ├── pool.rs            # Redis connection pool
│   └── operations.rs      # Cache operations (sessions, nonces, blacklist)
│
├── vaultwarden/           # Vaultwarden Integration
│   ├── mod.rs
│   └── client.rs          # Vaultwarden API client
│
└── utils/                 # Utilities
    ├── mod.rs
    └── crypto.rs          # Cryptographic helpers
```

## Database Schema (`migrations/`)

Key tables:
- `users` - User accounts (UUID primary key)
- `web3_wallets` - Multi-chain wallet addresses
- `fido2_credentials` - WebAuthn passkey storage
- `sessions` - JWT session management
- `siwe_nonces` - SIWE nonces with expiration

## API Routes

**Public Routes:**
- `GET /health` - Health check
- `GET /ready` - Readiness check (DB + Redis)
- `POST /api/v1/auth/web3/nonce` - Request SIWE nonce
- `POST /api/v1/auth/web3/verify` - Verify signature
- `POST /api/v1/auth/fido2/authenticate/options` - Get auth challenge
- `POST /api/v1/auth/fido2/authenticate/verify` - Verify assertion

**Protected Routes (JWT required):**
- `POST /api/v1/auth/fido2/register/options` - Get registration challenge
- `POST /api/v1/auth/fido2/register/verify` - Complete registration
- `GET /api/v1/auth/fido2/credentials` - List passkeys
- `DELETE /api/v1/auth/fido2/credentials/:id` - Delete passkey
- `GET /api/v1/sessions` - List active sessions
- `DELETE /api/v1/sessions/current` - Logout
- `POST /api/v1/sessions/refresh` - Refresh tokens
- `GET /api/v1/users/me` - Get current user

## Key Dependencies

**Core:**
- `tokio` - Async runtime
- `axum` - Web framework
- `tower` - Middleware

**Database:**
- `sqlx` - Database toolkit with compile-time checks
- `redis` - Redis client

**Auth:**
- `ethers` - Ethereum library (signature verification)
- `siwe` - Sign-In with Ethereum
- `webauthn-rs` - WebAuthn implementation
- `jsonwebtoken` - JWT handling

**Error handling:**
- `thiserror` - Error derives
- `anyhow` - Error context
