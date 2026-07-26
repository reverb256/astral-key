-- Key store: one row per Ed25519 key generated or imported
CREATE TABLE IF NOT EXISTS keys (
    key_id              TEXT PRIMARY KEY,
    pubkey_hex          TEXT NOT NULL UNIQUE,
    privkey_pkcs8_hex   TEXT,           -- NULL for keys we only import pubkey for
    algorithm           TEXT NOT NULL DEFAULT 'Ed25519',
    created_at          TEXT NOT NULL,
    rotated_from        TEXT,
    -- Post-quantum (ML-DSA-65, FIPS 204) keypair, minted alongside Ed25519.
    -- NULL when the key was created without the `pq` feature.
    ml_dsa_pubkey_hex   TEXT,
    ml_dsa_privkey_hex  TEXT
);

-- Identity bindings: links a Mosaic key to an external identity
CREATE TABLE IF NOT EXISTS bindings (
    key_id          TEXT NOT NULL REFERENCES keys(key_id),
    protocol        TEXT NOT NULL,      -- 'atproto', 'nostr', 'matrix', 'irc'
    external_id     TEXT NOT NULL,      -- the DID, npub, MXID, or IRC nick
    proof           TEXT,               -- optional attestation signature
    claimed_at      TEXT NOT NULL,
    PRIMARY KEY (key_id, protocol)
);

-- Key rotation history
CREATE TABLE IF NOT EXISTS rotations (
    old_key_id      TEXT NOT NULL,
    new_key_id      TEXT NOT NULL REFERENCES keys(key_id),
    cross_sig       TEXT NOT NULL,      -- old key signing the rotation
    rotated_at      TEXT NOT NULL,
    PRIMARY KEY (old_key_id, rotated_at)
);
