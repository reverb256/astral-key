'use strict';

/**
 * Mosaic Signed Event System (Phase 5)
 *
 * Every user action (post, follow, profile update, label, reaction) is
 * wrapped in a signed envelope and chained into an append-only per-pubkey
 * event log via SHA-256 hash pointers.  This provides:
 *   - Non-repudiation (Ed25519 signatures over canonical JSON)
 *   - Integrity (each event hashes its predecessor)
 *   - Ordering (monotonic seq + hash chain per pubkey)
 *   - Portability (events are self-validating JSON blobs)
 *
 * No new required dependencies — uses a built-in stable JSON serialiser.
 */

const crypto = require('crypto');

// ─── Stable JSON serialisation ──────────────────────────────
// JSON.stringify with sorted keys so the hash is deterministic
// regardless of object key insertion order.

function stableStringify(obj) {
  if (obj === null || obj === undefined) return 'null';
  if (typeof obj === 'string') return JSON.stringify(obj);
  if (typeof obj === 'number' || typeof obj === 'boolean') return String(obj);
  if (Array.isArray(obj)) {
    return '[' + obj.map(stableStringify).join(',') + ']';
  }
  if (typeof obj === 'object') {
    const keys = Object.keys(obj).sort();
    const pairs = keys.map(k => stableStringify(k) + ':' + stableStringify(obj[k]));
    return '{' + pairs.join(',') + '}';
  }
  return String(obj);
}

// ─── Hashing ─────────────────────────────────────────────────

/**
 * SHA-256 hash of the stable JSON representation of an event's content.
 * The hash covers {type, pubkey, payload, prev_hash, timestamp} — the
 * fields that define the event's meaning (not the signature or event_hash
 * itself).
 *
 * @param {object} event - Event object
 * @returns {string} hex-encoded SHA-256 digest
 */
function hashEvent(event) {
  const payload = {
    type: event.type,
    pubkey: event.pubkey,
    payload: event.payload,
    prev_hash: event.prev_hash || null,
    timestamp: event.timestamp,
  };
  const canon = stableStringify(payload);
  return crypto.createHash('sha256').update(canon, 'utf8').digest('hex');
}

// ─── Event creation ─────────────────────────────────────────

/**
 * Create a signed event.
 *
 * Chains to the previous event for this pubkey by reading prev_hash and
 * seq from an async/sync provided `getLatestHash` / `getLatestSeq` lookup
 * function — the caller passes these so this module stays DB-agnostic.
 *
 * @param {string}   type    - Event type
 * @param {string}   pubkey  - Native ed25519:<base64> public key
 * @param {object}   payload - Action-specific data (must be JSON-serializable)
 * @param {function} signFn  - Signing function: signFn(canonicalPayload) => base64url sig
 * @param {object}   [opts]  - Optional overrides
 * @param {number}   [opts.timestamp]  - Unix epoch seconds (default: now)
 * @param {string|null} [opts.prev_hash] - Override prev hash (default: null = genesis)
 * @param {number}   [opts.seq]  - Sequence number (default: 1)
 * @returns {object} Signed event envelope
 */
function createEvent(type, pubkey, payload, signFn, opts) {
  opts = opts || {};
  const timestamp = opts.timestamp || Math.floor(Date.now() / 1000);
  const prevHash = opts.prev_hash || null;
  const seq = opts.seq || 1;

  const event = {
    type,
    pubkey,
    payload,
    prev_hash: prevHash,
    timestamp,
    seq,
  };

  // Deterministic content hash for chaining (before signing)
  event.event_hash = hashEvent(event);

  // Sign the event_hash
  const signingPayload = stableStringify({ event_hash: event.event_hash });
  event.signature = signFn(signingPayload);

  return event;
}

// ─── Verification ───────────────────────────────────────────

/**
 * Verify a signed event.
 *
 * Checks:
 *   - Ed25519 signature is valid for pubkey
 *   - event_hash matches the event content
 *
 * @param {object}   event    - Signed event object
 * @param {function} verifyFn - verifyFn(canonicalPayload, signature, pubkey) => boolean
 * @returns {{ valid: boolean, reason?: string }}
 */
function verifyEvent(event, verifyFn) {
  // Re-hash content and compare with stored event_hash
  const computedHash = hashEvent(event);
  if (computedHash !== event.event_hash) {
    return { valid: false, reason: 'event_hash mismatch' };
  }

  // Verify the signature over event_hash
  const signingPayload = stableStringify({ event_hash: event.event_hash });
  const ok = verifyFn(signingPayload, event.signature, event.pubkey);
  if (!ok) {
    return { valid: false, reason: 'signature verification failed' };
  }

  return { valid: true };
}

// ─── Event type validation ──────────────────────────────────

const VALID_EVENT_TYPES = new Set([
  'post',
  'follow',
  'profile_update',
  'label',
  'reaction',
]);

function isValidType(type) {
  return VALID_EVENT_TYPES.has(type);
}

module.exports = {
  createEvent,
  verifyEvent,
  hashEvent,
  isValidType,
  VALID_EVENT_TYPES,
};
