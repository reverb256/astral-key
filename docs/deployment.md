# Deployment Guide

Astral Key is a single-binary service with no external dependencies (SQLite
database). This guide covers deployment options.

## Table of Contents

- [Which build file do I use?](#which-build-file-do-i-use) **(read this first)**
- [Quick Start (local, no registry)](#quick-start-local-no-registry)
- [Docker Compose (Detailed)](#docker-compose-detailed)
- [Nix / NixOS](#nix--nixos)
- [Kubernetes (K3s)](#kubernetes-k3s)
- [Recovering a Missing Image](#recovering-a-missing-image)
- [Environment Variables](#environment-variables)
- [Health Checks](#health-checks)
- [Production Checklist](#production-checklist)

---

## Which build file do I use?

**This repo has two container build files and they build different services.**

| File | Builds | Port | Deployed by the chart? |
|------|--------|------|------------------------|
| **`Containerfile`** | `astral-key` — the auth service | 8080 | **Yes.** This is the one. |
| `Dockerfile` | `mosaic-identity` — the PKI sidecar | 8081 | No. Secondary service. |

Always pass `-f Containerfile` for the astral-key service. `docker build .`
with no `-f` picks `Dockerfile` and silently produces a *mosaic-identity*
image. This is not theoretical: CI did exactly that for a long time and went
green on every run while building the wrong binary. There is no image published
to any registry for either service, so a bare `docker compose up` cannot work
either (see below).

---

## Quick Start (local, no registry)

```bash
# Clone the repository
git clone https://github.com/reverb256/astral-key.git
cd astral-key

# Set a strong JWT secret
export JWT_SECRET=$(openssl rand -hex 32)

# Build the SERVICE image -- -f Containerfile is required
docker build -f Containerfile -t astral-key:local .

# Run it. -w /data AND DATABASE_URL are both required -- see the note below.
docker run -d --name astral-key -p 8080:8080 -w /data \
  -e JWT_SECRET="$JWT_SECRET" \
  -e SERVER_HOST=0.0.0.0 \
  -e DATABASE_URL="sqlite:astral-key.db?mode=rwc" \
  -v "$PWD/data:/data" \
  astral-key:local

# Verify
curl http://localhost:8080/health    # => 200
```

No external database, Redis, or Vaultwarden is required. Astral Key embeds
SQLite and persists data on a Docker volume.

### Why `-w /data`, `DATABASE_URL`, and a uid-1000 data dir are all mandatory

Three separate conditions, each verified on nexus 2026-10-06. Getting any of
them wrong produces the same single opaque error:

```
Error: Database error: error returned from database: (code: 14) unable to open database file
```

1. **`-w /data` is required.** The image sets no `WORKDIR`, so it starts in `/`,
   which uid 1000 cannot write. `DATABASE_URL` defaults to the *relative*
   `sqlite:astral_key.db?mode=rwc`, so the db file is created in the current
   directory — `/` — and fails.

2. **`DATABASE_URL` must be relative, not an absolute host path.**
   `sqlite:///home/you/data/ak.db` is resolved by SQLite *inside the container*,
   where that host path does not exist, so it fails identically. A relative name
   plus `-w /data` is the form that resolves on both sides.

3. **The data directory must be owned by uid 1000.** A Docker **named volume**
   does not satisfy this: Docker creates it root-owned `0755`, so `USER 1000`
   gets `Permission denied`. Use a **bind mount of a directory you chown**:

   ```bash
   mkdir -p data && sudo chown 1000:1000 data
   ```

   ```bash
   # works — verified: /health => 200, docker health => healthy
   docker run -d -w /data -e DATABASE_URL="sqlite:astral-key.db?mode=rwc" \
     -v "$PWD/data:/data" astral-key:local

   # both of these die with (code: 14)
   -v astral-key-data:/data                                   # named volume, root-owned
   -e DATABASE_URL="sqlite:astral_key.db?mode=rwc"            # relative, cwd=/ unwritable
   ```

In-cluster this is handled for you: the chart sets `nodeName`,
`persistence.hostPath: /var/lib/astral-key`, mounts it at `/data`, and sets
`env.databaseUrl`. The deployed pod is unaffected by any of the above —
confirmed live: the running pod serves `/health` 200.

### Docker Compose

`docker-compose.yml` encodes all three requirements (`working_dir: /data`,
a relative `DATABASE_URL`, and a bind mount via `AK_DATA_DIR`), so:

```bash
docker build -f Containerfile -t ghcr.io/reverb256/astral-key:latest .
mkdir -p data && sudo chown 1000:1000 data
AK_DATA_DIR="$PWD/data" JWT_SECRET=$(openssl rand -hex 32) docker compose up -d
curl http://localhost:8080/health     # => 200 OK
```

`AK_DATA_DIR` uses `:?` in the compose file, so compose refuses to start with a
clear message instead of failing later with `(code: 14)`.

The compose healthcheck is a `bash` TCP probe rather than `curl`: the runtime
stage is `debian:bookworm-slim`, which ships **neither curl nor wget**, so the
previous `curl -f` test could never pass and reported unhealthy forever.

### There is no published image

`docker-compose.yml` references `ghcr.io/reverb256/astral-key:latest`, and older
revisions of this document claimed that image was published. **It does not
exist.** Verified 2026-10-06:

```
$ gh api /users/reverb256/packages/container/astral-key
{"message":"Package not found.", "status":"404"}
```

So `docker compose up -d` fails on image pull. Build locally with the
`Containerfile` as above, or point compose at your own tag:

```bash
docker build -f Containerfile -t astral-key:local .
docker tag astral-key:local ghcr.io/reverb256/astral-key:latest   # local only
docker compose up -d                                             # now resolves
```

Nothing is pushed anywhere. The `ghcr.io/` prefix is just a local tag name at
that point.

---

## Docker Compose (Detailed)

See [`docker-compose.yml`](../docker-compose.yml) for the canonical file.

### Building the image locally

```bash
docker build -f Containerfile -t astral-key:local .   # Containerfile, not Dockerfile
docker tag astral-key:local ghcr.io/reverb256/astral-key:latest
docker compose up -d
```

### Using a pre-built image

There isn't one. No registry in this cluster holds an astral-key image, and
`ghcr.io/reverb256/astral-key` was never published (see above). Build it
locally, or see [K3s deployment](#kubernetes-k3s) for how the deployed image is
produced and where it lives.

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

No NixOS hosts remain in this fleet — everything is Omarchy/Arch since
2026-09-17, so `/etc/nixos/k8s/` paths in older docs are historical. The flake
below is still useful as a dev shell.

### Nix Flake (Dev Shell)

```bash
nix develop
cargo build --release
./target/release/astral-key
```

### NixOS Module

> **Note:** No NixOS module currently exists. A NixOS module for the auth
> sidecar, MIS, and bridges is tracked in [issue #19](https://github.com/reverb256/astral-key/issues/19).
>
> The `flake.nix` at project root provides a dev shell (`nix develop`) and
> package build via Crane. No production-ready NixOS service declaration yet.
>
> The `nix/` directory referenced in older documentation was never created.
> Deploy via Docker Compose or K3s instead (see below).

---

## Kubernetes (K3s)

Astral Key is deployed by **ArgoCD** from the Helm chart at
[`charts/astral-key/`](../charts/astral-key/), not from manifests in this repo.
There is no `k8s/` directory here; the deployment templates are
`charts/astral-key/templates/*.yaml` and values are in `charts/astral-key/values.yaml`.
Ignore any older reference to `k8s/astral-key-deployment.yaml` — it does not
exist in the repo.

### There is no registry

`nexus:5000` was decommissioned on 2026-09-17 and must not be resurrected. No
other registry exists in this cluster. The deployment therefore runs
registry-free:

- `pullPolicy: Never` — kubelet never pulls.
- the image lives **only** in the containerd store of one node (`nodeName: nexus`),
- the reference is `docker.io/library/astral-key@sha256:<digest>` — a *named*
  digest reference, not a tag.

### Build, import, and name it

Run this on **nexus** (the node that runs the pod). Never on zephyr: ~12 GB RAM,
cordoned, and a Rust release build OOMs it.

```bash
# 1. Build the service image. -f Containerfile is mandatory.
sudo -n docker build -f Containerfile -t docker.io/library/astral-key:local .

# 2. Read back the digest this build actually produced.
sudo -n docker image inspect docker.io/library/astral-key:local \
  --format '{{index .RepoDigests 0}}' 2>/dev/null || \
sudo -n docker image inspect docker.io/library/astral-key:local \
  --format '{{.Id}}'

# 3. Import into the node's containerd store.
sudo -n docker save docker.io/library/astral-key:local \
  | sudo -n k3s ctr images import -

# 4. THE STEP THAT IS EASY TO MISS: name the pinned digest in the store.
#    Under pullPolicy: Never, CRI resolves repo@sha256:... ONLY if a reference
#    of that exact name exists. A tag alone does not satisfy it, so a fully
#    imported image can still leave the pod at ErrImageNeverPull.
sudo -n k3s ctr images tag --force \
  docker.io/library/astral-key:local \
  docker.io/library/astral-key@sha256:<digest-from-step-2>

# 5. Confirm the NAME is present (not just the content digest).
sudo -n k3s ctr images ls -q | grep 'astral-key@sha256:'
```

A one-liner that does 3-5 correctly:

```bash
scripts/ensure-image.sh --build    # reads repo+digest from the chart, no-ops if already present
```

Note that `--build` produces a **new** digest, so it does not satisfy an
existing pin. See [Recovering a Missing Image](#recovering-a-missing-image) for
the correct recovery semantics.

### Re-pin after a rebuild

A rebuild never reproduces the previous digest, so `values.yaml` must be updated
or the pod will sit at `ErrImageNeverPull` against the old pin. Edit
`charts/astral-key/values.yaml`:

```yaml
images:
  astral-key:
    digest: "sha256:<new digest>"
    tag: "<date>-sha.<short digest>"   # annotation only, not used to pull
```

ArgoCD self-heals from `main`, so this is the only place the change belongs —
a cluster-only edit gets reverted.

### Minimal deployment shape

For reference, the rendered essentials (full templates in
`charts/astral-key/templates/`):

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
        image: docker.io/library/astral-key@sha256:<digest>
        imagePullPolicy: Never
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

## Recovering a Missing Image

There is no registry. `nexus:5000` was decommissioned on 2026-09-17, so the
image exists only in each node's containerd store and the Deployment references
it as `<repo>@sha256:<digest>` with `pullPolicy: Never`. Nothing re-pulls it.
If the store loses it — `ctr` garbage collection, a rebuilt node, a node
restored from backup — the pod waits at `ErrImageNeverPull` until it is put
back by hand.

```bash
# On the node that runs the pod. Reads the digest from the chart values, so it
# cannot drift from what is deployed. Safe to re-run.
scripts/ensure-image.sh

# If the content is gone entirely and you have a `docker save` tar of the
# original build:
scripts/ensure-image.sh --from-tar ./astral-key.tar
```

Confirm what the store actually holds:

```bash
sudo k3s ctr -n k8s.io images ls -q | grep 'astral-key@'
```

**The trap:** a `:tag` is not enough, even when it points at exactly the right
layers. Under `pullPolicy: Never` CRI resolves `repo@sha256:...` only if the
store carries that *name*. So an image can be fully present, visible in
`ctr images ls`, and still leave the pod at `ErrImageNeverPull`. `ensure-image.sh`
always ends by creating the `@sha256:` name, so it is that final step that
matters:

```bash
sudo k3s ctr -n k8s.io images tag --force <repo>:<tag> <repo>@sha256:<digest>
```

Read the digest back from `ctr images ls` rather than typing it — `ctr images
tag` accepts any well-formed digest, so a typo produces a pin that looks valid
and resolves to nothing.

Rebuilding is *not* recovery. A fresh build produces a different digest, so if
the content is genuinely gone, restoring the existing pin requires the original
tar; a rebuild means changing the pin and letting ArgoCD sync.

Build with the `Containerfile`. The root `Dockerfile` builds `mosaic-identity`
on port 8081 — a different service — and is not what this chart deploys.

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

---

## Mosaic Identity Service (MIS) / Bridges

### Architecture

MIS crate (`crates/mosaic-identity/`) is a standalone Rust binary with 16 REST endpoints for Ed25519 key management, cross-protocol identity binding, ML-DSA-65 PQ hybrid signing, BIP-39 mnemonic HD derivation, and agent ephemeral certs.

All bridges were rewritten from Node.js to Rust and live as workspace crates under `crates/`. They deploy as sidecar containers selected via the `BRIDGE_TYPE` env var:

| Bridge | Protocol | Crate Path |
|--------|----------|-----------|
| atproto | PLC/BSky DID resolution | `crates/mosaic-bridge-atproto/` |
| buzz | Nostr WebSocket relay | `crates/mosaic-bridge-buzz/` |
| matrix | Matrix Application Service | `crates/mosaic-bridge-matrix/` (AS :8082) |
| irc | IRC TLS client | `crates/mosaic-bridge-irc/` |
| activitypub | ActivityPub federation | `crates/mosaic-bridge-activitypub/` |
| telegram | Telegram bot | `crates/mosaic-bridge-telegram/` |
| discord | Discord bot | `crates/mosaic-bridge-discord/` |
| haven | Socket.IO adapter | `crates/mosaic-bridge-haven/` |

Plus a shared library: `crates/mosaic-client/`.

### Quick start (standalone)

```bash
cargo run -p mosaic-identity -- --database "sqlite:///tmp/mis.db?mode=rwc"
curl http://localhost:8081/health
curl -X POST http://localhost:8081/keys/generate -H 'Content-Type: application/json' -d '{}'
curl -X POST http://localhost:8081/bindings/resolve -H 'Content-Type: application/json' -d '{"did_or_handle":"bsky.app"}'
```

### Docker build (MIS)

The root `Dockerfile` builds `mosaic-identity`:

```bash
# On nexus. -f Dockerfile (the default) is correct here -- this IS the MIS image.
sudo -n docker build -f Dockerfile -t mosaic-identity:local .

# Same registry-free import path as the service image.
sudo -n docker save mosaic-identity:local | sudo -n k3s ctr images import -
```

The builder stage pins `rust:1.88-slim-bookworm`. It cannot be older than
**1.85**: `Cargo.lock` resolves `clap_lex` 1.1.1, whose manifest requires the
`edition2024` cargo feature stabilized in 1.85, and the build fails on 1.75 with
`feature 'edition2024' is required`. The workspace `rust-version` fields were
raised from 1.75 to 1.85 for the same reason.

### No MIS/bridge deployment manifests exist

Earlier revisions of this document gave commands for deploying MIS and the
bridges. **All of those commands are dead**, for three separate reasons:

1. **`nexus:5000` was decommissioned 2026-09-17.** Verified 2026-10-06:
   nothing listens on port 5000 and `curl http://nexus:5000/v2/` returns
   `000` (no route to host). Do not resurrect it.
2. **`Dockerfile.mosaic-identity` and `Dockerfile.bridges` do not exist.** The
   root `Dockerfile` builds MOSAIC identity; the bridges have per-protocol
   files under `docker/` (`Dockerfile.atproto`, `Dockerfile.buzz`, …). Use
   `-f Dockerfile` for MIS and `-f docker/Dockerfile.<proto>` for a bridge.
3. **`/etc/nixos/k8s/` does not exist.** All hosts are Omarchy/Arch since
   2026-09-17; there are no NixOS hosts, and no MIS or bridge workload is
   deployed from this repo today. Astral Key itself deploys via ArgoCD from
   `charts/astral-key/`, not via `kubectl apply`.

If MIS needs deploying, the deploy path has to be written and committed first —
there is no existing one to copy.

### Known issues

- **PVC slow**: `local-path` provisioner takes ~30s. Workaround: `emptyDir: {}`.
- **Bridge UID**: Container `appuser` = UID 100. k8s `runAsUser: 100` required.
- **Container image must be named, not just present** — under `pullPolicy: Never`
  the `@sha256:` name has to exist in the store or the pod stays at
  `ErrImageNeverPull`. `scripts/ensure-image.sh` handles this; see
  [Recovering a Missing Image](#recovering-a-missing-image).
