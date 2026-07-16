# Mosaic Fork Plan

**From Haven (Discord-alike) → Mosaic (Discord + MySpace + Facebook + Matrix)**\n\n**Status: v0.1.8-full — All 6 phases implemented.**\n**[Deployed](https://github.com/reverb256/Mosaic) on K3s cluster at 10.1.1.120:32100.**\n\nMosaic is a fork of [Haven](https://github.com/ancsemi/Haven) (AGPL-3.0) that adds sovereign identity,
customizable profiles, activity feeds, and P2P federation on top of Haven's realtime chat/voice/screenshare
foundation. No domain required. No KYC. No Big Tech.

---

## Architecture

```
┌─────────────────────────────────────────┐
│  Mosaic Core (always runs)              │
│  ├─ Identity (Ed25519 + Passkey + QR)   │
│  ├─ Profiles (optional external)        │
│  ├─ Feeds / Bulletins (optional)        │
│  ├─ Connections (optional)              │
│  └─ Signed Event Bus (optional)         │
├─────────────────────────────────────────┤
│  Mosaic Plugins (modular, swappable)    │
│  ├─ Chat/Discord (Haven, external)      │ ← can skip or delegate
│  ├─ Voice/Video (Haven WebRTC)          │
│  ├─ Music (Haven music system)          │
│  └─ Federation (P2P gossip)            │
└─────────────────────────────────────────┘
```

**Key design decisions:**
- Haven's auth (bcrypt+JWT) co-exists alongside pubkey auth during migration
- No domain required — discovery via QR, IP, onion addresses
- All new features are additive; Haven code is modified as little as possible
- Client-side plugin system (`plugins/`) extended for profile rendering
- **Every feature is a module: optional, skippable, delegatable**

---

## Phase 1: Identity Layer (MVP)

Replace/supplement Haven's bcrypt auth with Ed25519 keypair + Passkey + QR.

### Files to add

| File | Purpose |
|------|---------|
| `src/identity.js` | Ed25519 key generation, signing, verification, pubkey fingerprinting |
| `src/keychain.js` | Seed phrase generation (BIP39), key storage/encryption at rest, social recovery scheme |
| `src/qr.js` | QR code encoding/decoding for pubkey+node info exchange |
| `src/passkey.js` | WebAuthn registration and authentication endpoints |
| `public/js/modules/app-identity.js` | Client-side identity UI (keygen wizard, QR scan/display, Passkey prompts) |
| `public/identity.html` or route | Identity management page |

### Files to modify

| File | Change |
|------|--------|
| `src/auth.js` | Add `/api/pubkey/register`, `/api/pubkey/login`, `/api/identity/info` routes alongside existing auth |
| `src/database.js` | Add `identities`, `passkeys`, `connections` tables to schema |
| `server.js` | Mount new routes; initialize identity module at boot |
| `public/sw.js` | Add identity page to precache |
| `package.json` | Add `@noble/ed25519`, `@noble/hashes`, `@simplewebauthn/server`, `qrcode` (already present) |

### Auth flow

1. First boot: keygen wizard generates Ed25519 keypair + BIP39 seed phrase
2. User registers a Passkey (WebAuthn) — the credential is bound to the pubkey
3. On login: Passkey biometric → signs challenge → JWT issued
4. QR code encodes `{pubkey, display_name, node_url, proto_version}`
5. Scanning a QR adds a "connection" (pubkey-based follow)

### QR payload schema

```json
{
  "v": 1,
  "pk": "ed25519:<base64_pubkey>",
  "n": "display_name",
  "u": "http://10.1.1.120:3000",
  "h": "onion_address.onion"  // optional
}
```

---

## Phase 2: Profiles (MySpace Layer)

Customizable per-user profile pages alongside the chat UI.

### Files to add

| File | Purpose |
|------|---------|
| `src/profiles.js` | Profile manifest CRUD, template engine, media reference resolution |
| `src/profiles-sandbox.js` | CSP sandbox configuration for user-supplied HTML/CSS/JS |
| `public/js/modules/app-profile.js` | Client-side profile editor and viewer |
| `public/js/modules/app-profile-widgets.js` | Widget system (music player, about, recent posts, friends) |
| `themes/mosaic-default/` | Default Mosaic profile theme |

### Files to modify

| File | Change |
|------|--------|
| `src/auth.js` | Add profile endpoint, profile manifest is signed by identity key |
| `src/database.js` | Add `profiles` table with manifest JSON blob |
| `server.js` | Mount `/profile/:pubkey` route |
| `public/sw.js` | Profile caching strategy |

### Profile manifest schema

```json
{
  "version": 1,
  "pubkey": "ed25519:<base64>",
  "display_name": "cooluser",
  "bio": "building sovereign social",
  "avatar": "ipfs://Qm...",
  "background": "ipfs://Qm...",
  "theme": "mosaic-dark",
  "template": "html",  // or "sandboxed_html"
  "content": "<div>my custom profile HTML</div>",
  "widgets": [
    {"type": "music_player", "src": "ipfs://Qm..."},
    {"type": "friends", "limit": 10},
    {"type": "recent_posts", "limit": 5}
  ],
  "links": [
    {"label": "website", "url": "http://..."},
    {"label": "nostr", "url": "nostr:..."}
  ],
  "signature": "<ed25519_sig_of_above>"
}
```

---

## Phase 3: Feeds & Posts (Facebook/Matrix Layer)

Broadcast-style content alongside chat channels.

### Files to add

| File | Purpose |
|------|---------|
| `src/feeds.js` | Signed post creation, feed aggregation, bulletins |
| `public/js/modules/app-feeds.js` | Feed UI component (timeline view, composer) |

### Files to modify

| File | Change |
|------|--------|
| `src/socketHandlers/channels.js` | Add "feed" channel type |
| `src/database.js` | Add `posts`, `reactions`, `bulletins` tables |
| `public/js/modules/app-channels.js` | Feed renderer alongside chat renderer |

### Signed event envelope

```json
{
  "type": "post",
  "pubkey": "ed25519:<base64>",
  "payload": {
    "content": "hello world",
    "media": ["ipfs://Qm..."],
    "reply_to": null,
    "channel": null  // or pubkey for DMs
  },
  "timestamp": 1700000000,
  "signature": "<ed25519_sig>"
}
```

---

## Phase 4: Connections & Following

Pubkey-based social graph.

### Files to add

| File | Purpose |
|------|---------|
| `src/connections.js` | Follow/unfollow, blocklists, connection discovery |

### Files to modify

| File | Change |
|------|--------|
| `src/database.js` | Add `follows`, `blocked`, `groups` tables |
| `src/socketHandlers/users.js` | Add follow/unfollow events |

---

## Phase 5: Signed Event Bus (Federation Foundation)

All user actions produce signed events, laying groundwork for P2P gossip.

### Files to add

| File | Purpose |
|------|---------|
| `src/events.js` | Signed event creation, verification, appending to per-user log |
| `src/event-log.js` | Append-only per-pubkey event log, configurable pruning |

### Files to modify

| File | Change |
|------|--------|
| `src/database.js` | Add `event_log` table |
| `server.js` | Wrap all write operations in event signing |

---

## Phase 6: Federation (P2P Gossip)

Gossip protocol between Mosaic nodes.

### Files to add

| File | Purpose |
|------|---------|
| `src/gossip.js` | P2P gossip protocol over WebSocket |
| `src/ipfs.js` | IPFS client for content-addressed media |
| `src/transport-tor.js` | Tor hidden service discovery |
| `src/transport-lan.js` | LAN discovery via mDNS |

### Files to modify

| File | Change |
|------|--------|
| `server.js` | Initialize gossip module on boot |

---

## Dependencies to Add (by phase)

```
Phase 1:
  @noble/ed25519       — fast Ed25519 operations (no native deps)
  @noble/hashes        — SHA-256, HMAC, etc.
  @simplewebauthn/server — WebAuthn server-side
  bip39                — BIP39 mnemonics (optional, for seed phrases)
  caniuse              — already have qrcode

Phase 2:
  (pure CSS/JS — no new npm deps needed)

Phase 3:
  (uses existing EventEmitter pattern)

Phase 4:
  (uses existing database patterns)

Phase 5:
  (uses existing crypto)

Phase 6:
  libp2p               — P2P networking (or lighter alternative)
  kubo-rpc-client      — IPFS HTTP API client
```

---

## Deployment (K3s)

Already running: `ghcr.io/ancsemi/haven:3.1.1` on nexus, `haven` namespace.

Mosaic will ship as:
- `ghcr.io/reverb256/mosaic:<tag>` (or nexus registry mirror)
- Single OCI container, same pattern as Haven
- Persistent volumes: `/data` for keys, db, media
- No domain needed — FORCE_HTTP=true, expose via NodePort or ClusterIP + Tailscale funnel

---

## Git Strategy

- `main` branch tracks upstream `ancsemi/Haven` (git pull to merge upstream fixes)
- `mosaic` branch is our divergence point
- Feature branches off `mosaic`: `mosaic/phase-1-identity`, `mosaic/phase-2-profiles`, etc.
- Upstream changes merged into `main`, then cherry-pick or rebase `mosaic` onto `main`

---

## Compatibility Guarantee: Total Haven Backward Compatibility

Mosaic is an **additive layer** over Haven. Haven must continue working exactly as it did before Mosaic existed — no behavioral changes, no config migration, no database migration, no frontend changes.

### The Rules

| Rule | Example |
|------|---------|
| **Zero modifications to existing Haven routes** | New Mosaic routes are mounted separately (`/mosaic/*`) or appended to route files (`/api/auth/passkey/*`) |
| **Zero modifications to existing Haven database schema** | Mosaic tables use `CREATE TABLE IF NOT EXISTS` and are added after Haven's existing tables |
| **Zero modifications to existing Haven frontend** | Mosaic UI is in separate files (`identity.html`, `app-identity.js`, `__MOSAIC_IDENTITY__.css`) |
| **Zero new required dependencies for Haven users** | New npm packages (`tweetnacl`, `@simplewebauthn/server`) are only required if Mosaic features are used |
| **Zero config changes for existing Haven installs** | Mosaic reads its config from env vars that default to non-domain localhost values |
| **Zero migration for existing Haven databases** | Identity tables are new — they don't touch users, channels, messages, etc. |

### The One Database Rule

Haven's database contains user chat data. Mosaic's database contains identity keys, passkeys, and contacts. **They coexist in the same SQLite file** but are completely independent. A query against Haven's tables never touches Mosaic's tables and vice versa.

```sql
-- Haven tables (untouched):
CREATE TABLE users (...);
CREATE TABLE channels (...);
CREATE TABLE messages (...);

-- Mosaic tables (added alongside, never modified):
CREATE TABLE identities (...);
CREATE TABLE passkeys (...);
CREATE TABLE contacts (...);
CREATE TABLE sessions (...);
```

### What This Means

- `git pull` upstream Haven changes merge cleanly — Mosaic additions don't conflict
- A fresh install of upstream Haven works identically with or without Mosaic code
- Users who only want chat see no difference
- Users who discover `/identity.html` get the Mosaic layer

---

## Modularity Principle: Every Feature Is Optional, Skippable, Delegatable

Every Mosaic feature is a **module**. You can run it, skip it, self-host it, or delegate it to someone else's server. The system degrades gracefully when a module is absent.

### The Module Contract

Each module:
1. Has a **standalone entry point** (e.g. `node server.js` for identity)
2. Has **zero runtime dependencies** on other Mosaic modules
3. Can **point to an external service** instead of running its own
4. When unavailable, the **frontend hides or degrades** the feature

### Concrete Examples

| Module | Run It | Skip It | Delegate It |
|--------|--------|---------|-------------|
| **Identity** | `docker run reverb256/__MOSAIC_IDENTITY__` | — (core) | Not delegatable |
| **Chat** | Run Haven locally | Remove chat tab from UI | Point to `https://friend.haven.lan` |
| **Profiles** | `node src/profiles.js` | Profile tab hidden | Use external profile service |
| **Feeds** | `node src/feeds.js` | Feed tab hidden | Use external feed service |
| **Music** | Run Haven's music bot | Music widget hidden | Use external streaming |
| **Federation** | `node src/gossip.js` | No P2P (REST only) | Relay through public gateway |

### Implementation Pattern

```javascript
// In the frontend, each module checks if its backend is available:
const features = {
  chat:     await checkEndpoint('/api/auth/me'),
  identity: await checkEndpoint('/mosaic/identity'),
  feeds:    await checkEndpoint('/mosaic/feed/posts'),
  music:    await checkEndpoint('/api/music/status'),
};

// Features not available are hidden from the UI
if (!features.chat)  document.getElementById('chat-tab').style.display = 'none';
if (!features.feeds) document.getElementById('feed-tab').style.display = 'none';
```

### What This Enables

- **Chat-only users**: Run just Haven. No Mosaic code needed.
- **Identity-only users**: Run `__MOSAIC_IDENTITY__` container. No chat server. Use Mosaic for profiles/feeds/auth, link to a friend's Haven for chat.
- **Experienced users**: Self-host everything. Maximum sovereignty.
- **Resource-constrained users**: Delegate chat/music/federation to trusted peers. Run only identity locally.

---

## Composability Principle: Mix and Match Features from Any Servers

Modularity means each feature *can* run independently. Composability means they *do* — a user's Mosaic node pulls features from multiple backends simultaneously, and the system treats this as the normal case, not an edge case.

### The Composability Model

User's Mosaic Frontend connects to:
- Identity from self-hosted Raspberry Pi (__MOSAIC_IDENTITY__)
- Chat from friend's Haven server
- Profiles from self-hosted alongside identity
- Feeds from a community hub
- Music from a different friend's Haven
- Media from IPFS (distributed, no server)

Each feature resolves to a different backend. The frontend discovers which backend serves which feature through a capabilities endpoint.

### The Capabilities Endpoint

Every Mosaic module exposes GET /capabilities returning:
```json
{
  "features": ["identity", "profiles", "feeds", "chat", "music"],
  "identity": { "pubkey": "ed25519:...", "auth_methods": ["passkey"] },
  "chat": { "type": "haven", "version": "3.24.0" },
  "feeds": { "max_post_length": 5000 }
}
```

The frontend discovers available features at boot by probing each configured backend URL.

### Auth Across Backends

1. User authenticates to their own identity server (Passkey → JWT)
2. JWT is presented to external services (chat, feeds, music)
3. External services validate the JWT signature using the user's pubkey
4. No shared database needed — trust is cryptographic

### Implementation Rule

No Mosaic module may assume it is the only backend. Every API call must be made against a configurable base URL. The frontend must never assume all features share an origin.

---

## Discoverability: How to Find People Without a Directory

The MySpace-style profile page is useless if no one can find it. In a decentralized, domain-free system, there is no central search, no directory, no "username squatting" protection. Discoverability must be solved at multiple layers, each with different trust/effort tradeoffs.

### Layer 1: Direct Address (The QR Business Card)

The baseline. Every user has a QR code encoding `{pubkey, node_url}`.

```
pubkey: ed25519:a3f8c91e...
node_url: http://10.1.1.50:3000
```

You scan someone's QR → you get their pubkey + address → you can fetch their profile at `/profile/:pubkey`. This is the handshake. Everything else builds on it.

### Layer 2: Connection Graph (Friend-of-Friend Browsing)

If Alice follows Bob, and Bob follows Carol, Alice can discover Carol through Bob's connection list:

```
Alice's Profile
  └─ "Connections" tab
       ├─ Following: Bob (pubkey, node_url)
       └─ Followers: (none yet)

Bob's Profile (fetched from Bob's node)
  └─ "Connections" tab
       ├─ Following: Alice, Carol, Dave
       └─ Followers: Alice
```

Alice clicks "Carol" → fetches Carol's profile from Carol's node. This is the social graph as discovery mechanism — exactly how MySpace worked, but distributed.

**Privacy control**: Users can mark their connection list as private (only followers see it) or public (anyone can browse).

### Layer 3: Gossip Propagation (Passive Discovery)

As the federation layer (Phase 6) gossips signed event logs between nodes, profile metadata propagates organically:

- When Alice follows Bob, a `follow` event is signed and appended to Alice's event log
- Bob's node receives the event (because they're connected)
- Bob's node learns Alice's node address from the event signature
- Bob's node can now fetch Alice's profile
- Over time, nodes accumulate a "known peers" list from gossip

