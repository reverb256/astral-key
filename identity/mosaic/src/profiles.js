'use strict';

/**
 * Mosaic Profile Module — Profile CRUD with Ed25519 signing.
 *
 * Profile manifest schema:
 * {
 *   "version": 1,
 *   "pubkey": "ed25519:<base64>",
 *   "display_name": "cooluser",
 *   "bio": "building sovereign social",
 *   "avatar": null,
 *   "theme": "mosaic-dark",
 *   "content": {"html": "<h1>Welcome</h1>", "css": "body { color: #eee; }"},
 *   "widgets": [
 *     {"type": "music_player", "pinned_track": null},
 *     {"type": "friends", "limit": 10},
 *     {"type": "recent_posts", "limit": 5}
 *   ],
 *   "links": [{"label": "website", "url": "http://..."}],
 *   "signature": "<ed25519_sig>"
 * }
 */

const identity = require('./identity');
const { getDb } = require('./database');

// ─── Schema validation ──────────────────────────────────────────────────────

const ALLOWED_WIDGET_TYPES = ['music_player', 'friends', 'recent_posts', 'custom_html'];

/**
 * Validate a profile manifest structure.
 * Returns null on valid, or an error string on invalid.
 */
function validateManifest(manifest) {
  if (!manifest || typeof manifest !== 'object') return 'Manifest must be an object';
  if (manifest.version !== 1) return 'Manifest version must be 1';
  if (!manifest.pubkey || typeof manifest.pubkey !== 'string') return 'Manifest must have a pubkey string';
  if (!manifest.pubkey.startsWith('ed25519:')) return 'pubkey must start with ed25519:';
  if (!manifest.display_name || typeof manifest.display_name !== 'string') return 'Manifest must have a display_name string';
  if (manifest.display_name.length > 64) return 'display_name too long (max 64 chars)';
  if (manifest.bio && typeof manifest.bio !== 'string') return 'bio must be a string';
  if (manifest.bio && manifest.bio.length > 512) return 'bio too long (max 512 chars)';
  if (manifest.avatar && typeof manifest.avatar !== 'string') return 'avatar must be a string';
  if (manifest.theme && typeof manifest.theme !== 'string') return 'theme must be a string';
  if (manifest.content) {
    if (typeof manifest.content !== 'object') return 'content must be an object';
    if (manifest.content.html && typeof manifest.content.html !== 'string') return 'content.html must be a string';
    if (manifest.content.css && typeof manifest.content.css !== 'string') return 'content.css must be a string';
    if (manifest.content.html && manifest.content.html.length > 50000) return 'content.html too long (max 50000 chars)';
    if (manifest.content.css && manifest.content.css.length > 50000) return 'content.css too long (max 50000 chars)';
  }
  if (manifest.widgets) {
    if (!Array.isArray(manifest.widgets)) return 'widgets must be an array';
    for (const w of manifest.widgets) {
      if (!w.type || !ALLOWED_WIDGET_TYPES.includes(w.type)) {
        return `Unknown widget type: ${w.type}`;
      }
    }
  }
  if (manifest.links) {
    if (!Array.isArray(manifest.links)) return 'links must be an array';
    for (const link of manifest.links) {
      if (!link.label || !link.url) return 'Each link must have label and url';
      if (typeof link.label !== 'string' || typeof link.url !== 'string') return 'Link label and url must be strings';
    }
  }
  return null;
}

// ─── Validation: verify the manifest can be signed ─────────────────────────
// Strip existing signature before signing/verifying.

function manifestWithoutSignature(manifest) {
  const { signature, ...rest } = manifest;
  return rest;
}

// ─── Profile CRUD ───────────────────────────────────────────────────────────

/**
 * Create a new profile manifest for the given pubkey.
 *
 * @param {string} pubkey - The ed25519:<base64> pubkey
 * @param {object} manifestData - The profile fields (without signature)
 * @param {Function} signFn - signing function: signFn(pubkey, dataString) => signature string
 * @returns {object} The stored profile row
 */
