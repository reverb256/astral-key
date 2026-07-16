---
version: 1.0.0
name: Mosaic
description: >
  Sovereign self-hosted social platform — Discord + MySpace + Facebook + Matrix.
  Built on Haven. Ed25519 identity. P2P federation. No domain required.
colors:
  bg-primary: "#0d1117"
  bg-secondary: "#161b22"
  bg-card: "#1c2128"
  bg-hover: "#252b33"
  border: "#30363d"
  text-primary: "#e6edf3"
  text-secondary: "#8b949e"
  text-muted: "#6e7681"
  accent: "#58a6ff"
  accent-hover: "#79c0ff"
  success: "#3fb950"
  danger: "#f85149"
  warning: "#d29922"
  verified: "#3fb950"
  pending: "#d29922"
  trusted: "#58a6ff"
typography:
  font-family: >
    -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, 'Noto Sans', Ubuntu, sans-serif
  font-mono: >
    'JetBrains Mono', 'SF Mono', 'Fira Code', 'Consolas', monospace
  body:
    size: 0.95rem
    weight: 400
    line-height: 1.6
  h1:
    size: 2.2rem
    weight: 700
    line-height: 1.2
  h2:
    size: 1.4rem
    weight: 600
    line-height: 1.3
  h3:
    size: 1.1rem
    weight: 600
    line-height: 1.4
  small:
    size: 0.85rem
    weight: 400
    line-height: 1.4
  code:
    size: 0.85rem
    weight: 400
    line-height: 1.4
spacing:
  xs: 4px
  sm: 8px
  md: 16px
  lg: 24px
  xl: 32px
  xxl: 48px
  section: 60px
border-radius:
  sm: 4px
  default: 8px
  lg: 12px
  pill: 999px
motion:
  instant: 0ms
  fast: 100ms
  normal: 200ms
  slow: 300ms
  marketing: 500ms
  eased: "cubic-bezier(0.22, 1, 0.36, 1)"
  ease-out: "cubic-bezier(0.22, 1, 0.36, 1)"
  ease-in-out: "cubic-bezier(0.65, 0, 0.35, 1)"
  spring-cta: "spring(stiffness 600, damping 30, mass 0.8)"
---

# Mosaic Design System

> **Sovereign, self-hosted social platform.** Built on Haven.
> Ed25519 identity + Passkey auth + MySpace profiles + algorithmic feeds
> + P2P federation. No domain required. No Big Tech. No KYC.

---

## Architecture

```
┌─────────────────────────────────────────┐
│  Mosaic Core (always runs)              │
│  ├─ Identity (Ed25519 + Passkey + QR)   │ ✅ Phase 1
│  ├─ Profiles (optional external)        │ ✅ Phase 2
│  ├─ Feeds / Bulletins (optional)        │ ✅ Phase 3
│  ├─ Connections (optional)              │ ✅ Phase 4
│  ├─ Signed Event Bus (optional)         │ ✅ Phase 5
│  └─ Moderation (optional)               │ ✅
├─────────────────────────────────────────┤
│  Mosaic Plugins (modular, swappable)    │
│  ├─ Chat/Discord (Haven, external)      │ ← delegated
│  ├─ Voice/Video (Haven WebRTC)          │
│  ├─ Music (Haven music system)          │
│  └─ Federation (P2P gossip)            │ ✅ Phase 6
└─────────────────────────────────────────┘
```

### Key Design Principles

1. **Zero modifications to existing Haven code** — New routes mount at `/mosaic/*`, new tables use `CREATE TABLE IF NOT EXISTS`, new UI lives in separate files.
2. **Every feature is a module: optional, skippable, delegatable** — Controlled by `FEATURES` env var.
3. **No domain required** — Discovery via QR codes, IP addresses, mDNS, Tor onion services.
4. **Ed25519 root identity** — Every user has a Ed25519 keypair as their cryptographic anchor. Format: `ed25519:<base64URL>`.
5. **Additive atproto interop** — Optional did:key export, CAR format for event log export, label protocol patterns — but Mosaic's native formats are simpler and self-contained.

---

## Color System

### Theme: Dark (default)

