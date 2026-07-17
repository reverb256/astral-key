'use strict';

/**
 * Identity Module — auto-selector.
 *
 * Tries Mosaic Identity Service (MIS) via HTTP first.
 * Falls back to local tweetnacl if MIS is unreachable.
 *
 * This file replaces the original src/identity.js (renamed to identity-local.js).
 * All existing require('./identity') imports pick up this resolver automatically.
 *
 * Exports the same API as identity-local.js for drop-in compatibility:
 *   toBase64URL, fromBase64URL, toHex, fromHex
 *   generateKeyPair, derivePublicKey, sign, verify
 */

const http = require('http');

const MIS_URL = process.env.MIS_URL || 'http://mosaic-identity:8081';
const MIS_TIMEOUT = parseInt(process.env.MIS_TIMEOUT || '2000', 10);
const MIS_DISABLED = process.env.MIS_DISABLED === 'true';

let _local = null;
let _misAvailable = null;

function getLocal() {
  if (!_local) _local = require('./identity-local');
  return _local;
}

// ─── MIS availability check ─────────────────────────────────────────────────

function checkMisAvailable() {
  if (_misAvailable !== null) return Promise.resolve(_misAvailable);
  if (MIS_DISABLED) {
    _misAvailable = false;
    return Promise.resolve(false);
  }

  return new Promise((resolve) => {
    const req = http.get(`${MIS_URL}/health`, { timeout: MIS_TIMEOUT }, (res) => {
      let data = '';
      res.on('data', (c) => data += c);
      res.on('end', () => {
        try {
          const parsed = JSON.parse(data);
          _misAvailable = parsed.status === 'ok' || parsed.service === 'mosaic-identity';
        } catch {
          _misAvailable = false;
        }
        resolve(_misAvailable);
      });
    });
    req.on('error', () => { _misAvailable = false; resolve(false); });
    req.on('timeout', () => { req.destroy(); _misAvailable = false; resolve(false); });
  });
}

function misRequest(method, path, body) {
  return new Promise((resolve, reject) => {
    const url = new URL(path, MIS_URL);
    const payload = body ? JSON.stringify(body) : null;
    const opts = {
      hostname: url.hostname,
      port: parseInt(url.port, 10) || 80,
      path: url.pathname + url.search,
      method,
      timeout: MIS_TIMEOUT * 2,
      headers: { 'Content-Type': 'application/json' },
    };
    const req = http.request(opts, (res) => {
      let data = '';
      res.on('data', (c) => data += c);
      res.on('end', () => {
        try {
          const parsed = JSON.parse(data);
          if (res.statusCode >= 400) reject(new Error(parsed.error || `MIS error ${res.statusCode}`));
          else resolve(parsed);
        } catch { reject(new Error(`MIS invalid response: ${data.slice(0, 100)}`)); }
      });
    });
    req.on('error', reject);
    req.on('timeout', () => { req.destroy(); reject(new Error('MIS timeout')); });
    if (payload) req.write(payload);
    req.end();
  });
}

// ─── Exported API (async, with MIS fallback) ────────────────────────────────

function toBase64URL(buf) { return getLocal().toBase64URL(buf); }
function fromBase64URL(str) { return getLocal().fromBase64URL(str); }
function toHex(buf) { return getLocal().toHex(buf); }
function fromHex(hex) { return getLocal().fromHex(hex); }

async function generateKeyPair(rotatedFrom) {
  try {
    if (await checkMisAvailable()) {
      const resp = await misRequest('POST', '/keys/generate',
        rotatedFrom ? { rotated_from: rotatedFrom } : {});
      return {
        pubkey: fromBase64URL(resp.pubkey_hex),
        privkey: fromBase64URL(resp.privkey_pkcs8_hex),
        pubkeyHex: resp.pubkey_hex,
        key_id: resp.key_id,
      };
    }
  } catch { /* fall through */ }
  return getLocal().generateKeyPair(rotatedFrom);
}

async function derivePublicKey(privkeyHex) {
  try {
    if (await checkMisAvailable()) {
      const resp = await misRequest('POST', '/keys/import', { privkey_hex: privkeyHex });
      return { pubkey: fromBase64URL(resp.pubkey_hex), pubkeyHex: resp.pubkey_hex, key_id: resp.key_id };
    }
  } catch { /* fall through */ }
  return getLocal().derivePublicKey(privkeyHex);
}

async function sign(privkeyHex, msg) {
  // MIS signing requires key_id — if only privkey hex is available, sign locally
  return getLocal().sign(privkeyHex, msg);
}

async function verify(pubkeyHex, msg, signature) {
  try {
    if (await checkMisAvailable()) {
      const resp = await misRequest('POST', '/verify', {
        pubkey_hex: pubkeyHex,
        message_hex: toHex(msg),
        signature_hex: toHex(signature),
      });
      return resp.valid;
    }
  } catch { /* fall through */ }
  return getLocal().verify(pubkeyHex, msg, signature);
}

module.exports = {
  toBase64URL, fromBase64URL, toHex, fromHex,
  generateKeyPair, derivePublicKey, sign, verify,
};
