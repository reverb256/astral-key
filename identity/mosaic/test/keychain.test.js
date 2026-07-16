'use strict';

const { describe, it } = require('node:test');
const assert = require('node:assert');

const keychain = require('../src/keychain');

// ─── Helper: detect valid Base64URL ────────────────────────────────────────
const B64URL_RE = /^[A-Za-z0-9\-_]+$/;

describe('Keychain Module', () => {

  // ─── BIP39 Mnemonic Generation ─────────────────────────────────────────

  describe('generateMnemonic()', () => {
    it('should generate a 12-word phrase with strength=128 (default)', () => {
      const mnemonic = keychain.generateMnemonic();
      const words = mnemonic.split(/\s+/);
      assert.equal(words.length, 12, 'should be 12 words');
      assert.equal(words.filter(w => w.length > 0).length, 12);
    });

    it('should generate a 24-word phrase with strength=256', () => {
      const mnemonic = keychain.generateMnemonic(256);
      const words = mnemonic.split(/\s+/);
      assert.equal(words.length, 24, 'should be 24 words');
    });

    it('should only contain words from the BIP39 English wordlist', () => {
      // We validate this via validateMnemonic since that checks the wordlist
      const mnemonic12 = keychain.generateMnemonic(128);
      assert.ok(keychain.validateMnemonic(mnemonic12));

      const mnemonic24 = keychain.generateMnemonic(256);
      assert.ok(keychain.validateMnemonic(mnemonic24));
    });

    it('should generate unique phrases on successive calls', () => {
      const a = keychain.generateMnemonic();
      const b = keychain.generateMnemonic();
      const c = keychain.generateMnemonic();
      assert.notEqual(a, b);
      assert.notEqual(b, c);
      assert.notEqual(a, c);
    });

    it('should throw on invalid strength values', () => {
      assert.throws(() => keychain.generateMnemonic(64), /Invalid/);
      assert.throws(() => keychain.generateMnemonic(512), /Invalid/);
      assert.throws(() => keychain.generateMnemonic(0), /Invalid/);
      assert.throws(() => keychain.generateMnemonic(129), /Invalid/);
    });

    it('should produce valid mnemonics that pass checksum', () => {
      for (let i = 0; i < 20; i++) {
        const m12 = keychain.generateMnemonic(128);
        assert.ok(keychain.validateMnemonic(m12),
          `12-word mnemonic should be valid: "${m12.slice(0, 20)}..."`);

        const m24 = keychain.generateMnemonic(256);
        assert.ok(keychain.validateMnemonic(m24),
          `24-word mnemonic should be valid: "${m24.slice(0, 20)}..."`);
      }
    });
  });

  // ─── Key Derivation ────────────────────────────────────────────────────

  describe('mnemonicToKeypair()', () => {
    it('should produce keys matching identity.js format', () => {
      const mnemonic = keychain.generateMnemonic();
      const kp = keychain.mnemonicToKeypair(mnemonic);

      // pubkey: 32 bytes → 43-44 chars Base64URL
      assert.ok(kp.pubkey.length >= 43, `pubkey length: ${kp.pubkey.length}`);
      assert.ok(kp.pubkey.length <= 44);
      assert.match(kp.pubkey, B64URL_RE);

      // privkey: 64 bytes → 86-88 chars Base64URL
      assert.ok(kp.privkey.length >= 86, `privkey length: ${kp.privkey.length}`);
      assert.ok(kp.privkey.length <= 88);
      assert.match(kp.privkey, B64URL_RE);

      // pubkeyHex: 64 hex chars
      assert.equal(kp.pubkeyHex.length, 64);
      assert.match(kp.pubkeyHex, /^[0-9a-f]+$/);
    });

    it('should be deterministic — same phrase + same passphrase → same keys', () => {
      const phrase = 'abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about';
      const kp1 = keychain.mnemonicToKeypair(phrase);
      const kp2 = keychain.mnemonicToKeypair(phrase);
      assert.equal(kp1.pubkey, kp2.pubkey);
      assert.equal(kp1.privkey, kp2.privkey);
      assert.equal(kp1.pubkeyHex, kp2.pubkeyHex);
    });

    it('should be deterministic — same phrase + different passphrase → different keys', () => {
      const phrase = 'abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about';
      const kp1 = keychain.mnemonicToKeypair(phrase, '');
      const kp2 = keychain.mnemonicToKeypair(phrase, 'hunter2');
      assert.notEqual(kp1.pubkey, kp2.pubkey);
      assert.notEqual(kp1.privkey, kp2.privkey);
    });

    it('should produce keys compatible with identity.js derivePublicKey', () => {
      const identity = require('../src/identity');
      const mnemonic = keychain.generateMnemonic();
      const kp = keychain.mnemonicToKeypair(mnemonic);

      // Should be able to derive the pubkey from the privkey
      const derived = identity.derivePublicKey(kp.privkey);
      assert.equal(derived.pubkey, kp.pubkey);
      assert.equal(derived.pubkeyHex, kp.pubkeyHex);
    });

    it('should produce keys capable of signing and verifying', () => {
      const identity = require('../src/identity');
      const mnemonic = keychain.generateMnemonic();
      const kp = keychain.mnemonicToKeypair(mnemonic);

      const msg = 'Mosaic keychain test message';
      const sig = identity.sign(msg, kp.privkey);
      assert.ok(identity.verify(msg, sig, kp.pubkey));

      // Tampered message should fail
      assert.strictEqual(identity.verify('wrong', sig, kp.pubkey), false);
    });

    it('should produce different keys for different mnemonics', () => {
      const a = keychain.mnemonicToKeypair(keychain.generateMnemonic());
      const b = keychain.mnemonicToKeypair(keychain.generateMnemonic());
      assert.notEqual(a.pubkey, b.pubkey);
      assert.notEqual(a.privkey, b.privkey);
    });

    it('should work with 24-word mnemonics', () => {
      const mnemonic = keychain.generateMnemonic(256);
      const kp = keychain.mnemonicToKeypair(mnemonic);
      assert.ok(kp.pubkey.length >= 43);
      assert.ok(kp.privkey.length >= 86);
    });

    it('should produce deterministic keys from known test vector', () => {
      // Known BIP39 test vector (empty passphrase)
      const phrase = 'legal winner thank year wave sausage worth useful legal winner thank yellow';
      const kp = keychain.mnemonicToKeypair(phrase);
      // Just verify determinism — the exact key values depend on the
      // ed25519 implementation, but they must be repeatable
      const kp2 = keychain.mnemonicToKeypair(phrase);
      assert.deepStrictEqual(kp, kp2);
    });
  });

  // ─── Mnemonic Validation ───────────────────────────────────────────────

  describe('validateMnemonic()', () => {
    it('should return true for a valid generated mnemonic', () => {
      const mnemonic = keychain.generateMnemonic();
      assert.strictEqual(keychain.validateMnemonic(mnemonic), true);
    });

    it('should return false for an invalid mnemonic (bad word)', () => {
      const bad = 'apple banana cherry notarealword grape honey';
      assert.strictEqual(keychain.validateMnemonic(bad), false);
    });

    it('should return false for a mnemonic with wrong checksum', () => {
      // Valid 12-word phrase with last word changed
      const valid = keychain.generateMnemonic();
      const words = valid.split(' ');
      words[words.length - 1] = 'abandon';
      const tampered = words.join(' ');
      assert.strictEqual(keychain.validateMnemonic(tampered), false);
    });

    it('should return false for garbage input', () => {
      assert.strictEqual(keychain.validateMnemonic(''), false);
      assert.strictEqual(keychain.validateMnemonic('   '), false);
      assert.strictEqual(keychain.validateMnemonic(null), false);
      assert.strictEqual(keychain.validateMnemonic(undefined), false);
      assert.strictEqual(keychain.validateMnemonic(12345), false);
      assert.strictEqual(keychain.validateMnemonic(''), false);
    });

    it('should return false for wrong word count', () => {
      assert.strictEqual(keychain.validateMnemonic('abandon'), false);
      assert.strictEqual(keychain.validateMnemonic(
        'abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon',
      ), false);
    });
  });

  describe('validateMnemonicDetailed()', () => {
    it('should return { valid: true } for a valid mnemonic', () => {
      const mnemonic = keychain.generateMnemonic();
      const result = keychain.validateMnemonicDetailed(mnemonic);
      assert.deepStrictEqual(result, { valid: true });
    });

    it('should return error details for invalid mnemonics', () => {
      const result = keychain.validateMnemonicDetailed('not even close');
      assert.strictEqual(result.valid, false);
      assert.ok(result.error);
    });

    it('should detect wrong word count', () => {
      const result = keychain.validateMnemonicDetailed('abandon');
      assert.strictEqual(result.valid, false);
      assert.ok(result.error.includes('12 or 24'));
    });

    it('should detect words not in BIP39 wordlist', () => {
      const result = keychain.validateMnemonicDetailed(
        'notreal notreal notreal notreal notreal notreal notreal notreal notreal notreal notreal notreal'
      );
      assert.strictEqual(result.valid, false);
      assert.ok(result.error.includes('not in BIP39'));
    });
  });

  // ─── Encryption at Rest ────────────────────────────────────────────────

  describe('encryptPrivkey() and decryptPrivkey()', () => {
    it('should round-trip a private key with the same passphrase', () => {
      const mnemonic = keychain.generateMnemonic();
      const kp = keychain.mnemonicToKeypair(mnemonic);
      const passphrase = 'test-passphrase-123';

      const enc = keychain.encryptPrivkey(kp.privkey, passphrase);
      assert.ok(enc.encrypted);
      assert.ok(enc.iv);
      assert.ok(enc.tag);
      assert.match(enc.encrypted, B64URL_RE);
      assert.match(enc.iv, B64URL_RE);
      assert.match(enc.tag, B64URL_RE);

      const decrypted = keychain.decryptPrivkey(enc.encrypted, enc.iv, enc.tag, passphrase);
      assert.equal(decrypted, kp.privkey);
    });

    it('should reject wrong passphrase', () => {
      const mnemonic = keychain.generateMnemonic();
      const kp = keychain.mnemonicToKeypair(mnemonic);

      const enc = keychain.encryptPrivkey(kp.privkey, 'correct-passphrase');
      assert.throws(() => {
        keychain.decryptPrivkey(enc.encrypted, enc.iv, enc.tag, 'wrong-passphrase');
      }, /auth/); // GCM auth tag mismatch
    });

    it('should produce different ciphertext for same key with different passphrase', () => {
      const mnemonic = keychain.generateMnemonic();
      const kp = keychain.mnemonicToKeypair(mnemonic);

      const enc1 = keychain.encryptPrivkey(kp.privkey, 'pass-1');
      const enc2 = keychain.encryptPrivkey(kp.privkey, 'pass-2');

      // Different passphrases → different encrypted output
      // (Note: could theoretically collide but astronomically unlikely)
      assert.notEqual(enc1.encrypted, enc2.encrypted);
    });

    it('should reject tampered encrypted data', () => {
      const mnemonic = keychain.generateMnemonic();
      const kp = keychain.mnemonicToKeypair(mnemonic);

      const enc = keychain.encryptPrivkey(kp.privkey, 'my-pass');
      // Decode, modify a byte in the middle, re-encode (reliable method,
      // unlike flipping trailing Base64URL chars which may be padding)
      const buf = keychain.fromBase64URL(enc.encrypted);
      buf[Math.floor(buf.length / 2)] ^= 0xFF; // flip all bits in middle byte
      const tampered = keychain.toBase64URL(buf);

      assert.throws(() => {
        keychain.decryptPrivkey(tampered, enc.iv, enc.tag, 'my-pass');
      });
    });

    it('should reject tampered IV', () => {
      const mnemonic = keychain.generateMnemonic();
      const kp = keychain.mnemonicToKeypair(mnemonic);

      const enc = keychain.encryptPrivkey(kp.privkey, 'my-pass');
      const badIv = enc.iv.slice(0, -2) + 'AA';

      assert.throws(() => {
        keychain.decryptPrivkey(enc.encrypted, badIv, enc.tag, 'my-pass');
      });
    });

    it('should handle empty passphrase', () => {
      const mnemonic = keychain.generateMnemonic();
      const kp = keychain.mnemonicToKeypair(mnemonic);

      const enc = keychain.encryptPrivkey(kp.privkey, '');
      const decrypted = keychain.decryptPrivkey(enc.encrypted, enc.iv, enc.tag, '');
      assert.equal(decrypted, kp.privkey);
    });

    it('should produce unique IV each encryption call', () => {
      const mnemonic = keychain.generateMnemonic();
      const kp = keychain.mnemonicToKeypair(mnemonic);

      const enc1 = keychain.encryptPrivkey(kp.privkey, 'pass');
      const enc2 = keychain.encryptPrivkey(kp.privkey, 'pass');
      assert.notEqual(enc1.iv, enc2.iv);
    });
  });

  // ─── Social Recovery (Shamir's Secret Sharing) ─────────────────────────

  describe('generateShares() and recoverFromShares()', () => {
    it('should split and recover a seed with 3-of-5', () => {
      const seed = 'deadbeefcafebabedeadbeefcafebabe';
      const shares = keychain.generateShares(seed, 5, 3);
      assert.equal(shares.length, 5);

      // Recover with first 3 shares
      const recovered = keychain.recoverFromShares(shares.slice(0, 3));
      assert.equal(recovered.toString('hex'), seed);
    });

    it('should recover with any subset of the required threshold', () => {
      const seed = '0123456789abcdef0123456789abcdef';
      const shares = keychain.generateShares(seed, 5, 3);

      // Recover with last 3 shares (different set)
      const recovered = keychain.recoverFromShares(shares.slice(2));
      assert.equal(recovered.toString('hex'), seed);
    });

    it('should work with 2-of-3', () => {
      const seed = 'a1b2c3d4e5f67890';
      const shares = keychain.generateShares(seed, 3, 2);

      const recovered = keychain.recoverFromShares(shares.slice(0, 2));
      assert.equal(recovered.toString('hex'), seed);
    });

    it('should work with long seeds (64 hex chars)', () => {
      const seed = 'ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff';
      const shares = keychain.generateShares(seed, 4, 2);

      const recovered = keychain.recoverFromShares(shares.slice(0, 2));
      assert.equal(recovered.toString('hex'), seed);
    });

    it('should fail to recover with insufficient shares', () => {
      const seed = 'deadbeef01234567';
      const shares = keychain.generateShares(seed, 3, 3);

      // Even with 2/3 shares, recovery should produce wrong data
      // (technically 2-of-3 will produce *something* but not the original)
      const incomplete = keychain.recoverFromShares(shares.slice(0, 2));
      assert.notEqual(incomplete.toString('hex'), seed);
    });

    it('should reject fewer than 2 shares', () => {
      assert.throws(() => keychain.recoverFromShares([]), /At least 2 shares/);
      assert.throws(() => keychain.recoverFromShares([{ index: 1, data: '00' }]), /At least 2 shares/);
    });

    it('should reject invalid threshold values', () => {
      const seed = 'deadbeef';
      assert.throws(() => keychain.generateShares(seed, 3, 1), /Threshold must be >= 2/);
      assert.throws(() => keychain.generateShares(seed, 3, 5), /Threshold must be <= total/);
    });

    it('should reject empty seeds', () => {
      assert.throws(() => keychain.generateShares('', 3, 2), /Seed must not be empty/);
    });

    it('should reject mismatched share data lengths', () => {
      const shares = [
        { index: 1, data: 'deadbeef' },
        { index: 2, data: 'cafe' },
      ];
      assert.throws(() => keychain.recoverFromShares(shares), /same data length/);
    });
  });

  // ─── Encoding Helpers ──────────────────────────────────────────────────

  describe('encoding helpers', () => {
    it('toBase64URL / fromBase64URL should round-trip', () => {
      const original = Buffer.from('Hello Mosaic!');
      const encoded = keychain.toBase64URL(original);
      const decoded = keychain.fromBase64URL(encoded);
      assert.ok(decoded.equals(original));
      assert.match(encoded, /^[A-Za-z0-9\-_]+$/);
    });

    it('toHex / fromHex should round-trip', () => {
      const original = Buffer.from([0xDE, 0xAD, 0xBE, 0xEF]);
      const hex = keychain.toHex(original);
      assert.equal(hex, 'deadbeef');
      const decoded = keychain.fromHex(hex);
      assert.ok(decoded.equals(original));
    });

    it('should handle Base64URL without padding', () => {
      const buf = crypto.randomBytes(32);
      const enc = keychain.toBase64URL(buf);
      assert.strictEqual(enc.includes('='), false, 'no padding');
      assert.match(enc, /^[A-Za-z0-9\-_]+$/);
    });
  });

  // ─── Integration: generate → keypair → encrypt → decrypt ──────────────

  describe('Integration: full lifecycle', () => {
    it('should generate a mnemonic, derive keys, encrypt, and decrypt', () => {
      // 1. Generate mnemonic
      const mnemonic = keychain.generateMnemonic(256);
      assert.ok(keychain.validateMnemonic(mnemonic));

      // 2. Derive keypair
      const kp = keychain.mnemonicToKeypair(mnemonic, 'my-recovery-phrase');
      assert.ok(kp.pubkey);
      assert.ok(kp.privkey);

      // 3. Encrypt private key
      const enc = keychain.encryptPrivkey(kp.privkey, 'disk-encryption-pass');
      assert.ok(enc.encrypted);
      assert.ok(enc.iv);
      assert.ok(enc.tag);

      // 4. Decrypt private key
      const decrypted = keychain.decryptPrivkey(enc.encrypted, enc.iv, enc.tag, 'disk-encryption-pass');
      assert.equal(decrypted, kp.privkey);

      // 5. Verify the decrypted key still works
      const identity = require('../src/identity');
      const derived = identity.derivePublicKey(decrypted);
      assert.equal(derived.pubkey, kp.pubkey);
    });
  });
});

// Import crypto for the Base64URL padding test
const crypto = require('crypto');
