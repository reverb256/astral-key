-- Astral Key — OIDC groups
-- Migration: 007_oidc_groups
--
-- Membership drives the `groups` claim. The claim is ALWAYS present as an
-- array — empty when the user has no membership — in both the id_token and
-- /oidc/userinfo.
--
-- Seeding with SQL is the management path for now:
--
--   INSERT INTO oidc_groups (name) VALUES ('admins');
--   INSERT INTO oidc_group_members (user_id, group_name)
--       VALUES ('<user uuid>', 'admins');

CREATE TABLE IF NOT EXISTS oidc_groups (
    name       TEXT PRIMARY KEY NOT NULL,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE TABLE IF NOT EXISTS oidc_group_members (
    user_id    TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    group_name TEXT NOT NULL REFERENCES oidc_groups(name) ON DELETE CASCADE,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    PRIMARY KEY (user_id, group_name)
);

CREATE INDEX IF NOT EXISTS idx_oidc_group_members_user ON oidc_group_members (user_id);
