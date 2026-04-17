# Astral Key - Development Commands

## Using `just` (Recommended)

The project uses a `justfile` for common development tasks.

### Development
```bash
just dev           # Start dev server with hot reload (cargo watch)
just run           # Start server without hot reload
just shell         # Enter Nix development shell
just update        # Update flake inputs
just flake-check   # Check flake validity
```

### Testing
```bash
just test            # Run all tests with all features
just test-coverage   # Run tests with tarpaulin coverage report
```

### Database
```bash
just db-up        # Start PostgreSQL and Redis via docker-compose
just db-down      # Stop database services
just migrate      # Run database migrations
just migrate-new <name>  # Create a new migration
```

### Code Quality
```bash
just fmt          # Format code (cargo fmt + nixpkgs-fmt)
just lint         # Run clippy with -D warnings + cargo audit
just audit        # Security audit (cargo audit + cargo deny)
just deps-check   # Check for outdated dependencies
```

### Building
```bash
just build        # Build production binary with release optimizations
just container    # Build container image via Nix
just clean        # Clean build artifacts
```

### Infrastructure
```bash
just vaultwarden-up    # Start Vaultwarden for local development
just logs <service>    # View logs for a service
```

### Documentation
```bash
just docs         # Generate and open rustdoc documentation
```

## Using `cargo` directly

```bash
# Building
cargo build                          # Debug build
cargo build --release               # Release build
cargo build --release --features production  # Production build

# Running
cargo run                           # Start server
cargo watch -x run                  # Start with hot reload

# Testing
cargo test                          # Run all tests
cargo test --lib                    # Unit tests only
cargo test --test <test_name>       # Specific integration test
cargo test -- --nocapture           # Show test output

# Formatting & Linting
cargo fmt                           # Format code
cargo clippy                        # Run linter
cargo clippy --all-features -- -D warnings  # Strict linting

# Documentation
cargo doc --no-deps                 # Generate documentation
cargo doc --no-deps --open          # Generate and open docs
```

## Using `nix`

```bash
nix develop              # Enter development environment
nix build                # Build default package
nix build .#container    # Build container image
nix flake update         # Update flake inputs
nix flake check          # Validate flake
```

## Docker Commands

```bash
# Start all services
docker-compose up -d

# Start specific services
docker-compose up -d postgres redis vaultwarden

# Check service status
docker-compose ps

# View logs
docker-compose logs -f astral-key
docker-compose logs -f postgres

# Stop services
docker-compose down
```

## Database Migrations (SQLx)

```bash
# Run migrations
sqlx migrate run --database-url postgresql://postgres:postgres@localhost/astral_key

# Create new migration
sqlx migrate add <migration_name> --database-url postgresql://postgres:postgres@localhost/astral_key

# Build migrations to Rust (compile-time check)
cargo build --features sqlx-macros
```

## Manual Testing

```bash
# Health check
curl http://localhost:8080/health

# Readiness check (includes DB and Redis)
curl http://localhost:8080/ready

# Request Web3 nonce
curl -X POST http://localhost:8080/api/v1/auth/web3/nonce \
  -H "Content-Type: application/json" \
  -d '{"domain": "localhost", "address": "0x742d35Cc6634C0532925a3b844Bc9e7595f0bEb", "chain_id": 1}'
```

## System Utilities (Linux/NixOS)

The project runs on Linux. Common utilities:
- `ls`, `cd`, `grep`, `find` - Standard file operations
- `git` - Version control
- `docker`, `docker-compose` - Container management
- `nix`, `nix-shell` - Nix package manager
