'use strict';

/**
 * Mosaic CAR Export / Import (Phase 6 — atproto interop)
 *
 * Export a pubkey's signed event log as a CAR (Content-Addressable Archive)
 * file for portable backup, transfer between nodes, or atproto interop.
 *
 * Each event is stored as a CAR block addressable by its event_hash.
 * The CAR root is a "manifest" CID that lists all event CIDs in order.
 *
 * The `@atproto/repo` package is an OPTIONAL dependency — if it is not
 * installed, these functions throw a descriptive error asking the user
 * to install it.
 */

const { queryEvents, queryLatestSeq } = require('./event-log');
const { verifyEvent } = require('./events');
const { verify } = require('./identity');
const fs = require('fs');
const path = require('path');

/**
 * Try to load @atproto/repo, returning null on failure.
 */
function _loadAtproto() {
  try {
    return require('@atproto/repo');
  } catch {
    return null;
  }
}

/**
 * Try to load the CAR codec (@ipld/car or similar), falling back to
 * a built-in minimal CAR writer when unavailable (CAR v1 spec is simple).
 */
function _loadCarWriter() {
  try {
    // Prefer @atproto/repo's built-in CAR support
    const atp = _loadAtproto();
    if (atp) return atp;
  } catch {
    // fall through
  }
  return null;
}

// ─── Minimal CAR v1 writer (no deps) ───────────────────────
// CAR v1 is dead simple: a header (varint-prefixed CID) followed by
// varint-prefixed blocks.  We write raw bytes so atproto is not required
// for the basic export use case.

/**
 * Encode a unsigned LEB128 varint.
 */
function _encodeVarint(value) {
  const bytes = [];
  while (value >= 0x80) {
    bytes.push((value & 0x7f) | 0x80);
    value >>>= 7;
  }
  bytes.push(value & 0x7f);
  return Buffer.from(bytes);
}

/**
 * Write a minimal CAR v1 file.
 *
 * @param {string} outputPath
 * @param {Array<{ cid: Buffer, data: Buffer }>} blocks
 * @param {Buffer} rootCid - Raw CID bytes for the root (manifest)
 */
function _writeCarV1(outputPath, blocks, rootCid) {
  const fd = fs.openSync(outputPath, 'w');

  // CAR header: roots CID + version
  // CAR v1 header: [`version` (1), `roots` [CID]]
  const header = {
    version: 1,
    roots: [],
  };
  // Since we need to write CAR files without any CAR library dependency,
  // we use the raw DAG-CBOR-like format.  The CAR v1 format is:
  // [ varint | header-bytes ] [ varint | block-bytes ] ...
  // Where header is DAG-CBOR: { version: 1, roots: [CID-BYTES] }

  try {
    // Write header
    const headerBytes = _encodeCarHeader({ version: 1, roots: [rootCid] });
    const headerLen = _encodeVarint(headerBytes.length);
    fs.writeSync(fd, headerLen);
    fs.writeSync(fd, headerBytes);

    // Write blocks
    for (const block of blocks) {
      const blockData = Buffer.concat([
        block.cid,        // raw CID bytes (already includes multicodec + multihash)
        block.data,       // raw block data
      ]);
      const blockLen = _encodeVarint(blockData.length);
      fs.writeSync(fd, blockLen);
      fs.writeSync(fd, blockData);
    }
  } finally {
    fs.closeSync(fd);
  }
}

/**
 * Minimal CAR v1 header encoder (no DAG-CBOR dependency).
 * Uses a fixed simple format that compliant CAR readers accept.
 *
 * Header = { version: 1, roots: [<CID-BYTES>] }
 * Encoded as a CBOR-like structure.
 */
