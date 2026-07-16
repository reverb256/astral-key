# Mosaic Developer Tooling

## Overview

Mosaic extends the Haven chat platform with sovereign Ed25519 identity, profiles,
feeds, and P2P federation. This document covers the developer tooling available
for building, testing, and debugging Mosaic features.

---

## Quick Start

```bash
# Install dependencies
npm install

# Copy the example env file (creates ~/.config/mosaic/.env on first run)
cp .env.example ~/.config/mosaic/.env   # or let server.js bootstrap it

# Set a JWT secret
export JWT_SECRET="your-secret-here"

# Start in development mode with auto-reload
npm run dev

# Start in production mode
npm start
```

### Environment Variables

| Variable            | Default  | Description                                                |
|---------------------|----------|------------------------------------------------------------|
| `JWT_SECRET`        | (auto)   | HMAC secret for JWT tokens                                 |
| `FEATURES`          | `all`    | Comma-separated feature flags (see below)                  |
| `CHAT_SERVER_URL`   | (none)   | URL of the upstream Haven chat server (for composable deployment) |
| `IDENTITY_SERVER_URL` | (none) | URL of the identity server (for separated deployments)     |
| `PORT`              | `3000`   | HTTP server port                                           |

---

## FEATURES Environment Variable

The `FEATURES` env var controls which Mosaic subsystems activate at startup.
Every feature is a module: optional, skippable, and delegatable.

### Values

| Value              | Meaning                          |
|--------------------|----------------------------------|
| `all` (default)    | Enable all Mosaic features       |
| `chat`             | Haven-only — no Mosaic features  |
| `identity`         | Ed25519 keys, passkeys, auth     |
| `profiles`         | User profiles                    |
| `feeds`            | Content feeds and posts          |
| `connections`      | Contacts and P2P connections     |
| `moderation`       | Label system, reports, appeals   |

### Examples

```bash
# Run with all Mosaic features (default)
FEATURES=all node server.js

# Run with identity and profile features only
FEATURES=identity,profiles node server.js

# Run as pure Haven chat (no Mosaic features)
FEATURES=chat node server.js

# Run a dedicated moderation node
FEATURES=moderation node server.js
```

---

## CHAT_SERVER_URL

In a composable deployment, `CHAT_SERVER_URL` tells the frontend where the
upstream Haven chat (Socket.IO) server is running. This allows operators to
split identity and chat onto separate processes or hosts.

```bash
# Run identity server pointing to a remote chat backend
CHAT_SERVER_URL=https://chat.example.com FEATURES=identity node server.js
```

The frontend reads this from `GET /mosaic/config` and configures its
Socket.IO connection accordingly.

---

## Capabilities Endpoint

`GET /mosaic/config` returns the server's full capabilities:

```json
{
  "features": ["identity", "profiles", "feeds", "moderation"],
  "chat_server": "https://chat.example.com",
  "identity_server": null,
  "identity": {
    "pubkey": "ed25519:abc123...",
    "auth_methods": ["passkey", "jwt"]
  },
  "profiles": {
    "version": 1,
    "max_html_size": 50000
  },
  "feeds": {
    "max_post_length": 3000,
    "algos": ["recent", "local", "friends"]
  },
  "chat": {
    "type": "haven",
    "server_url": "https://chat.example.com"
  },
  "mosaic_version": "0.1.0"
}
```

---

## Running Tests

### Test Suite

```bash
# Run all tests
npm test

# Run with watch mode
npm run test:watch

# Run a specific test file
npx node --test test/features.test.js
```

### CI

The `.github/workflows/test.yml` workflow runs on every push and pull request:

```yaml
name: test
on: [push, pull_request]
jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: actions/setup-node@v4
        with: { node-version: 22 }
      - run: npm ci
      - run: JWT_SECRET=ci-test npx node --test test/*.test.js
```

---

## Moderation Label System

Mosaic implements a moderation label infrastructure inspired by atproto,
but with Mosaic-specific policies:

- **Labels** are signed key-value assertions (e.g. `spam`, `harassment`) on a URI
- **Labelers** are Ed25519 identities (`ed25519:<base64>` format)
- **Mandatory notes** — every label includes a human-readable explanation
- **Mandatory TTL** — all labels expire automatically
- **Appeals** — users can appeal labels; appeals are public
- **Negation labels** — a labeler can negate/correct a previous label

### API Endpoints

| Method | Path                        | Description               |
|--------|-----------------------------|---------------------------|
| POST   | `/mosaic/label/apply`       | Apply a label             |
| POST   | `/mosaic/label/negate`      | Negate a label            |
| GET    | `/mosaic/label/list`        | Get labels for a URI      |
| POST   | `/mosaic/report/create`     | Submit a report           |
| POST   | `/mosaic/appeal/create`     | Appeal a label            |
| GET    | `/mosaic/appeal/list`       | List appeals by user      |

### Client-Side Filtering

The `src/label-filter.js` module provides a `LabelFilter` class that runs
entirely client-side. Each user configures:

- **Trusted labelers** — whose labels to respect
- **Per-label behaviours** — `hide`, `blur`, `warn`, or `none`

```js
const { LabelFilter } = require('./src/label-filter');

const filter = new LabelFilter({
  trustedLabelers: ['ed25519:abc...', 'ed25519:def...'],
  labelBehaviors: { spam: 'hide', nsfw: 'blur' },
});

const { visible, blurred, warned } = filter.filter(items);
```

---

## CLI Tools

### goat (atproto/moderation toolkit)

The `goat` CLI is a standalone Go binary for working with atproto DIDs,
lexicons, and firehose subscriptions. Useful for building and testing
moderation bridges.

**Install:**
```bash
# macOS
brew install bluesky-social/tap/goat

# Linux — download from releases
curl -sSfL https://github.com/bluesky-social/indigo/releases/latest/download/goat_linux_amd64.tar.gz | tar xz
sudo mv goat /usr/local/bin/
```

**Usage:**
```bash
# Verify a did:key
goat did verify did:key:zQ3sh...

# Lint a lexicon schema
goat lexicon lint path/to/lexicon.json

# Monitor the firehose for labels
goat firehose --watch
```

---

## Architecture

```
┌─────────────────────────────────────────────────────┐
│                   Node.js Server                     │
│  ┌─────────────┐  ┌──────────────┐  ┌────────────┐ │
│  │  Identity    │  │  Profiles    │  │  Chat      │ │
│  │  Module     │  │  Module     │  │  (Haven)   │ │
│  └──────┬──────┘  └──────┬───────┘  └─────┬──────┘ │
│         │                │                 │        │
│  ┌──────┴────────────────┴─────────────────┴──────┐ │
│  │              Features Module                     │ │
│  │           (src/features.js)                     │ │
│  └──────────────────────┬──────────────────────────┘ │
│                         │                             │
│  ┌──────────────────────┴──────────────────────────┐ │
│  │              SQLite (better-sqlite3)              │ │
│  └───────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────┘
```

### Feature Module Pattern

Every Mosaic feature follows these design rules:

1. **Zero modifications to existing Haven code** — only append, never edit
2. **Zero new required dependencies for Haven users** — npm packages are optional
3. **Every feature is a module: optional, skippable, delegatable** — controlled by `FEATURES`
4. **All new tables use `CREATE TABLE IF NOT EXISTS`** — never modify existing tables
5. **Ed25519 via tweetnacl** — `ed25519:<base64URL>` format throughout
