'use strict';

/**
 * Passkey Module — WebAuthn (Passkey) Registration & Authentication.
 *
 * Binds WebAuthn credentials to Ed25519 identities.
 * Coexists alongside Haven's bcrypt+JWT auth.
 */

const {
  generateRegistrationOptions,
  verifyRegistrationResponse,
  generateAuthenticationOptions,
  verifyAuthenticationResponse,
} = require('@simplewebauthn/server');
const crypto = require('crypto');
const { getIdentityDb } = require('./sqlite-adapter');

const RP_NAME = 'Mosiac';
const RP_ID = process.env.MOSIAC_RP_ID || 'localhost';
const ORIGIN = (process.env.MOSIAC_ORIGIN || 'http://localhost:3000').replace(/\/+$/, '');

// In-memory challenge store (per session)
const challengeStore = new Map();
const registrationStore = new Map();

/* ─── Registration ─── */

function beginRegistration({ label } = {}) {
  const nacl = require('tweetnacl');
  const kp = nacl.sign.keyPair();
  const pubkey = Buffer.from(kp.publicKey).toString('base64url');
  const privkey = Buffer.from(kp.secretKey).toString('base64url');

  const ident = getIdentityDb().prepare(`
    INSERT INTO identities (pubkey, privkey, label, is_current)
    VALUES (?, ?, ?, (SELECT COUNT(*) = 0 FROM identities))
  `).run(pubkey, privkey, label || null);

  const challenge = crypto.randomBytes(32);
  const userId = crypto.createHash('sha256').update(pubkey).digest();

  const options = generateRegistrationOptions({
    rpName: RP_NAME,
    rpID: RP_ID,
    userName: pubkey.slice(0, 16) + '\u2026',
    userDisplayName: label || `Mosiac ${pubkey.slice(0, 8)}`,
    challenge,
    userID: userId,
    attestationType: 'none',
    excludeCredentials: [],
    authenticatorSelection: { residentKey: 'required', userVerification: 'required' },
  });

  challengeStore.set(options.challenge, { identityId: ident.lastInsertRowid, pubkey, expiresAt: Date.now() + 120000 });
  return { identityId: ident.lastInsertRowid, pubkey, options };
}

async function completeRegistration({ challenge, credential, nickname }) {
  const state = challengeStore.get(challenge);
  if (!state || state.expiresAt < Date.now()) throw new Error('Challenge expired. Start over.');

  const verification = await verifyRegistrationResponse({
    response: credential, expectedChallenge: challenge,
    expectedOrigin: ORIGIN, expectedRPID: RP_ID,
  });
  if (!verification.verified || !verification.registrationInfo) throw new Error('Verification failed');

  const { credentialID, credentialPublicKey, counter } = verification.registrationInfo;
  const credId = Buffer.from(credentialID).toString('base64url');
  const credJson = JSON.stringify({
    credentialID: credId,
    credentialPublicKey: Buffer.from(credentialPublicKey).toString('base64url'),
    counter,
  });

  getIdentityDb().prepare(`
    INSERT INTO passkeys (id, identity_id, credential, transports, nickname)
    VALUES (?, ?, ?, ?, ?)
    ON CONFLICT(id) DO UPDATE SET credential=excluded.credential, counter=0
  `).run(credId, state.identityId, credJson, JSON.stringify(credential.response?.transports || []), nickname || null);

  challengeStore.delete(challenge);
  return { verified: true, identityId: state.identityId, pubkey: state.pubkey };
}

/* ─── Authentication ─── */

function beginAuthentication() {
  const challenge = crypto.randomBytes(32);
  const options = generateAuthenticationOptions({
    rpID: RP_ID, challenge, allowCredentials: [], userVerification: 'required',
  });
  challengeStore.set(options.challenge, { type: 'auth', expiresAt: Date.now() + 120000 });
  return { options, challenge: options.challenge };
}

async function completeAuthentication({ credential }) {
  const challenge = credential.response?.clientDataJSON
    ? JSON.parse(Buffer.from(credential.response.clientDataJSON, 'base64').toString('utf8')).challenge
    : null;
  if (!challenge) throw new Error('No challenge found');

  const state = challengeStore.get(challenge);
  if (!state || state.expiresAt < Date.now()) throw new Error('Challenge expired');

  const credId = typeof credential.id === 'string' ? credential.id : Buffer.from(credential.id).toString('base64url');
  const row = getIdentityDb().prepare('SELECT * FROM passkeys WHERE id = ?').get(credId);
  if (!row) throw new Error('Passkey credential not found');

  const storedCred = JSON.parse(row.credential);
  const authenticator = {
    credentialID: credId,
    credentialPublicKey: storedCred.credentialPublicKey,
    counter: row.counter,
    transports: row.transports ? JSON.parse(row.transports) : ['internal'],
  };

  const verification = await verifyAuthenticationResponse({
    response: credential, expectedChallenge: challenge,
    expectedOrigin: ORIGIN, expectedRPID: RP_ID, authenticator,
  });
  if (!verification.verified) throw new Error('Authentication verification failed');

  getIdentityDb().prepare('UPDATE passkeys SET counter = ?, last_used_at = datetime(\'now\') WHERE id = ?').run(verification.authenticationInfo.newCounter, credId);
  const ident = getIdentityDb().prepare('SELECT * FROM identities WHERE id = ?').get(row.identity_id);
  if (!ident) throw new Error('Identity not found');

  const sessionToken = crypto.randomBytes(48).toString('base64url');
  const tokenHash = crypto.createHash('sha256').update(sessionToken).digest('hex');
  getIdentityDb().prepare(`
    INSERT INTO sessions (token_hash, identity_id, pubkey, expires_at)
    VALUES (?, ?, ?, datetime('now', '+7 days'))
  `).run(tokenHash, ident.id, ident.pubkey);

  challengeStore.delete(challenge);
  return { verified: true, identityId: ident.id, pubkey: ident.pubkey, sessionToken };
}

/* ─── Session helpers ─── */

function validateSession(token) {
  if (!token) return null;
  const hash = crypto.createHash('sha256').update(token).digest('hex');
  const row = getIdentityDb().prepare("SELECT * FROM sessions WHERE token_hash = ? AND expires_at > datetime('now')").get(hash);
  return row ? { identityId: row.identity_id, pubkey: row.pubkey } : null;
}

function invalidateSession(token) {
  if (!token) return;
  const hash = crypto.createHash('sha256').update(token).digest('hex');
  getIdentityDb().prepare('DELETE FROM sessions WHERE token_hash = ?').run(hash);
}

/* ─── Express middleware ─── */

function sessionMiddleware(req, res, next) {
  const header = req.headers.authorization;
  const cookie = req.headers.cookie
    ? Object.fromEntries(req.headers.cookie.split(';').map(c => c.trim().split('=').map(decodeURIComponent)))
    : {};
  const token = header?.startsWith('Bearer ') ? header.slice(7) : cookie?.mosiac_session;
  if (token) {
    const session = validateSession(token);
    if (session) { req.identity = session; req.sessionToken = token; }
  }
  next();
}

function requireAuth(req, res, next) {
  if (!req.identity) return res.status(401).json({ error: 'Unauthorized', message: 'Valid session required' });
  next();
}

module.exports = {
  beginRegistration, completeRegistration,
  beginAuthentication, completeAuthentication,
  validateSession, invalidateSession,
  sessionMiddleware, requireAuth,
};
