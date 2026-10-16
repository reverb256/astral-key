-- Astral Key — OIDC refresh tokens
-- Migration: 008_oidc_refresh_tokens
--
-- The token endpoint issues an opaque refresh token alongside the id_token.
-- Only its SHA-256 hash is stored, so a database leak cannot be replayed.
-- Tokens are rotated on every refresh: the presented row is deleted and a
-- new one inserted.

CREATE TABLE IF NOT EXISTS oidc_refresh_tokens (
    token_hash TEXT PRIMARY KEY NOT NULL,
    client_id  TEXT NOT NULL REFERENCES oidc_clients(client_id) ON DELETE CASCADE,
    user_id    TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    scope      TEXT NOT NULL,
    expires_at TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE INDEX IF NOT EXISTS idx_oidc_refresh_tokens_user ON oidc_refresh_tokens (user_id);
