---

## SecretSpec Integration (Planned)

> **⚠️ Aspirational — Not yet implemented.** This document describes a
> planned integration with [SecretSpec](https://secretspec.dev). As of
> 2026-07-27, no `secretspec.toml` exists in the repo and no SecretSpec
> provider endpoint has been built. Tracked in [issue #16](https://github.com/reverb256/astral-key/issues/16).

[SecretSpec](https://secretspec.dev) is a declarative secret management tool that
separates **what** secrets an app needs from **where** they're stored (15 provider
backends). When implemented, Astral Key will integrate with SecretSpec for
environment-portable secret resolution.

### Planned quick start

```bash
# Install secretspec (available in nixpkgs)
nix profile install nixpkgs#secretspec

# Initialize secretspec.toml from existing .env
cd /path/to/astral-key
secretspec init

# Run astral-key with resolved secrets
secretspec run -- astral-key

# With a specific profile and provider
secretspec run --profile production --provider vault -- astral-key
```

### Planned secretspec.toml

```toml
[project]
name = "astral-key"
revision = "1.0"

[profiles.default]
JWT_SECRET = { description = "JWT signing key (256-bit hex)", required = true }
DATABASE_URL = { description = "SQLite DSN", required = false, default = "sqlite://./astral-key.db?mode=rwc" }
FIDO2_RP_ID = { description = "Relying Party domain", required = false, default = "localhost" }
FIDO2_RP_NAME = { description = "Relying Party name", required = false, default = "Astral Key" }
FIDO2_ORIGINS = { description = "Allowed origins (comma-separated)", required = false, default = "http://localhost:8080" }

[profiles.production]
JWT_SECRET = { required = true, providers = ["vault://http://vault:8200"] }
DATABASE_URL = { required = true, providers = ["vault://http://vault:8200"] }
```

### Planned provider credentials chain

```toml
[providers]
keyring = "keyring://"
vaultwarden_token = { uri = "keyring://", providers = ["keyring"] }

[profiles.production]
JWT_SECRET = { providers = ["vaultwarden://..."] }
```

### Planned Vault-compatible endpoint

Astral Key would expose a Vault-compatible KV endpoint (`GET /v1/secret/data/<path>`)
that SecretSpec's existing `vault` provider could speak to, allowing SecretSpec
consumers to resolve secrets through astral-key's authentication layer:

```toml
[providers]
astral = "vault://http://astral-key:8080/v1/secret"

[profiles.production]
DATABASE_URL = { providers = ["astral"] }
```

### Planned SOPS provider integration

A SecretSpec SOPS provider is under upstream development (tracking PR #58).
When complete, astral-key would store the SOPS decryption key and serve it
via the Vault-compatible endpoint:

```toml
[providers]
astral = "vault://http://astral-key:8080/v1/secret"
sops_prod = { uri = "sops://./secrets/prod.yaml", credentials = { age_key = "astral" } }
```

---

**Status:** All of the above is design speculation. The `secretspec.toml`
does not exist, the Vault-compatible endpoint does not exist, and no
SOPS provider integration has been built. See [issue #16](https://github.com/reverb256/astral-key/issues/16).
