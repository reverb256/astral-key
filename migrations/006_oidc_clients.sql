-- Astral Key — OIDC client registry (relying parties)
-- Migration: 006_oidc_clients
--
-- /oidc/authorize and /oidc/token resolve their client from this table.
-- redirect_uris is matched EXACTLY — no prefixes, no wildcards, no
-- normalisation — so an attacker cannot widen a registered callback.
--
--   client_secret_hash : lowercase hex SHA-256 of the client secret.
--                        NULL marks a PUBLIC client, which must use PKCE
--                        (S256) and must not present a secret.
--   redirect_uris      : JSON array of absolute redirect URIs.
--   allowed_scopes     : JSON array of scopes the client may request.
--                        NULL allows every advertised scope.
--
-- The legacy env-configured client (OIDC_CLIENT_ID / OIDC_CLIENT_SECRET) is
-- upserted into this table at boot so existing installs keep working.

CREATE TABLE IF NOT EXISTS oidc_clients (
    client_id          TEXT PRIMARY KEY NOT NULL,
    client_secret_hash TEXT,
    redirect_uris      TEXT NOT NULL,
    allowed_scopes     TEXT,
    created_at         TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);
