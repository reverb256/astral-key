# Deployment Guide

Astral Key is a single-binary service with no external dependencies (SQLite
database). This guide covers deployment options.

## Table of Contents

- [Quick Start (Docker Compose)](#quick-start-docker-compose)
- [Docker Compose (Detailed)](#docker-compose-detailed)
- [Nix / NixOS](#nix--nixos)
- [Kubernetes (K3s)](#kubernetes-k3s)
- [Environment Variables](#environment-variables)
- [Health Checks](#health-checks)
- [Production Checklist](#production-checklist)

---

## Quick Start (Docker Compose)

```bash
# Clone the repository
git clone https://github.com/reverb256/astral-key.git
cd astral-key

# Set a strong JWT secret
export JWT_SECRET=$(openssl rand -hex 32)

# Start the service
docker compose up -d

# Verify
curl http://localhost:8080/health
```

No external database, Redis, or Vaultwarden is required. Astral Key embeds
SQLite and persists data on a Docker volume.

---

## Docker Compose (Detailed)

See [`docker-compose.yml`](../docker-compose.yml) for the canonical file.

### Building the image locally

```bash
docker build -t ghcr.io/reverb256/astral-key:latest .
docker compose up -d
```

### Using a pre-built image

Images are published to `ghcr.io/reverb256/astral-key`. The Docker Compose
file references this image by default.

### Environment overrides

Create an `.env` file:

```bash
# .env
JWT_SECRET=your-256-bit-hex-secret-here
FIDO2_RP_ID=auth.example.com
FIDO2_ORIGINS=https://auth.example.com
```

Then:

```bash
docker compose --env-file .env up -d
```

---

## Nix / NixOS

### Nix Flake (Dev Shell)

```bash
nix develop
cargo build --release
./target/release/astral-key
```

### NixOS Module

A NixOS module is available in the `nix/` directory. Example usage:

```nix
{
  imports = [
    (builtins.fetchTarball {
      url = "https://github.com/reverb256/astral-key/archive/main.tar.gz";
    } + "/nix/nixos-module.nix")
  ];

  services.astral-key = {
    enable = true;
    host = "0.0.0.0";
    port = 8080;
    database.url = "sqlite:/var/lib/astral-key/db.sqlite?mode=rwc";
    fido2.rpId = "auth.example.com";
    fido2.origin = "https://auth.example.com";
    jwt.secretFile = "/run/secrets/jwt-secret";
    openFirewall = true;
  };
}
```

> **Note:** The NixOS module is currently being developed. The `nix/`
> directory is a work in progress.

---

## Kubernetes (K3s)

Astral Key is deployed on a K3s cluster in production. See the `k8s/`
directory for manifests. Example deployment:

```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: astral-key
spec:
  replicas: 1
  selector:
    matchLabels:
      app: astral-key
  template:
    metadata:
      labels:
        app: astral-key
    spec:
      containers:
      - name: astral-key
        image: ghcr.io/reverb256/astral-key:latest
        ports:
        - containerPort: 8080
        env:
        - name: DATABASE_URL
          value: "sqlite:/data/astral-key.db?mode=rwc"
        - name: JWT_SECRET
          valueFrom:
            secretKeyRef:
              name: astral-key-secrets
              key: jwt-secret
        - name: FIDO2_RP_ID
          value: "auth.example.com"
        - name: FIDO2_ORIGINS
          value: "https://auth.example.com"
        volumeMounts:
        - name: data
          mountPath: /data
        livenessProbe:
          httpGet:
            path: /health
            port: 8080
          initialDelaySeconds: 10
          periodSeconds: 30
        readinessProbe:
          httpGet:
            path: /ready
            port: 8080
          initialDelaySeconds: 5
          periodSeconds: 10
      volumes:
      - name: data
        persistentVolumeClaim:
          claimName: astral-key-data
---
apiVersion: v1
kind: Service
metadata:
  name: astral-key
spec:
  selector:
    app: astral-key
  ports:
  - port: 80
    targetPort: 8080
  type: ClusterIP
```

---

## Environment Variables

| Variable | Required | Default | Description |
|----------|----------|---------|-------------|
| `SERVER_HOST` | No | `127.0.0.1` | Network interface to bind to |
| `SERVER_PORT` | No | `8080` | TCP port |
| `DATABASE_URL` | No | `sqlite:astral_key.db?mode=rwc` | SQLite database URL |
| `DATABASE_MAX_CONNECTIONS` | No | `5` | Max SQLite connections |
| `JWT_SECRET` | **Yes** | — | JWT signing key (≥32 bytes). Generate: `openssl rand -hex 32` |
| `FIDO2_RP_ID` | No | `localhost` | WebAuthn Relying Party ID |
| `FIDO2_RP_NAME` | No | `Astral Key` | Human-readable RP name |
| `FIDO2_ORIGINS` | No | `http://localhost:8080` | Comma-separated allowed origins |
| `FIDO2_ATTESTATION` | No | `indirect` | Attestation preference: `none`, `indirect`, `direct` |
| `ASTRAL_WEB3_DOMAIN` | No | `maplespike.ca` | Canonical SIWE domain |
| `JIT_ISSUER_KEY` | No* | — | Ed25519 private key (64 hex chars). Enables JIT capability token minting when set. Generate: `openssl rand -hex 32` |
| `JIT_ISSUER_ID` | No | `ak:issuer:01` | Issuer identifier embedded in minted JIT tokens |
| `JIT_DEFAULT_TTL` | No | `3600` | Default TTL (seconds) for JIT tokens. Min: `1`, max depends on use case |
| `OAUTH_BASE_URL` | No | `http://localhost:8080` | Base URL for OAuth redirects |
| `OAUTH_GITHUB_CLIENT_ID` | No | — | GitHub OAuth client ID (optional — omit to disable) |
| `OAUTH_GITHUB_CLIENT_SECRET` | No | — | GitHub OAuth client secret |
| `OAUTH_GITHUB_REDIRECT_URI` | No | `{OAUTH_BASE_URL}/auth/oauth/github/callback` | OAuth redirect URI |
| `RUST_LOG` | No | `info,astral_key=debug` | Tracing/log filter |

---

## Health Checks

| Endpoint | Type | Description |
|----------|------|-------------|
| `GET /health` | Liveness | Always returns `200` if the process is running |
| `GET /ready` | Readiness | Returns `200` when the database is reachable, `503` otherwise |

---

## Production Checklist

- [ ] **Set a strong `JWT_SECRET`** — at least 32 bytes, generated via
      `openssl rand -hex 32`. Never commit this to version control.
- [ ] **Use HTTPS** — WebAuthn requires a secure context (HTTPS or
      `localhost`) in browsers. Deploy behind a TLS-terminating reverse
      proxy (Nginx, Traefik, Caddy) or use a K3s ingress with cert-manager.
- [ ] **Set `FIDO2_RP_ID` and `FIDO2_ORIGINS`** to match your production
      domain. These must match exactly what the browser sees.
- [ ] **Persist the SQLite database** — mount a Docker volume or host path
      to `/data` (or wherever `DATABASE_URL` points).
- [ ] **Back up the database** regularly — the entire state is in a single
      `.db` file.
- [ ] **Configure `RUST_LOG`** — set to `warn,astral_key=info` in production
      to reduce noise, or `astral_key=debug` during incident response.
- [ ] **Monitor health** — configure your orchestration to use `/health` and
      `/ready` probes as shown above.
- [ ] **Resource limits** — Astral Key is lightweight. 256 MiB RAM and
      0.5 CPU cores are sufficient for most workloads.

> Snapshot from August 2026 cleanup; verify current state via /etc/nixos/SOPS-NIX.md.

## See Also — SOPS-NIX (canonical on this host)

The canonical local reference is `/etc/nixos/SOPS-NIX.md`. Other repos should cross-link rather than maintain their own copy.

---

## Mosaic Identity Service (MIS) / Bridges

### Architecture

MIS crate (`crates/mosaic-identity/`) is a standalone Rust binary with 16 REST endpoints for Ed25519 key management, cross-protocol identity binding, PQ hybrid signing, and agent ephemeral certs.

Four Node.js transport plugins sidecar as k8s pods in `orchestration` namespace:

| Bridge | Protocol | Entrypoint |
|--------|----------|------------|
| atproto | PLC/BSky DID resolution | `bridges/atproto/index.js` (daemon :8083) |
| buzz | Nostr WebSocket relay | `bridges/buzz/index.js` |
| matrix | Matrix Application Service | `bridges/matrix/index.js` (AS :8082) |
| irc | IRC TLS client | `bridges/irc/index.js` |

### Quick start (standalone)

```bash
cargo run -p mosaic-identity -- --database "sqlite:///tmp/mis.db?mode=rwc"
curl http://localhost:8081/health
curl -X POST http://localhost:8081/keys/generate -H 'Content-Type: application/json' -d '{}'
curl -X POST http://localhost:8081/bindings/resolve -H 'Content-Type: application/json' -d '{"did_or_handle":"bsky.app"}'
```

### Docker (k3s deploy)

```bash
docker build --no-cache -t nexus:5000/mosaic-identity:v0.1.0 -f Dockerfile.mosaic-identity .
docker save nexus:5000/mosaic-identity:v0.1.0 | sudo ctr -n k8s.io images import -

docker build --no-cache -t nexus:5000/mosaic-bridges:v0.1.0 -f Dockerfile.bridges .
docker save nexus:5000/mosaic-bridges:v0.1.0 | sudo ctr -n k8s.io images import -

kubectl apply -f /etc/nixos/k8s/mosaic-identity/deployment.yaml
kubectl apply -f /etc/nixos/k8s/mosaic-bridges/
```

### Known issues

- **PVC slow**: `local-path` provisioner takes ~30s. Workaround: `emptyDir: {}`.
- **Bridge UID**: Container `appuser` = UID 100. k8s `runAsUser: 100` required.
- **NixOS k3s unit bug**: No ExecStart when `role=server` + `clusterInit=false`. Workaround: systemd drop-in at `/run/systemd/system/k3s.service.d/override.conf`.
- **No registry push**: `nexus:5000` resolves to `127.0.0.2` on host. Use `docker save | ctr import`.
