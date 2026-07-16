'use strict';

/**
 * Friends Feed Algorithm — Posts from followed pubkeys.
 *
 * SELECT * FROM feed_posts
 * WHERE pubkey IN (SELECT followee FROM follows WHERE follower = ?)
 * ORDER BY created_at DESC
 *
 * Params: { limit, cursor, follower_pubkey }
 *   - follower_pubkey (required): whose follow graph to query
 */

const { getDb } = require('../database');

/**
 * Get feed posts from followed pubkeys.
 *
 * @param {object} params
 * @param {string} params.follower_pubkey - The pubkey whose follows to use (required)
 * @param {number} [params.limit=50] - Max posts to return
 * @param {string} [params.cursor] - ISO-8601 timestamp cursor
 * @returns {object[]} Array of feed posts
 */
function getPosts(params = {}) {
  const followerPubkey = params.follower_pubkey;
  if (!followerPubkey) throw new Error('follower_pubkey is required for friends feed');

  const limit = Math.min(params.limit || 50, 200);
  const cursor = params.cursor || null;

  const db = getDb();

  let sql;
  let queryParams;

  if (cursor) {
    sql = `
      SELECT * FROM feed_posts
      WHERE pubkey IN (SELECT followee FROM follows WHERE follower = ?)
        AND created_at < ?
      ORDER BY created_at DESC
      LIMIT ?
    `;
    queryParams = [followerPubkey, cursor, limit];
  } else {
    sql = `
      SELECT * FROM feed_posts
      WHERE pubkey IN (SELECT followee FROM follows WHERE follower = ?)
      ORDER BY created_at DESC
      LIMIT ?
    `;
    queryParams = [followerPubkey, limit];
  }

  return db.prepare(sql).all(...queryParams);
}

module.exports = { getPosts };
