---

## SecretSpec Integration (Recommended)

[SecretSpec](https://secretspec.dev) is a declarative secret management tool that
separates **what** secrets an app needs from **where** they're stored (15 provider
backends). Astral Key integrates with SecretSpec for environment-portable
secret resolution.

### Quick start with SecretSpec

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

### secretspec.toml

Create a `secretspec.toml` in the project root to declare astral-key's secrets:

```toml
[project]
name = "astral-key"
revision = "1.0"

[profiles.default]
JWT_SECRET = { description = "JWT signing key (256-bit hex)", required = true }
DATABASE_URL = { description = "SQLite or Vaultwarden DSN", required = false, default = "sqlite://./astral-key.db?mode=rwc" }
FIDO2_RP_ID = { description = "Relying Party domain", required = false, default = "localhost" }
FIDO2_RP_NAME = { description = "Relying Party name", required = false, default = "Astral Key" }
FIDO2_ORIGINS = { description = "Allowed origins (comma-separated)", required = false, default = "http://localhost:8080" }

[profiles.production]
JWT_SECRET = { required = true, providers = ["vault://http://vault:8200"] }
DATABASE_URL = { required = true, providers = ["vault://http://vault:8200"] }
```

### Provider credentials chain

Because SecretSpec supports per-secret fallback chains, astral-key can resolve
its own secrets from Vaultwarden without hardcoding credentials:

```toml
[providers]
keyring = "keyring://"
vaultwarden_token = { uri = "keyring://", providers = ["keyring"] }

[profiles.production]
JWT_SECRET = { providers = ["vaultwarden://..."] }
```

### Expose via Vault-compatible endpoint (no SecretSpec fork)

Astral Key can expose a Vault-compatible KV endpoint (`GET /v1/secret/data/<path>`)
that SecretSpec's existing `vault` provider already speaks. This lets any
SecretSpec consumer resolve secrets through astral-key's authentication layer
with zero changes to SecretSpec:

```toml
[providers]
astral = "vault://http://astral-key:8080/v1/secret"

[profiles.production]
DATABASE_URL = { providers = ["astral"] }
```

### SOPS provider integration

A SecretSpec SOPS provider is under active development (tracking upstream PR
#58). When complete, astral-key can store the SOPS decryption key and serve it
via the Vault-compatible endpoint, enabling encrypted config resolution through
passkey-authenticated identity:

```toml
[providers]
astral = "vault://http://astral-key:8080/v1/secret"
sops_prod = { uri = "sops://./secrets/prod.yaml", credentials = { age_key = "astral" } }
```

This avoids hardcoding any encryption keys — the SOPS age key is resolved at
runtime from astral-key, which authenticated the operator via their passkey.

### NixOS integration

On the NixOS module side, `secretspec run -- astral-key` wraps the service
startup. The NixOS module supports an `environmentFile` that can point to
secrets resolved by SecretSpec, or the service can be wrapped directly:

```nix
systemd.services.astral-key = {
  description = "Astral Key auth service";
  wantedBy = [ "multi-user.target" ];
  serviceConfig.ExecStart = "${pkgs.secretspec}/bin/secretspec run -- ${pkgs.astral-key}/bin/astral-key";
  serviceConfig.User = "astral-key";
  serviceConfig.Group = "astral-key";
};
```
