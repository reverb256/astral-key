'use strict';

/**
 * Local Feed Algorithm — Posts from this node's own identities only.
 *
 * SELECT * FROM feed_posts
 * WHERE pubkey IN (SELECT pubkey FROM identities WHERE is_current = 1)
 * ORDER BY created_at DESC
 *
 * Params: { limit, cursor }
 */

const { getDb } = require('../database');

/**
 * Get feed posts from local/current identities.
 *
 * @param {object} params
 * @param {number} [params.limit=50] - Max posts to return
 * @param {string} [params.cursor] - ISO-8601 timestamp cursor
 * @returns {object[]} Array of feed posts
 */
function getPosts(params = {}) {
  const limit = Math.min(params.limit || 50, 200);
  const cursor = params.cursor || null;

  const db = getDb();

  let sql;
  let queryParams;

  if (cursor) {
    sql = `
      SELECT * FROM feed_posts
      WHERE pubkey IN (SELECT pubkey FROM identities WHERE is_current = 1)
        AND created_at < ?
      ORDER BY created_at DESC
      LIMIT ?
    `;
    queryParams = [cursor, limit];
  } else {
    sql = `
      SELECT * FROM feed_posts
      WHERE pubkey IN (SELECT pubkey FROM identities WHERE is_current = 1)
      ORDER BY created_at DESC
      LIMIT ?
    `;
    queryParams = [limit];
  }

  return db.prepare(sql).all(...queryParams);
}

module.exports = { getPosts };
