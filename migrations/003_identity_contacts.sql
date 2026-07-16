-- Astral Key - Identity and Contacts Schema (SQLite)
-- Migration: 003_identity_contacts
--
-- Ed25519 public-key identities and contact graph, migrated from mosiac-identity.
-- Private keys are intentionally NOT stored server-side; clients hold them.

-- Identities Table
-- Each Astral Key user may have one or more Ed25519 public-key identities.
CREATE TABLE IF NOT EXISTS identities (
    id TEXT PRIMARY KEY NOT NULL,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    pubkey TEXT NOT NULL,
    label TEXT,
    is_current INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    UNIQUE (user_id, pubkey)
);

CREATE INDEX IF NOT EXISTS idx_identities_user_id ON identities(user_id);
CREATE INDEX IF NOT EXISTS idx_identities_pubkey ON identities(pubkey);
CREATE INDEX IF NOT EXISTS idx_identities_current ON identities(user_id, is_current);
-- Enforce only one current identity per user at the DB level
CREATE UNIQUE INDEX IF NOT EXISTS idx_identities_user_current
  ON identities(user_id) WHERE is_current = 1;

-- Contacts Table
-- Per-user contact graph. owner_user_id is the user who saved the contact.
CREATE TABLE IF NOT EXISTS contacts (
    id TEXT PRIMARY KEY NOT NULL,
    owner_user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    pubkey TEXT NOT NULL,
    label TEXT,
    discovered_via TEXT DEFAULT 'qr',
    first_seen_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    last_seen_at TEXT,
    UNIQUE (owner_user_id, pubkey)
);

CREATE INDEX IF NOT EXISTS idx_contacts_owner_user_id ON contacts(owner_user_id);
CREATE INDEX IF NOT EXISTS idx_contacts_pubkey ON contacts(pubkey);
