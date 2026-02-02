# Astral Key

[![NixOS](https://img.shields.io/badge/NixOS-5277C3?logo=nixos&logoColor=white)](https://nixos.org)
[![Rust](https://img.shields.io/badge/Rust-000000?logo=rust&logoColor=white)](https://rust-lang.org)
[![License](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

> Microservice for Web3, FIDO2, and Passkey authentication with Vaultwarden backend

## Overview

Astral Key is a next-generation authentication microservice designed for Web3, FIDO2, and Passkey authentication. Built on NixOS with cutting-edge Nix features, it provides a secure, declarative, and reproducible authentication infrastructure backed by Vaultwarden for credential management.

## Features

- **Web3 Authentication**: Sign-In with Ethereum (SIWE), multi-chain support (Ethereum, Polygon, Arbitrum, Optimism, Solana)
- **FIDO2/Passkey**: Full WebAuthn implementation with platform and roaming authenticator support
- **Vaultwarden Integration**: Secure credential storage and management
- **High Performance**: Built with Rust and Axum for maximum throughput
- **NixOS Native**: Declarative configuration, reproducible builds, seamless deployment

## Quick Start

```bash
# Enter development shell
nix develop

# Start database services
just db-up

# Run migrations
just migrate

# Start development server
just dev
```

## Architecture

See [ARCHITECTURE.md](ARCHITECTURE.md) for detailed system design and implementation details.

## API Documentation

API documentation is available at `/docs` when running the server, or see [docs/api.md](docs/api.md).

## Development

### Prerequisites

- [Nix](https://nixos.org/download.html) with flakes enabled
- [direnv](https://direnv.net/) (optional but recommended)

### Setup

```bash
# Clone the repository
git clone https://github.com/reverb256/astral-key.git
cd astral-key

# Enter development environment
nix develop

# Or with direnv
direnv allow
```

### Commands

| Command | Description |
|---------|-------------|
| `just dev` | Start development server with hot reload |
| `just test` | Run all tests |
| `just db-up` | Start PostgreSQL and Redis services |
| `just migrate` | Run database migrations |
| `just fmt` | Format code |
| `just lint` | Run clippy and other linters |
| `just build` | Build production binary |
| `just container` | Build container image |

## Deployment

### NixOS Module

```nix
# In your NixOS configuration
{
  imports = [ inputs.astral-key.nixosModules.default ];

  services.astral-key = {
    enable = true;
    host = "0.0.0.0";
    port = 8080;
    database.url = "postgresql://astral:secret@localhost/astral_key";
    vaultwarden.url = "http://localhost:8000";
    fido2.rpId = "auth.example.com";
    fido2.origin = "https://auth.example.com";
    openFirewall = true;
  };
}
```

### Container

```bash
# Build container image
nix build .#container

# Load and run
docker load < result
docker run -p 8080:8080 astral-key:latest
```

## License

MIT License - see [LICENSE](LICENSE) for details.

## Contributing

Contributions are welcome! Please read our [Contributing Guide](CONTRIBUTING.md) for details.
