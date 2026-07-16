'use strict';

/**
 * Recent Feed Algorithm — Most recent public posts globally.
 *
 * SELECT * FROM feed_posts ORDER BY created_at DESC LIMIT ?
 *
 * Params: { limit, cursor }
 *   - limit: max number of posts (default 50)
 *   - cursor: ISO-8601 timestamp cursor for pagination (optional)
 */

const { getDb } = require('../database');

/**
 * Get recent public feed posts.
 *
 * @param {object} params
 * @param {number} [params.limit=50] - Max posts to return
 * @param {string} [params.cursor] - ISO-8601 timestamp cursor (posts before this timestamp)
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
      WHERE created_at < ?
      ORDER BY created_at DESC
      LIMIT ?
    `;
    queryParams = [cursor, limit];
  } else {
    sql = `
      SELECT * FROM feed_posts
      ORDER BY created_at DESC
      LIMIT ?
    `;
    queryParams = [limit];
  }

  return db.prepare(sql).all(...queryParams);
}

module.exports = { getPosts };