function createProfile(pubkey, manifestData, signFn) {
  const db = getDb();

  // Validate pubkey
  if (!pubkey || typeof pubkey !== 'string') throw new Error('pubkey is required');
  if (!pubkey.startsWith('ed25519:')) throw new Error('pubkey must start with ed25519:');

  // Build the full manifest
  const manifest = {
    version: 1,
    pubkey,
    display_name: manifestData.display_name || 'Anonymous',
    bio: manifestData.bio || '',
    avatar: manifestData.avatar || null,
    theme: manifestData.theme || 'mosaic-dark',
    content: manifestData.content || null,
    widgets: manifestData.widgets || [],
    links: manifestData.links || [],
  };

  // Validate
  const err = validateManifest(manifest);
  if (err) throw new Error(`Invalid manifest: ${err}`);

  // Sign
  const dataToSign = JSON.stringify(manifestWithoutSignature(manifest));
  const signature = signFn(pubkey, dataToSign);
  manifest.signature = signature;

  // Check if profile already exists
  const existing = db.prepare('SELECT pubkey FROM profiles WHERE pubkey = ?').get(pubkey);
  if (existing) throw new Error('Profile already exists for this pubkey');

  // Store
  db.prepare(`
    INSERT INTO profiles (pubkey, manifest, published)
    VALUES (?, ?, 1)
  `).run(pubkey, JSON.stringify(manifest));

  return getProfile(pubkey);
}

/**
 * Get a profile by pubkey.
 *
 * @param {string} pubkey
 * @returns {object|null} Profile with parsed manifest, or null
 */
function getProfile(pubkey) {
  if (!pubkey) return null;
  const row = getDb().prepare('SELECT * FROM profiles WHERE pubkey = ?').get(pubkey);
  if (!row) return null;
  return {
    ...row,
    manifest: JSON.parse(row.manifest),
  };
}

/**
 * Update an existing profile.
 *
 * @param {string} pubkey
 * @param {object} manifestData - Updated profile fields
 * @param {Function} signFn - signing function
 * @returns {object} Updated profile
 */
function updateProfile(pubkey, manifestData, signFn) {
  const db = getDb();

  const existing = db.prepare('SELECT * FROM profiles WHERE pubkey = ?').get(pubkey);
  if (!existing) throw new Error('Profile not found');

  const currentManifest = JSON.parse(existing.manifest);

  // Merge: new values override, missing fields fall through to current
  const manifest = {
    ...currentManifest,
    ...manifestData,
    pubkey, // ensure pubkey stays the same
    version: 1,
  };

  // Re-validate
  const err = validateManifest(manifest);
  if (err) throw new Error(`Invalid manifest: ${err}`);

  // Re-sign
  const dataToSign = JSON.stringify(manifestWithoutSignature(manifest));
  const signature = signFn(pubkey, dataToSign);
  manifest.signature = signature;

  db.prepare(`
    UPDATE profiles SET manifest = ?, updated_at = datetime('now')
    WHERE pubkey = ?
  `).run(JSON.stringify(manifest), pubkey);

  return getProfile(pubkey);
}

/**
 * Verify a profile manifest's Ed25519 signature.
 *
 * @param {object} manifest - Full manifest including signature
 * @returns {boolean} true if signature is valid
 */
function verifyProfile(manifest) {
  if (!manifest || !manifest.signature) return false;
  if (!manifest.pubkey) return false;

  // Extract the raw pubkey (strip "ed25519:" prefix)
  const rawPubkey = manifest.pubkey.startsWith('ed25519:')
    ? manifest.pubkey.slice(8)
    : manifest.pubkey;

  const dataToVerify = JSON.stringify(manifestWithoutSignature(manifest));
  return identity.verify(dataToVerify, manifest.signature, rawPubkey);
}

/**
 * List all published profiles.
 */
function listProfiles() {
  const rows = getDb().prepare(
    'SELECT * FROM profiles WHERE published = 1 ORDER BY updated_at DESC'
  ).all();
  return rows.map(r => ({ ...r, manifest: JSON.parse(r.manifest) }));
}

/**
 * Delete a profile.
 */
function deleteProfile(pubkey) {
  getDb().prepare('DELETE FROM profiles WHERE pubkey = ?').run(pubkey);
}

module.exports = {
  createProfile,
  getProfile,
  updateProfile,
  verifyProfile,
  listProfiles,
  deleteProfile,
  validateManifest,
};
