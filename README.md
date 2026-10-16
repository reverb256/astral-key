<div align="center">

# Astral Key

**Passkey-first authentication middleware — self-hosted, Web3-ready, NixOS-native**

[![Rust](https://img.shields.io/badge/Rust-000000?logo=rust&logoColor=white)](https://rust-lang.org)
[![License](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![NixOS](https://img.shields.io/badge/NixOS-5277C3?logo=nixos&logoColor=white)](https://nixos.org)
[![SecretSpec](https://img.shields.io/badge/SecretSpec-ready-8B5CF6)](https://secretspec.dev)

</div>

Astral Key is an open-source authentication middleware written in Rust
([Axum](https://github.com/tokio-rs/axum)). It authenticates users with
**FIDO2/WebAuthn passkeys** (Touch ID, Windows Hello, YubiKey) and
**Web3/SIWE** (Ethereum wallet signatures), and issues JWT sessions.

**Key differentiator:** Astral Key is the *only* self-hostable auth service that
combines passkey-native + Web3 wallet auth + a NixOS deployment module in a
single Rust binary backed by **SQLite** (no external dependencies).

---

## Why Astral Key?

Most auth services sit at one of two extremes:

| Extreme | Examples | Problems |
|---------|----------|----------|
| **Cloud consumer auth** | Auth0, Clerk, Firebase Auth | You don't control the keys. Passkeys roam through Apple/Google/Microsoft clouds. |
| **Enterprise SSO** | Keycloak, Authentik, Casdoor | Heavy (Java/Go), password-first, own database required. |

Astral Key fills the **unoccupied middle**: lightweight (single Rust binary,
~5MB), passkey-native (no passwords required), Web3-ready (SIWE wallet auth),self-hosted with SQLite storage — deployable via Docker Compose in one line.

### Who this is for

- **NixOS homelab operators** running multi-service stacks (Grafana, Gitea, n8n, OpenWebUI, Nextcloud) who want **one passkey** to authenticate across all of them — using Astral Key's SQLite-backed identity store for lightweight self-sovereign auth.
- **DAOs and Web3 communities** that want members to authenticate with their Ethereum wallet (SIWE) for gated access to Discourse, governance apps, or treasury dashboards. No email, no password.
- **Privacy-conscious passkey users** who want self-hosted passkey roaming — keys live in Astral Key's SQLite database, not in Apple iCloud or Google Password Manager.
- **AI agent platforms** that need scoped, revocable session tokens for agent tool access — authenticate once with a passkey, issue short-lived capability tokens to Claude Code, Hermes, or custom agents.

---

## Quick Start

### With Docker Compose

```bash
git clone https://github.com/reverb256/astral-key.git
cd astral-key
export JWT_SECRET=$(openssl rand -hex 32)
docker compose up -d
curl http://localhost:8080/health
```

### With Nix

```bash
nix develop
cargo build --release
JWT_SECRET=$(openssl rand -hex 32) ./target/release/astral-key
```

### With NixOS module (declarative)

```nix
# flake.nix
{
  inputs.astral-key.url = "github:reverb256/astral-key";

  # In your host config:
  imports = [ inputs.astral-key.nixosModules.default ];

  services.astral-key = {
    enable = true;
    environmentFile = "/run/secrets/astral-key-env";
  };
}
```

---

## Features

### ✅ Implemented

| Feature | Description |
|---------|-------------|
| **FIDO2 / WebAuthn Passkeys** | Register and authenticate with platform or roaming authenticators. Full WebAuthn ceremony. |
| **Web3 / SIWE** | Sign-In with Ethereum (EIP-4361). Multi-chain: Ethereum, Polygon, Arbitrum, Optimism. |
| **JWT Sessions** | Access tokens + refresh token rotation. Verify endpoint for downstream services. |
| **SQLite Storage** | Embedded SQLite. No external dependencies. Single binary, single file. |
| **Passkey CRUD** | List and delete registered passkeys. |
| **Multi-Factor JWT Sessions** | Access tokens + refresh token rotation. Verify endpoint for downstream services. |
| **Ed25519 Identity** | Public-key identity management, signature verification, contact graph. |
| **ZK JIT Capability Tokens** | Ed25519-signed scoped tokens with epoch-based revocation. |
| **API Key Management** | Argon2id-hashed API keys with prefix format `ak_prod_...`. |
| **OIDC Provider** | Astral Key as an OpenID Connect identity provider (authorization-code flow + PKCE, RS256 or EdDSA id_tokens, groups) for oauth2-proxy and other relying parties. |

### 🔜 Coming Soon

| Feature | Target |
|---------|--------|
| **SecretSpec integration** | Declare secrets in `secretspec.toml`, resolve from any of 15 providers, inject via `secretspec run -- astral-key` |
| **SOPS provider (planned)** | Encrypted config files decrypted via SecretSpec, keys resolved through astral-key's auth layer |
| **Vault-compatible endpoint** | Expose `/v1/secret/data/<path>` so SecretSpec's existing `vault` provider can read secrets from astral-key — no SecretSpec fork needed |
| **AI agent tokens** | Short-lived, scoped, revocable tokens for agent tool access |

---

## Architecture

```
┌──────────────┐     ┌──────────────────┐
│   Client     │────>│   Astral Key     │
│ (Browser /   │     │   (Axum API)     │
│  Wallet)     │<────│   Port 8080      │
└──────────────┘     └──────────────────┘
                          │
                     ┌────┴────┐
                     │  SQLite │
                     │   DB    │
                     └─────────┘

In-memory FIDO2 challenge state (TTL-based HashMap) replaces Redis.
No PostgreSQL, Redis, or Vaultwarden required.
```

The server authenticates users via passkey or wallet → issues JWT sessions.
All data persists in SQLite (single file, no external services).

### SecretSpec integration (planned)

> **⚠️ Aspirational — Not yet implemented.** The `secretspec.toml` shown
> below does not exist in the repo. Tracked in [issue #16](https://github.com/reverb256/astral-key/issues/16).
> Until implemented, all configuration is via environment variables as
> shown in [Configuration](#configuration) above.

SecretSpec integration would allow Astral Key to resolve secrets from
15+ provider backends via a declarative `secretspec.toml`:

```toml
# (planned) secretspec.toml
[project]
name = "astral-key"

[profiles.default]
JWT_SECRET = { required = true }
DATABASE_URL = { default = "sqlite://./astral-key.db?mode=rwc" }

[profiles.production]
JWT_SECRET = { providers = ["vault://http://vault:8200"] }
DATABASE_URL = { providers = ["vault://http://vault:8200"] }
```

```bash
# (planned)
secretspec run -- astral-key
```

---

## API Overview

| Endpoint | Auth | Description |
|----------|------|-------------|
| `GET /health` | — | Liveness check |
| `GET /ready` | — | Readiness check (database) |
| `GET /api/v1/auth/web3/chains` | — | List supported chains |
| `POST /api/v1/auth/web3/nonce` | — | Request SIWE nonce |
| `POST /api/v1/auth/web3/verify` | — | Verify SIWE signature → JWT |
| `POST /api/v1/auth/fido2/register/options` | JWT | Start passkey registration |
| `POST /api/v1/auth/fido2/register/verify` | JWT | Complete passkey registration |
| `POST /api/v1/auth/fido2/authenticate/options` | — | Start passkey authentication |
| `POST /api/v1/auth/fido2/authenticate/verify` | — | Complete passkey auth → JWT |
| `GET /api/v1/auth/fido2/credentials` | JWT | List registered passkeys |
| `DELETE /api/v1/auth/fido2/credentials/:id` | JWT | Delete a passkey |

Full API reference: [`docs/api.md`](docs/api.md)

---

## Configuration

All configuration is via environment variables.

```bash
# Required
export JWT_SECRET=$(openssl rand -hex 32)

# Optional overrides
export SERVER_HOST=0.0.0.0
export SERVER_PORT=8080
export DATABASE_URL="sqlite:/data/astral-key.db?mode=rwc"
export FIDO2_RP_ID=localhost
export FIDO2_RP_NAME="My App"
export FIDO2_ORIGINS=http://localhost:8080
```

### OIDC provider (identity-provider side)

Enable the OIDC provider to let oauth2-proxy (or any RP) authenticate users
with passkeys:

```bash
export OIDC_ENABLED=true
export OIDC_ISSUER=https://auth.lan
export OIDC_CLIENT_ID=astral-key-oidc
export OIDC_CLIENT_SECRET_FILE=/run/secrets/astral-key-oidc-client-secret
export OIDC_REDIRECT_URIS=https://auth.lan/oauth2/callback
# 64-hex Ed25519 seed (pin this — the JWKS changes across restarts otherwise)
export OIDC_SIGNING_KEY_FILE=/run/secrets/astral-key-oidc-signing-key
# Optional RSA key: when set, every token is signed RS256 instead of EdDSA
export OIDC_RSA_KEY_FILE=/run/secrets/astral-key-oidc-rsa-key
```

| Variable | Default | Description |
|----------|---------|-------------|
| `OIDC_ENABLED` | `false` | Master switch for the provider |
| `OIDC_ISSUER` | `https://auth.lan` | Public issuer URL — must match the RP configuration |
| `OIDC_SIGNING_KEY[_FILE]` | ephemeral | 64-hex Ed25519 seed. Pin it in production, or the JWKS changes on restart |
| `OIDC_RSA_KEY[_FILE]` | — | PKCS#8/PKCS#1 RSA private key PEM. When set, tokens are signed **RS256** with `kid` = SHA-256 of the modulus (truncated); otherwise **EdDSA** |
| `OIDC_CLIENT_ID` / `OIDC_CLIENT_SECRET[_FILE]` | `astral-key-oidc` / — | Legacy single client, upserted into the `oidc_clients` table at boot |
| `OIDC_REDIRECT_URIS` | `https://auth.lan/oauth2/callback` | Comma-separated redirect URIs for the legacy client |
| `OIDC_ACCESS_TOKEN_TTL` / `OIDC_ID_TOKEN_TTL` | `3600` / `600` | Token lifetimes (seconds) |
| `OIDC_REFRESH_TOKEN_TTL` | `604800` | Refresh-token lifetime (seconds, max 10 years) |

Generate an RSA signing key:

```bash
openssl genpkey -algorithm RSA -pkeyopt rsa_keygen_bits:2048 -out oidc_rsa.pem
```

Endpoints (all relative to the issuer root):

| Endpoint | Description |
|----------|-------------|
| `/.well-known/openid-configuration` | OIDC discovery document |
| `/.well-known/jwks.json` | Signing key(s): the Ed25519 key, plus the RSA key when configured |
| `/oidc/authorize` | Authorization endpoint (GET: login page, POST: code issue) |
| `/oidc/token` | Token endpoint (authorization-code and refresh_token grants, PKCE) |
| `/oidc/userinfo` | Userinfo endpoint |

**Clients.** Relying parties are rows in the `oidc_clients` table
(`migrations/006_oidc_clients.sql`), resolved on every authorize and token
request — a client added with SQL is live without a restart. Redirect URIs
are matched **exactly**. A client with a `client_secret_hash` is
confidential: the stored value is the lowercase hex SHA-256 of the secret,
and the token endpoint requires `client_secret_post` or
`client_secret_basic`. A client with `client_secret_hash IS NULL` is public
and MUST use PKCE S256 (`code_challenge_method=S256`). At boot the legacy
`OIDC_CLIENT_ID`/`OIDC_CLIENT_SECRET` client is upserted into the table, so
existing installs keep working; an empty configured secret registers that
client as public.

```sql
-- Confidential client (SHA-256 hash of the secret)
INSERT INTO oidc_clients (client_id, client_secret_hash, redirect_uris, allowed_scopes)
VALUES ('grafana', '<sha256-hex-of-secret>', '["https://grafana.lan/login/generic_oauth"]',
        '["openid","profile","email"]');

-- Public PKCE client
INSERT INTO oidc_clients (client_id, client_secret_hash, redirect_uris, allowed_scopes)
VALUES ('spa', NULL, '["https://spa.lan/callback"]', NULL);
```

**Groups.** Membership in `oidc_groups` + `oidc_group_members`
(`migrations/007_oidc_groups.sql`) drives the `groups` claim. It is always an
array — empty when the user has no membership — in both the id_token and
`/oidc/userinfo`. SQL seeding is the management path:

```sql
INSERT INTO oidc_groups (name) VALUES ('admins');
INSERT INTO oidc_group_members (user_id, group_name) VALUES ('<user uuid>', 'admins');
```

**Claims.** `email_verified` is emitted as `true` only together with `email`,
and omitted when the account has no address. No verification e-mail is sent:
it asserts that the address was set by the owner or an upstream provider.
`refresh_token` is issued with every authorization-code exchange and rotates
on use (`grant_type=refresh_token`); each refresh token is single-use and
only its SHA-256 hash is stored.

The first user is bootstrapped via `POST /api/v1/auth/fido2/bootstrap/options` +
`/bootstrap/verify` (enabled only while no passkey exists) — the login page
offers this automatically on a fresh install.

**With SecretSpec** (recommended):

```toml
# secretspec.toml
[project]
name = "astral-key"
revision = "1.0"

[profiles.default]
JWT_SECRET = { description = "JWT signing key (256-bit hex)", required = true }
DATABASE_URL = { description = "SQLite DSN", required = false, default = "sqlite://./astral-key.db?mode=rwc" }
FIDO2_RP_NAME = { description = "Relying Party name", required = false, default = "Astral Key" }

[profiles.production]
JWT_SECRET = { providers = ["vault://http://vault:8200"] }
DATABASE_URL = { providers = ["vault://http://vault:8200"] }
```

```bash
secretspec run -- astral-key
```

---

## Documentation

| Document | Description |
|----------|-------------|
| [`docs/architecture.md`](docs/architecture.md) | Module layout and authentication flows |
| [`docs/api.md`](docs/api.md) | Full API reference with curl examples |
| [`docs/deployment.md`](docs/deployment.md) | Docker Compose, Nix, K8s, SecretSpec |
| [`docs/errors.md`](docs/errors.md) | Error code reference |
| [`CONTRIBUTING.md`](CONTRIBUTING.md) | How to build, test, and submit PRs |

---

## Related Projects

| Project | Comparison |
|---------|------------|
| **Pocket-ID** | Passkey auth, own DB, no Web3, no NixOS module |
| **Hanko** | Passkey + password, own DB, cloud-dependent |
| **Casdoor** | OIDC/SAML/Web3, own DB, Go, heavy |
| **Authelio** | FIDO2 only, no Web3, no NixOS |
| **Keycloak** | Full-featured SSO, Java, heavy, no Web3 |

Astral Key is the only self-hostable auth service that combines **passkey-native
+ Web3 wallet + SQLite storage** in a single Rust binary.

---

## License

MIT License — see [LICENSE](LICENSE) for details.

Copyright © Jeremy Kroeker ([reverb256](https://github.com/reverb256))
