-- Astral Key - Initial Database Schema
-- Migration: 001_initial
-- Created: 2026-02-18
--
-- This migration creates the core tables for Astral Key:
-- - users: Core user records
-- - web3_wallets: Web3 wallet addresses for SIWE authentication
-- - fido2_credentials: FIDO2/WebAuthn passkey credentials
-- - sessions: User sessions with refresh tokens
-- - nonces: SIWE nonces with expiration

-- Enable UUID extension
CREATE EXTENSION IF NOT EXISTS "uuid-ossp";

-- ============================================================================
-- Users Table
-- ============================================================================
-- Core user records. Users can have multiple authentication methods
-- (Web3 wallets, FIDO2 credentials) linked to their account.

CREATE TABLE users (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Index for common queries
CREATE INDEX idx_users_created_at ON users(created_at);

-- ============================================================================
-- Web3 Wallets Table
-- ============================================================================
-- Stores Ethereum (and other EVM) wallet addresses for SIWE authentication.
-- A user can have multiple wallets (e.g., mainnet + testnet).

CREATE TABLE web3_wallets (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    address TEXT NOT NULL,
    chain_id INTEGER NOT NULL,

    -- Metadata
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_used_at TIMESTAMPTZ,

    -- Constraints
    CONSTRAINT unique_wallet_address UNIQUE (address, chain_id)
);

-- Indexes for common queries
CREATE INDEX idx_web3_wallets_user_id ON web3_wallets(user_id);
CREATE INDEX idx_web3_wallets_address ON web3_wallets(address);
CREATE INDEX idx_web3_wallets_chain_id ON web3_wallets(chain_id);

-- ============================================================================
-- FIDO2 Credentials Table
-- ============================================================================
-- Stores WebAuthn passkey credentials. The credential_id is the unique
-- identifier from the authenticator. The public_key is stored for verification.

CREATE TABLE fido2_credentials (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,

    -- WebAuthn credential data
    credential_id TEXT NOT NULL UNIQUE,
    public_key TEXT NOT NULL,

    -- Authenticator metadata
    counter BIGINT NOT NULL DEFAULT 0,

    -- Transport type (usb, nfc, ble, internal)
    transport TEXT,

    -- Attestation type (none, basic, self, attca)
    attestation_type TEXT,

    -- Metadata
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_used_at TIMESTAMPTZ,

    -- User-friendly name for the credential
    name TEXT
);

-- Indexes for common queries
CREATE INDEX idx_fido2_credentials_user_id ON fido2_credentials(user_id);
CREATE INDEX idx_fido2_credentials_credential_id ON fido2_credentials(credential_id);

-- ============================================================================
-- Sessions Table
-- ============================================================================
-- Stores user sessions with refresh tokens. Access tokens are JWTs and
-- don't need to be stored (they're stateless). Only refresh tokens are
-- stored in the database for token rotation and revocation.

CREATE TABLE sessions (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,

    -- Refresh token (hashed)
    refresh_token_hash TEXT NOT NULL UNIQUE,

    -- Session metadata
    expires_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    -- Device/browser info for session management
    user_agent TEXT,
    ip_address TEXT,

    -- Revoked flag (for logout)
    revoked_at TIMESTAMPTZ
);

-- Indexes for common queries
CREATE INDEX idx_sessions_user_id ON sessions(user_id);
CREATE INDEX idx_sessions_expires_at ON sessions(expires_at);
CREATE INDEX idx_sessions_refresh_token_hash ON sessions(refresh_token_hash);

-- ============================================================================
-- Nonces Table
-- ============================================================================
-- Stores SIWE nonces with expiration. Nonces are single-use and expire
-- after a short time (typically 5-15 minutes) to prevent replay attacks.

CREATE TABLE nonces (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    nonce TEXT NOT NULL UNIQUE,

    -- Expiration
    expires_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    -- Track if used (for replay protection)
    used_at TIMESTAMPTZ,

    -- Optional: Associate with a specific user ID if we want to
    -- track which user requested the nonce
    user_id UUID REFERENCES users(id) ON DELETE SET NULL
);

-- Indexes for lookups and cleanup
CREATE INDEX idx_nonces_nonce ON nonces(nonce);
CREATE INDEX idx_nonces_expires_at ON nonces(expires_at);
CREATE INDEX idx_nonces_created_at ON nonces(created_at);

-- ============================================================================
-- Trigger: Update updated_at timestamp
-- ============================================================================
-- Automatically update the updated_at column on rows in the users table

CREATE OR REPLACE FUNCTION update_updated_at_column()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ language 'plpgsql';

CREATE TRIGGER update_users_updated_at BEFORE UPDATE ON users
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

-- ============================================================================
-- Migration Complete
-- ============================================================================
-- Schema version: 001
-- Total tables: 5
-- Total indexes: 15
-- Total triggers: 1
