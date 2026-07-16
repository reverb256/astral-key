'use strict';

/**
 * Mosaic Feed Module — Feed post creation, query, and reactions.
 *
 * Post schema (signed event envelope):
 * {
 *   "type": "post",
 *   "pubkey": "ed25519:<base64>",
 *   "payload": {"content": "hello world", "media": [], "reply_to": null},
 *   "timestamp": 1700000000,
 *   "signature": "<ed25519_sig>"
 * }
 */

const crypto = require('crypto');
const identity = require('./identity');
const { getDb } = require('./database');

const ALGO_REGISTRY = {
  recent: require('./feed-algos/recent'),
  local: require('./feed-algos/local'),
  friends: require('./feed-algos/friends'),
};

// ─── Feed Post CRUD ─────────────────────────────────────────────────────────

/**
 * Create a signed feed post.
 *
 * @param {string} pubkey - The ed25519:<base64> pubkey
 * @param {object|string} content - Post content (string or object with content/media/reply_to)
 * @param {Function} signFn - signing function: signFn(pubkey, dataString) => signature
 * @returns {object} The created post
 */
function createPost(pubkey, content, signFn) {
  if (!pubkey || !pubkey.startsWith('ed25519:')) throw new Error('Invalid pubkey');
  if (!content) throw new Error('Content is required');

  const db = getDb();

  // Normalize content
  const payload = typeof content === 'string'
    ? { content, media: [], reply_to: null }
    : {
        content: content.content || '',
        media: content.media || [],
        reply_to: content.reply_to || null,
      };

  if (!payload.content || payload.content.length === 0) throw new Error('Post content cannot be empty');
  if (payload.content.length > 2000) throw new Error('Post content too long (max 2000 chars)');

  // Build post envelope
  const timestamp = Math.floor(Date.now() / 1000);
  const cid = crypto.createHash('sha256')
    .update(`${pubkey}:${timestamp}:${payload.content}`)
    .digest('hex');

  const postData = JSON.stringify({ type: 'post', pubkey, payload, timestamp });
  const signature = signFn(pubkey, postData);

  const createdAt = new Date(timestamp * 1000).toISOString().replace('T', ' ').replace('Z', '');

  // Check for duplicate CID
  const existing = db.prepare('SELECT cid FROM feed_posts WHERE cid = ?').get(cid);
  if (existing) throw new Error('Duplicate post (same content and timestamp)');

  db.prepare(`
    INSERT INTO feed_posts (cid, pubkey, content, created_at, signature, reply_to)
    VALUES (?, ?, ?, ?, ?, ?)
  `).run(cid, pubkey, payload.content, createdAt, signature, payload.reply_to || null);

  return getPost(cid);
}

/**
 * Fetch a single post by CID.
 *
 * @param {string} cid
 * @returns {object|null}
 */
function getPost(cid) {
  if (!cid) return null;
  return getDb().prepare('SELECT * FROM feed_posts WHERE cid = ?').get(cid) || null;
}

/**
 * Query feed posts using a named algorithm.
 *
 * @param {string} algo - Algorithm name: 'recent', 'local', 'friends'
 * @param {object} params - Algorithm-specific parameters
 * @returns {object[]} Array of feed posts
 */
function getFeed(algo, params = {}) {
  if (!algo) algo = 'recent';
  const algoImpl = ALGO_REGISTRY[algo];
  if (!algoImpl) throw new Error(`Unknown feed algorithm: ${algo}`);

  const posts = algoImpl.getPosts(params);

  // Attach reaction counts to each post
  return posts.map(post => {
    const reactions = getReactionCounts(post.cid);
    return { ...post, reactions };
  });
}

/**
 * List available feed algorithms.
 */
function listAlgorithms() {
  return Object.keys(ALGO_REGISTRY);
}

// ─── Reactions ──────────────────────────────────────────────────────────────

/**
 * Add a reaction (like/repost) to a post.
 *
 * @param {string} cid - Post CID
 * @param {string} pubkey - Reactor's pubkey
 * @param {string} type - Reaction type ('like' or 'repost')
 */
function addReaction(cid, pubkey, type) {
  if (!['like', 'repost'].includes(type)) throw new Error('Reaction type must be "like" or "repost"');
  if (!cid) throw new Error('cid is required');
  if (!pubkey) throw new Error('pubkey is required');

  const db = getDb();

  // Verify post exists
  const post = db.prepare('SELECT cid FROM feed_posts WHERE cid = ?').get(cid);
  if (!post) throw new Error('Post not found');

  // Upsert
  db.prepare(`
    INSERT INTO feed_reactions (cid, pubkey, type)
    VALUES (?, ?, ?)
    ON CONFLICT(cid, pubkey, type) DO NOTHING
  `).run(cid, pubkey, type);

  return { cid, pubkey, type };
}

/**
 * Remove a reaction from a post.
 *
 * @param {string} cid - Post CID
 * @param {string} pubkey - Reactor's pubkey
 * @param {string} type - Reaction type
 */
function removeReaction(cid, pubkey, type) {
  if (!cid || !pubkey || !type) throw new Error('cid, pubkey, and type are required');

  getDb().prepare(
    'DELETE FROM feed_reactions WHERE cid = ? AND pubkey = ? AND type = ?'
  ).run(cid, pubkey, type);

  return { ok: true };
}

/**
 * Get reaction counts for a post.
 *
 * @param {string} cid
 * @returns {object} { likes: number, reposts: number }
 */
function getReactionCounts(cid) {
  const db = getDb();
  const likes = db.prepare(
    "SELECT COUNT(*) as count FROM feed_reactions WHERE cid = ? AND type = 'like'"
  ).get(cid);
  const reposts = db.prepare(
    "SELECT COUNT(*) as count FROM feed_reactions WHERE cid = ? AND type = 'repost'"
  ).get(cid);

  return {
    likes: likes ? likes.count : 0,
    reposts: reposts ? reposts.count : 0,
  };
}

/**
 * Get all reactions for a post.
 *
 * @param {string} cid
 * @returns {object[]} Array of reaction rows
 */
function getReactions(cid) {
  return getDb().prepare(
    'SELECT * FROM feed_reactions WHERE cid = ? ORDER BY created_at ASC'
  ).all(cid);
}

module.exports = {
  createPost,
  getPost,
  getFeed,
  listAlgorithms,
  addReaction,
  removeReaction,
  getReactionCounts,
  getReactions,
};
