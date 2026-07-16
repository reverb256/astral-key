'use strict';

/**
 * Mosiac SQLite Adapter — universal SQLite for every platform.
 *
 * Strategy:
 *   1. Try `better-sqlite3` (native, fast, requires C++ build tools)
 *   2. Fall back to `sql.js` (WASM, zero native deps, runs everywhere)
 *
 * Exposes a unified API matching better-sqlite3's conventions so
 * the rest of the codebase never knows which backend is active.
 */

const path = require('path');
const fs = require('fs');

const DATA_DIR = process.env.MOSIAC_DATA_DIR || path.join(__dirname, '..', 'data');
const DB_PATH = path.join(DATA_DIR, 'mosiac-identity.db');

let db = null;
let backend = null; // 'better-sqlite3' | 'sql.js'

// ─── Schema ────────────────────────────────────────────────

const SCHEMA_SQL = `
  CREATE TABLE IF NOT EXISTS identities (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    pubkey        TEXT    NOT NULL UNIQUE,
    privkey       TEXT    NOT NULL,
    created_at    TEXT    NOT NULL DEFAULT (datetime('now')),
    label         TEXT,
    is_current    INTEGER NOT NULL DEFAULT 0
  );
  CREATE TABLE IF NOT EXISTS passkeys (
    id            TEXT    PRIMARY KEY,
    identity_id   INTEGER NOT NULL REFERENCES identities(id) ON DELETE CASCADE,
    credential    TEXT    NOT NULL,
    transports    TEXT,
    counter       INTEGER NOT NULL DEFAULT 0,
    nickname      TEXT,
    created_at    TEXT    NOT NULL DEFAULT (datetime('now')),
    last_used_at  TEXT
  );
  CREATE TABLE IF NOT EXISTS contacts (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    pubkey        TEXT    NOT NULL UNIQUE,
    label         TEXT,
    discovered_via TEXT   DEFAULT 'qr',
    first_seen_at TEXT    NOT NULL DEFAULT (datetime('now')),
    last_seen_at  TEXT
  );
  CREATE TABLE IF NOT EXISTS sessions (
    token_hash    TEXT    PRIMARY KEY,
    identity_id   INTEGER NOT NULL REFERENCES identities(id) ON DELETE CASCADE,
    pubkey        TEXT    NOT NULL,
    created_at    TEXT    NOT NULL DEFAULT (datetime('now')),
    expires_at    TEXT    NOT NULL
  );
  CREATE INDEX IF NOT EXISTS idx_passkeys_identity ON passkeys(identity_id);
  CREATE INDEX IF NOT EXISTS idx_sessions_pubkey ON sessions(pubkey);
  CREATE INDEX IF NOT EXISTS idx_identities_current ON identities(is_current);
`;

// ─── sql.js compatibility layer ────────────────────────────
// Wraps sql.js to match better-sqlite3's prepare().get/.run/.all interface

function createSqlJsWrapper(SqlJsLib, dbPath) {
  let raw;

  if (dbPath && fs.existsSync(dbPath)) {
    // Load existing database
    const buf = fs.readFileSync(dbPath);
    raw = new SqlJsLib.Database(buf);
  } else {
    raw = new SqlJsLib.Database();
  }

  raw.run('PRAGMA journal_mode=WAL');

  function prepare(sql) {
    return {
      get(params) {
        const stmt = raw.prepare(sql);
        if (!stmt) return null;
        const result = params !== undefined ? stmt.getAsObject(params) : stmt.getAsObject();
        stmt.free();
        return result || null;
      },
      all(params) {
        const stmt = raw.prepare(sql);
        if (!stmt) return [];
        if (params !== undefined) stmt.bind(params);
        const results = [];
        while (stmt.step()) results.push(stmt.getAsObject());
        stmt.free();
        return results;
      },
      run(params) {
        const stmt = raw.prepare(sql);
        if (!stmt) return { changes: 0, lastInsertRowid: 0 };
        if (params !== undefined) stmt.bind(params);
        stmt.step();
        stmt.free();
        const lastId = raw.exec("SELECT last_insert_rowid() AS id");
        const changes = raw.getRowsModified();
        const lastInsertRowid = lastId.length > 0 ? lastId[0].values[0][0] : 0;
        return { changes, lastInsertRowid };
      },
    };
  }

  return {
    raw,
    prepare,
    exec(sql) { raw.run(sql); },
    close() { raw.close(); },
    pragma() {}, // no-op for sql.js
  };
}

// ─── Init ───────────────────────────────────────────────────

function initDatabase() {
  if (!fs.existsSync(DATA_DIR)) fs.mkdirSync(DATA_DIR, { recursive: true });

  // 1. Try better-sqlite3 (native, fast)
  try {
    const Database = require('better-sqlite3');
    db = new Database(DB_PATH);
    db.pragma('journal_mode = WAL');
    db.exec(SCHEMA_SQL);
    backend = 'better-sqlite3';
    console.log(`  [sqlite] backend: better-sqlite3 (native, fast)`);
    return db;
  } catch (e) {
    console.log(`  [sqlite] better-sqlite3 unavailable: ${e.message}`);
    console.log(`  [sqlite] falling back to sql.js (WASM — zero native deps)`);
  }

  // 2. Fall back to sql.js (WASM, zero native dependencies)
  try {
    const SqlJsLib = require('sql.js');
    db = createSqlJsWrapper(SqlJsLib, DB_PATH);
    db.exec(SCHEMA_SQL);

    // Periodically save to disk (sql.js is in-memory by default)
    setInterval(() => {
      try {
        const data = db.raw.export();
        fs.writeFileSync(DB_PATH, Buffer.from(data));
      } catch (e) {
        console.error(`  [sqlite] failed to save database: ${e.message}`);
      }
    }, 30000); // every 30s

    // Save on exit
    process.on('exit', () => {
      try {
        const data = db.raw.export();
        fs.writeFileSync(DB_PATH, Buffer.from(data));
      } catch {}
    });

    backend = 'sql.js';
    console.log(`  [sqlite] backend: sql.js (WASM — truly universal)`);
    return db;
  } catch (e) {
    console.error(`  [sqlite] FATAL: no SQLite backend available.`);
    console.error(`    tried better-sqlite3: failed (${e.message})`);
    console.error(`    tried sql.js:          failed`);
    console.error(`  Install one of: npm install better-sqlite3  OR  npm install sql.js`);
    process.exit(1);
  }
}

function getDb() {
  if (!db) throw new Error('Database not initialized. Call initDatabase() first.');
  return db;
}

function getIdentityDb() {
  if (!db) throw new Error('Database not initialized. Call initDatabase() first.');
  return db;
}

module.exports = { initDatabase, getDb, getIdentityDb };