| Token | Hex | Usage |
|-------|-----|-------|
| `--bg-primary` | `#0d1117` | Page background |
| `--bg-secondary` | `#161b22` | Sidebar, secondary surfaces |
| `--bg-card` | `#1c2128` | Card, modal, dropdown backgrounds |
| `--bg-hover` | `#252b33` | Hover state for interactive elements |
| `--border` | `#30363d` | Borders, dividers, separators |
| `--text-primary` | `#e6edf3` | Primary body text |
| `--text-secondary` | `#8b949e` | Secondary text, labels, descriptions |
| `--text-muted` | `#6e7681` | Disabled text, placeholders |
| `--accent` | `#58a6ff` | Primary actions, links, active states |
| `--accent-hover` | `#79c0ff` | Hover state for accent elements |
| `--success` | `#3fb950` | Verified, approved, online |
| `--danger` | `#f85149` | Errors, destructive actions, blocked |
| `--warning` | `#d29922` | Pending, requires attention |

### Semantic Color Mapping

| Context | Token | Example |
|---------|-------|---------|
| Identity verified | `--success` | Passkey registered |
| Key generation complete | `--success` | Key wizard step done |
| Appeal pending | `--warning` | Moderation appeal awaiting review |
| Label applied | `--danger` | Content labeled as spam/harassment |
| Blocked user | `--danger` | Connection list, blocked tab |
| Trusted labeler | `--accent` | Moderation settings |

---

## Component Tree

```
public/index.html (Haven's SPA)
  └─ public/identity.html (Mosaic identity management)
       ├─ Keygen wizard (first-boot flow)
       ├─ Passkey registration
       ├─ QR code display/scan
       └─ Key backup (seed phrase)
  └─ public/profile.html (profile editor) — TODO: mount point
       ├─ HTML/CSS code editor (split-pane)
       ├─ Sandboxed live preview
       └─ Publish to Neocities button
  └─ [in-app tabs, inside Haven's existing UI]
       ├─ Feed tab (app-feeds.js)
       │   ├─ Timeline view (cursor-paginated)
       │   ├─ Feed composer
       │   └─ Reaction buttons (like/repost)
       ├─ Profile tab (app-profile.js)
       │   ├─ Profile viewer
       │   └─ Profile editor
       └─ Connections tab (app-connections.js)
           ├─ Following list
           ├─ Followers list
           ├─ Blocked users
           └─ Follow/unfollow buttons
       └─ Moderation panel (app-moderation.js)
           ├─ Report button (on posts/profiles)
           ├─ Label viewer (settings)
           ├─ Appeal form
           └─ Trusted labelers management
```

### Module Dependency Graph

```
identity.js (Ed25519)
  ├── passkey.js (WebAuthn) 
  ├── qr.js (QR encode/decode)
  ├── keychain.js (BIP39 backup)
  │
  ├── profiles.js (profile CRUD, signed manifests)
  │   └── profiles-sandbox.js (CSP sandbox for HTML/CSS)
  │
  ├── feeds.js (post CRUD, reactions)
  │   └── feed-algos/{recent,local,friends}.js
  │
  ├── connections.js (follow/unfollow, block)
  │
  ├── events.js (signed event envelopes) ✅
  │   └── event-log.js (append-only per-pubkey log) ✅
  │       └── gossip.js (P2P sync over WebSocket) ✅
  │           └── repo-export.js (CAR format backup) ✅
  │
  ├── labels.js (moderation labels, reports, appeals)
  │   └── label-filter.js (per-user filter engine)
  │
  └── features.js (FEATURES env var gating)
```

---

## Route Map

All Mosaic routes mount at `/mosaic/*` on the Express app.

### Phase 1 — Identity

| Method | Path | Handler | Auth |
|--------|------|---------|------|
| `GET` | `/mosaic/health` | Server health check | No |
| `GET` | `/mosaic/config` | Runtime config + capabilities | No |
| `GET` | `/mosaic/identity/current` | Get current identity | No |
| `POST` | `/api/auth/identity/generate` | Generate Ed25519 keypair | No |
| `GET` | `/mosaic/qr/:pubkey` | Generate QR code SVG | No |
| `POST` | `/mosaic/qr/scan` | Decode + store scanned pubkey | No |
| `GET` | `/mosaic/contacts` | List known contacts | No |
| `DELETE` | `/mosaic/contacts/:pubkey` | Remove contact | No |

### Phase 2 — Profiles

| Method | Path | Handler | Auth |
|--------|------|---------|------|
| `GET` | `/mosaic/profile/:pubkey` | Get profile manifest | No |
| `POST` | `/mosaic/profile` | Create/update profile | Yes |

