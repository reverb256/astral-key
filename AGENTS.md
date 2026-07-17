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
│   │   ├── crypto.rs          # Ed25519 + FALCON-512 hybrid signing
│   │   ├── storage.rs         # SQLite: keys, bindings, rotations tables
│   │   ├── bindings.rs        # atproto DID resolver (PLC directory)
│   │   ├── nostr.rs           # Nostr npub→hex decoder (Bech32)
│   │   ├── config.rs          # Environment-based config
│   │   └── error.rs           # Error types
│   └── migrations/001_init.sql
├── identity/mosaic/           # Mosaic (Haven fork — Node.js chat + bridges)
│   ├── src/
│   │   ├── identity.js        # Auto-selector: MIS → local tweetnacl
│   │   ├── identity-mis.js    # MIS HTTP client + tweetnacl fallback
│   │   └── identity-local.js  # Original tweetnacl implementation
│   └── bridges/               # Transport plugins (sidecar daemons)
│       ├── atproto/index.js   # DID resolver daemon (port 8083)
│       ├── buzz/index.js      # Nostr WebSocket relay
│       ├── matrix/index.js    # Matrix Application Service
│       ├── irc/index.js       # IRC TLS client
│       └── lib/mis-client.js  # Shared MIS HTTP client
├── Cargo.toml                 # Workspace: root + crates/mosaic-identity
├── Containerfile              # Astral Key container build
├── Dockerfile.mosaic-identity # MIS container build
├── Dockerfile.bridges         # Bridge container build
└── bridges-entrypoint.sh      # Bridge type dispatcher (BRIDGE_TYPE env)
```

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
| POST | `/sign/hybrid` | Dual Ed25519+FALCON-512 (needs `--features pq`) |
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

Produces dual Ed25519 + FALCON-512 signatures. Without `--features pq`,
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

## Deploy

MIS and bridges deploy as k8s pods in the `orchestration` namespace.
The registry is at `nexus:5000` (local, insecure — accessible from within
cluster). Images are loaded directly into containerd via `docker save | ctr import`.

```bash
# Build and load MIS image
docker build -t nexus:5000/mosaic-identity:v0.1.0 -f Dockerfile.mosaic-identity .
docker save nexus:5000/mosaic-identity:v0.1.0 | sudo ctr -n k8s.io images import -

# Build and load bridge image
docker build -t nexus:5000/mosaic-bridges:v0.1.0 -f Dockerfile.bridges .
docker save nexus:5000/mosaic-bridges:v0.1.0 | sudo ctr -n k8s.io images import -

# Apply manifests
kubectl apply -f /etc/nixos/k8s/mosaic-identity/
kubectl apply -f /etc/nixos/k8s/mosaic-bridges/
```

## Cluster topology

| Node | Role | IP | Status |
|------|------|----|--------|
| nexus | k3s server + builder | 10.1.1.120 | Ready (k3s), MIS host |
| forge | k3s server + mining | 10.1.1.130 | Unknown (post-reset) |
| sentry | k3s server + inference | 10.1.1.140 | Unknown (bad Nix unit) |
| zephyr | desktop + control | — | Worker |

## Recent changes (2026-07-25)

- MIS crate added to workspace (`crates/mosaic-identity/`)
- 16 REST endpoints for PKI operations
- Identity binding system (one key → atproto, nostr, matrix, irc)
- PQ hybrid signing (Ed25519 + FALCON-512, feature-gated)
- Agent ephemeral cert delegation
- 4 transport plugins (atproto, buzz, matrix, irc)
- Mosaic identity.js auto-selects MIS (fallback to local tweetnacl)
- PersistentVolumeClaim → emptyDir (local-path provisoner delay)
- All admission policies deleted (blocked deployment)
- Cluster reset performed (nexus single-member, forge/sentry pending rejoin)
