'use strict';

/**
 * Mosiac Identity Routes — mounted alongside Haven's existing routes.
 * All identity/auth/QR/contact/signing endpoints in one place.
 */
const express = require('express');
const router = express.Router();
const path = require('path');

const identity = require('./identity');
const qr = require('./qr');
const passkey = require('./passkey');
const { getIdentityDb } = require('./sqlite-adapter');
const { t, bilingualError } = require('../i18n');

/* ─── Health ─── */
router.get('/health', (req, res) => res.json({ ok: true, mosiac: '0.1.0' }));

/* ─── Identity ─── */
router.get('/identity', (req, res) => {
  try {
    const rows = getIdentityDb().prepare('SELECT id, pubkey, label, is_current, created_at FROM identities ORDER BY created_at DESC').all();
    res.json(rows);
  } catch (e) { res.status(500).json(bilingualError('auth.server_error', 'AUTH_500')); }
});

router.get('/identity/current', (req, res) => {
  try {
    const row = getIdentityDb().prepare('SELECT * FROM identities WHERE is_current = 1').get();
    if (!row) return res.json({ identity: null });
    res.json({ identity: { id: row.id, pubkey: row.pubkey, label: row.label, pubkeyHex: identity.toHex(identity.fromBase64URL(row.pubkey)) } });
  } catch (e) { res.status(500).json(bilingualError('auth.server_error', 'AUTH_500')); }
});

router.post('/identity/generate', (req, res) => {
  try {
    const kp = identity.generateKeyPair();
    const ident = getIdentityDb().prepare(`
      INSERT INTO identities (pubkey, privkey, label, is_current)
      VALUES (?, ?, ?, (SELECT COUNT(*) = 0 FROM identities))
    `).run(kp.pubkey, kp.privkey, req.body?.label || null);
    res.json({ identityId: ident.lastInsertRowid, pubkey: kp.pubkey, pubkeyHex: kp.pubkeyHex });
  } catch (e) { res.status(500).json(bilingualError('auth.server_error', 'AUTH_500')); }
});

/* ─── WebAuthn Registration ─── */
router.post('/auth/register/begin', async (req, res) => {
  try {
    const result = passkey.beginRegistration({ label: req.body?.label });
    res.json(result);
  } catch (e) { res.status(400).json(bilingualError('auth.registration_failed', 'AUTH_001')); }
});

router.post('/auth/register/complete', async (req, res) => {
  try {
    const result = await passkey.completeRegistration({
      challenge: req.body.challenge,
      credential: req.body.credential,
      nickname: req.body.nickname,
    });
    res.json(result);
  } catch (e) { res.status(400).json(bilingualError('auth.passkey_failed', 'AUTH_002')); }
});

/* ─── WebAuthn Authentication ─── */
router.post('/auth/login/begin', async (req, res) => {
  try {
    const result = passkey.beginAuthentication();
    res.json(result);
  } catch (e) { res.status(400).json(bilingualError('auth.login_failed', 'AUTH_003')); }
});

router.post('/auth/login/complete', async (req, res) => {
  try {
    const result = await passkey.completeAuthentication({ credential: req.body.credential });
    res.json(result);
  } catch (e) { res.status(400).json(bilingualError('auth.auth_failed', 'AUTH_004')); }
});

router.post('/auth/logout', (req, res) => {
  const token = req.headers.authorization?.startsWith('Bearer ') ? req.headers.authorization.slice(7) : null;
  if (token) passkey.invalidateSession(token);
  res.json({ ok: true });
});

router.get('/auth/me', passkey.requireAuth, (req, res) => {
  const ident = getIdentityDb().prepare('SELECT * FROM identities WHERE id = ?').get(req.identity.identityId);
  if (!ident) return res.status(404).json(bilingualError('auth.identity_not_found', 'AUTH_005'));
  res.json({ identity: { identityId: ident.id, pubkey: ident.pubkey, label: ident.label } });
});

/* ─── QR ─── */
router.get('/qr/:pubkey', async (req, res) => {
  try {
    const svg = await qr.generatePubkeyQR_SVG(req.params.pubkey);
    res.type('image/svg+xml').send(svg);
  } catch (e) { res.status(400).json(bilingualError('auth.qr_generation_failed', 'AUTH_006')); }
});

router.post('/qr/scan', (req, res) => {
  try {
    const result = qr.processQRScan(req.body.scanned, req.body.label);
    res.json(result);
  } catch (e) { res.status(400).json(bilingualError('auth.scan_failed', 'AUTH_007')); }
});

/* ─── Contacts ─── */
router.get('/contacts', (req, res) => {
  try {
    const rows = getIdentityDb().prepare('SELECT * FROM contacts ORDER BY first_seen_at DESC').all();
    res.json(rows);
  } catch (e) { res.status(500).json(bilingualError('auth.server_error', 'AUTH_500')); }
});

router.delete('/contacts/:pubkey', (req, res) => {
  try {
    getIdentityDb().prepare('DELETE FROM contacts WHERE pubkey = ?').run(req.params.pubkey);
    res.json({ ok: true });
  } catch (e) { res.status(500).json(bilingualError('auth.contact_add_failed', 'AUTH_008')); }
});

/* ─── Signing (event bus foundation) ─── */
router.post('/sign', passkey.requireAuth, (req, res) => {
  try {
    const ident = getIdentityDb().prepare('SELECT * FROM identities WHERE id = ?').get(req.identity.identityId);
    if (!ident) return res.status(404).json(bilingualError('auth.identity_not_found', 'AUTH_005'));
    const signed = identity.signJSON(req.body.data, ident.privkey, ident.pubkey);
    res.json(signed);
  } catch (e) { res.status(400).json(bilingualError('auth.signing_failed', 'AUTH_009')); }
});

router.post('/verify', (req, res) => {
  try {
    const valid = identity.verifyJSON(req.body);
    res.json({ valid });
  } catch (e) { res.status(400).json(bilingualError('auth.verification_failed', 'AUTH_010')); }
});

module.exports = router;