### Phase 3 — Feeds

| Method | Path | Handler | Auth |
|--------|------|---------|------|
| `GET` | `/mosaic/feed` | Get feed (algo/cursor/limit) | No |
| `POST` | `/mosaic/feed/post` | Create signed feed post | Yes |
| `POST` | `/mosaic/feed/react` | Add reaction (like/repost) | No |
| `DELETE` | `/mosaic/feed/react` | Remove reaction | No |

### Phase 4 — Connections

| Method | Path | Handler | Auth |
|--------|------|---------|------|
| `POST` | `/mosaic/follow` | Follow a pubkey | Yes |
| `DELETE` | `/mosaic/follow` | Unfollow a pubkey | Yes |
| `POST` | `/mosaic/block` | Block a pubkey | Yes |
| `DELETE` | `/mosaic/block` | Unblock a pubkey | Yes |
| `GET` | `/mosaic/connections/:pubkey` | Get followers/following | No |

### Moderation

| Method | Path | Handler | Auth |
|--------|------|---------|------|
| `POST` | `/mosaic/label/apply` | Apply signed label | Yes (labeler) |
| `POST` | `/mosaic/label/negate` | Negate a label | Yes (labeler) |
| `GET` | `/mosaic/label/list` | Get labels for URI | No |
| `GET` | `/mosaic/label/subscribe` | WebSocket label stream | No |
| `POST` | `/mosaic/report/create` | Submit report | No |
| `GET` | `/mosaic/report/list` | List reports by user | Yes |
| `POST` | `/mosaic/appeal/create` | Appeal a label | Yes |
| `GET` | `/mosaic/appeal/list` | List appeals by user | Yes |

---

## Data Flow Diagrams

### Auth Flow

```
User → Identity Page → "Generate Key" 
  → identity.generateKeyPair() → Ed25519 keypair
  → passkey.register() → WebAuthn credential bound to pubkey
  → JWT issued (signed with privkey, verified with pubkey)
  → JWT presented to all Mosaic endpoints

On subsequent visits:
  → passkey.login() → biometric challenge
  → challenge signed → JWT reissued
```

### Feed Post Flow

```
User writes post in Feed Composer
  → feeds.createPost(pubkey, content, signFn)
  → Content hashed → SHA-256 CID (dedup key)
  → Envelope signed with Ed25519 privkey
  → INSERT INTO feed_posts
  → Event emitted on Socket.IO (type: 'feed')
  → Gossip: event broadcast to connected peers
  → Peer nodes append to their feed_posts table
  → Feed algorithms re-query with new data
```

### Moderation Label Flow

```
User A reports content via /mosaic/report/create
  → Stored in moderation_reports
  → Labeler B's subscribed clients receive report
  → Labeler B applies label via /mosaic/label/apply
  → Label signed with B's Ed25519 key
  → Stored in moderation_labels with mandatory note + expiresAt
  → Peers subscribed to B's label stream receive label
  → User A (content owner) sees label in label viewer
  → User A appeals via /mosaic/appeal/create
  → Labeler B resolves appeal (accept → negate label, reject → close)
```

### P2P Sync Flow (Phase 5-6)

```
Node A starts → mDNS broadcasts presence
  → Node B discovers Node A on LAN
  → WebSocket handshake (Ed25519 challenge-response)
  → Exchange latest event cursors per pubkey
  → Request missing events by range
  → Verify signatures + chain hashes
  → Merge verified events into local DB
  → Switch to subscribe-repos streaming mode
  → New events pushed in real-time over persistent WS
```

---

## Database Schema

All Mosaic tables coexist in Haven's existing SQLite file via `CREATE TABLE IF NOT EXISTS`. They never touch Haven's tables.

### Identity Tables (Phase 1)

```sql
CREATE TABLE identities (
  id          INTEGER PRIMARY KEY AUTOINCREMENT,
  pubkey      TEXT    NOT NULL UNIQUE,        -- ed25519:<base64>
  privkey     TEXT    NOT NULL,               -- encrypted at rest
  label       TEXT,                            -- human label
  is_current  INTEGER NOT NULL DEFAULT 0,
  created_at  TEXT    NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE passkeys (
  id           INTEGER PRIMARY KEY AUTOINCREMENT,
  identity_id  INTEGER NOT NULL REFERENCES identities(id),
  credential_id TEXT   NOT NULL UNIQUE,
  public_key   TEXT    NOT NULL,
  counter      INTEGER NOT NULL DEFAULT 0,
  transports   TEXT,
  created_at   TEXT   NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE contacts (
  pubkey       TEXT   PRIMARY KEY,             -- ed25519:<base64>
  label        TEXT,                            -- human-readable label
  first_seen_at TEXT  NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE sessions (
  token_hash   TEXT   PRIMARY KEY,
  identity_id  INTEGER NOT NULL REFERENCES identities(id),
  expires_at   TEXT   NOT NULL,
  created_at   TEXT   NOT NULL DEFAULT (datetime('now'))
);
```

