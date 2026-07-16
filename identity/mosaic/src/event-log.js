'use strict';

/**
 * Mosaic Event Log (Phase 5)
 *
 * Append-only per-pubkey event log backed by SQLite.  Events are stored
 * in the `event_log` table (defined in src/database.js) and accessed via
 * the CRUD functions exported there.
 *
 * This module provides a higher-level API for the gossip and export
 * subsystems, wrapping the raw database operations.
 */

const { getDb, getLatestSeq, getLatestHash, getEvents, appendEvent, getEventsSince, pruneEvents } = require('./database');
const { hashEvent } = require('./events');

// ─── Append ─────────────────────────────────────────────────

/**
 * Append a signed event to the log for its pubkey.
 *
 * Automatically computes seq and prev_hash from the current tip so the
 * caller does not need to manage the chain state.
 *
 * @param {object} event - Signed event (must have type, pubkey, payload, timestamp, signature, event_hash)
 * @returns {number|null} The assigned sequence number, or null on failure
 */
function appendToLog(event) {
  const { pubkey } = event;
  const db = getDb();

  // Atomically compute next seq and insert
  const tx = db.transaction(() => {
    const latestSeq = getLatestSeq(pubkey);
    const latestHash = getLatestHash(pubkey);
    const seq = latestSeq + 1;

    // Override chain fields on the event
    event.seq = seq;
    event.prev_hash = latestHash;

    // Re-hash now that we set seq and prev_hash
    event.event_hash = hashEvent(event);

    appendEvent(pubkey, event);
    return seq;
  });

  try {
    return tx();
  } catch (err) {
    console.error('[event-log] append failed:', err.message);
    return null;
  }
}

// ─── Query ──────────────────────────────────────────────────

/**
 * Query events for a pubkey with pagination.
 *
 * @param {string}  pubkey
 * @param {number}  [fromSeq=0]   - Return events with seq > fromSeq
 * @param {number}  [limit=100]
 * @returns {object[]} Parsed event rows
 */
function queryEvents(pubkey, fromSeq, limit) {
  const rows = getEvents(pubkey, fromSeq || 0, limit || 100);
  return rows.map(parseRow);
}

/**
 * Get all events since a Unix timestamp (for sync).
 *
 * @param {string}  pubkey
 * @param {number}  sinceTimestamp - Unix epoch seconds
 * @returns {object[]}
 */
function queryEventsSince(pubkey, sinceTimestamp) {
  const rows = getEventsSince(pubkey, sinceTimestamp);
  return rows.map(parseRow);
}

/**
 * Get the latest sequence number for a pubkey.
 *
 * @param {string} pubkey
 * @returns {number}
 */
function queryLatestSeq(pubkey) {
  return getLatestSeq(pubkey);
}

/**
 * Get the latest event_hash for a pubkey (for chaining).
 *
 * @param {string} pubkey
 * @returns {string|null}
 */
function queryLatestHash(pubkey) {
  return getLatestHash(pubkey);
}

// ─── Maintenance ────────────────────────────────────────────

/**
 * Prune events older than a given timestamp.
 *
 * @param {number} beforeTimestamp - Unix epoch seconds
 * @returns {number} Number of deleted rows
 */
function prune(beforeTimestamp) {
  return pruneEvents(beforeTimestamp);
}

// ─── Internal helpers ───────────────────────────────────────

function parseRow(row) {
  return {
    type: row.event_type,
    pubkey: row.pubkey,
    payload: JSON.parse(row.payload),
    prev_hash: row.prev_hash,
    timestamp: row.timestamp,
    seq: row.seq,
    event_hash: row.event_hash,
    signature: row.signature,
    created_at: row.created_at,
  };
}

module.exports = {
  appendToLog,
  queryEvents,
  queryEventsSince,
  queryLatestSeq,
  queryLatestHash,
  prune,
};