This means: **just by using the network, you learn about other nodes**. You don't need to actively search.

### Layer 4: LAN/mDNS (Zero-Config Local Discovery)

On a local network, Mosaic nodes broadcast their existence via mDNS/Bonjour:

```
mosaic._tcp.local.  →  "ed25519:a3f8c91e...@10.1.1.50:3000"
```

Any Mosaic node on the LAN automatically discovers all others. A "People Nearby" tab in the UI shows the list. This is zero-config, works on any LAN, and requires no infrastructure.

### Layer 5: DHT (Global Discovery, No Server)

A Kademlia-style distributed hash table maps `pubkey → {node_url, last_seen}`. Nodes participate in the DHT when they have resources to spare. Lookups resolve in logarithmic time relative to network size.

```
Client → DHT.lookup(pubkey) → node_url → fetch profile
          DHT.store(pubkey, node_url) → others can find you
```

The DHT is entirely P2P — no servers, no directories. Implemented via libp2p or a lightweight Kademlia module.

### Layer 6: Optional Directory Nodes (Delegated Discoverability)

Some users may choose to run a **profile index** — a node that crawls profiles and provides search:

```http
GET https://index.mosaic.lan/search?q=username
→ [{ pubkey, display_name, node_url, avatar }]
```

Directory nodes are optional, user-configurable, and swappable. You can:
- Run your own (for your community)
- Use a friend's (trust-based)
- Use a public one (convenience, less private)
- Use none (pure P2P discovery only)

