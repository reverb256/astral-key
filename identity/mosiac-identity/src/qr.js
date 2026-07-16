'use strict';

/**
 * QR Module — Encode/decode public keys as QR codes for contact sharing.
 *
 * Two representations:
 *   1. mosiac:// URI format — data URI with QR code inside
 *   2. Plain text format — just the Base64URL pubkey
 *
 * Uses the `qrcode` npm package for generation and parsing.
 */

const QRCode = require('qrcode');
const identity = require('./identity');

// ─── Generation ────────────────────────────────────────────────────────────

/**
 * Generate a QR code as a data URI (SVG or PNG) containing the user's
 * public key in mosiac:// format.
 *
 * @param {string} pubkey - Base64URL-encoded public key
 * @param {object} [options]
 * @param {'svg'|'png'} [options.format='svg'] - Output format
 * @param {number} [options.width=300] - Width in pixels (PNG only)
 * @returns {Promise<string>} Data URI string
 */
async function generatePubkeyQR(pubkey, { format = 'svg', width = 300 } = {}) {
  const uri = identity.pubkeyURI(pubkey);

  if (format === 'svg') {
    return await QRCode.toString(uri, {
      type: 'svg',
      margin: 2,
      color: { dark: '#000000', light: '#ffffff' },
    });
  }

  // PNG as data URL
  return await QRCode.toDataURL(uri, {
    width,
    margin: 2,
    color: { dark: '#000000', light: '#ffffff' },
  });
}

/**
 * Generate a QR code as an SVG string (no data URI wrapper).
 * Returns raw SVG markup suitable for direct embedding in HTML.
 *
 * @param {string} pubkey
 * @returns {Promise<string>} Raw SVG
 */
async function generatePubkeyQR_SVG(pubkey) {
  return await generatePubkeyQR(pubkey, { format: 'svg' });
}

/**
 * Generate a QR code as a base64 PNG data URL.
 *
 * @param {string} pubkey
 * @param {number} [width=300]
 * @returns {Promise<string>} data:image/png;base64,...
 */
async function generatePubkeyQR_PNG(pubkey, width = 300) {
  return await generatePubkeyQR(pubkey, { format: 'png', width });
}

// ─── Parsing ───────────────────────────────────────────────────────────────

/**
 * Parse a QR-scanned string to extract a mosiac public key.
 * Accepts both mosiac:// URIs and raw Base64URL pubkeys.
 *
 * @param {string} scanned - Text scanned from a QR code
 * @returns {{ pubkey: string, fingerprint: string } | null}
 */
function parseQR(scanned) {
  if (!scanned || typeof scanned !== 'string') return null;

  const trimmed = scanned.trim();

  // Try mosiac:// URI format
  if (trimmed.startsWith('mosiac://')) {
    return identity.parsePubkeyURI(trimmed);
  }

  // Try raw Base64URL pubkey (44 chars, alphanumeric + -_)
  if (/^[A-Za-z0-9\-_]{43,44}$/.test(trimmed)) {
    // Validate it looks like a real Ed25519 key by checking length
    const buf = identity.fromBase64URL(trimmed);
    if (buf.length === 32) {
      return { pubkey: trimmed, fingerprint: identity.fingerprint(trimmed) };
    }
  }

  return null;
}

// ─── Contact management with QR discovery ─────────────────────────────────

const db = require('./sqlite-adapter');

/**
 * Process a QR scan: parse the pubkey and save as a contact.
 *
 * @param {string} scanned - Text scanned from QR
 * @param {string} [label] - Optional label for the contact
 * @returns {{ success: boolean, pubkey: string, fingerprint: string, contact: object }}
 */
function processQRScan(scanned, label) {
  const parsed = parseQR(scanned);
  if (!parsed) {
    throw new Error('Invalid QR content: expected mosiac:// URI or Base64URL pubkey');
  }

  db.addContact({
    pubkey: parsed.pubkey,
    label: label || null,
    discoveredVia: 'qr',
  });

  const contact = db.getContact(parsed.pubkey);
  return {
    success: true,
    pubkey: parsed.pubkey,
    fingerprint: parsed.fingerprint,
    contact,
  };
}

module.exports = {
  generatePubkeyQR,
  generatePubkeyQR_SVG,
  generatePubkeyQR_PNG,
  parseQR,
  processQRScan,
};
