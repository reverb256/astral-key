# PROJECT KNOWLEDGE BASE

**Generated:** 2026-07-25
**Type:** Rust microservice (auth sidecar) + Mosaic Identity Service (PKI layer)
**Status:** Core auth complete, MIS shipped, bridges deployed

## OVERVIEW

Astral Key is a single-binary authentication sidecar for FIDO2/WebAuthn
passkey and Web3/SIWE authentication. Built with Rust (Axum) and SQLite.

The repo also contains the **Mosaic Identity Service (MIS)** crate — a
standalone PKI service for Ed25519 key management, cross-protocol identity
binding, PQ hybrid signing, and agent ephemeral certs. MIS is consumed by
Mosaic (Haven fork), transport plugins, and Hermes agents.

## STRUCTURE

```
astral-key/
├── src/                       # Astral Key auth microservice
│   ├── main.rs, config.rs, error.rs, state.rs
│   ├── api/routes.rs + handlers/   # FIDO2, SIWE, JWT, OAuth endpoints
│   ├── auth/                    # WebAuthn, Web3, JWT logic
│   └── db/                      # SQLx SQLite migrations + models
├── crates/mosaic-identity/    # ← Mosaic Identity Service (Rust binary)
│   ├── src/
│   │   ├── main.rs            # HTTP server entry point
│   │   ├── lib.rs             # Re-exports all modules
│   │   ├── api.rs             # 16 route handlers (keys, sign, verify, bindings)
│   │   ├── crypto.rs          # Ed25519 + ML-DSA-65 (FIPS 204) hybrid signing
│   │   ├── storage.rs         # SQLite: keys, bindings, rotations tables
│   │   ├── bindings.rs        # atproto DID resolver (PLC directory)
│   │   ├── nostr.rs           # Nostr npub→hex decoder (Bech32)
│   │   ├── config.rs          # Environment-based config
│   │   └── error.rs           # Error types
│   └── migrations/001_init.sql
├── crates/mosaic-bridge-haven/  # Haven adapter (Socket.IO) — see below
├── Cargo.toml                 # Workspace: root + 10 crates under crates/
├── Containerfile              # Astral Key SERVICE image <- what the chart deploys
├── Dockerfile                 # mosaic-identity sidecar (port 8081) <- different service
├── docker/Dockerfile.<proto>  # Per-bridge images (atproto, buzz, matrix, ...)
└── scripts/ensure-image.sh    # Registry-free image recovery
```

**Two build files, two services.** Always pass `-f Containerfile` for the
astral-key service; the bare `Dockerfile` builds the mosaic-identity sidecar.
CI omitted `-f` for a long time and stayed green while producing the wrong
binary, so this is a known, previously-shipped defect — not a style preference.

## Mosaic Identity Service (MIS)

Standalone Rust binary in `crates/mosaic-identity/`. 16 HTTP endpoints:

| Method | Path | Purpose |
|--------|------|---------|
| GET | `/health` | Liveness |
| POST | `/keys/generate` | Create Ed25519 keypair |
| POST | `/keys/import` | Import PKCS#8 key |
| GET | `/keys` | List keys |
| GET | `/keys/{key_id}` | Key info |
| GET | `/keys/{key_id}/history` | Rotation history |
| POST | `/sign` | Ed25519 sign |
| POST | `/verify` | Ed25519 verify |
| POST | `/sign/hybrid` | Dual Ed25519+ML-DSA-65 (needs `--features pq`) |
| POST | `/verify/hybrid` | Verify hybrid signature |
| POST | `/bindings/resolve` | atproto DID → Mosaic key |
| POST | `/bindings/claim` | Bind key to external identity |
| GET | `/keys/{key_id}/bindings` | List bindings |
| GET | `/resolve` | Resolve external ID → key |
| POST | `/nostr/resolve` | npub → hex |
| POST | `/agent/cert` | Time-bound agent delegation |

### Key identity binding workflow

One Mosaic Ed25519 key maps to multiple protocol identities:

```
POST /keys/generate         → key_id k-xxx, pubkey_hex
POST /bindings/resolve      → resolve bsky.app → did:plc:... → secp256k1 key
POST /bindings/claim        → k-xxx ↔ did:plc:... (bidirectional signed binding)
GET  /resolve?protocol=...  → resolve any external ID → Mosaic key
```

### Post-quantum hybrid signing

```bash
cargo build -p mosaic-identity --features pq --release
```

