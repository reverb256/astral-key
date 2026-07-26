# Mosaic Identity Foundation

> **Your identity, your keys, every protocol.**
> One Ed25519 keypair (doubled with ML-DSA-65 for post-quantum safety) that
> federates across every chat and social protocol you use — and an auth
> sidecar that lets you unlock it with a passkey or a wallet signature.

The Mosaic Identity Foundation is a **self-sovereign identity stack** in three layers, all in a single Rust workspace:

```
┌─────────────────────────────────────────────────┐
│   Astral Key — auth sidecar (port 8080)          │
│   FIDO2/WebAuthn passkeys · Web3/SIWE · JWT     │
│   API keys · ZK JIT capability tokens           │
│   SQLite-only (no Postgres, no Redis)           │
├─────────────────────────────────────────────────┤
│   Mosaic Identity Service (MIS) — PKI (port 8081)│
│   Ed25519 + ML-DSA-65 hybrid keys               │
│   Sign, verify, hybrid verify                   │
│   Agent ephemeral cert delegation               │
│   Identity bindings: one key → many protocols   │
├─────────────────────────────────────────────────┤
│   9 bridge daemons (one per protocol)            │
│   atproto · buzz (nostr) · matrix · irc         │
│   activitypub · telegram · discord · haven       │
│   + mosaic-client shared lib                    │
└─────────────────────────────────────────────────┘
```

**What this is not.** It is not an SSO portal (Keycloak), not a cloud identity
provider (Auth0), not a wallet (MetaMask), and not a DID/VC framework. It is
a **local identity sidecar** — a Rust binary you run on your own machine (or
your homelab NixOS node) that holds your keys, signs on your behalf, and
translates between protocol identities. No cloud, no token, no third party.

---

## Core concept: one key, many identities

Most people have:

| Identity | How it works today |
|---|---|
| Bluesky | You control the `did:plc:...` key — but only through Bluesky's PDS. |
| Matrix | Keys tied to your homeserver. Different server = different identity. |
| Nostr | You already control the npub — but managing it across relays is fragmented. |
| Discord / Telegram | The platform owns the identity. You're a tenant. |
| Haven | Self-hosted chat, but identity is SQLite-based, not portable. |

Mosaic binds all of these to **one Ed25519 key that you generate and hold**. The
flow is:

```
1.  POST /keys/generate            →  Ed25519 + ML-DSA-65 keypair
2.  POST /bindings/claim           →  key_id ↔ atproto:DID ↔ nostr:npub ↔ ...
3.  POST /sign                     →  sign a message with your unified key
4.  POST /verify                   →  anyone verifies using your pubkey
```

A bridge daemon for each protocol watches for events from that network and
resolves identities back to your Mosaic key via the MIS `resolve` endpoint.
The whole system works offline — the PKI is local SQLite; only the bridges
need network connectivity.

---

## Quickest possible start

### Prerequisites

- Rust toolchain (`rustup` or Nix)
- SQLite (provided by the binary — `sqlx` opens/creates the file)

### Run the identity engine (MIS)

```bash
cd crates/mosaic-identity

# With Ed25519 only (default, ~5 MB binary):
cargo run -- --database /tmp/mis.db

# With ML-DSA-65 post-quantum hybrid (adds ~1 MB, FIPS 204):
cargo run --release --features pq -- --database /tmp/mis-pq.db
```

The MIS starts on `0.0.0.0:8081` (configurable via `MIS_HOST`/`MIS_PORT` or
`--host`/`--port`).

### Generate a key

```bash
curl -s -X POST http://localhost:8081/keys/generate | jq .
```

Returns:
```json
{
  "key_id": "k-a1b2c3d4e5f6g7h8",
  "pubkey_hex": "d75a980182b10ab7...",
  "privkey_pkcs8_hex": "...",
  "algorithm": "Ed25519+ML-DSA-65",
  "created_at": "2026-07-26T...",
  "rotated_from": null
}
```

The keypair is persisted in SQLite. The `key_id` is all you need for
subsequent operations — private key material is only returned once (at mint
time) and never re-exposed. **Save it outside the database.**

### Sign and verify (Ed25519)

