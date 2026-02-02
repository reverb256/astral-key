# Astral Key - Justfile
# Quick command reference for development

# Default recipe to display help
_default:
    @just --list

# Development server with hot reload
dev:
    cargo watch -x run

# Run all tests
test:
    cargo test --all-features

# Run tests with coverage
test-coverage:
    cargo tarpaulin --out Html --out Stdout

# Start database services (PostgreSQL + Redis)
db-up:
    docker-compose up -d postgres redis

# Stop database services
db-down:
    docker-compose down

# Run database migrations
migrate:
    sqlx migrate run

# Create a new migration
migrate-new name:
    sqlx migrate add {{name}}

# Format code (Rust + Nix)
fmt:
    cargo fmt
    nixpkgs-fmt .

# Run linters
lint:
    cargo clippy --all-features -- -D warnings
    cargo audit

# Build production binary
build:
    cargo build --release --features production

# Build container image
container:
    nix build .#container

# Enter Nix development shell
shell:
    nix develop

# Update flake inputs
update:
    nix flake update

# Check flake
flake-check:
    nix flake check

# Clean build artifacts
clean:
    cargo clean
    rm -rf result result-*

# Generate documentation
docs:
    cargo doc --no-deps --open

# Run security audit
audit:
    cargo audit
    cargo deny check

# Check dependencies for updates
deps-check:
    cargo outdated

# Run pre-commit hooks
pre-commit:
    pre-commit run --all-files

# Start Vaultwarden for local development
vaultwarden-up:
    docker-compose up -d vaultwarden

# View logs
logs service="astral-key":
    docker-compose logs -f {{service}}
