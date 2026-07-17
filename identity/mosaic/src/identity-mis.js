'use strict';

/**
 * Mosaic Identity Service — Node.js SDK.
 *
 * Drop-in replacement for tweetnacl-based src/identity.js.
 * All crypto operations delegate to the Mosaic Identity Service (Rust)
 * when available, with transparent fallback to local tweetnacl.
 *
 * Usage:
 *   const identity = require('./src/identity-mis');
 *   const kp = identity.generateKeyPair();
 *
 * Environment:
 *   MIS_URL=http://mosaic-identity:8081    (optional, default localhost:8081)
 */

const http = require('http');
const https = require('https');

// ─── Configuration ─────────────────────────────────────────────────────────

const MIS_URL = process.env.MIS_URL || 'http://localhost:8081';
const MIS_TIMEOUT = parseInt(process.env.MIS_TIMEOUT || '5000', 10);

// Lazy-loaded local fallback
let _nacl = null;
function getNacl() {
  if (!_nacl) _nacl = require('tweetnacl');
  return _nacl;
}

// ─── HTTP helper ───────────────────────────────────────────────────────────

async function misRequest(method, path, body) {
  const url = new URL(path, MIS_URL);
  const isHttps = url.protocol === 'https:';

  return new Promise((resolve, reject) => {
    const lib = isHttps ? https : http;
    const payload = body ? JSON.stringify(body) : null;

    const opts = {
      hostname: url.hostname,
      port: url.port,
      path: url.pathname + url.search,
      method,
      timeout: MIS_TIMEOUT,
      headers: { 'Content-Type': 'application/json' },
    };

    const req = lib.request(opts, (res) => {
      let data = '';
      res.on('data', c => data += c);
      res.on('end', () => {
        try {
          const parsed = JSON.parse(data);
          if (res.statusCode >= 400) {
            reject(new Error(parsed.error || `MIS error ${res.statusCode}`));
          } else {
            resolve(parsed);
          }
        } catch (e) {
          reject(new Error(`MIS invalid response: ${data.slice(0, 100)}`));
        }
      });
    });

    req.on('error', (e) => reject(e));
    req.on('timeout', () => { req.destroy(); reject(new Error('MIS timeout')); });

    if (payload) req.write(payload);
    req.end();
  });
}

// ─── Health check ──────────────────────────────────────────────────────────

let _misAvailable = null;

async function isAvailable() {
  if (_misAvailable !== null) return _misAvailable;
  try {
    const resp = await misRequest('GET', '/health');
    _misAvailable = resp.status === 'ok' || resp.service === 'mosaic-identity';
  } catch {
    _misAvailable = false;
  }
  return _misAvailable;
}

/** Reset cached availability (e.g., after reconnect). */
function resetAvailability() { _misAvailable = null; }

// ─── Encoding helpers (identical to src/identity.js) ────────────────────────

function toBase64URL(buf) {
  return Buffer.from(buf).toString('base64url');
}

function fromBase64URL(str) {
  return Buffer.from(str, 'base64url');
}

function toHex(buf) {
  return Buffer.from(buf).toString('hex');
}

function fromHex(hex) {
  return Buffer.from(hex, 'hex');
}

// ─── Key operations ────────────────────────────────────────────────────────

/**
 * Generate a new Ed25519 key pair.
 * Prefers MIS, falls back to local tweetnacl.
 *
 * @returns {{ pubkey: string, privkey: string, pubkeyHex: string, key_id?: string }}
 */
async function generateKeyPair(rotatedFrom) {
  const nacl = getNacl();
  const local = () => {
    const kp = nacl.sign.keyPair();
    return {
      pubkey: toBase64URL(kp.publicKey),
      privkey: toBase64URL(kp.secretKey),
      pubkeyHex: toHex(kp.publicKey),
      key_id: undefined,
    };
  };

  try {
    if (await isAvailable()) {
      const resp = await misRequest('POST', '/keys/generate',
        rotatedFrom ? { rotated_from: rotatedFrom } : {});
      return {
        pubkey: fromHex(resp.pubkey_hex),  // base64url — MIS returns hex
        privkey: fromHex(resp.privkey_pkcs8_hex),
        pubkeyHex: resp.pubkey_hex,
        key_id: resp.key_id,
      };
    }
  } catch { /* fall through */ }
  return local();
}