```bash
# Sign a message (hex-encoded)
SIG=$(curl -s -X POST http://localhost:8081/sign \
  -H 'Content-Type: application/json' \
  -d '{"key_id": "k-a1b2c3d4e5f6g7h8", "message_hex": "48656c6c6f"}' | jq -r '.signature_hex')

# Verify the signature (anyone with your pubkey can do this)
curl -s -X POST http://localhost:8081/verify \
  -H 'Content-Type: application/json' \
  -d "{\"pubkey_hex\": \"d75a980182b10ab7...\", \"message_hex\": \"48656c6c6f\", \"signature_hex\": \"$SIG\"}" | jq .
# → {"valid": true}
```

### Hybrid sign and verify (Ed25519 + ML-DSA-65, requires `--features pq`)

```bash
# Hybrid sign — produces both sigs
curl -s -X POST http://localhost:8081/sign/hybrid \
  -H 'Content-Type: application/json' \
  -d '{"key_id": "k-a1b2c3d4e5f6g7h8", "message_hex": "48656c6c6f"}' | jq .
# → {"ed25519_sig": "...", "ml_dsa_sig": "...", "algorithm": "ed25519+ml-dsa-65", ...}

# Hybrid verify — checks both sigs; rejects if either is forged
curl -s -X POST http://localhost:8081/verify/hybrid \
  -H 'Content-Type: application/json' \
  -d '{"pubkey_hex": "...", "message_hex": "48656c6c6f", "ed25519_sig": "...",
       "ml_dsa_sig": "...", "ml_dsa_pubkey_hex": "..."}' | jq .
```

**This is the post-quantum safe path.** A quantum computer breaking Ed25519
(Shor) still cannot forge the ML-DSA half — hybrid verification requires
*both* to pass.

### Bind a key to an external identity

```bash
# Bind your Mosaic key to your Bluesky DID
curl -s -X POST http://localhost:8081/bindings/claim \
  -H 'Content-Type: application/json' \
  -d '{"key_id": "k-a1b2c3d4e5f6g7h8", "protocol": "atproto", "external_id": "did:plc:abc123"}'

# Resolve the binding back to your Mosaic key
curl -s "http://localhost:8081/resolve?protocol=atproto&id=did:plc:abc123" | jq .
# → {"key_id": "k-...", "pubkey_hex": "...", "algorithm": "Ed25519", ...}
```

The same pattern works for any registered bridge protocol: `nostr`, `matrix`,
`irc`, `discord`, `telegram`, `activitypub`, `haven`.

---

## What's implemented vs what's planned

| Layer | Status |
|---|---|
| **MIS PKI** — key gen, sign/verify, binding, rotation, agent certs | ✅ Complete, compiled, tested |
| **ML-DSA-65 hybrid** (FIPS 204) | ✅ Feature-gated (`pq`), verified |
| **9 bridge daemons** — atproto, buzz, matrix, irc, activitypub, telegram, discord, haven | ✅ All compile, all use `mosaic-client` |
| **Auth sidecar** — passkeys, SIWE, JWT, JIT tokens, API keys | ✅ Complete per `IMPLEMENTATION_SUMMARY.md` |
| **Haven adapter** | ✅ `mosaic-bridge-haven` (Socket.IO client) |
| **NixOS module** — declarative service config for MIS + bridges | ⬜ Planned — `flake.nix` exists but no production module |
| **CI/CD** — release pipeline, container publish | ⬜ Planned |
| **Web UI** — key management dashboard | ⬜ Not started |
| **Onboarding docs** — "run your own identity layer in 5 minutes" | 📝 This document |

---

## Architecture notes

**Every bridge is a standalone daemon** with the same structure:

```
GET  /health    → {"status": "ok", "service": "discord-bridge", "guilds": 5}
POST /send      → {"channel_id": "...", "content": "..."}
```

Each bridge uses `mosaic-client` to resolve users on that protocol back to
Mosaic keys via MIS. The bridges are sidecars — they don't modify the
protocol's identity model; they just map it onto the Mosaic key space.

**The auth sidecar (Astral Key)** is the authentication entry point for the
whole Foundation. It lives on port 8080, handles passkey registrations,
wallet logins, and JWT issuance, and its Ed25519 JIT tokens are now ML-DSA
hybrid too.

---

## Status (2026-07-26)

This is a **live homelab project** running on a 4-node NixOS/K3s cluster
at `reverb256` (forge, nexus, sentry, krash3). The MIS and bridges run as
k8s pods in the `orchestration` namespace. The auth sidecar is deployed via
`k8s/astral-key-deployment.yaml`. Continuous deployment is manual
(`cargo build → docker save → ctr import`); CI/CD is the next gap.

The Mosaic Identity Foundation is what the code does. **What to call it**
is the open question — see the repository README for the current name.
