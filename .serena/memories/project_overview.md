# Astral Key - Project Overview

## Purpose

Astral Key is a **production-ready authentication microservice** providing Web3, FIDO2, and Passkey authentication with Vaultwarden backend integration.

**Status:** ~95% Complete - Production Ready

## Tech Stack

### Core
- **Language:** Rust 2021 (edition 2021, minimum 1.75)
- **Runtime:** Tokio async runtime
- **Web Framework:** Axum 0.7 with Tower middleware
- **Database:** PostgreSQL with SQLx (compile-time checked queries)
- **Cache:** Redis for sessions, nonces, rate limiting, token blacklist

### Authentication
- **Web3:** ethers-rs 2.0 for Ethereum signature verification
- **SIWE:** Sign-In with Ethereum (siwe 0.6)
- **FIDO2/WebAuthn:** webauthn-rs 0.5 with passkey support
- **JWT:** jsonwebtoken 9.2 with access/refresh token rotation

### Deployment
- **NixOS:** Native NixOS module for declarative deployment
- **Docker:** Multi-stage Dockerfile for containers
- **CI/CD:** GitHub Actions workflow

### Key Dependencies
- `tracing` + `tracing-subscriber` - Structured logging
- `thiserror` + `anyhow` - Error handling
- `validator` - Input validation
- `governor` - Rate limiting
- `opentelemetry` - Metrics and tracing

## Architecture Highlights

- **Modular structure:** Clear separation of API, auth, database, cache layers
- **JWT middleware:** Protected routes with `AuthenticatedUser` extractor
- **Session management:** Token rotation with Redis-backed blacklist
- **Multi-chain Web3:** Ethereum, Polygon, Arbitrum, Optimism, Solana support
- **Health checks:** Database and Redis connectivity checks

## Repository

- **License:** MIT
- **Repository:** https://github.com/reverb256/astral-key
