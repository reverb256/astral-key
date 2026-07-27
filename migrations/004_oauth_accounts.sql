-- Astral Key - OAuth Accounts
-- Migration: 004_oauth_accounts
--
-- Links external OAuth provider accounts (e.g. GitHub) to internal users.

CREATE TABLE IF NOT EXISTS oauth_accounts (
    id TEXT PRIMARY KEY NOT NULL,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    provider TEXT NOT NULL,
    provider_user_id TEXT NOT NULL,
    email TEXT,
    name TEXT,
    avatar_url TEXT,
    access_token TEXT,
    refresh_token TEXT,
    expires_at TEXT,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    UNIQUE (provider, provider_user_id)
);

CREATE INDEX IF NOT EXISTS idx_oauth_accounts_user_id ON oauth_accounts(user_id);
CREATE INDEX IF NOT EXISTS idx_oauth_accounts_provider ON oauth_accounts(provider, provider_user_id);