### Profile Tables (Phase 2)

```sql
CREATE TABLE profiles (
  pubkey      TEXT    PRIMARY KEY REFERENCES identities(pubkey),
  manifest    TEXT    NOT NULL,                -- signed JSON manifest
  published   INTEGER NOT NULL DEFAULT 0,
  updated_at  TEXT    NOT NULL DEFAULT (datetime('now'))
);
```

### Feed Tables (Phase 3)

```sql
CREATE TABLE feed_posts (
  cid         TEXT    PRIMARY KEY,             -- SHA-256 content hash
  pubkey      TEXT    NOT NULL REFERENCES identities(pubkey),
  content     TEXT    NOT NULL,
  created_at  TEXT    NOT NULL,
  signature   TEXT    NOT NULL,
  reply_to    TEXT,                            -- parent post CID
  indexed_at  TEXT    NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE feed_reactions (
  id          INTEGER PRIMARY KEY AUTOINCREMENT,
  cid         TEXT    NOT NULL REFERENCES feed_posts(cid),
  pubkey      TEXT    NOT NULL REFERENCES identities(pubkey),
  type        TEXT    NOT NULL,                -- 'like' | 'repost'
  created_at  TEXT    NOT NULL DEFAULT (datetime('now')),
  UNIQUE(cid, pubkey, type)
);
```

### Connection Tables (Phase 4)

```sql
CREATE TABLE follows (
  follower    TEXT    NOT NULL REFERENCES identities(pubkey),
  followee    TEXT    NOT NULL,
  created_at  TEXT    NOT NULL DEFAULT (datetime('now')),
  PRIMARY KEY (follower, followee)
);

CREATE TABLE blocked (
  blocker     TEXT    NOT NULL REFERENCES identities(pubkey),
  blockee     TEXT    NOT NULL,
  reason      TEXT,
  created_at  TEXT    NOT NULL DEFAULT (datetime('now')),
  PRIMARY KEY (blocker, blockee)
);
```

### Event Log Table (Phase 5)

```sql
CREATE TABLE event_log (
  pubkey      TEXT    NOT NULL,
  seq         INTEGER NOT NULL,
  event_hash  TEXT    NOT NULL,
  prev_hash   TEXT,
  event_type  TEXT    NOT NULL,
  payload     TEXT    NOT NULL,                -- JSON
  timestamp   INTEGER NOT NULL,
  signature   TEXT    NOT NULL,
  created_at  TEXT    NOT NULL DEFAULT (datetime('now')),
  PRIMARY KEY (pubkey, seq)
);
```

### Moderation Tables

```sql
CREATE TABLE moderation_labels (
  cid         TEXT    PRIMARY KEY,             -- SHA-256 of payload
  uri         TEXT    NOT NULL,                -- what's being labelled
  val         TEXT    NOT NULL,                -- 'spam', 'harassment', 'misinfo', 'nsfw'
  src         TEXT    NOT NULL,                -- ed25519 pubkey of labeler
  neg         INTEGER NOT NULL DEFAULT 0,      -- negation flag
  note        TEXT,                            -- mandatory explanation
  expires_at  TEXT    NOT NULL,                -- auto-expiry timestamp
  sig         TEXT    NOT NULL,
  created_at  TEXT    NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE moderation_reports (
  id           INTEGER PRIMARY KEY AUTOINCREMENT,
  uri          TEXT    NOT NULL,
  reason_type  TEXT    NOT NULL,               -- 'spam', 'harassment', 'illegal', 'other'
  reason       TEXT,
  reported_by  TEXT    NOT NULL,
  created_at   TEXT    NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE moderation_appeals (
  id           INTEGER PRIMARY KEY AUTOINCREMENT,
  label_cid    TEXT    NOT NULL,
  pubkey       TEXT    NOT NULL,               -- appealing user
  reason       TEXT    NOT NULL,
  evidence     TEXT,
  status       TEXT    NOT NULL DEFAULT 'pending',  -- 'pending', 'accepted', 'rejected'
  resolution   TEXT,
  created_at   TEXT    NOT NULL DEFAULT (datetime('now'))
);
```

