-- Astral Key - Initial Database Schema (SQLite)
-- Migration: 001_initial
--
-- Core tables for Passkey + Web3 (SIWE) auth sidecar.

-- Users Table
CREATE TABLE IF NOT EXISTS users (
    id TEXT PRIMARY KEY NOT NULL,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE INDEX IF NOT EXISTS idx_users_created_at ON users(created_at);

-- Web3 Wallets Table
CREATE TABLE IF NOT EXISTS web3_wallets (
    id TEXT PRIMARY KEY NOT NULL,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    address TEXT NOT NULL,
    chain_id INTEGER NOT NULL,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    last_used_at TEXT,
    UNIQUE (address, chain_id)
);

CREATE INDEX IF NOT EXISTS idx_web3_wallets_user_id ON web3_wallets(user_id);
CREATE INDEX IF NOT EXISTS idx_web3_wallets_address ON web3_wallets(address);
CREATE INDEX IF NOT EXISTS idx_web3_wallets_chain_id ON web3_wallets(chain_id);

-- FIDO2 Credentials Table
CREATE TABLE IF NOT EXISTS fido2_credentials (
    id TEXT PRIMARY KEY NOT NULL,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    credential_id TEXT NOT NULL UNIQUE,
    public_key TEXT NOT NULL,
    counter INTEGER NOT NULL DEFAULT 0,
    transport TEXT,
    attestation_type TEXT,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    last_used_at TEXT,
    name TEXT
);

CREATE INDEX IF NOT EXISTS idx_fido2_credentials_user_id ON fido2_credentials(user_id);
CREATE INDEX IF NOT EXISTS idx_fido2_credentials_credential_id ON fido2_credentials(credential_id);

-- Nonces Table (for SIWE)
CREATE TABLE IF NOT EXISTS nonces (
    id TEXT PRIMARY KEY NOT NULL,
    nonce TEXT NOT NULL UNIQUE,
    expires_at TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    used_at TEXT,
    user_id TEXT REFERENCES users(id) ON DELETE SET NULL
);

CREATE INDEX IF NOT EXISTS idx_nonces_nonce ON nonces(nonce);
CREATE INDEX IF NOT EXISTS idx_nonces_expires_at ON nonces(expires_at);
CREATE INDEX IF NOT EXISTS idx_nonces_created_at ON nonces(created_at);
