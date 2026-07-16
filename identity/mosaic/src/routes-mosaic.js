'use strict';

/**
 * Mosaic Routes — non-auth endpoints that don't fit in auth.js.
 * Auth endpoints (WebAuthn, identity CRUD) now live in src/auth.js
 * under /api/auth/passkey/* and /api/auth/identity/*.
 */
const express = require('express');
const router = express.Router();

const identity = require('./identity');
const qr = require('./qr');
const { getDb } = require('./database');
const { AstralKeyClient } = require('./astral-key-client');

const ASTRAL_KEY_URL = process.env.ASTRAL_KEY_URL || 'http://localhost:3001';
const astral = new AstralKeyClient(ASTRAL_KEY_URL);

/* ─── Health ─── */
router.get('/health', (req, res) => res.json({ ok: true, mosaic: '0.1.0' }));

/* ─── Runtime Config ─── */
router.get('/config', (req, res) => {
  res.json({
    features: (process.env.FEATURES || 'all').split(',').map(s => s.trim()),
    chat_server: process.env.CHAT_SERVER_URL || null,
    identity_server: process.env.IDENTITY_SERVER_URL || null,
  });
});

/* ─── Identity query (read-only — CRUD is in auth.js under /api/auth/identity/*) ─── */
router.get('/identity/current', (req, res) => {
  try {
    const row = getDb().prepare('SELECT * FROM identities WHERE is_current = 1').get();
    if (!row) return res.json({ identity: null });
    res.json({ identity: { id: row.id, pubkey: row.pubkey, label: row.label, pubkeyHex: identity.toHex(identity.fromBase64URL(row.pubkey)) } });
  } catch (e) { res.status(500).json({ error: e.message }); }
});

/* ─── QR ─── */
router.get('/qr/:pubkey', async (req, res) => {
  try {
    const svg = await qr.generatePubkeyQR_SVG(req.params.pubkey);
    res.type('image/svg+xml').send(svg);
  } catch (e) { res.status(400).json({ error: e.message }); }
});

router.post('/qr/scan', (req, res) => {
  try {
    const result = qr.processQRScan(req.body.scanned, req.body.label);
    res.json(result);
  } catch (e) { res.status(400).json({ error: e.message }); }
});

/* ─── Contacts ─── */
router.get('/contacts', (req, res) => {
  try {
    const rows = getDb().prepare('SELECT * FROM contacts ORDER BY first_seen_at DESC').all();
    res.json(rows);
  } catch (e) { res.status(500).json({ error: e.message }); }
});

router.delete('/contacts/:pubkey', (req, res) => {
  try {
    getDb().prepare('DELETE FROM contacts WHERE pubkey = ?').run(req.params.pubkey);
    res.json({ ok: true });
  } catch (e) { res.status(500).json({ error: e.message }); }
});

/* ─── Signing (event bus foundation) ─── */
router.post('/sign', (req, res) => {
  try {
    const { pubkey, data } = req.body;
    const row = getDb().prepare('SELECT * FROM identities WHERE pubkey = ?').get(pubkey);
    if (!row) return res.status(404).json({ error: 'Identity not found' });
    const signed = identity.signJSON(data, row.privkey, row.pubkey);
    res.json(signed);
  } catch (e) { res.status(400).json({ error: e.message }); }
});

router.post('/verify', (req, res) => {
  try {
    const valid = identity.verifyJSON(req.body);
    res.json({ valid });
  } catch (e) { res.status(400).json({ error: e.message }); }
});

/* ─── Moderation: Labels ────────────────────────────────────── */

/**
 * POST /mosaic/label/apply — Apply a label to a URI.
 * Body: { uri, value, labeler_pubkey, note, expires_at, signature }
 */
router.post('/label/apply', (req, res) => {
  try {
    const { uri, value, labeler_pubkey, note, expires_at, signature } = req.body;
    if (!uri || !value || !labeler_pubkey || !note || !expires_at) {
      return res.status(400).json({ error: 'Missing required fields: uri, value, labeler_pubkey, note, expires_at' });
    }
    if (!/^ed25519:/.test(labeler_pubkey)) {
      return res.status(400).json({ error: 'Invalid labeler pubkey format. Expected ed25519:<base64>' });
    }
    const { applyLabel } = require('./labels');
    const signFn = signature ? () => signature : null;
    const result = applyLabel(uri, value, labeler_pubkey, note, expires_at, signFn);
    res.status(201).json(result);
  } catch (e) { res.status(500).json({ error: e.message }); }
});