---

## UI Component Patterns

All motion follows the Apple design principles from `motion:` frontmatter: ease-out for entrances, sub-300ms UI, property-specific transitions only, never `scale(0)`, gate reduced-motion.

### Motion Guidelines

- **Entrances:** `opacity: 0; transform: translateY(8px)` → 300ms `ease-out`
- **Hover states:** system response (150ms `ease-out`), gated behind `@media (hover: hover)`
- **Active states:** `scale(0.97)` immediate (100ms), never from `scale(0)`
- **Transitions:** property-specific only (`transition: opacity, transform` — never `transition: all`)
- **Reduced motion:** cross-fade replaces slide/spring, gated by `prefers-reduced-motion`

### Buttons

```
.btn-primary   → accent background, white text. Primary CTA.
.btn-secondary → card background, primary text. Default action.
.btn-ghost     → transparent, accent text. Subtle action.
.btn-danger    → danger background, white text. Destructive action.
```

All buttons share: `border-radius: 8px`, `padding: 10px 20px`, `font-weight: 500`, `transition: 0.15s ease`.

### Cards

Cards use `--bg-card` background with `--border` 1px border and `border-radius: 8px`. Padding is `--xl` (32px). Cards stack vertically with `--md` (16px) gap.

### Modals

Modal overlays use `rgba(0,0,0,0.6)` backdrop, centered card container with `max-width: 480px`, close button in top-right corner.

### Form Elements

Inputs: `--bg-secondary` background, `--border` 1px border, `border-radius: 6px`, `padding: 8px 12px`. Focus state: `--accent` border, `box-shadow: 0 0 0 3px rgba(88,166,255,0.15)`.

Labels: `--text-secondary` color, `font-size: 0.85rem`, `margin-bottom: 6px`.

---

## Identity Format

### Native (primary)

```
ed25519:<base64URL_32_byte_pubkey>
```

Example: `ed25519:AxqR_V4kY7zL3pWnB9mCtD8fG2hJ5kM1oP0sQ6rUvXw`

Used in: QR codes, API payloads, database, WebSocket handshake.

### Atproto Interop (derived, optional)

```
did:key:zQ3shuMW7q4KBdsFcdvebGi2EVv8KcqS24tF9Pg7Wh5NLB2NM
```

Derived at call time via `identity.did(pubkey)`. Used in: labeler identity, CAR metadata, atproto-compatible handshakes.

### QR Payload Schema (v1)

```json
{
  "v": 1,
  "pk": "ed25519:<base64_pubkey>",
  "n": "display_name",
  "u": "http://10.1.1.120:3000",
  "h": "onion_address.onion"
}
```

### Profile Manifest Schema (v1)

```json
{
  "version": 1,
  "pubkey": "ed25519:<base64>",
  "display_name": "cooluser",
  "bio": "building sovereign social",
  "avatar": null,
  "theme": "mosaic-dark",
  "content": {"html": "<h1>Welcome</h1>", "css": "body { color: #eee; }"},
  "widgets": [
    {"type": "music_player"},
    {"type": "friends", "limit": 10},
    {"type": "recent_posts", "limit": 5}
  ],
  "links": [{"label": "website", "url": "http://..."}],
  "signature": "<ed25519_sig_of_above>"
}
```

### Signed Event Envelope (Phase 5)

```json
{
  "type": "post",
  "pubkey": "ed25519:<base64>",
  "payload": {"content": "hello world", "media": [], "reply_to": null},
  "prev_hash": "<sha256_of_previous_event>",
  "timestamp": 1700000000,
  "signature": "<ed25519_sig_of_envelope>"
}
```

---

## Moderation Philosophy

Mosaic uses atproto's labeling **infrastructure** (signed labels, label streams, labeler identity) but with Mosaic's own **policies**:

| Policy | Bluesky | Mosaic |
|--------|---------|--------|
| Label visibility | Opaque to user | **Labels visible to the labelled user with note** |
| Expiry | No TTL required | **Mandatory `expiresAt` on every label** |
| Appeals | Manual/email | **Signed appeals as records with resolution workflow** |
| Labelers | Centralized team | **Anyone can run a labeler; users choose who to trust** |
| Labels on own content | Hidden | **Shown in settings "Content Labels" with note + expiry** |