### The Discovery Decision Tree

```
I have someone's QR code?
  → Fetch profile from their node_url (Layer 1)

I know someone who knows them?
  → Browse their connection list (Layer 2)

I've been using the network for a while?
  → Check my gossip peer list (Layer 3)

We're on the same LAN?
  → Check mDNS (Layer 4)

I know their pubkey but nothing else?
  → DHT lookup (Layer 5)

I want to search by name/keyword?
  → Query configured directory node (Layer 6)

Nothing works?
  → Network unreachable or node offline
  → Profile may be cached from last contact
```

### Implementation Priority

| Layer | Effort | Impact | Phase |
|-------|--------|--------|-------|
| 1. QR + direct address | Already done | High — baseline | Done |
| 2. Connection graph | Medium | High — viral growth | Phase 4 |
| 3. Gossip propagation | Medium | Medium — passive | Phase 6 |
| 4. LAN/mDNS | Low | Medium — local networks | Phase 6 |
| 5. DHT | High | Low — large network needed | Post-v1 |
| 6. Directory nodes | Medium | High — searchability | Post-v1 |

---

## Neocities Integration: Bridge to the Static Web

[Neocities](https://neocities.org) is a modern GeoCities revival — free static site hosting with 1GB storage, CLI tools, a REST API, and an open-source backend ([github.com/neocities/neocities](https://github.com/neocities/neocities), 1.8k stars, MIT). It serves the exact same spirit as Mosaic: user-owned, creative, expressive personal web pages. Integrating with Neocities bridges Mosaic's decentralized identity layer with the browsable, crawlable static web.

### Webrings: The Original Social Graph

Webrings are circular chains of related sites. Each site links to "previous" and "next" in the ring, and a central "ringmaster" page lists all members. They were the original decentralized discovery mechanism — no central search needed, just trust-based linking.

Mosaic's connection graph maps directly to webrings:

```
Webring:         prev ← [Your Profile] → next
Mosaic Follows:  follower ← [Your Profile] → following
```

A Mosaic node implements:

```http
GET /webring/prev   → the previous person in your follow graph
GET /webring/next   → the next person
GET /webring/random → a random person from your extended network
```

Any Mosaic profile automatically becomes part of the webring ecosystem. A Neocities user can link to `mosaic.lan/webring/next` and get a live, always-up-to-date trail through their social graph.

### Integration Points

| Feature | What It Does | How |
|---------|-------------|-----|
| **Profile Publishing** | Mosaic profile manifest auto-deploys as static HTML to Neocities | POST to `/api/upload` on profile save. Viewers see `you.neocities.org` without a Mosaic node |
| **Webring Bridge** | Mosaic connection graph feeds into Neocities webrings | `GET /webring/*` routes. Neocities site embeds webring nav iframe pointing to your Mosaic node |
| **Media Hosting** | Profile assets optionally served from Neocities | Upload avatars, backgrounds, music to Neocities via API. Link from profile manifest |
| **Discovery Relay** | Neocities tags/browse feed into Mosaic directory | Mosaic directory node crawls Neocities tags matching `mosaic` → discovers new profiles |
| **Cross-Posting** | Feed posts auto-publish as static page on Neocities | Blog-style archive of your public posts at `you.neocities.org/blog/` |
| **Two-Way Linking** | Neocities profiles link back to Mosaic identity | `mosaic://<pubkey>` link on your Neocities page. QR code in your site's sidebar |

### The Capabilities Endpoint

A Neocities-connected Mosaic module exposes:

```http
GET /capabilities
→ {
  "features": ["identity", "profiles", "neocities"],
  "neocities": {
    "site": "myuser",
    "profile_url": "https://myuser.neocities.org",
    "webring": { "prev": "...", "next": "..." }
  }
}
```

### What This Enables

| | Without Neocities | With Neocities |
|--|------------------|----------------|
| **Viewing a profile** | Need a Mosaic node or QR scan | Visit `you.neocities.org` in any browser |
| **Discovering people** | QR exchange, friend-of-friend | Neocities browse, tags, search engines |
| **Your profile's reach** | Your Mosaic node only | The open web — indexed, crawled, linked |
| **Webring navigation** | Connection graph traversal | Standard webring nav — works everywhere |
| **Media storage** | Self-hosted disk or IPFS | Neocities free tier (1GB, 200GB bandwidth) |

### Implementation

Neocities integration is a Mosaic module that follows the Modularity and Composability principles:

- **Optional**: Only turns on if the user configures `MOSAIC_NEOCITIES_USER` and `MOSAIC_NEOCITIES_PASS`
- **Delegatable**: User can publish to Neocities or skip it entirely
- **Graceful degradation**: Without Neocities, everything still works — profile served from local node

---

## Matrix Bridge Strategy: Connect Haven to Everything via mautrix

Matrix has the most mature bridge ecosystem in the self-hosted world. The [mautrix project](https://github.com/mautrix) provides production-ready bridges (AGPL-3.0, Go) connecting Matrix to WhatsApp, Telegram, Discord, Signal, IRC, Slack, Bluesky, Instagram, Google Messages, and 10+ more platforms. Building a `mautrix-haven` bridge unlocks all of these networks for Haven users.

### Architecture

```
Matrix User                                   Haven User
    │                                              │
    ├── mautrix-whatsapp ──→ WhatsApp              │
    ├── mautrix-telegram ──→ Telegram              │
    ├── mautrix-discord  ──→ Discord               │
    ├── mautrix-signal   ──→ Signal                │
    │                                              │
    └── mautrix-haven ──────────→ Haven channel ───┤
                   ←───────── Socket.IO events ────┘

Every bridged platform can reach every Haven channel.
Every Haven user can reach every bridged platform.
```

### How mautrix Bridges Work

Each mautrix bridge is a Go application that registers as a Matrix application service. It:

1. Connects to the remote platform (e.g. Discord via discordgo)
2. Connects to the Matrix homeserver via the appservice API
3. Maps remote channels ↔ Matrix rooms
4. Maps remote users ↔ Matrix puppets
5. Translates messages, reactions, files, and events between protocols

The [bridgev2 framework](https://github.com/mautrix/bridgev2) handles all Matrix protocol complexity. Bridge authors only implement the remote platform client, portal mapping, and message conversion.

### mautrix-haven Bridge Spec

See GitHub issue #21 for the full spec. Key points:

| Component | Implementation |
|-----------|---------------|
| Protocol | Haven REST API (outgoing) + Socket.IO (incoming) |
| Auth | Haven JWT token for bridge bot user |
| Portal | 1:1 Haven channel code ↔ Matrix room alias |
| Puppet | 1:1 Haven user ID ↔ Matrix user |
| Messages | Send via REST, receive via Socket.IO events |
| Files | Download from Haven uploads, upload to Matrix media |
| Double-puppeting | Matrix user can link their Haven account |

### The Deployment Stack

To reach every platform from Haven:

```bash
# 1. Run Matrix homeserver (lightweight Conduit or Synapse)
docker run -d --name conduit matrix-conduit

# 2. Run mautrix bridges for desired platforms
docker run -d --name mautrix-discord  mautrix/discord
docker run -d --name mautrix-whatsapp mautrix/whatsapp
docker run -d --name mautrix-signal   mautrix/signal

# 3. Run mautrix-haven bridge (once built)
docker run -d --name mautrix-haven reverb256/mautrix-haven

# 4. Access everything from Element or any Matrix client
# No need to open Haven's UI for bridged conversations
```

### What This Enables

| Scenario | Before | After |
|----------|--------|-------|
| Reach WhatsApp contacts | Not possible | Via mautrix-whatsapp → mautrix-haven |
| Join IRC channels | Not possible | Via mautrix-irc → mautrix-haven |
| Cross-platform group chat | Not possible | All platforms bridged into one Matrix room |
| Use any Matrix client | Not possible | Element, FluffyChat, SchildiChat, etc. |
| Federation with other servers | Not possible | Matrix federation — talk to any Matrix server |

### Complexity & Timeline

| Phase | Scope | Effort |
|-------|-------|--------|
| v1 | Bidirectional text, basic channel/room mapping | 1 week |
| v2 | Reactions, edits, deletes, files | +1 week |
| v3 | Double-puppeting, backfill, presence | +1 week |
| v4 | Voice/video (Haven WebRTC ↔ Matrix VoIP) | Complex — post-v1 |

See GitHub issue #21 for the detailed implementation plan.

---

## Modular Runtime: FEATURES and CHAT_SERVER_URL

Two env vars make the modular/Composability principles operational at the deployment level.

### FEATURES: Choose What Runs

Controls which subsystems are active. Set at container boot:

```
# Full stack (default)
FEATURES=all

# Chat only (Discord replacement, lightweight)
FEATURES=chat

# Identity + profiles + feeds only (no chat)
FEATURES=identity,profiles,feeds

# Identity only (just key management and WebAuthn)
FEATURES=identity
```

When a feature is disabled, its routes are not mounted and its frontend elements are hidden. The server logs which features are active at boot:

```
[mosaic] chat enabled
[mosaic] identity/profiles/feeds routes at /mosaic/*
```

### CHAT_SERVER_URL: Delegate Chat to Another Server

When set, the frontend connects its socket.io and WebRTC to a remote server instead of the local one. Your identity, profiles, and feeds stay on your node — only the chat/voice/screenshare delegates.

```
# Point chat at a friend's server
CHAT_SERVER_URL=https://friend.haven.lan

# Or run your own chat server separately
CHAT_SERVER_URL=http://chat-server:32100
```

The frontend fetches `/mosaic/config` at boot, reads `chat_server`, and passes it to `io()`:

```javascript
const cfg = await fetch('/mosaic/config').then(r => r.json());
this.socket = io(cfg.chat_server || undefined, { auth: { token } });
```

If `CHAT_SERVER_URL` is unset, the frontend connects to the default origin (local). Zero config change for existing deployments.

### What This Enables

| Config | Identity | Chat | Profiles | Feeds |
|--------|----------|------|----------|-------|
| `FEATURES=all` | ✓ local | ✓ local | ✓ local | ✓ local |
| `FEATURES=chat` | ✗ | ✓ local | ✗ | ✗ |
| `FEATURES=identity,profiles,feeds` | ✓ local | ✗ | ✓ local | ✓ local |
| `FEATURES=identity` + `CHAT_SERVER_URL=...` | ✓ local | ✓ remote | ✗ | ✗ |
| `FEATURES=all` + `CHAT_SERVER_URL=...` | ✓ local | ✓ remote | ✓ local | ✓ local |

### The Composability Model

```
User's Mosaic Frontend
│
├─ Identity  ← self-hosted on Raspberry Pi (__MOSAIC_IDENTITY__)
├─ Chat      ← friend's Haven server (haven.lan)
├─ Profiles  ← self-hosted alongside identity
├─ Feeds     ← community hub (feeds.mosaic.lan)
├─ Music     ← different friend's Haven (music.haven.lan)
└─ Media     ← IPFS (distributed, no server)
```

Each feature resolves to a different backend. The frontend discovers which backend serves which feature through a **capabilities endpoint**.

### The Capabilities Endpoint

Every Mosaic module exposes:

```http
GET /capabilities
→ {
  "features": ["identity", "profiles", "feeds", "chat", "music"],
  "identity": { "pubkey": "ed25519:...", "auth_methods": ["passkey", "jwt"] },
  "chat": { "type": "haven", "version": "3.24.0" },
  "feeds": { "max_post_length": 5000, "supports_sockets": true }
}
```

The frontend discovers available features at boot:

```javascript
const backends = {
  identity: 'http://my-pi:3002',
  chat:     'https://friend.haven.lan',
  feeds:    'http://community-hub:3003',
  music:    'https://music.haven.lan',
};

async function discover() {
  const available = {};
  for (const [feature, url] of Object.entries(backends)) {
    try {
      const res = await fetch(`${url}/capabilities`);
      if (res.ok) available[feature] = await res.json();
    } catch { /* service unavailable, degrade gracefully */ }
  }
  return available;
}
```

### Auth Across Backends

Since identity is self-hosted but chat may be on a friend's server, auth tokens must be portable:

1. User authenticates to **their own identity server** (Passkey → JWT)
2. JWT is presented to **external services** (chat, feeds, music)
3. External services validate the JWT signature using the user's pubkey
4. No shared database needed — trust is cryptographic

```
User → Identity Server (Passkey login)
  → JWT signed with user's Ed25519 keypair
  → JWT presented to Friend's Haven Server
  → Friend's server verifies signature against pubkey (stored or fetched)
  → Access granted (guest role or mapped to local user)
```

### What Composability Enables That Modularity Alone Doesn't

| Scenario | Modularity | + Composability |
|----------|-----------|----------------|
| I want chat but not to host it | Skip chat module | Point chat to friend's server |
| I want to try different profile hosts | Skip profiles module | Swap profile backend URL |
| I want to aggregate feeds from multiple communities | Run feeds module | Pull from multiple feed servers |
| My friend runs better music infrastructure | Skip music module | Point music to friend's server |
| I want to migrate my social graph to a new server | — | Change backend URLs, keep identity key |

### Implementation Rule

**No Mosaic module may assume it is the only backend.** Every API call must be made against a configurable base URL, not a hardcoded path. The frontend must never assume all features share an origin.

---

## Deployment Philosophy: Domain-Free by Default

Mosaic must **never require** a domain name. It is designed to work on bare IP addresses, LAN hostnames, Tor onion addresses, or Tailscale IPs — whatever the user has. This is a hard architectural constraint, not a preference.

### The Stack

| Layer | Mechanism | Domain Required? |
|-------|-----------|------------------|
| **Identity** | `mosaic://<pubkey>` URI scheme | Never |
| **Authentication** | WebAuthn RP_ID defaults to IP/hostname | Never — configurable via env |
| **Transport** | Direct TCP, WebRTC, or Tor | Never |
| **Discovery** | QR codes, LAN mDNS, DHT, manual pubkey | Never |
| **Media** | Content-addressed (IPFS CIDs) | Never |

### Convenience Layer (Optional, Post-Deployment)

After deployment, users may optionally add local hostnames:

```bash
# /etc/hosts entry for LAN convenience
echo '<ip> mosaic.lan' >> /etc/hosts

# mDNS broadcast (Avahi)
avahi-publish -a -R mosaic.lan <ip>

# LAN DNS (unbound/dnsmasq)
local-data: "mosaic.lan A <ip>"
```

These are **never referenced in code**. They are post-deployment niceties that the deployment script may offer but the architecture never depends on.

### The Deployment Contract

When someone installs Mosaic (OCI, npm, Nix, K8s), the output says:

```
Your Mosaic node is running at http://<this-ip>:3000
Your pubkey is: ed25519:<base64>
Share this QR code to let others connect.
```

No domain. No DNS. No certificate authority. Just an IP, a port, and a cryptographic identity.

---

## Current Status

- [x] Repo cloned and rebranded to Mosaic (package.json, Dockerfile, paths.js)
- [x] `mosaic` branch created
- [x] Phase 1: Identity layer — Ed25519 keys, Passkeys, QR, signing
- [x] `reverb256/__MOSAIC_IDENTITY__` — standalone sidecar (multi-platform OCI + Nix + direct)
- [x] sql.js WASM fallback for exotic platforms (Termux, Wii Linux, etc.)
- [x] Phase 2: Profiles
- [x] Phase 3: Feeds & posts
- [x] Phase 4: Connections & following
- [x] Phase 5: Signed event bus
- [x] Phase 6: Federation

### Multi-Platform Targets

| Platform | Method | Status |
|----------|--------|--------|
| Linux (x86_64, aarch64) | npm install, OCI, Nix | ✓ |
| macOS (Intel, Apple Silicon) | npm install, OCI | ✓ |
| Docker/Podman (amd64, arm64, arm/v7) | OCI multi-arch | ✓ |
| NixOS | buildNpmPackage | ✓ |
| Termux (Android) | npm start (sql.js fallback) | ✓ documented |
| Raspberry Pi (arm64) | npm install, OCI | ✓ |
| Windows (WSL, Git Bash) | npm install, OCI | ✓ |
| Nintendo Wii Linux | npm start (sql.js, zero native deps) | ✓ in principle |
