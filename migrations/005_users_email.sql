-- Astral Key — OIDC identity support
--
-- Add email + display name to users so they can act as OIDC subjects
-- (oauth2-proxy requires an email claim). Columns are nullable so the
-- existing Web3/OAuth-created users keep working unchanged.

ALTER TABLE users ADD COLUMN email TEXT;
ALTER TABLE users ADD COLUMN display_name TEXT;

-- A user's email is their stable login identifier (case-insensitive).
-- Partial unique index: NULL emails (legacy users) are not constrained.
CREATE UNIQUE INDEX IF NOT EXISTS users_email_unique ON users (lower(email)) WHERE email IS NOT NULL;