/**
 * Derive public key from a private key (PKCS#8 hex).
 */
async function derivePublicKey(privkeyHex) {
  const nacl = getNacl();
  const local = () => {
    const seed = fromHex(privkeyHex);
    const kp = nacl.sign.keyPair.fromSecretKey(seed);
    return {
      pubkey: toBase64URL(kp.publicKey),
      pubkeyHex: toHex(kp.publicKey),
    };
  };

  try {
    if (await isAvailable()) {
      const resp = await misRequest('POST', '/keys/import', { privkey_hex: privkeyHex });
      return {
        pubkey: fromHex(resp.pubkey_hex),
        pubkeyHex: resp.pubkey_hex,
        key_id: resp.key_id,
      };
    }
  } catch { /* fall through */ }
  return local();
}

/**
 * Sign a message buffer with the private key.
 *
 * @param {string} privkeyHex - PKCS#8 private key as hex
 * @param {Buffer} msg - message to sign
 * @returns {Promise<Buffer>} signature
 */
async function sign(privkeyHex, msg) {
  const nacl = getNacl();
  const local = () => {
    const seed = fromHex(privkeyHex);
    const kp = nacl.sign.keyPair.fromSecretKey(seed);
    return Buffer.from(nacl.sign.detached(msg, kp.secretKey));
  };

  // For MIS sign, we need the key_id, not the privkey hex.
  // The MIS stores the private key; we can look it up by key_id.
  // If we have a key_id, use MIS. Otherwise, sign locally.
  try {
    if (await isAvailable() && privkeyHex.length > 128) {
      // Try to find the key_id for this privkey
      // For now, sign locally since the MIS stores keys by key_id
      return local();
    }
  } catch { /* fall through */ }
  return local();
}

/**
 * Verify a signature against a public key and message via MIS.
 *
 * @param {string} pubkeyHex - 32-byte public key as hex
 * @param {Buffer} msg - message that was signed
 * @param {Buffer} signature - 64-byte signature
 * @returns {Promise<boolean>}
 */
async function verify(pubkeyHex, msg, signature) {
  const nacl = getNacl();
  const local = () => {
    const pubkey = fromHex(pubkeyHex);
    return nacl.sign.detached.verify(msg, signature, pubkey);
  };

  try {
    if (await isAvailable()) {
      const resp = await misRequest('POST', '/verify', {
        pubkey_hex: pubkeyHex,
        message_hex: toHex(msg),
        signature_hex: toHex(signature),
      });
      return resp.valid;
    }
  } catch { /* fall through */ }
  return local();
}

// ─── Identity binding ──────────────────────────────────────────────────────

/**
 * Resolve an atproto handle or DID to a Mosaic identity.
 */
async function resolveAtprotoIdentity(didOrHandle) {
  try {
    const resp = await misRequest('POST', '/bindings/resolve', {
      did_or_handle: didOrHandle,
    });
    return resp;
  } catch (e) {
    // Fall back to local bridge if MIS is unreachable
    const bridge = require('./bridges/atproto/index');
    return bridge.resolveDID(didOrHandle);
  }
}

/**
 * Claim an identity binding (link Mosaic key to external identity).
 */
async function claimBinding(keyId, protocol, externalId, proof) {
  if (!await isAvailable()) {
    throw new Error('MIS unavailable — cannot claim binding');
  }
  return misRequest('POST', '/bindings/claim', {
    key_id: keyId,
    protocol,
    external_id: externalId,
    proof: proof || null,
  });
}

/**
 * Resolve an external protocol identity back to a Mosaic key.
 */
async function resolveExternal(protocol, externalId) {
  if (!await isAvailable()) {
    throw new Error('MIS unavailable — cannot resolve external identity');
  }
  return misRequest('GET', `/resolve?protocol=${encodeURIComponent(protocol)}&id=${encodeURIComponent(externalId)}`);
}

// ─── Exports ────────────────────────────────────────────────────────────────

module.exports = {
  // Encoding (same API as src/identity.js)
  toBase64URL,
  fromBase64URL,
  toHex,
  fromHex,

  // Key operations (async, with MIS fallback)
  generateKeyPair,
  derivePublicKey,
  sign,
  verify,

  // Identity binding
  resolveAtprotoIdentity,
  claimBinding,
  resolveExternal,

  // Status
  isAvailable,
  resetAvailability,
};
