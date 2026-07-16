'use strict';

/**
 * Mosaic Moderation Label System
 *
 * Infrastructure inspired by atproto's label system (signed labels,
 * label streams, labeler identity) with Mosaic-specific policies:
 *
 * - Labels carry a mandatory `note` (reason for labelling)
 * - Labels carry a mandatory `expiresAt` TTL
 * - Labels are visible to the labelled user
 * - Appeals are public and have a resolution workflow
 *
 * Labeler identity uses Ed25519 pubkeys in ed25519:<base64> format.
 * Optional atproto DID interop can be added via identity.did() later.
 */

const { getDb } = require('./database');

// ─── Label CRUD ──────────────────────────────────────────────

/**
 * Apply a label to a URI.
 *
 * @param {string}     uri          — what is being labelled (e.g. a post URI or profile DID)
 * @param {string}     value        — label value: 'spam', 'harassment', 'misinfo', 'nsfw'
 * @param {string}     labelerPubkey — Ed25519 pubkey of the labeler (ed25519:<base64>)
 * @param {string}     note         — mandatory reason/explanation
 * @param {string}     expiresAt    — ISO 8601 expiry timestamp
 * @param {function}   signFn       — async or sync function(payload) => signature string
 * @returns {object}   the inserted label row
 */
function applyLabel(uri, value, labelerPubkey, note, expiresAt, signFn) {
  const db = getDb();
  const cid = cryptoHash(uri + value + labelerPubkey + Date.now() + Math.random());

  const payload = { uri, val: value, src: labelerPubkey, note, expires_at: expiresAt };
  const sig = signFn ? signFn(JSON.stringify(payload)) : 'unsigned';

  db.prepare(`
    INSERT INTO moderation_labels (cid, uri, val, src, note, expires_at, sig, created_at)
    VALUES (?, ?, ?, ?, ?, ?, ?, datetime('now'))
  `).run(cid, uri, value, labelerPubkey, note, expiresAt, sig);

  return { cid, uri, val: value, src: labelerPubkey, note, expires_at: expiresAt, sig };
}

/**
 * Negate a label — marks that a previous label should no longer apply.
 *
 * @param {string}     labelCid      — the CID of the label to negate
 * @param {string}     labelerPubkey — pubkey of the negating labeler
 * @param {function}   signFn        — signing function
 * @returns {object}   the negation label row
 */
function negateLabel(labelCid, labelerPubkey, signFn) {
  const db = getDb();
  const original = db.prepare('SELECT * FROM moderation_labels WHERE cid = ?').get(labelCid);
  if (!original) throw new Error('Label not found: ' + labelCid);

  const cid = cryptoHash('negate:' + labelCid + labelerPubkey + Date.now());
  const payload = {
    uri: original.uri,
    val: original.val,
    src: labelerPubkey,
    neg: 1,
    note: 'Negation of ' + labelCid,
    expires_at: new Date(Date.now() + 90 * 24 * 60 * 60 * 1000).toISOString(), // 90d default
  };
  const sig = signFn ? signFn(JSON.stringify(payload)) : 'unsigned';

  db.prepare(`
    INSERT INTO moderation_labels (cid, uri, val, src, neg, note, expires_at, sig, created_at)
    VALUES (?, ?, ?, ?, 1, ?, ?, ?, datetime('now'))
  `).run(cid, payload.uri, payload.val, labelerPubkey, payload.note, payload.expires_at, sig);

  return { cid, ...payload, sig };
}

/**
 * Get all labels for a URI (including negated and expired).
 */
function getLabels(uri) {
  const db = getDb();
  return db.prepare('SELECT * FROM moderation_labels WHERE uri = ? ORDER BY created_at DESC').all(uri);
}

/**
 * Get non-expired, non-negated labels for a URI.
 */
function getActiveLabels(uri) {
  const db = getDb();
  return db.prepare(`
    SELECT * FROM moderation_labels
    WHERE uri = ? AND neg = 0 AND expires_at > datetime('now')
    ORDER BY created_at DESC
  `).all(uri);
}

/**
 * Get all labels created by a specific labeler.
 */
function getLabelsByLabeler(pubkey) {
  const db = getDb();
  return db.prepare('SELECT * FROM moderation_labels WHERE src = ? ORDER BY created_at DESC').all(pubkey);
}

/**
 * Subscribe to labels since a cursor (for streaming to peers).
 *
 * @param {string} [since] — ISO 8601 timestamp cursor; omit for all labels
 * @returns {object[]}
 */
function subscribeLabels(since) {
  const db = getDb();
  if (since) {
    return db.prepare('SELECT * FROM moderation_labels WHERE created_at > ? ORDER BY created_at ASC').all(since);
  }
  return db.prepare('SELECT * FROM moderation_labels ORDER BY created_at ASC LIMIT 100').all();
}

// ─── Reports ─────────────────────────────────────────────────

/**
 * Submit a moderation report.
 */
function createReport(uri, reasonType, reason, reportedBy) {
  const db = getDb();
  const result = db.prepare(`
    INSERT INTO moderation_reports (uri, reason_type, reason, reported_by, created_at)
    VALUES (?, ?, ?, ?, datetime('now'))
  `).run(uri, reasonType, reason || null, reportedBy);
  return { id: result.lastInsertRowid, uri, reason_type: reasonType, reason, reported_by: reportedBy };
}

// ─── Appeals ─────────────────────────────────────────────────

/**
 * Appeal a label.
 */
function createAppeal(labelCid, pubkey, reason, evidence) {
  const db = getDb();
  const result = db.prepare(`
    INSERT INTO moderation_appeals (label_cid, pubkey, reason, evidence, status, created_at)
    VALUES (?, ?, ?, ?, 'pending', datetime('now'))
  `).run(labelCid, pubkey, reason, evidence || null);
  return { id: result.lastInsertRowid, label_cid: labelCid, pubkey, reason, evidence, status: 'pending' };
}

/**
 * Resolve an appeal (accept or reject).
 */
function resolveAppeal(appealId, status, resolution) {
  if (!['accepted', 'rejected'].includes(status)) {
    throw new Error('Invalid appeal status. Must be accepted or rejected.');
  }
  const db = getDb();
  db.prepare(`
    UPDATE moderation_appeals SET status = ?, resolution = ? WHERE id = ?
  `).run(status, resolution || null, appealId);
  return { id: appealId, status, resolution };
}

// ─── Helpers ─────────────────────────────────────────────────

function cryptoHash(str) {
  const crypto = require('crypto');
  return crypto.createHash('sha256').update(str, 'utf8').digest('hex');
}

module.exports = {
  applyLabel,
  negateLabel,
  getLabels,
  getActiveLabels,
  getLabelsByLabeler,
  subscribeLabels,
  createReport,
  createAppeal,
  resolveAppeal,
};
