# Contributing to Astral Key

Thank you for your interest in contributing to Astral Key! This document
covers the development workflow, code conventions, and pull request process.

## Table of Contents

- [Prerequisites](#prerequisites)
- [Getting Started](#getting-started)
- [Project Structure](#project-structure)
- [Development Workflow](#development-workflow)
- [Code Conventions](#code-conventions)
- [Testing](#testing)
- [Pull Request Process](#pull-request-process)
- [Release Process](#release-process)

## Prerequisites

- **Rust 1.75+** — [Install via rustup](https://rustup.rs)
- **Nix** with flakes (optional, for `nix develop`) — [nixos.org](https://nixos.org/download.html)
- **OpenSSL** development headers (for `cargo build`)

## Getting Started

```bash
# Clone the repository
git clone https://github.com/reverb256/astral-key.git
cd astral-key

# Enter the Nix development shell (optional — provides pinned Rust toolchain)
nix develop

# Or use your system Rust directly
cargo build

# Run tests
cargo test

# Start the server (SQLite database created automatically)
cargo run
```

The server starts on `http://localhost:8080`. See [`docs/api.md`](docs/api.md)
for endpoint documentation.

## Project Structure

```
astral-key/
├── src/
│   ├── main.rs           # Application entry point
│   ├── config.rs         # Environment variable configuration
│   ├── error.rs          # Error types and HTTP response mapping
│   ├── state.rs          # Shared application state
│   ├── api/              # HTTP handlers, routes, middleware
│   │   ├── routes.rs     # Route definitions
│   │   └── handlers/     # Request handlers
│   ├── auth/             # Authentication implementations
│   │   ├── jwt/          # JWT service and middleware
│   │   ├── fido2/        # WebAuthn ceremony logic
│   │   └── web3/         # SIWE signature verification
│   ├── db/               # SQLite database layer
│   │   ├── pool.rs       # SQLx connection pool
│   │   └── models/       # Database models (user, credential, etc.)
│   └── utils/            # Utility functions
├── migrations/            # SQLx database migrations
├── docs/                  # Documentation
├── scripts/              # ensure-image.sh (registry-free image recovery)
├── Containerfile         # astral-key SERVICE image  <- what the chart deploys
├── Dockerfile            # mosaic-identity sidecar    <- different service, port 8081
├── Cargo.toml            # Rust workspace (astral-key + 10 crates)
├── flake.nix             # Nix flake (dev shell, package)
└── docker-compose.yml    # Single-service Docker Compose
```

**Two build files, two services.** `docker build .` defaults to `Dockerfile`
(mosaic-identity) — pass `-f Containerfile` for the astral-key service. CI once
omitted it and went green while producing the wrong binary.

## Development Workflow

### Building

```bash
# Debug build
cargo build

# Release build
cargo build --release

# Nix build (reproducible)
nix build
```

### Running

```bash
# Development server (auto-migrates on startup)
cargo run

# With custom settings
DATABASE_URL=sqlite:test.db?mode=rwc JWT_SECRET=dev-secret-key-1234567890123456 cargo run

# Watch mode (requires cargo-watch)
cargo watch -x run
```

### Code Quality

```bash
# Format code
cargo fmt

# Run linter
cargo clippy -- -D warnings

# Security audit (install: cargo install cargo-audit)
cargo audit
```

### Database Migrations

Migrations live in `migrations/` and are automatically applied when the
server starts. To create a new migration:

```bash
# Install sqlx-cli
cargo install sqlx-cli

# Create a migration
sqlx migrate add -r description_of_change
```

**Important:** Astral Key uses SQLite. Always write migrations compatible
with SQLite's subset of SQL.

## Code Conventions

- **Async-first**: All I/O is async with Tokio. Avoid `std::thread::spawn`.
- **Type safety**: Use Rust's type system to make invalid states
  unrepresentable. Prefer `enum` over stringly-typed values.
- **Error handling**: Use `thiserror` for library errors and `anyhow` for
  application-level error propagation. Never use `unwrap()` or `expect()`
  outside of tests; use `?` instead.
- **Configuration**: All configuration comes from environment variables
  read via `std::env::var` in `src/config.rs`. No config files.
- **Comments**: Document public APIs with doc comments (`///`). Use
  `//!` for module-level documentation.
- **Commit messages**: Follow [Conventional Commits](https://www.conventionalcommits.org/) —
  `feat:`, `fix:`, `docs:`, `refactor:`, `test:`, `chore:`.

## Testing

```bash
# Run all tests
cargo test

# Run tests with output
cargo test -- --nocapture

# Run a specific test
cargo test test_name

# Run clippy on tests too
cargo clippy --tests -- -D warnings
```

### Test Guidelines

1. Unit tests live next to the code they test (in the same file, in a
   `#[cfg(test)] mod tests` block).
2. Integration tests live in `tests/` (one file per integration scenario).
3. Mock external dependencies (e.g., Web3 providers) with traits.
4. Test error paths, not just happy paths.

## Pull Request Process

1. **Create a feature branch** from `main`:
   ```bash
   git checkout -b feat/my-feature
   ```

2. **Make your changes**, keeping commits small and focused.

3. **Run the full check suite**:
   ```bash
   cargo fmt --check
   cargo clippy --all-targets -- -D warnings
   cargo test
   cargo audit
   ```

4. **Push and open a PR** against the `main` branch.

5. **PR title** should be a conventional commit message (e.g., `feat: add
   rate-limiting middleware`).

6. **PR body** should describe what changed and why. If it fixes an issue,
   reference it (e.g., `Closes #42`).

7. **Review** — at least one maintainer must approve. Address all feedback.

8. **Merge** — squash-merge into `main` once approved. The `main` branch
   is protected; direct pushes are not allowed.

## Release Process

1. Update the version in `Cargo.toml`.
2. Update `CHANGELOG.md` (if one exists).
3. Tag the release:
   ```bash
   git tag v0.1.0
   git push origin v0.1.0
   ```
4. CI builds the service image from `Containerfile` and verifies it starts
   astral-key. It **does not push anywhere** — the publish steps only run when
   Docker Hub credentials are present in repo secrets, and as of 2026-10-06 they
   are not. `ghcr.io/reverb256/astral-key` has never been published.
5. To deploy, build on nexus and re-pin the digest in
   `charts/astral-key/values.yaml` — see
   [docs/deployment.md](docs/deployment.md#kubernetes-k3s). A rebuild produces a
   new digest, so the pin must be updated or the pod sits at `ErrImageNeverPull`.

## Operational recovery

The deployed image is registry-free: it lives only in nexus's containerd store,
pinned by digest with `pullPolicy: Never`. Nothing re-pulls it. If it goes
missing, the procedure is:

```bash
scripts/ensure-image.sh                  # every pin in charts/astral-key/values.yaml
scripts/ensure-image.sh --from-tar ./astral-key.tar   # restore from a docker save tar
scripts/ensure-image.sh --build          # build from source (yields a NEW digest — re-pin after)
```

The script is idempotent and safe to re-run. The step it exists to guarantee is
creating the `<repo>@sha256:<digest>` **name** in the store — an imported image
that is only tagged will still leave the pod at `ErrImageNeverPull`, and
`ctr images ls` reports this state as looking fine. Full explanation:
[docs/deployment.md → Recovering a Missing Image](docs/deployment.md#recovering-a-missing-image).

This is a manual procedure. There is no automation, webhook, or CI job that
runs `ensure-image.sh` for you.

## License

By contributing, you agree that your contributions will be licensed under
the [MIT License](LICENSE).
