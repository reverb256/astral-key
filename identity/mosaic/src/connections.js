'use strict';

/**
 * Mosaic Connections Module — Follow/unfollow + blocklist CRUD.
 *
 * All operations use Ed25519 pubkey references (ed25519:<base64> format).
 */

const { getDb } = require('./database');

// ─── Follows ────────────────────────────────────────────────────────────────

/**
 * Create a follow relationship.
 *
 * @param {string} follower - Pubkey doing the following
 * @param {string} followee - Pubkey being followed
 * @returns {object} The created follow row
 */
function follow(follower, followee) {
  if (!follower || !followee) throw new Error('follower and followee are required');
  if (follower === followee) throw new Error('Cannot follow yourself');

  const db = getDb();

  // Verify identities exist
  const followerExists = db.prepare('SELECT pubkey FROM identities WHERE pubkey = ?').get(follower);
  if (!followerExists) throw new Error('Follower identity not found');

  // Insert (ignore if already following)
  db.prepare(`
    INSERT OR IGNORE INTO follows (follower, followee)
    VALUES (?, ?)
  `).run(follower, followee);

  return { follower, followee };
}

/**
 * Remove a follow relationship.
 *
 * @param {string} follower
 * @param {string} followee
 * @returns {object} { ok: true }
 */
function unfollow(follower, followee) {
  if (!follower || !followee) throw new Error('follower and followee are required');

  getDb().prepare(
    'DELETE FROM follows WHERE follower = ? AND followee = ?'
  ).run(follower, followee);

  return { ok: true };
}

/**
 * Get followers of a pubkey.
 *
 * @param {string} pubkey
 * @returns {object[]} Array of follower pubkeys
 */
function getFollowers(pubkey) {
  if (!pubkey) throw new Error('pubkey is required');
  return getDb().prepare(
    'SELECT follower FROM follows WHERE followee = ? ORDER BY created_at DESC'
  ).all(pubkey).map(r => r.follower);
}

/**
 * Get who a pubkey is following.
 *
 * @param {string} pubkey
 * @returns {object[]} Array of followee pubkeys
 */
function getFollowing(pubkey) {
  if (!pubkey) throw new Error('pubkey is required');
  return getDb().prepare(
    'SELECT followee FROM follows WHERE follower = ? ORDER BY created_at DESC'
  ).all(pubkey).map(r => r.followee);
}

/**
 * Get full follower and following counts for a pubkey.
 *
 * @param {string} pubkey
 * @returns {object} { followers: number, following: number }
 */
function getConnectionCounts(pubkey) {
  const db = getDb();
  const followers = db.prepare(
    'SELECT COUNT(*) as count FROM follows WHERE followee = ?'
  ).get(pubkey);
  const following = db.prepare(
    'SELECT COUNT(*) as count FROM follows WHERE follower = ?'
  ).get(pubkey);
  return {
    followers: followers ? followers.count : 0,
    following: following ? following.count : 0,
  };
}

// ─── Blocks ─────────────────────────────────────────────────────────────────

/**
 * Block a pubkey.
 *
 * @param {string} blocker - Pubkey doing the blocking
 * @param {string} blockee - Pubkey being blocked
 * @param {string} [reason] - Optional reason
 * @returns {object} The created block row
 */
function block(blocker, blockee, reason) {
  if (!blocker || !blockee) throw new Error('blocker and blockee are required');
  if (blocker === blockee) throw new Error('Cannot block yourself');

  const db = getDb();

  // Remove any follow relationship between the two
  db.prepare('DELETE FROM follows WHERE follower = ? AND followee = ?').run(blocker, blockee);
  db.prepare('DELETE FROM follows WHERE follower = ? AND followee = ?').run(blockee, blocker);

  db.prepare(`
    INSERT OR IGNORE INTO blocked (blocker, blockee, reason)
    VALUES (?, ?, ?)
  `).run(blocker, blockee, reason || null);

  return { blocker, blockee, reason: reason || null };
}

/**
 * Unblock a pubkey.
 *
 * @param {string} blocker
 * @param {string} blockee
 * @returns {object} { ok: true }
 */
function unblock(blocker, blockee) {
  if (!blocker || !blockee) throw new Error('blocker and blockee are required');

  getDb().prepare(
    'DELETE FROM blocked WHERE blocker = ? AND blockee = ?'
  ).run(blocker, blockee);

  return { ok: true };
}

/**
 * Check if a pubkey is blocked by another.
 *
 * @param {string} pubkey - The pubkey to check
 * @param {string} byPubkey - The pubkey who may have blocked
 * @returns {boolean}
 */
function isBlocked(pubkey, byPubkey) {
  if (!pubkey || !byPubkey) return false;
  const row = getDb().prepare(
    'SELECT 1 FROM blocked WHERE blocker = ? AND blockee = ?'
  ).get(byPubkey, pubkey);
  return !!row;
}

/**
 * Get all blocks for a pubkey.
 *
 * @param {string} pubkey
 * @returns {object[]} Array of blocked pubkeys with reasons
 */
function getBlocks(pubkey) {
  if (!pubkey) throw new Error('pubkey is required');
  return getDb().prepare(
    'SELECT blockee, reason, created_at FROM blocked WHERE blocker = ? ORDER BY created_at DESC'
  ).all(pubkey);
}

module.exports = {
  follow,
  unfollow,
  getFollowers,
  getFollowing,
  getConnectionCounts,
  block,
  unblock,
  isBlocked,
  getBlocks,
};
