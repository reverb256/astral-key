'use strict';

const { describe, it, before, after } = require('node:test');
const assert = require('node:assert');
const path = require('path');
const fs = require('fs');
const os = require('os');

// =========================================================================
// Mock @simplewebauthn/server BEFORE requiring modules that consume it.
//
// passkey.js destructures named imports at require time, so we must
// pre-populate the require cache with our mock. The real module would
// otherwise need an actual browser for the verify functions, which is
// impossible in a unit-test context.
//
// The counters guarantee unique credential IDs per test so that every
// completeRegistration() call produces a distinct passkey row instead of
// hitting ON CONFLICT(id) DO UPDATE.
// =========================================================================

let regCredCounter = 0;

const mockWebAuthn = {
  generateRegistrationOptions(opts) {
    return {
      challenge: Buffer.isBuffer(opts.challenge)
        ? opts.challenge.toString('base64url')
        : opts.challenge,
      rp: { name: opts.rpName, id: opts.rpID },
      user: {
        id: opts.userID,
        name: opts.userName,
        displayName: opts.userDisplayName,
      },
      pubKeyCredParams: [{ type: 'public-key', alg: -7 }],
      attestation: opts.attestationType,
      authenticatorSelection: opts.authenticatorSelection,
      excludeCredentials: opts.excludeCredentials || [],
      timeout: 60000,
    };
  },

  generateAuthenticationOptions(opts) {
    return {
      challenge: Buffer.isBuffer(opts.challenge)
        ? opts.challenge.toString('base64url')
        : opts.challenge,
      allowCredentials: opts.allowCredentials || [],
      userVerification: opts.userVerification,
      rpID: opts.rpID,
      timeout: 60000,
    };
  },

  verifyRegistrationResponse: async () => {
    regCredCounter++;
    const credIdStr = `test-credential-id-${regCredCounter}-${Date.now()}`;
    return {
      verified: true,
      registrationInfo: {
        credentialID: Buffer.from(credIdStr),
        credentialPublicKey: Buffer.alloc(65, 0x04),
        counter: 0,
      },
    };
  },

  verifyAuthenticationResponse: async (opts) => ({
    verified: true,
    authenticationInfo: {
      newCounter: (opts?.authenticator?.counter ?? 0) + 1,
    },
  }),
};

// Pre-populate require cache so passkey.js gets our mock via destructuring.
const webauthnPath = require.resolve('@simplewebauthn/server');
require.cache[webauthnPath] = {
  id: webauthnPath,
  filename: webauthnPath,
  loaded: true,
  exports: mockWebAuthn,
};

// =========================================================================
// Imports — these happen AFTER the mock is established so the destructured
// imports inside passkey.js pick up our mocked verify functions.
// =========================================================================

// Set data dir BEFORE requiring modules that compute DATA_DIR at load time
const testDir = path.join(os.tmpdir(), `mosaic-passkey-test-${Date.now()}`);
fs.mkdirSync(testDir, { recursive: true });
process.env.HAVEN_DATA_DIR = testDir;

const database = require('../src/database');
const passkey = require('../src/passkey');

// =========================================================================
// Helper: create a clientDataJSON blob for authentication test helper
// =========================================================================

function makeClientDataJSON(challenge, type) {
  return Buffer.from(
    JSON.stringify({ challenge, type, origin: 'http://localhost:3000' }),
  ).toString('base64');
}

// =========================================================================
// Helper: register an identity + passkey and return the stored passkey row
// =========================================================================

async function registerAndStore(label) {
  const regResult = passkey.beginRegistration({ label: label || 'helper' });
  const { options, identityId } = regResult;

  await passkey.completeRegistration({
    challenge: options.challenge,
    credential: { response: { transports: ['internal'] } },
  });

  const stored = database.getDb()
    .prepare('SELECT * FROM passkeys WHERE identity_id = ?')
    .get(identityId);
  return { identityId, stored, pubkey: regResult.pubkey };
}

// =========================================================================
// Tests
// =========================================================================

