# Astral Key — Development Commands

## Using `cargo` directly

```bash
# Building
cargo build                              # Debug build (whole workspace)
cargo build -p mosaic-identity           # Debug build (MIS only)
cargo build --release                    # Release build
cargo build -p mosaic-identity --features pq --release  # MIS with PQ

# Running
JWT_SECRET=$(openssl rand -hex 32) cargo run   # Auth sidecar on :8080
cargo run -p mosaic-identity -- --database "sqlite:///tmp/mis.db?mode=rwc"  # MIS on :8081

# Testing
cargo test --lib                         # All unit tests (no external deps)
cargo test --lib -- --nocapture          # With stdout
cargo test --lib jwt::                   # Specific module
cargo test -p mosaic-identity            # MIS tests
cargo watch -x 'test --lib'              # Watch mode (needs cargo-watch)

# Formatting & Linting
cargo fmt                                # Format code
cargo clippy -- -D warnings              # Lint
cargo clippy --all-features -- -D warnings  # Strict linting (CI level)

# Documentation
cargo doc --no-deps                      # Generate documentation
```

## Using Docker

```bash
# Start the auth sidecar (SQLite, single service)
docker compose up -d

# With custom secret
JWT_SECRET=$(openssl rand -hex 32) docker compose up -d

# Build images
docker build -t ghcr.io/reverb256/astral-key:latest -f Containerfile .
docker build -t nexus:5000/mosaic-identity:v0.1.0 -f Dockerfile.mosaic-identity .
docker build -t nexus:5000/mosaic-bridges:v0.1.0 -f Dockerfile.bridges .
```

## Using Nix

```bash
nix develop              # Enter development environment
nix build                # Build default package
nix flake check          # Validate flake (runs tests + clippy + fmt)
```

## Manual Testing

```bash
# Health check (returns plain text "OK")
curl http://localhost:8080/health

# Readiness check
curl http://localhost:8080/ready

# Request Web3 nonce
curl -X POST http://localhost:8080/api/v1/auth/web3/nonce \
  -H "Content-Type: application/json" \
  -d '{"domain": "maplespike.ca", "address": "0x742d35Cc6634C0532925a3b844Bc9e7595f0bEb", "chain_id": 1}'
```

## Database Migrations

```bash
# Install sqlx-cli
cargo install sqlx-cli

# Create a migration
sqlx migrate add -r description_of_change

# Migrations run automatically on server start
```

## CI/CD

GitHub Actions workflow at `.github/workflows/ci.yml`:
- Lint: `cargo fmt --check`, `cargo clippy`
- Test: `cargo test`
- Audit: `cargo audit`
- Build: Docker image build + push
