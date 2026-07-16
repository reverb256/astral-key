'use strict';

const { describe, it, before, after } = require('node:test');
const assert = require('node:assert');
const path = require('path');
const fs = require('fs');
const os = require('os');

// ─── Identity Module ───────────────────────────────────────────────────────

const identity = require('../src/identity');

describe('Identity Module', () => {
  describe('generateKeyPair()', () => {
    it('should generate a valid Ed25519 key pair', () => {
      const kp = identity.generateKeyPair();

      // Pubkey: 32 bytes → 44 chars Base64URL (no padding if cleanly divisible)
      assert.ok(kp.pubkey.length >= 43, `pubkey length: ${kp.pubkey.length}`);
      assert.ok(kp.pubkey.length <= 44);
      assert.match(kp.pubkey, /^[A-Za-z0-9\-_]+$/);

      // Privkey: 64 bytes → 87 or 88 chars Base64URL
      assert.ok(kp.privkey.length >= 86, `privkey length: ${kp.privkey.length}`);
      assert.ok(kp.privkey.length <= 88);

      // Hex: 64 hex chars
      assert.equal(kp.pubkeyHex.length, 64);
      assert.match(kp.pubkeyHex, /^[0-9a-f]+$/);
    });

    it('should generate unique keys each call', () => {
      const a = identity.generateKeyPair();
      const b = identity.generateKeyPair();
      assert.notEqual(a.pubkey, b.pubkey);
      assert.notEqual(a.privkey, b.privkey);
    });
  });

  describe('derivePublicKey()', () => {
    it('should recover the public key from a private key', () => {
      const kp = identity.generateKeyPair();
      const derived = identity.derivePublicKey(kp.privkey);
      assert.equal(derived.pubkey, kp.pubkey);
      assert.equal(derived.pubkeyHex, kp.pubkeyHex);
    });

    it('should throw on invalid private key length', () => {
      assert.throws(() => identity.derivePublicKey('too-short'), /Invalid private key length/);
    });
  });

  describe('sign() and verify()', () => {
    it('should sign and verify a string message', () => {
      const kp = identity.generateKeyPair();
      const msg = 'Hello, Mosaic!';
      const sig = identity.sign(msg, kp.privkey);

      assert.ok(sig.length >= 86); // 64 bytes → ~88 chars Base64URL
      assert.ok(identity.verify(msg, sig, kp.pubkey));
    });

    it('should reject tampered messages', () => {
      const kp = identity.generateKeyPair();
      const sig = identity.sign('original message', kp.privkey);
      assert.strictEqual(identity.verify('tampered message', sig, kp.pubkey), false);
    });

    it('should reject signature from different key', () => {
      const alice = identity.generateKeyPair();
      const bob = identity.generateKeyPair();
      const sig = identity.sign('hello', alice.privkey);
      assert.strictEqual(identity.verify('hello', sig, bob.pubkey), false);
    });

    it('should sign and verify Buffer messages', () => {
      const kp = identity.generateKeyPair();
      const msg = Buffer.from([0x00, 0x01, 0x02, 0xFF]);
      const sig = identity.sign(msg, kp.privkey);
      assert.ok(identity.verify(msg, sig, kp.pubkey));
    });
  });

  describe('signJSON() and verifyJSON()', () => {
    it('should create a verifiable signed envelope', () => {
      const kp = identity.generateKeyPair();
      const payload = { type: 'post', content: 'hello', ts: Date.now() };

      const envelope = identity.signJSON(payload, kp.privkey, kp.pubkey);
      assert.deepStrictEqual(envelope.data, payload);
      assert.equal(envelope.pubkey, kp.pubkey);
      assert.ok(identity.verifyJSON(envelope));
    });

    it('should reject tampered envelope', () => {
      const kp = identity.generateKeyPair();
      const envelope = identity.signJSON({ a: 1 }, kp.privkey, kp.pubkey);
      envelope.data.a = 2;
      assert.strictEqual(identity.verifyJSON(envelope), false);
    });
  });

  describe('fingerprint() and pubkeyURI()', () => {
    it('should generate consistent fingerprints', () => {
      const kp = identity.generateKeyPair();
      const fp1 = identity.fingerprint(kp.pubkey);
      const fp2 = identity.fingerprint(kp.pubkey);
      assert.equal(fp1, fp2);
      assert.equal(fp1.length, 8);
    });

    it('should generate and parse URIs', () => {
      const kp = identity.generateKeyPair();
      const uri = identity.pubkeyURI(kp.pubkey);
      assert.ok(uri.startsWith('mosaic://'));

      const parsed = identity.parsePubkeyURI(uri);
      assert.ok(parsed);
      assert.equal(parsed.pubkey, kp.pubkey);
    });

    it('should return null for invalid URIs', () => {
      assert.strictEqual(identity.parsePubkeyURI('https://example.com'), null);
      assert.strictEqual(identity.parsePubkeyURI('mosaic://'), null); // empty pubkey
    });
  });
});

// ─── Database Module (requires routes to test) ─────────────────────────────

const database = require('../src/database');