Produces dual Ed25519 + ML-DSA-65 signatures. Without `--features pq`,
`POST /sign/hybrid` returns an error: "PQ feature not enabled. Rebuild with
--features pq". No fake/placeholder signatures.

### Agent ephemeral certs

`POST /agent/cert` issues time-bound, scope-limited delegation certificates.
Long-lived keys never enter agent memory. Certificate structure:

```json
{
  "owner_pubkey": "...", "agent_pubkey": "...",
  "expires_at": "ISO8601", "scope": ["#channel"],
  "signature": "<signed by owner Ed25519 key>"
}
```

## Transport plugins (bridges)

Each bridge is a sidecar container. Selection via `BRIDGE_TYPE` env var.

| Bridge | Dependencies | Deploy | Notes |
|--------|-------------|--------|-------|
| atproto | MIS, public PLC/BSky APIs | `BRIDGE_TYPE=atproto` | Daemon on :8083, DID resolution |
| buzz | MIS, Nostr relay URL | `BRIDGE_TYPE=buzz` | WebSocket relay, identity binding |
| matrix | MIS, Matrix homeserver | `BRIDGE_TYPE=matrix` | AS server on :8082 |
| irc | MIS, IRC server | `BRIDGE_TYPE=irc` | TLS client, channel mapping |
| activitypub | MIS, ActivityPub | `BRIDGE_TYPE=activitypub` | Federation protocol |
| telegram | MIS, Telegram Bot API | `BRIDGE_TYPE=telegram` | Bot API client |
| discord | MIS, Discord Bot API | `BRIDGE_TYPE=discord` | Bot API client |
| haven | MIS, Haven server | `BRIDGE_TYPE=haven` | Socket.IO adapter |

## Environment variables (key subset)

All config is read from env vars at runtime (see `src/config.rs` — `from_env()`).
Full table with defaults: `docs/deployment.md` → Environment Variables.

| Variable | Default | Description |
|----------|---------|-------------|
| `SERVER_HOST` / `SERVER_PORT` | `127.0.0.1` / `8080` | Listen address |
| `DATABASE_URL` | `sqlite:astral_key.db?mode=rwc` | SQLite URL |
| `JWT_SECRET` | — (required) | JWT signing key, ≥32 bytes. `openssl rand -hex 32` |
| `FIDO2_RP_ID` | `localhost` | WebAuthn relying party ID |
| `ASTRAL_WEB3_DOMAIN` | `maplespike.ca` | Canonical SIWE domain |
| `JIT_ISSUER_KEY` | — (optional) | Ed25519 private key (64 hex). Enables JIT token minting |
| `JIT_ISSUER_ID` | `ak:issuer:01` | Issuer ID embedded in minted JIT tokens |
| `JIT_DEFAULT_TTL` | `3600` | Default TTL (seconds) for JIT tokens |
| `OAUTH_BASE_URL` + `OAUTH_GITHUB_*` | — (optional) | GitHub OAuth (omit to disable) |
| `RUST_LOG` | `info,astral_key=debug` | Tracing filter |

## Deploy

**Astral Key is deployed by ArgoCD from `charts/astral-key/` in this repo.**
There is no registry, and `nexus:5000` was decommissioned 2026-09-17 — do not
resurrect it. The image is built on **nexus** and imported into that node's
containerd store with `pullPolicy: Never` and a digest pin.

```bash
# Build the SERVICE image (Containerfile, not Dockerfile)
sudo -n docker build -f Containerfile -t docker.io/library/astral-key:local .

# Import, then create the @sha256 NAME (a tag alone will not resolve)
sudo -n docker save docker.io/library/astral-key:local | sudo -n k3s ctr images import -
sudo -n k3s ctr images tag --force docker.io/library/astral-key:local \
  docker.io/library/astral-key@sha256:<digest>

# Then update images.astral-key.digest in charts/astral-key/values.yaml and push.
# ArgoCD is automated + selfHeal, so a cluster-only edit gets reverted.
```

A rebuild yields a NEW digest, so the `values.yaml` pin must be updated or the
pod sits at `ErrImageNeverPull`.

**Recovery when the image is genuinely missing from a node's store:**

```bash
scripts/ensure-image.sh --from-tar ./astral-key.tar   # restores content + name
scripts/ensure-image.sh                                # idempotent check
```