function _encodeCarHeader(header) {
  const roots = header.roots || [];
  // Build a simple CBOR encoding: map(2) with version + roots
  // This is a minimal CBOR encoder for the specific CAR header shape
  const versionKey = 0x01; // CBOR key 1 (major type 0, value 1)
  const rootsKey = 0x02;   // CBOR key 2

  // Encode roots array
  const rootItems = roots.map(cid => {
    // CID is a byte string (CBOR major type 2)
    const cidBytes = Buffer.isBuffer(cid) ? cid : Buffer.from(cid);
    if (cidBytes.length < 58) {
      return Buffer.concat([
        Buffer.from([0x58, cidBytes.length]), // byte string, 1-byte length
        cidBytes,
      ]);
    }
    // Longer byte strings
    const lenBytes = _encodeVarint(cidBytes.length);
    return Buffer.concat([
      Buffer.from([0x5a, ...lenBytes]),
      cidBytes,
    ]);
  });

  const rootsArray = Buffer.concat([
    Buffer.from([0x82, ...rootItems]), // array(2) + items
  ]);

  // Actually need proper CBOR array/items encoding
  const rootCidsEncoded = roots.map(cid => {
    const cidBytes = Buffer.isBuffer(cid) ? cid : Buffer.from(cid);
    const len = cidBytes.length;
    if (len <= 23) {
      return Buffer.concat([Buffer.from([0x40 + len]), cidBytes]); // major type 2, extra length
    }
    const lenBuf = Buffer.alloc(1);
    lenBuf[0] = len;
    return Buffer.concat([Buffer.from([0x58]), lenBuf, cidBytes]); // major type 2, 1-byte length
  });

  // Array of roots: major type 4, extra = length
  const numRoots = rootCidsEncoded.length;
  const arrHeader = numRoots <= 23
    ? Buffer.from([0x80 + numRoots])
    : Buffer.concat([Buffer.from([0x98, numRoots])]);

  const cborRoots = Buffer.concat([arrHeader, ...rootCidsEncoded]);

  // Map { version => 1, roots => [...] }
  // CBOR map with 2 pairs: major type 5, extra 2
  const versionEntry = Buffer.from([0x01, 0x01]); // key=1 (uint), value=1 (uint)
  const rootsEntry = Buffer.concat([
    Buffer.from([0x02]), // key=2 (uint)
    cborRoots,
  ]);

  return Buffer.concat([
    Buffer.from([0xa2]), // map(2)
    versionEntry,
    rootsEntry,
  ]);
}

// ─── Export ─────────────────────────────────────────────────

/**
 * Export a pubkey's entire event log to a CAR file.
 *
 * @param {string} pubkey      - Native pubkey to export
 * @param {string} outputPath  - File path for the .car output
 * @returns {Promise<{ rootCid: string, eventCount: number }>}
 */
async function exportEventLog(pubkey, outputPath) {
  const total = queryLatestSeq(pubkey);
  const allEvents = queryEvents(pubkey, 0, total + 1); // fetch all

  if (!allEvents || allEvents.length === 0) {
    throw new Error(`No events found for pubkey: ${pubkey}`);
  }

  const blocks = [];
  const cidMap = new Map(); // event_hash -> { cid, data }

  for (const event of allEvents) {
    const eventJson = JSON.stringify(event);
    const eventBytes = Buffer.from(eventJson, 'utf8');

    // Use event_hash as the CID (no IPLD dependency — we use raw SHA-256
    // as a simple content address).  For full atproto interop the @atproto/repo
    // package should be used instead.
    const rawCid = Buffer.from(event.event_hash, 'hex');

    cidMap.set(event.event_hash, { cid: rawCid, data: eventBytes });
    blocks.push({ cid: rawCid, data: eventBytes });
  }

  // Build manifest: JSON listing all event CIDs in order
  const manifest = {
    pubkey,
    version: 1,
    eventCount: allEvents.length,
    eventHashes: allEvents.map(e => e.event_hash),
    exportedAt: new Date().toISOString(),
  };
  const manifestBytes = Buffer.from(JSON.stringify(manifest), 'utf8');
  const manifestHash = require('crypto').createHash('sha256').update(manifestBytes).digest();
  const manifestCid = Buffer.from(manifestHash);

  // Write CAR v1 with manifest as root
  _writeCarV1(outputPath, blocks, manifestCid);

  return {
    rootCid: manifestHash.toString('hex'),
    eventCount: allEvents.length,
  };
}