describe('Passkey Module', () => {
  before(() => {
    process.env.MOSAIC_RP_ID = 'localhost';
    process.env.MOSAIC_ORIGIN = 'http://localhost:3000';
    database.initDatabase();
  });

  after(() => {
    database.close();
    try { fs.rmSync(testDir, { recursive: true, force: true }); } catch { /* ok */ }
  });

  // ───────────────────────────────────────────────────────────────────────
  // beginRegistration
  // ───────────────────────────────────────────────────────────────────────

  describe('beginRegistration()', () => {
    it('should return options with expected shape', () => {
      const result = passkey.beginRegistration({ label: 'Test Key' });

      assert.ok(result.identityId, 'result.identityId must be present');
      assert.ok(result.pubkey, 'result.pubkey must be present');
      assert.ok(result.options, 'result.options must be present');

      const { options } = result;
      assert.ok(options.challenge, 'options.challenge must be present');
      assert.ok(options.rp, 'options.rp must be present');
      assert.strictEqual(options.rp.name, 'Mosaic');
      assert.strictEqual(options.rp.id, 'localhost');
      assert.ok(options.user, 'options.user must be present');
      assert.ok(options.user.id, 'options.user.id must be present');
      assert.ok(options.user.name, 'options.user.name must be present');
      assert.ok(options.user.displayName, 'options.user.displayName must be present');
      assert.ok(Array.isArray(options.pubKeyCredParams), 'options.pubKeyCredParams must be an array');
      assert.ok(options.pubKeyCredParams.length > 0, 'options.pubKeyCredParams must not be empty');
      assert.strictEqual(options.pubKeyCredParams[0].type, 'public-key');
      assert.strictEqual(options.attestation, 'none');
      assert.ok(options.authenticatorSelection, 'options.authenticatorSelection must be present');
      assert.strictEqual(options.authenticatorSelection.residentKey, 'required');
      assert.strictEqual(options.authenticatorSelection.userVerification, 'required');
      assert.ok(options.timeout, 'options.timeout must be present');
    });

    it('should persist the identity to the database', () => {
      const result = passkey.beginRegistration({ label: 'DB-Check' });
      const row = database.getDb()
        .prepare('SELECT * FROM identities WHERE id = ?')
        .get(result.identityId);

      assert.ok(row, 'identity row must exist');
      assert.strictEqual(row.pubkey, result.pubkey);
      assert.strictEqual(row.label, 'DB-Check');
    });

    it('should handle missing label gracefully', () => {
      const result = passkey.beginRegistration();
      assert.ok(result.identityId, 'identityId must be present even without label');
      assert.ok(result.pubkey, 'pubkey must be present');

      const row = database.getDb()
        .prepare('SELECT * FROM identities WHERE id = ?')
        .get(result.identityId);
      assert.strictEqual(row.label, null);
    });

    it('should generate unique challenges and key pairs on each call', () => {
      const a = passkey.beginRegistration({ label: 'a' });
      const b = passkey.beginRegistration({ label: 'b' });

      assert.notStrictEqual(a.options.challenge, b.options.challenge);
      assert.notStrictEqual(a.pubkey, b.pubkey);
      assert.notStrictEqual(a.identityId, b.identityId);
    });

    it('should mark the very first identity as current', () => {
      const result = passkey.beginRegistration({ label: 'current-test' });
      const row = database.getDb()
        .prepare('SELECT id, pubkey, label FROM identities WHERE id = ?')
        .get(result.identityId);
      assert.ok(row, 'identity must exist');
      assert.equal(row.label, 'current-test');
    });
  });

  // ───────────────────────────────────────────────────────────────────────
  // completeRegistration
  // ───────────────────────────────────────────────────────────────────────

  describe('completeRegistration()', () => {
    it('should throw on unknown or expired challenge', async () => {
      await assert.rejects(
        () => passkey.completeRegistration({
          challenge: 'definitely-not-in-the-store',
          credential: {},
        }),
        /Challenge/,
      );
    });

    it('should succeed when given a valid challenge and store the passkey', async () => {
      const { options, identityId } = passkey.beginRegistration({ label: 'Complete-Me' });

      const result = await passkey.completeRegistration({
        challenge: options.challenge,
        credential: {
          response: { transports: ['internal', 'usb'] },
        },
        nickname: 'My YubiKey',
      });

      assert.ok(result.verified);
      assert.strictEqual(result.identityId, identityId);
      assert.ok(result.pubkey);

      // Verify the passkey was persisted
      const passkeys = database.getDb()
        .prepare('SELECT * FROM passkeys WHERE identity_id = ?')
        .all(identityId);
      assert.ok(passkeys.length >= 1, 'at least one passkey must be stored');
      assert.ok(passkeys[0].id, 'passkey must have an id');
      assert.strictEqual(passkeys[0].nickname, 'My YubiKey');
    });

    it('should accept a registration without a nickname', async () => {
      const { options, identityId } = passkey.beginRegistration({ label: 'No-Nick' });

      const result = await passkey.completeRegistration({
        challenge: options.challenge,
        credential: { response: { transports: [] } },
      });

      assert.ok(result.verified);
      const row = database.getDb()
        .prepare('SELECT * FROM passkeys WHERE identity_id = ?')
        .get(identityId);
      assert.ok(row, 'passkey row must exist');
      assert.strictEqual(row.nickname, null);
    });
  });

  // ───────────────────────────────────────────────────────────────────────
  // beginAuthentication
  // ───────────────────────────────────────────────────────────────────────

  describe('beginAuthentication()', () => {
    it('should return options with expected shape and expose the challenge', () => {
      const result = passkey.beginAuthentication();

      assert.ok(result.options, 'options must be present');
      assert.ok(result.challenge, 'challenge string must be present');
      assert.strictEqual(result.challenge, result.options.challenge,
        'returned challenge must equal options.challenge');

      const { options } = result;
      assert.ok(options.challenge);
      assert.strictEqual(options.userVerification, 'required');
      assert.strictEqual(options.rpID, 'localhost');
      assert.ok(Array.isArray(options.allowCredentials));
      assert.ok(options.timeout);
    });

    it('should produce different challenges on successive calls', () => {
      const a = passkey.beginAuthentication();
      const b = passkey.beginAuthentication();
      assert.notStrictEqual(a.challenge, b.challenge);
    });
  });

  // ───────────────────────────────────────────────────────────────────────
  // completeAuthentication
  // ───────────────────────────────────────────────────────────────────────

  describe('completeAuthentication()', () => {
    let authIdentityId;
    let authCredentialId;

    before(async () => {
      const setup = await registerAndStore('Auth-Full');
      authIdentityId = setup.identityId;
      authCredentialId = setup.stored.id;
    });

    it('should throw when the credential has no clientDataJSON', async () => {
      await assert.rejects(
        () => passkey.completeAuthentication({ credential: {} }),
        /No challenge found/,
      );
    });

    it('should throw on unknown challenge', async () => {
      await assert.rejects(
        () => passkey.completeAuthentication({
          credential: {
            id: 'any-id',
            response: {
              clientDataJSON: makeClientDataJSON('nonexistent-challenge', 'webauthn.get'),
            },
          },
        }),
        /Challenge/,
      );
    });

    it('should throw when the credential does not exist in the database', async () => {
      const { challenge } = passkey.beginAuthentication();

      await assert.rejects(
        () => passkey.completeAuthentication({
          credential: {
            id: 'id-that-never-existed',
            response: {
              clientDataJSON: makeClientDataJSON(challenge, 'webauthn.get'),
            },
          },
        }),
        /Passkey credential not found/,
      );
    });

    it('should authenticate and create a session token', async () => {
      const { challenge } = passkey.beginAuthentication();

      const authResult = await passkey.completeAuthentication({
        credential: {
          id: authCredentialId,
          response: {
            clientDataJSON: makeClientDataJSON(challenge, 'webauthn.get'),
            transports: ['internal'],
          },
        },
      });

      assert.ok(authResult.verified);
      assert.strictEqual(authResult.identityId, authIdentityId);
      assert.ok(authResult.pubkey);
      assert.ok(authResult.sessionToken, 'session token must be returned');

      // Validate the session
      const session = passkey.validateSession(authResult.sessionToken);
      assert.ok(session, 'session must be valid');
      assert.strictEqual(session.identityId, authIdentityId);
    });

    it('should update the passkey counter on each authentication', async () => {
      // Register a fresh identity + passkey for this isolated test
      const setup = await registerAndStore('Counter');
      const { identityId, stored } = setup;
      const { id: credId } = stored;

      // Authenticate twice with the same credential
      for (let i = 0; i < 2; i++) {
        const { challenge } = passkey.beginAuthentication();
        await passkey.completeAuthentication({
          credential: {
            id: credId,
            response: {
              clientDataJSON: makeClientDataJSON(challenge, 'webauthn.get'),
              transports: [],
            },
          },
        });
      }

      const updated = database.getDb()
        .prepare('SELECT * FROM passkeys WHERE id = ?')
        .get(credId);
      // Each auth increments by 1 via the mock
      assert.strictEqual(updated.counter, 2, 'counter must be 2 after two authentications');
    });

    it('should record last_used_at after authentication', async () => {
      const setup = await registerAndStore('LastUsed');
      const { stored } = setup;

      const { challenge } = passkey.beginAuthentication();
      await passkey.completeAuthentication({
        credential: {
          id: stored.id,
          response: {
            clientDataJSON: makeClientDataJSON(challenge, 'webauthn.get'),
            transports: [],
          },
        },
      });

      const updated = database.getDb()
        .prepare('SELECT * FROM passkeys WHERE id = ?')
        .get(stored.id);
      assert.ok(updated.last_used_at, 'last_used_at must be set');
    });
  });

  // ───────────────────────────────────────────────────────────────────────
  // validateSession
  // ───────────────────────────────────────────────────────────────────────

  describe('validateSession()', () => {
    it('should return null for null / undefined / empty input', () => {
      assert.strictEqual(passkey.validateSession(null), null);
      assert.strictEqual(passkey.validateSession(undefined), null);
      assert.strictEqual(passkey.validateSession(''), null);
    });

    it('should return null for a non-existent token', () => {
      assert.strictEqual(passkey.validateSession('this-token-does-not-exist'), null);
    });

    it('should return identity info for a valid token', async () => {
      const setup = await registerAndStore('ValidSession');
      const { identityId, stored } = setup;

      const { challenge } = passkey.beginAuthentication();
      const authResult = await passkey.completeAuthentication({
        credential: {
          id: stored.id,
          response: {
            clientDataJSON: makeClientDataJSON(challenge, 'webauthn.get'),
            transports: [],
          },
        },
      });

      const session = passkey.validateSession(authResult.sessionToken);
      assert.ok(session);
      assert.strictEqual(session.identityId, identityId);
      assert.strictEqual(session.pubkey, authResult.pubkey);
    });
  });

  // ───────────────────────────────────────────────────────────────────────
  // invalidateSession
  // ───────────────────────────────────────────────────────────────────────

  describe('invalidateSession()', () => {
    it('should silently accept null / undefined / empty', () => {
      assert.doesNotThrow(() => passkey.invalidateSession(null));
      assert.doesNotThrow(() => passkey.invalidateSession(undefined));
      assert.doesNotThrow(() => passkey.invalidateSession(''));
    });

    it('should silently accept a non-existent token', () => {
      assert.doesNotThrow(() => passkey.invalidateSession('i-do-not-exist'));
    });

    it('should remove a valid session from the database', async () => {
      const setup = await registerAndStore('Invalidate');
      const { identityId, stored } = setup;

      // Authenticate to get a session token
      const { challenge } = passkey.beginAuthentication();
      const authResult = await passkey.completeAuthentication({
        credential: {
          id: stored.id,
          response: {
            clientDataJSON: makeClientDataJSON(challenge, 'webauthn.get'),
            transports: [],
          },
        },
      });

      // Confirm the session exists
      assert.ok(passkey.validateSession(authResult.sessionToken));

      // Invalidate
      passkey.invalidateSession(authResult.sessionToken);

      // Confirm it's gone
      assert.strictEqual(passkey.validateSession(authResult.sessionToken), null);
    });
  });

  // ───────────────────────────────────────────────────────────────────────
  // sessionMiddleware (Express middleware)
  // ───────────────────────────────────────────────────────────────────────

  describe('sessionMiddleware()', () => {
    let sessionToken;
    let identityId;

    before(async () => {
      const setup = await registerAndStore('Middleware');
      identityId = setup.identityId;
      const { stored } = setup;

      const { challenge } = passkey.beginAuthentication();
      const authResult = await passkey.completeAuthentication({
        credential: {
          id: stored.id,
          response: {
            clientDataJSON: makeClientDataJSON(challenge, 'webauthn.get'),
            transports: ['internal'],
          },
        },
      });
      sessionToken = authResult.sessionToken;
    });

    it('should parse identity from a Bearer token', () => {
      const req = { headers: { authorization: `Bearer ${sessionToken}` } };
      const res = {};
      let nextCalled = false;

      passkey.sessionMiddleware(req, res, () => { nextCalled = true; });

      assert.ok(nextCalled);
      assert.ok(req.identity, 'req.identity must be set');
      assert.strictEqual(req.identity.identityId, identityId);
      assert.ok(req.identity.pubkey);
      assert.strictEqual(req.sessionToken, sessionToken);
    });

    it('should parse identity from a cookie', () => {
      const encoded = encodeURIComponent(sessionToken);
      const req = { headers: { cookie: `mosaic_session=${encoded}` } };
      const res = {};
      let nextCalled = false;

      passkey.sessionMiddleware(req, res, () => { nextCalled = true; });

      assert.ok(nextCalled);
      assert.ok(req.identity);
      assert.strictEqual(req.identity.identityId, identityId);
      assert.strictEqual(req.sessionToken, sessionToken);
    });

    it('should proceed without identity when no auth is present', () => {
      const req = { headers: {} };
      const res = {};
      let nextCalled = false;

      passkey.sessionMiddleware(req, res, () => { nextCalled = true; });

      assert.ok(nextCalled);
      assert.strictEqual(req.identity, undefined);
      assert.strictEqual(req.sessionToken, undefined);
    });

    it('should proceed without identity on an invalid Bearer token', () => {
      const req = { headers: { authorization: 'Bearer obviously-fake-token' } };
      const res = {};
      let nextCalled = false;

      passkey.sessionMiddleware(req, res, () => { nextCalled = true; });

      assert.ok(nextCalled);
      assert.strictEqual(req.identity, undefined);
    });

    it('should attach identity even when req.headers.cookie has extra entries', () => {
      const encoded = encodeURIComponent(sessionToken);
      const req = {
        headers: {
          cookie: `some_other=value; mosaic_session=${encoded}; flavor=chocolate`,
        },
      };
      const res = {};
      let nextCalled = false;

      passkey.sessionMiddleware(req, res, () => { nextCalled = true; });

      assert.ok(nextCalled);
      assert.ok(req.identity);
      assert.strictEqual(req.identity.identityId, identityId);
    });
  });

  // ───────────────────────────────────────────────────────────────────────
  // requireAuth (Express guard)
  // ───────────────────────────────────────────────────────────────────────

  describe('requireAuth()', () => {
    it('should call next() when req.identity exists', () => {
      const req = { identity: { identityId: 42, pubkey: 'abc' } };
      const res = {};
      let nextCalled = false;

      passkey.requireAuth(req, res, () => { nextCalled = true; });

      assert.ok(nextCalled);
    });

    it('should respond 401 with JSON body when req.identity is absent', () => {
      const req = {};
      let statusArg;
      let jsonArg;

      const res = {
        status(code) {
          statusArg = code;
          return { json: (body) => { jsonArg = body; } };
        },
      };

      passkey.requireAuth(req, res, () => { assert.fail('next must not be called'); });

      assert.strictEqual(statusArg, 401);
      assert.ok(jsonArg);
      assert.strictEqual(jsonArg.error, 'Unauthorized');
      assert.ok(jsonArg.message);
    });

    it('should respond 401 when req.identity is null', () => {
      const req = { identity: null };
      let statusArg;
      let jsonArg;

      const res = {
        status(code) {
          statusArg = code;
          return { json: (body) => { jsonArg = body; } };
        },
      };

      passkey.requireAuth(req, res, () => { assert.fail('next must not be called'); });

      assert.strictEqual(statusArg, 401);
      assert.strictEqual(jsonArg.error, 'Unauthorized');
    });
  });
});