The script guarantees the pinned digest is present *and named* in a node's
containerd, and exits 1 explaining why when it cannot. This is a manual
procedure — nothing runs it automatically. Details:
[`docs/deployment.md`](docs/deployment.md#recovering-a-missing-image).

**MIS and bridges have no deploy path in this repo.** No manifests, no
`/etc/nixos/k8s/` (all hosts are Omarchy/Arch since 2026-09-17), and no
`Dockerfile.mosaic-identity` / `Dockerfile.bridges` (use `-f Dockerfile` for
MIS, `-f docker/Dockerfile.<proto>` for a bridge). Any command of that shape is
historical — do not run it.

## Cluster topology

| Node | Role | Status |
|------|------|--------|
| nexus | k3s server + builder | Ready (k3s), MIS host |
| forge | k3s server + mining | Unknown (post-reset) |
| sentry | k3s server + inference | Unknown (bad Nix unit) |
| zephyr | desktop + control | Worker |

Addresses are deliberately not listed: this repository is public. They live in
the private `reverb256/ops-log` inventory.

## Recent changes (2026-07-25)

- MIS crate added to workspace (`crates/mosaic-identity/`)
- 16 REST endpoints for PKI operations
- Identity binding system (one key → atproto, nostr, matrix, irc)
- PQ hybrid signing (Ed25519 + ML-DSA-65, feature-gated)
- Agent ephemeral cert delegation
- 8 transport plugins (atproto, buzz, matrix, irc, activitypub, telegram, discord, haven)
- Mosaic identity.js auto-selects MIS (fallback to local tweetnacl)
- PersistentVolumeClaim → emptyDir (local-path provisoner delay)
- All admission policies deleted (blocked deployment)
- Cluster reset performed (nexus single-member, forge/sentry pending rejoin)

---

## Git hygiene rules for every agent on this fleet

Status: shared rule block. Paste into a repo's `AGENTS.md`, `CLAUDE.md`, or a
profile's `SOUL.md`. Owner: Reverb. Added 2026-10-23 by fleet-leakguard.

This block starts at H2 on purpose: pasting an H1 into a document that already
has a title trips markdownlint MD025 (`single-h1`). Every fenced block here is
fenced rather than indented so MD046 (`code-block-style`) stays clean in the
host repo.

These rules are mandatory for all Hermes profiles, bots, and delegated agents.
They exist because an agent ran `git add -A` in `reverb256.github.io` and
committed `.freebuff/` session databases that held live Cloudflare OAuth tokens
plus the operator's real name and city into a public repo (commit `051a6be`).

### Stage explicit paths. Never stage a tree.

- Use `git add <path> [<path> ...]` with the paths you actually changed.
- Never run `git add -A`, `git add .`, `git add --all`, or `git commit -a`.
- Never use `git stash --include-untracked` as a shortcut before staging.

### Never commit tool or session state

Forbidden in any commit, at any depth:

```text
.freebuff/  .sisyphus/  .serena/  .playwright-mcp/  .codebuff/
.cursor/    .claude/    .aider*   .hermes/          .opencode/
.continue/  playwright-report/    .wrangler/        .dev.vars
*.db  *.db-wal  *.db-shm  *.sqlite*  .env*  *.pem  *.key
```

A global exclude file blocks most of these, but the exclude list is a net, not
permission. If a path is not part of the change, do not stage it.

### Do not bypass the gates

- Never pass `--no-verify` to `git commit` or `git push`.
- Never set `core.hooksPath` to an empty directory inside a repo.
- Never delete or edit `~/.config/git/ignore` or anything in `~/.config/git/hooks/`.
- Never unset `core.excludesFile`.
- A blocked commit is a finding: report it to the operator. Do not retry with a
  different flag, do not rewrite the finding, do not edit the pattern list.

If a hook blocks work that is genuinely correct, stop and report it. The
operator decides.

### Identity

- Commit as `Reverb <129622142+reverb256@users.noreply.github.com>`.
- Never commit with a real-name or ISP-provided address.
- Never put the operator's real name, city, ISP mailbox, or the home LAN
  subnet plan into a tracked file.

  (This line names no literal address on purpose: a leak-guard that scans for
  the fleet's private subnet range flagged the earlier wording inside this very
  block, which is how `reverb256/blog` and `reverb256/reverb256.github.io`
  caught it. Keep the rule, drop the literal.)

### Before you push

```bash
git status --short          # read every line
git diff --cached --stat    # confirm the staged set
git log --oneline -3        # confirm the author identity
```

Install check on a host: `bash ~/.config/git/leakguard/doctor.sh` (17 checks).