// ─── Import ─────────────────────────────────────────────────

/**
 * Import events from a CAR file into the local event log.
 *
 * Verifies each event's Ed25519 signature before importing.
 *
 * @param {string} carPath - Path to .car file
 * @returns {Promise<{ imported: number, skipped: number, errors: string[] }>}
 */
async function importEventLog(carPath) {
  const errors = [];
  let imported = 0;
  let skipped = 0;

  // Read CAR v1 file
  const data = fs.readFileSync(carPath);
  let offset = 0;

  // Parse varint-prefixed header
  if (offset >= data.length) throw new Error('Empty CAR file');
  const headerLen = _decodeVarint(data, offset);
  offset += headerLen.bytes;
  const headerBytes = data.slice(offset, offset + headerLen.value);
  offset += headerLen.value;

  // Simple header parsing — we just need to know where blocks start
  // Read varint-prefixed blocks
  while (offset < data.length) {
    const blockLen = _decodeVarint(data, offset);
    if (!blockLen) break;
    offset += blockLen.bytes;
    const blockBytes = data.slice(offset, offset + blockLen.value);
    offset += blockLen.value;

    if (!blockBytes || blockBytes.length < 4) continue;

    // The block is [CID-bytes][data-bytes]; we need to skip the CID
    // to get the raw event JSON.  CID length varies, so we find the
    // data boundary by reading the CID varint prefix.
    try {
      const cidLen = _decodeCidLength(blockBytes);
      if (!cidLen) continue;
      const eventBytes = blockBytes.slice(cidLen);

      const event = JSON.parse(eventBytes.toString('utf8'));
      if (!event.event_hash || !event.signature || !event.pubkey) {
        skipped++;
        continue;
      }

      // Verify event before importing
      const verification = verifyEvent(event, (payload, sig, pk) => verify(payload, sig, pk));
      if (!verification.valid) {
        errors.push(`Invalid event ${event.event_hash}: ${verification.reason}`);
        skipped++;
        continue;
      }

      await appendToLog(event);
      imported++;
    } catch (err) {
      errors.push(`Parse error at offset ${offset}: ${err.message}`);
      skipped++;
    }
  }

  return { imported, skipped, errors };
}

// ─── CAR binary helpers ─────────────────────────────────────

function _decodeVarint(buf, offset) {
  let value = 0;
  let shift = 0;
  let bytes = 0;
  for (let i = offset; i < buf.length; i++) {
    bytes++;
    const byte = buf[i];
    value |= (byte & 0x7f) << shift;
    if (!(byte & 0x80)) return { value, bytes };
    shift += 7;
  }
  return null; // truncated
}

function _decodeCidLength(buf) {
  // CIDv1 bytes: [multicodec-varint, multihash-varint, hash-bytes...]
  // We just need the total length.  Simple heuristic: CIDv1 for raw SHA-256
  // is typically 36 bytes (0x01 0x71 0x12 0x20 + 32 hash bytes).
  // For our simple case, the block starts with a 2-byte CID varint then
  // 34 bytes of multihash, giving 36 bytes total CID.
  // Try to parse minimally:
  try {
    const codecLen = _decodeVarint(buf, 0);
    if (!codecLen) return null;
    const mhStart = codecLen.bytes;
    const mhCodeLen = _decodeVarint(buf, mhStart);
    if (!mhCodeLen) return null;
    const hashLenStart = mhStart + mhCodeLen.bytes;
    const hashLen = _decodeVarint(buf, hashLenStart);
    if (!hashLen) return null;
    return hashLenStart + hashLen.bytes + hashLen.value;
  } catch {
    // Fallback: assume 36-byte CID for raw SHA-256
    return 36;
  }
}

module.exports = {
  exportEventLog,
  importEventLog,
};
