# Astral Key — Project Overview

## Purpose

Astral Key is a **single-binary authentication sidecar** for FIDO2/WebAuthn
passkey and Web3/SIWE authentication. Built with Rust (Axum) and **SQLite only**
(no PostgreSQL, no Redis, no Vaultwarden).

The repo also contains the **Mosaic Identity Service (MIS)** crate — a
standalone PKI service for Ed25519 key management, cross-protocol identity
binding, ML-DSA-65 PQ hybrid signing, BIP-39 HD derivation, and agent
ephemeral certs.

**Status:** Core auth complete, MIS shipped, bridges deployed (9 Rust crates).

## Tech Stack

### Core
- **Language:** Rust 2021 edition, MSRV 1.75
- **Runtime:** Tokio async runtime
- **Web Framework:** Axum 0.7 with Tower middleware
- **Database:** SQLite via sqlx (no PostgreSQL, no Redis)
- **FIDO2 State:** In-memory HashMap with TTL (no Redis)

### Authentication
- **Web3:** ethers-rs 2.0 + siwe 0.6 for SIWE (EIP-4361)
- **FIDO2/WebAuthn:** webauthn-rs 0.5 with passkey support
- **JWT:** jsonwebtoken 9.x with HS256 access/refresh token rotation
- **JIT capability tokens:** Ed25519-signed (ed25519-dalek 2.x)
- **API keys:** Argon2id hashing, prefix `ak_prod_` / `ak_dev_`
- **Post-quantum:** ML-DSA-65 (FIPS 204) via pqcrypto-mldsa (feature-gated)

### Deployment
- **Docker:** Multi-stage Containerfile for containers
- **K3s:** Kubernetes manifests in `k8s/`
- **Nix:** Dev shell via `flake.nix` (no production NixOS module yet)
- **CI/CD:** GitHub Actions (lint, test, build, Docker publish)

### Key Dependencies
- `tracing` + `tracing-subscriber` — Structured logging
- `thiserror` + `anyhow` — Error handling
- `webauthn-rs` — WebAuthn ceremony
- `ethers` + `siwe` — Ethereum signature verification
- `jsonwebtoken` — JWT handling
- `ed25519-dalek` — Ed25519 signing (JIT tokens)
- `argon2` — API key hashing
- `qrcode` + `image` — QR code generation (SVG/PNG)

## Repository

- **License:** MIT
- **Repository:** https://github.com/reverb256/astral-key
