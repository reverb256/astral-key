'use strict';

/**
 * Identity Module — Ed25519 key management with NaCl.
 *
 * Every Mosiac user gets an Ed25519 key pair as their root identity.
 * The public key is the user's globally unique address/identity.
 * The private key never leaves the device (stored in SQLite via better-sqlite3).
 *
 * Key format:
 *   - Public key:  32 bytes, Base64URL encoded (44 chars, no padding)
 *   - Private key: 64 bytes (seed + public key), Base64URL encoded
 */

const nacl = require('tweetnacl');
const naclUtil = require('tweetnacl-util');
const crypto = require('crypto');

// ─── Encoding helpers ──────────────────────────────────────────────────────

/**
 * Base64URL encode (RFC 4648 §5) — no padding, URL-safe.
 */
function toBase64URL(buf) {
  return Buffer.from(buf).toString('base64url');
}

/**
 * Decode Base64URL to Buffer.
 */
function fromBase64URL(str) {
  return Buffer.from(str, 'base64url');
}

/**
 * Hex encode.
 */
function toHex(buf) {
  return Buffer.from(buf).toString('hex');
}

/**
 * Hex decode.
 */
function fromHex(hex) {
  return Buffer.from(hex, 'hex');
}

// ─── Key generation ────────────────────────────────────────────────────────

/**
 * Generate a new Ed25519 key pair.
 *
 * @returns {{ pubkey: string, privkey: string, pubkeyHex: string }}
 *   - pubkey:   Base64URL-encoded 32-byte public key
 *   - privkey:  Base64URL-encoded 64-byte secret key (seed || pubkey)
 *   - pubkeyHex: hex-encoded public key for human-friendly display
 */
function generateKeyPair() {
  const kp = nacl.sign.keyPair();
  const pubkey = toBase64URL(kp.publicKey);
  const privkey = toBase64URL(kp.secretKey);
  const pubkeyHex = toHex(kp.publicKey);

  return { pubkey, privkey, pubkeyHex };
}

/**
 * Derive the public key from a private key.
 * Useful when loading a stored private key to recover the public half.
 *
 * @param {string} privkey - Base64URL-encoded 64-byte secret key
 * @returns {{ pubkey: string, pubkeyHex: string }}
 */
function derivePublicKey(privkey) {
  const secretKey = fromBase64URL(privkey);
  if (secretKey.length !== 64) {
    throw new Error(`Invalid private key length: expected 64 bytes, got ${secretKey.length}`);
  }
  const kp = nacl.sign.keyPair.fromSecretKey(secretKey);
  return {
    pubkey: toBase64URL(kp.publicKey),
    pubkeyHex: toHex(kp.publicKey),
  };
}

/**
 * Reconstruct the full NaCl key pair from a stored private key.
 */
function keyPairFromSecret(privkey) {
  const secretKey = fromBase64URL(privkey);
  if (secretKey.length !== 64) {
    throw new Error(`Invalid private key length: expected 64 bytes, got ${secretKey.length}`);
  }
  return nacl.sign.keyPair.fromSecretKey(secretKey);
}

// ─── Signing & verification ────────────────────────────────────────────────

/**
 * Sign a message (string or Buffer) with the given private key.
 *
 * @param {string|Buffer} message
 * @param {string} privkey - Base64URL-encoded 64-byte secret key
 * @returns {string} Base64URL-encoded detached signature (64 bytes)
 */
function sign(message, privkey) {
  const msgBytes = Buffer.isBuffer(message) ? message : Buffer.from(message, 'utf8');
  const secretKey = fromBase64URL(privkey);
  const sig = nacl.sign.detached(msgBytes, secretKey);
  return toBase64URL(sig);
}

/**
 * Verify a detached signature against a message and public key.
 *
 * @param {string|Buffer} message
 * @param {string} signature - Base64URL-encoded 64-byte signature
 * @param {string} pubkey - Base64URL-encoded 32-byte public key
 * @returns {boolean}
 */
function verify(message, signature, pubkey) {
  const msgBytes = Buffer.isBuffer(message) ? message : Buffer.from(message, 'utf8');
  const sigBytes = fromBase64URL(signature);
  const pubkeyBytes = fromBase64URL(pubkey);
  return nacl.sign.detached.verify(msgBytes, sigBytes, pubkeyBytes);
}

/**
 * Sign JSON data by producing a compact JSON string, then signing.
 * Returns { data, signature, pubkey }.
 *
 * @param {object} data - JSON-serializable data
 * @param {string} privkey
 * @param {string} pubkey
 * @returns {{ data: object, signature: string, pubkey: string }}
 */
function signJSON(data, privkey, pubkey) {
  const json = JSON.stringify(data);
  const signature = sign(json, privkey);
  return { data, signature, pubkey };
}

/**
 * Verify a JSON-signed envelope produced by signJSON().
 *
 * @param {{ data: object, signature: string, pubkey: string }} envelope
 * @returns {boolean}
 */
function verifyJSON(envelope) {
  const { data, signature, pubkey } = envelope;
  const json = JSON.stringify(data);
  return verify(json, signature, pubkey);
}

// ─── Key fingerprint ───────────────────────────────────────────────────────

/**
 * Generate a short human-readable fingerprint for a public key.
 * Uses the first 8 hex chars (4 bytes) of a SHA-256 hash of the pubkey.
 *
 * @param {string} pubkey - Base64URL-encoded public key
 * @returns {string} e.g. "a3f8c91e"
 */
function fingerprint(pubkey) {
  const hash = crypto.createHash('sha256').update(pubkey, 'utf8').digest('hex');
  return hash.slice(0, 8);
}

/**
 * Generate a QR-friendly URI for sharing a public key.
 * Format: mosiac://<pubkey>?fn=<fingerprint>
 *
 * @param {string} pubkey - Base64URL-encoded public key
 * @returns {string}
 */
function pubkeyURI(pubkey) {
  const fp = fingerprint(pubkey);
  return `mosiac://${pubkey}?fn=${fp}`;
}

/**
 * Parse a mosiac:// URI back into the pubkey.
 *
 * @param {string} uri
 * @returns {{ pubkey: string, fingerprint: string } | null}
 */
function parsePubkeyURI(uri) {
  try {
    const u = new URL(uri);
    if (u.protocol !== 'mosiac:') return null;
    const pubkey = u.hostname || u.pathname.replace(/^\//, '');
    if (!pubkey) return null;
    const fp = u.searchParams.get('fn') || '';
    return { pubkey, fingerprint: fp };
  } catch {
    return null;
  }
}

module.exports = {
  generateKeyPair,
  derivePublicKey,
  keyPairFromSecret,
  sign,
  verify,
  signJSON,
  verifyJSON,
  fingerprint,
  pubkeyURI,
  parsePubkeyURI,
  toBase64URL,
  fromBase64URL,
  toHex,
  fromHex,
};