describe('Database Module', () => {
  let dbDir;

  before(() => {
    dbDir = path.join(os.tmpdir(), `mosaic-test-${Date.now()}`);
    fs.mkdirSync(dbDir, { recursive: true });
    process.env.MOSAIC_DATA_DIR = dbDir;
    database.initDatabase();
  });

  after(() => {
    database.close();
    try { fs.rmSync(dbDir, { recursive: true, force: true }); } catch { /* ok */ }
  });

  describe('Identity CRUD', () => {
    it('should create and retrieve an identity', () => {
      const kp = identity.generateKeyPair();
      const result = database.createIdentity({ pubkey: kp.pubkey, privkey: kp.privkey, label: 'test' });

      assert.ok(result.id);
      assert.equal(result.pubkey, kp.pubkey);

      const fetched = database.getIdentity(result.id);
      assert.ok(fetched);
      assert.equal(fetched.pubkey, kp.pubkey);
      assert.equal(fetched.label, 'test');
    });

    it('should get identity by pubkey', () => {
      const kp = identity.generateKeyPair();
      database.createIdentity({ pubkey: kp.pubkey, privkey: kp.privkey });
      const fetched = database.getIdentityByPubkey(kp.pubkey);
      assert.ok(fetched);
      assert.equal(fetched.pubkey, kp.pubkey);
    });

    it('should list all identities', () => {
      const list = database.listIdentities();
      assert.ok(Array.isArray(list));
      assert.ok(list.length >= 2); // we created at least 2
    });

    it('should have the first created identity as current', () => {
      const current = database.getCurrentIdentity();
      assert.ok(current);
      assert.equal(current.is_current, 1);
    });

    it('should switch current identity', () => {
      const list = database.listIdentities();
      const target = list[list.length - 1];
      database.setCurrentIdentity(target.id);

      const current = database.getCurrentIdentity();
      assert.equal(current.id, target.id);
    });
  });

  describe('Passkey CRUD', () => {
    it('should save and retrieve a passkey', () => {
      const ident = database.getCurrentIdentity();
      const cred = { credentialID: 'test-id', credentialPublicKey: 'test-pk', counter: 0 };

      database.savePasskey({
        id: 'test-passkey-1',
        identityId: ident.id,
        credential: cred,
        transports: ['internal', 'usb'],
        nickname: 'My Key',
      });

      const fetched = database.getPasskey('test-passkey-1');
      assert.ok(fetched);
      assert.equal(fetched.identity_id, ident.id);
      assert.deepStrictEqual(fetched.credential, cred);
      assert.deepStrictEqual(fetched.transports, ['internal', 'usb']);
    });

    it('should list passkeys for an identity', () => {
      const ident = database.getCurrentIdentity();
      const keys = database.listPasskeys(ident.id);
      assert.ok(keys.length >= 1);
    });

    it('should update passkey counter', () => {
      database.updatePasskeyCounter('test-passkey-1', 42);
      const fetched = database.getPasskey('test-passkey-1');
      assert.equal(fetched.counter, 42);
    });

    it('should delete a passkey', () => {
      database.deletePasskey('test-passkey-1');
      assert.strictEqual(database.getPasskey('test-passkey-1'), null);
    });
  });

  describe('Contact CRUD', () => {
    it('should add and retrieve contacts', () => {
      const kp = identity.generateKeyPair();
      database.addContact({ pubkey: kp.pubkey, label: 'Alice', discoveredVia: 'qr' });

      const contact = database.getContact(kp.pubkey);
      assert.ok(contact);
      assert.equal(contact.label, 'Alice');
    });

    it('should upsert contacts (update label)', () => {
      const kp = identity.generateKeyPair();
      database.addContact({ pubkey: kp.pubkey, label: 'Original' });
      database.addContact({ pubkey: kp.pubkey, label: 'Updated' });

      const contact = database.getContact(kp.pubkey);
      assert.equal(contact.label, 'Updated');
    });

    it('should list all contacts', () => {
      const list = database.listContacts();
      assert.ok(Array.isArray(list));
    });

    it('should delete a contact', () => {
      const kp = identity.generateKeyPair();
      database.addContact({ pubkey: kp.pubkey });
      database.deleteContact(kp.pubkey);
      assert.strictEqual(database.getContact(kp.pubkey), null);
    });
  });

  describe('Session CRUD', () => {
    it('should create and retrieve valid sessions', () => {
      const ident = database.getCurrentIdentity();
      database.createSession({
        tokenHash: 'test-token-hash',
        identityId: ident.id,
        pubkey: ident.pubkey,
        ttlSeconds: 3600,
      });

      const session = database.getSession('test-token-hash');
      assert.ok(session);
      assert.equal(session.identity_id, ident.id);
    });

    it('should delete sessions', () => {
      database.deleteSession('test-token-hash');
      assert.strictEqual(database.getSession('test-token-hash'), null);
    });
  });
});

// ─── QR Module ─────────────────────────────────────────────────────────────

const qr = require('../src/qr');

describe('QR Module', () => {
  describe('parseQR()', () => {
    it('should parse a mosaic:// URI', () => {
      const kp = identity.generateKeyPair();
      const uri = identity.pubkeyURI(kp.pubkey);
      const parsed = qr.parseQR(uri);
      assert.ok(parsed);
      assert.equal(parsed.pubkey, kp.pubkey);
    });

    it('should parse a raw Base64URL pubkey', () => {
      const kp = identity.generateKeyPair();
      const parsed = qr.parseQR(kp.pubkey);
      assert.ok(parsed);
      assert.equal(parsed.pubkey, kp.pubkey);
    });

    it('should return null for garbage', () => {
      assert.strictEqual(qr.parseQR('not-a-key'), null);
      assert.strictEqual(qr.parseQR(''), null);
      assert.strictEqual(qr.parseQR(null), null);
    });
  });

  describe('generatePubkeyQR()', () => {
    it('should generate an SVG QR code', async () => {
      const kp = identity.generateKeyPair();
      const svg = await qr.generatePubkeyQR_SVG(kp.pubkey);
      assert.ok(svg.startsWith('<svg'));
      assert.ok(svg.includes('</svg>'));
    });

    it('should generate a PNG data URL', async () => {
      const kp = identity.generateKeyPair();
      const dataUrl = await qr.generatePubkeyQR_PNG(kp.pubkey);
      assert.ok(dataUrl.startsWith('data:image/png;base64,'));
    });
  });
});

// (Server smoke tests moved to test/server.test.js)
