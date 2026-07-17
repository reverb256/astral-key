#!/usr/bin/env node
'use strict';

/**
 * Migrate identity data from mosiac-identity.db to astral_key.db.
 *
 * Usage:
 *   node scripts/migrate-mosiac-identity.js [options] <source-db> <target-db>
 *
 * Options:
 *   --dry-run    Print what would be migrated without writing to target.
 *
 * Example:
 *   node scripts/migrate-mosiac-identity.js \
 *     /path/to/mosiac-identity/data/mosiac-identity.db \
 *     /path/to/astral-key/data/astral_key.db
 *
 * The script is idempotent: it maps source identities to target users by
 * Ed25519 pubkey, so reruns update existing records instead of duplicating.
 */

const fs = require('fs');
const path = require('path');

// ---------------------------------------------------------------------------
// Argument parsing
// ---------------------------------------------------------------------------

const args = process.argv.slice(2);
const dryRun = args.includes('--dry-run');
const positional = args.filter(a => !a.startsWith('--'));

const [sourceDbPath, targetDbPath] = positional;

if (!sourceDbPath || !targetDbPath) {
  console.error('Usage: node migrate-mosiac-identity.js [--dry-run] <source-db> <target-db>');
  process.exit(1);
}

if (!fs.existsSync(sourceDbPath)) {
  console.error(`Source database not found: ${sourceDbPath}`);
  process.exit(1);
}

if (!fs.existsSync(targetDbPath)) {
  console.error(`Target database not found: ${targetDbPath}`);
  console.error('Run astral-key migrations first so the target schema exists.');
  process.exit(1);
}

// ---------------------------------------------------------------------------
// Dependencies
// ---------------------------------------------------------------------------

let Database;
try {
  Database = require('better-sqlite3');
} catch (e) {
  console.error('This script requires better-sqlite3:');
  console.error('  npm install better-sqlite3');
  console.error(e.message);
  process.exit(1);
}

const { v4: uuidv4 } = require('uuid');

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function toRfc3339(sqliteDate) {
  if (!sqliteDate) return new Date().toISOString();
  const d = new Date(sqliteDate);
  if (Number.isNaN(d.getTime())) {
    return new Date().toISOString();
  }
  return d.toISOString();
}

function uuid() {
  return uuidv4();
}

function backupPath(dbPath) {
  const dir = path.dirname(dbPath);
  const base = path.basename(dbPath);
  const timestamp = new Date().toISOString().replace(/[:.]/g, '-');
  return path.join(dir, `${base}.backup-${timestamp}`);
}

// ---------------------------------------------------------------------------
// Migration logic
// ---------------------------------------------------------------------------

function tableExists(db, name) {
  const row = db.prepare("SELECT 1 FROM sqlite_master WHERE type='table' AND name=?").get(name);
  return !!row;
}