/**
 * POST /mosaic/label/negate — Negate a previously applied label.
 * Body: { label_cid, labeler_pubkey, signature }
 */
router.post('/label/negate', (req, res) => {
  try {
    const { label_cid, labeler_pubkey, signature } = req.body;
    if (!label_cid || !labeler_pubkey) {
      return res.status(400).json({ error: 'Missing required fields: label_cid, labeler_pubkey' });
    }
    const { negateLabel } = require('./labels');
    const signFn = signature ? () => signature : null;
    const result = negateLabel(label_cid, labeler_pubkey, signFn);
    res.json(result);
  } catch (e) { res.status(500).json({ error: e.message }); }
});

/**
 * GET /mosaic/label/list — Get labels for a URI (query: ?uri=).
 * Optional query params: ?uri=<uri>&active=true
 */
router.get('/label/list', (req, res) => {
  try {
    const { uri, active } = req.query;
    const { getLabels, getActiveLabels } = require('./labels');
    if (uri) {
      const labels = active === 'true' ? getActiveLabels(uri) : getLabels(uri);
      return res.json({ uri, labels });
    }
    // Return recent labels when no URI
    const { subscribeLabels } = require('./labels');
    const labels = subscribeLabels();
    res.json({ labels });
  } catch (e) { res.status(500).json({ error: e.message }); }
});

/* ─── Moderation: Reports ──────────────────────────────────── */

/**
 * POST /mosaic/report/create — Submit a moderation report.
 * Body: { uri, reason_type, reason }
 */
router.post('/report/create', (req, res) => {
  try {
    const { uri, reason_type, reason } = req.body;
    if (!uri || !reason_type) {
      return res.status(400).json({ error: 'Missing required fields: uri, reason_type' });
    }
    const reportedBy = req.headers['x-mosaic-pubkey'] || 'anonymous';
    const { createReport } = require('./labels');
    const result = createReport(uri, reason_type, reason, reportedBy);
    res.status(201).json(result);
  } catch (e) { res.status(500).json({ error: e.message }); }
});

/**
 * GET /mosaic/report/list — List reports by the current user.
 */
router.get('/report/list', (req, res) => {
  try {
    const reportedBy = req.headers['x-mosaic-pubkey'] || 'anonymous';
    const db = getDb();
    const reports = db.prepare(
      'SELECT * FROM moderation_reports WHERE reported_by = ? ORDER BY created_at DESC'
    ).all(reportedBy);
    res.json({ reports });
  } catch (e) { res.status(500).json({ error: e.message }); }
});

/* ─── Moderation: Appeals ──────────────────────────────────── */

/**
 * POST /mosaic/appeal/create — Appeal a label.
 * Body: { label_cid, reason, evidence }
 */
router.post('/appeal/create', (req, res) => {
  try {
    const { label_cid, reason, evidence } = req.body;
    if (!label_cid || !reason) {
      return res.status(400).json({ error: 'Missing required fields: label_cid, reason' });
    }
    const pubkey = req.headers['x-mosaic-pubkey'] || 'anonymous';
    const { createAppeal } = require('./labels');
    const result = createAppeal(label_cid, pubkey, reason, evidence);
    res.status(201).json(result);
  } catch (e) { res.status(500).json({ error: e.message }); }
});

/**
 * GET /mosaic/appeal/list — List appeals by the current user.
 */
router.get('/appeal/list', (req, res) => {
  try {
    const pubkey = req.headers['x-mosaic-pubkey'] || 'anonymous';
    const db = getDb();
    const appeals = db.prepare(
      'SELECT * FROM moderation_appeals WHERE pubkey = ? ORDER BY created_at DESC'
    ).all(pubkey);
    res.json({ appeals });
  } catch (e) { res.status(500).json({ error: e.message }); }
});

module.exports = router;