Label values: `spam`, `harassment`, `misinfo`, `nsfw`, `custom`. Each label MUST include a `note` (reason for labelling) and `expiresAt` (auto-expiry timestamp).

---

## Key Design Decisions

| Decision | Rationale |
|----------|-----------|
| **Ed25519 over K-256** | tweetnacl is zero native deps, works everywhere (Wii Linux documented). K-256 requires more complex libs. |
| **`ed25519:<base64>` over `did:key`** | Simpler, self-describing, no multibase parsing needed. `did:key` is optional derived format. |
| **SQLite over Postgres** | Single-file deploy, zero infrastructure, same DB as Haven. |
| **Event log over MST** | Append-only per-pubkey log is simpler and sufficient for personal nodes. Merkle Search Tree is for millions of records. |
| **WebSocket gossip over relay** | P2P mesh, not hub-and-spoke. No single point of failure. |
| **QR + mDNS over DID PLC** | No external directory dependency. Discovery works on LAN with zero config. |
| **Express + Socket.IO over XRPC** | Already exists in Haven. XRPC adds no value without Lexicon schemas. |
| **`FEATURES` env var** | Every feature is optional. Default `all` but degrades gracefully to `chat` (Haven-only). |

---

## Deployment Architecture

```
┌─────────────────────────────────────────────┐
│  OCI Container (ghcr.io/reverb256/mosaic)   │
│  ├─ Node.js 22 + Express + Socket.IO        │
│  ├─ SQLite (/data/mosaic.db)                │
│  ├─ Ed25519 keys (/data/keys/)              │
│  └─ Uploads (/data/uploads/)                │
├─────────────────────────────────────────────┤
│  Env:                                       │
│  FEATURES=all                                │
│  CHAT_SERVER_URL= (optional)                 │
│  PORT=3000                                   │
│  JWT_SECRET= (auto or set)                   │
└─────────────────────────────────────────────┘
```

---

## File Inventory

```
src/
├── identity.js          — Ed25519 keygen/sign/verify/fingerprint/did:key
├── passkey.js           — WebAuthn registration + authentication
├── qr.js                — QR encode/decode for pubkey exchange
├── keychain.js          — BIP39 seed phrase backup/restore
├── profiles.js          — Profile manifest CRUD with Ed25519 signing
├── profiles-sandbox.js  — CSP sandbox config for user HTML/CSS
├── feeds.js             — Feed post CRUD + reactions + algo dispatch
├── feed-algos/
│   ├── recent.js        — Most recent public posts
│   ├── local.js         — Posts from this node's users
│   └── friends.js       — Posts from followed pubkeys
├── connections.js       — Follow/unfollow + blocklist CRUD
├── events.js            — Signed event envelope creation/verification
├── event-log.js         — Append-only per-pubkey event log
├── gossip.js            — P2P WebSocket peer sync
├── transport-lan.js     — mDNS/Bonjour LAN discovery
├── repo-export.js       — CAR format event log export
├── labels.js            — Moderation label CRUD + reports + appeals
├── label-filter.js      — Per-user label filter engine
├── features.js          — FEATURES env var flag system
├── routes-mosaic.js     — All /mosaic/* Express routes
└── database.js          — All Mosaic tables (appended to Haven's DB)

public/
├── identity.html         — Identity management SPA
├── css/__MOSAIC_IDENTITY__.css — Design token system
└── js/modules/
    ├── app-identity.js   — Client-side keygen wizard + identity UI
    ├── app-profile.js    — Profile editor/viewer + sandboxed preview
    ├── app-feeds.js      — Timeline view + composer + reactions
    ├── app-connections.js— Follow/unfollow + block management
    └── app-moderation.js — Report/label/appeal UI

test/
├── identity.test.js      — Identity + database + QR tests (33 cases)
└── server.test.js        — Server smoke tests (4 cases)

docs/
└── tooling.md            — Developer tooling guide
```

---

## Version History

| Version | Date | Changes |
|---------|------|---------|
| 0.2.0 | 2026-07-14 | Added Phases 2-4 architecture, moderation system, event bus design, route map, database schema, UI components |
| 0.1.0 | 2026-06 | Initial design: identity layer, Ed25519 keys, Passkey auth, QR codes |