function migrate(sourceDbPath, targetDbPath) {
  const source = Database(sourceDbPath, { readonly: true });

  // Verify source tables exist
  for (const table of ['identities', 'passkeys', 'contacts']) {
    if (!tableExists(source, table)) {
      console.error(`Source table '${table}' does not exist. Is this a mosiac-identity database?`);
      source.close();
      process.exit(1);
    }
  }

  const target = Database(targetDbPath);

  // Verify target tables exist
  for (const table of ['users', 'identities', 'fido2_credentials', 'contacts']) {
    if (!tableExists(target, table)) {
      console.error(`Target table '${table}' does not exist. Run astral-key migrations first.`);
      source.close();
      target.close();
      process.exit(1);
    }
  }

  if (dryRun) {
    console.log('DRY RUN — no changes will be written.');
  } else {
    const backupFile = backupPath(targetDbPath);
    fs.copyFileSync(targetDbPath, backupFile);
    console.log(`Backed up target database to ${backupFile}`);
  }

  target.pragma('foreign_keys = OFF');

  const tx = target.transaction(() => {
    // ----------------------------------------------------------------------
    // 1. Build a stable mapping from source identity -> target user
    // ----------------------------------------------------------------------
    const identities = source.prepare('SELECT * FROM identities').all();
    console.log(`Found ${identities.length} identities in source database.`);

    const identityToUser = new Map(); // source identity id -> target user id

    for (const identity of identities) {
      // Look for an existing user that already owns this pubkey identity.
      const existing = target
        .prepare('SELECT user_id FROM identities WHERE pubkey = ? LIMIT 1')
        .get(identity.pubkey);

      let userId;
      if (existing) {
        userId = existing.user_id;
        console.log(`  Reusing existing user ${userId} for identity ${identity.id}`);
      } else {
        userId = uuid();
        if (!dryRun) {
          target
            .prepare(
              `INSERT INTO users (id, created_at, updated_at)
               VALUES (?, ?, ?)`
            )
            .run(userId, toRfc3339(identity.created_at), toRfc3339(identity.created_at));
        }
      }

      identityToUser.set(identity.id, userId);
    }

    // ----------------------------------------------------------------------
    // 2. Migrate identities (idempotent by pubkey)
    // ----------------------------------------------------------------------
    for (const identity of identities) {
      const userId = identityToUser.get(identity.id);
      const existing = target
        .prepare('SELECT id FROM identities WHERE user_id = ? AND pubkey = ?')
        .get(userId, identity.pubkey);

      if (existing) {
        // Update label / current flag in case they changed
        if (!dryRun) {
          target
            .prepare(
              `UPDATE identities
               SET label = ?, is_current = ?, updated_at = ?
               WHERE id = ?`
            )
            .run(
              identity.label || null,
              identity.is_current ? 1 : 0,
              toRfc3339(identity.created_at),
              existing.id
            );
        }
      } else {
        const identityId = uuid();
        if (!dryRun) {
          target
            .prepare(
              `INSERT INTO identities
                 (id, user_id, pubkey, label, is_current, created_at, updated_at)
               VALUES (?, ?, ?, ?, ?, ?, ?)`
            )
            .run(
              identityId,
              userId,
              identity.pubkey,
              identity.label || null,
              identity.is_current ? 1 : 0,
              toRfc3339(identity.created_at),
              toRfc3339(identity.created_at)
            );
        }
      }
    }

    // ----------------------------------------------------------------------
    // 3. Migrate passkeys -> fido2_credentials
    // ----------------------------------------------------------------------
    const passkeys = source.prepare('SELECT * FROM passkeys').all();
    console.log(`Found ${passkeys.length} passkeys in source database.`);

    for (const passkey of passkeys) {
      const userId = identityToUser.get(passkey.identity_id);
      if (!userId) {
        console.warn(`Skipping passkey ${passkey.id}: no matching identity.`);
        continue;
      }

      const existing = target
        .prepare('SELECT id FROM fido2_credentials WHERE credential_id = ?')
        .get(passkey.id);

      if (existing) {
        if (!dryRun) {
          target
            .prepare(
              `UPDATE fido2_credentials
               SET public_key = ?, counter = ?, transport = ?, last_used_at = ?, name = ?
               WHERE id = ?`
            )
            .run(
              passkey.credential,
              passkey.counter || 0,
              passkey.transports || null,
              passkey.last_used_at ? toRfc3339(passkey.last_used_at) : null,
              passkey.nickname || null,
              existing.id
            );
        }
      } else if (!dryRun) {
        target
          .prepare(
            `INSERT INTO fido2_credentials
               (id, user_id, credential_id, public_key, counter, transport,
                attestation_type, created_at, last_used_at, name)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`
          )
          .run(
            uuid(),
            userId,
            passkey.id,
            passkey.credential,
            passkey.counter || 0,
            passkey.transports || null,
            null,
            toRfc3339(passkey.created_at),
            passkey.last_used_at ? toRfc3339(passkey.last_used_at) : null,
            passkey.nickname || null
          );
      }
    }

    // ----------------------------------------------------------------------
    // 4. Migrate contacts
    // ----------------------------------------------------------------------
    const contacts = source.prepare('SELECT * FROM contacts').all();
    console.log(`Found ${contacts.length} contacts in source database.`);

    for (const contact of contacts) {
      // In mosiac-identity contacts were global. In astral-key they are
      // per-user. Attach each global contact to every migrated user so the
      // contact graph is preserved for self-hosted single-user deployments.
      for (const [, userId] of identityToUser) {
        const existing = target
          .prepare('SELECT id FROM contacts WHERE owner_user_id = ? AND pubkey = ?')
          .get(userId, contact.pubkey);

        if (existing) {
          if (!dryRun) {
            target
              .prepare(
                `UPDATE contacts
                 SET label = ?, discovered_via = ?, last_seen_at = ?
                 WHERE id = ?`
              )
              .run(
                contact.label || null,
                contact.discovered_via || 'qr',
                contact.last_seen_at ? toRfc3339(contact.last_seen_at) : null,
                existing.id
              );
          }
        } else if (!dryRun) {
          target
            .prepare(
              `INSERT INTO contacts
                 (id, owner_user_id, pubkey, label, discovered_via, first_seen_at, last_seen_at)
               VALUES (?, ?, ?, ?, ?, ?, ?)`
            )
            .run(
              uuid(),
              userId,
              contact.pubkey,
              contact.label || null,
              contact.discovered_via || 'qr',
              toRfc3339(contact.first_seen_at),
              contact.last_seen_at ? toRfc3339(contact.last_seen_at) : null
            );
        }
      }
    }

    // ----------------------------------------------------------------------
    // 5. Migrate sessions (best-effort, only if target schema supports it)
    // ----------------------------------------------------------------------
    if (tableExists(source, 'sessions') && tableExists(target, 'sessions')) {
      const sessions = source.prepare('SELECT * FROM sessions').all();
      console.log(`Found ${sessions.length} sessions in source database.`);

      for (const session of sessions) {
        const userId = identityToUser.get(session.identity_id);
        if (!userId) {
          console.warn(`Skipping session ${session.token_hash}: no matching identity.`);
          continue;
        }

        const existing = target
          .prepare('SELECT id FROM sessions WHERE token_hash = ?')
          .get(session.token_hash);

        if (!existing && !dryRun) {
          target
            .prepare(
              `INSERT INTO sessions
                 (id, user_id, token_hash, expires_at, created_at, last_used_at)
               VALUES (?, ?, ?, ?, ?, ?)`
            )
            .run(
              uuid(),
              userId,
              session.token_hash,
              toRfc3339(session.expires_at),
              toRfc3339(session.created_at),
              toRfc3339(session.created_at)
            );
        }
      }
    }
  });

  if (!dryRun) {
    tx();
  }

  source.close();
  target.close();

  console.log(dryRun ? 'Dry run complete.' : 'Migration complete.');
}

migrate(sourceDbPath, targetDbPath);
