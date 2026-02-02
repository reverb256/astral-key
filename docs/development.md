# Astral Key - Development Guide

## Prerequisites

- [Nix](https://nixos.org/download.html) with flakes enabled
- [direnv](https://direnv.net/) (optional but recommended)
- Git

## Getting Started

### 1. Clone the Repository

```bash
git clone https://github.com/reverb256/astral-key.git
cd astral-key
```

### 2. Enter Development Environment

```bash
# Using nix develop
nix develop

# Or with direnv
direnv allow
```

### 3. Start Dependencies

```bash
# Start PostgreSQL, Redis, and Vaultwarden
just db-up
just vaultwarden-up
```

### 4. Run Migrations

```bash
just migrate
```

### 5. Start Development Server

```bash
just dev
```

The server will be available at `http://localhost:8080`.

## Project Structure

```
astral-key/
├── src/                    # Source code
│   ├── main.rs            # Application entry point
│   ├── lib.rs             # Library exports
│   ├── config.rs          # Configuration management
│   ├── error.rs           # Error types
│   ├── state.rs           # Application state
│   ├── api/               # HTTP API layer
│   ├── auth/              # Authentication modules
│   ├── vaultwarden/       # Vaultwarden integration
│   ├── db/                # Database layer
│   ├── cache/             # Redis cache layer
│   └── utils/             # Utilities
├── tests/                 # Integration tests
├── benches/               # Benchmarks
├── nix/                   # Nix-specific files
├── docs/                  # Documentation
├── migrations/            # Database migrations
├── Cargo.toml            # Rust dependencies
├── flake.nix             # Nix flake definition
└── justfile              # Task runner commands
```

## Common Tasks

### Running Tests

```bash
# Run all tests
just test

# Run with coverage
just test-coverage

# Run specific test
cargo test test_name
```

### Code Quality

```bash
# Format code
just fmt

# Run linters
just lint

# Run security audit
just audit
```

### Database Operations

```bash
# Create new migration
just migrate-new migration_name

# Run migrations
just migrate

# Reset database
just db-down && just db-up && just migrate
```

### Building

```bash
# Development build
cargo build

# Release build
just build

# Build container image
just container
```

## Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `SERVER_HOST` | Server bind address | 127.0.0.1 |
| `SERVER_PORT` | Server port | 8080 |
| `DATABASE_URL` | PostgreSQL connection URL | postgresql://localhost/astral_key |
| `REDIS_URL` | Redis connection URL | redis://localhost:6379 |
| `VAULTWARDEN_URL` | Vaultwarden instance URL | http://localhost:8000 |
| `FIDO2_RP_ID` | WebAuthn relying party ID | localhost |
| `FIDO2_ORIGIN` | WebAuthn allowed origin | http://localhost:8080 |
| `JWT_SECRET_FILE` | Path to JWT secret file | /var/lib/astral-key/jwt_secret |
| `RUST_LOG` | Log level | info,astral_key=debug |

## Architecture

See [ARCHITECTURE.md](../ARCHITECTURE.md) for detailed system design.

## Contributing

1. Create a new branch for your feature
2. Make your changes
3. Run tests and linters
4. Submit a pull request

## Troubleshooting

### Nix develop fails

```bash
# Update flake inputs
nix flake update

# Clear nix cache
rm -rf ~/.cache/nix
```

### Database connection errors

```bash
# Check if PostgreSQL is running
docker-compose ps

# View logs
docker-compose logs postgres
```

### Port already in use

```bash
# Find process using port 8080
lsof -i :8080

# Kill process
kill -9 <PID>
```
