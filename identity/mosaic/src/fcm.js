// ═══════════════════════════════════════════════════════════
// Haven — FCM Push Notification Module
// Sends mobile push notifications via Firebase Cloud Messaging.
// Uses only jsonwebtoken (already a Haven dependency) — no firebase-admin needed.
//
// Three modes:
//   1. Direct mode: Service account JSON present → sends to FCM API directly
//   2. Relay mode:  FCM_RELAY_URL set → forwards to a push relay server
//   3. Default:     Neither configured → uses the Haven Global Relay automatically
// ═══════════════════════════════════════════════════════════

const jwt = require('jsonwebtoken');
const fs = require('fs');
const path = require('path');

let serviceAccount = null;
let cachedToken = null;
let cachedTokenExpiry = 0;
let relayUrl = '';
let relayKey = '';
let projectId = '';

const FCM_SCOPES = 'https://www.googleapis.com/auth/firebase.messaging';
const TOKEN_URL = 'https://oauth2.googleapis.com/token';
const DEFAULT_RELAY = 'https://us-central1-amni-haven.cloudfunctions.net/sendPush';
const DEFAULT_KEY = 'firebase-notifications-007';

/**
 * Initialize FCM. Call once at startup.
 * @param {string} dataDir - Haven data directory (to find service account JSON)
 * @returns {{ mode: string }} - 'direct', 'relay', or 'disabled'
 */
function initFcm(dataDir) {
  const saPath = process.env.FIREBASE_SERVICE_ACCOUNT
    || findServiceAccount(dataDir)
    || findServiceAccount(__dirname);

  if (saPath && fs.existsSync(saPath)) {
    try {
      serviceAccount = JSON.parse(fs.readFileSync(saPath, 'utf-8'));
      projectId = serviceAccount.project_id;
      console.log(`🔔 FCM direct mode: ${projectId}`);
      return { mode: 'direct' };
    } catch (err) {
      console.warn('⚠️  Failed to parse Firebase service account:', err.message);
    }
  }

  // Fall back to relay mode — uses Haven Global Relay by default
  relayUrl = process.env.FCM_RELAY_URL || DEFAULT_RELAY;
  relayKey = process.env.FCM_PUSH_KEY || DEFAULT_KEY;

  if (relayUrl && relayKey) {
    if (relayUrl === DEFAULT_RELAY) {
      console.log('🔔 FCM enabled via Haven Global Relay');
    } else {
      console.log(`🔔 FCM enabled via Custom Relay: ${relayUrl}`);
    }
    return { mode: 'relay' };
  }

  return { mode: 'disabled' };
}

/**
 * Look for a service account JSON file in a directory.
 */
function findServiceAccount(dir) {
  try {
    const parentDir = path.resolve(dir, '..');
    for (const d of [dir, parentDir]) {
      if (!fs.existsSync(d)) continue;
      const files = fs.readdirSync(d).filter(f =>
        f.endsWith('.json') && (f.includes('service-account') || f.includes('adminsdk'))
      );
      if (files.length > 0) return path.join(d, files[0]);
    }
  } catch {}
  return null;
}

/**
 * Get an OAuth2 access token for the FCM API.
 * Caches token for ~55 minutes (tokens last 60 minutes).
 */
async function getAccessToken() {
  const now = Math.floor(Date.now() / 1000);
  if (cachedToken && now < cachedTokenExpiry) return cachedToken;

  const claim = {
    iss: serviceAccount.client_email,
    scope: FCM_SCOPES,
    aud: TOKEN_URL,
    iat: now,
    exp: now + 3600,
  };

  const assertion = jwt.sign(claim, serviceAccount.private_key, { algorithm: 'RS256' });

  const resp = await fetch(TOKEN_URL, {
    method: 'POST',
    headers: { 'Content-Type': 'application/x-www-form-urlencoded' },
    body: `grant_type=urn:ietf:params:oauth:grant-type:jwt-bearer&assertion=${assertion}`,
  });

  if (!resp.ok) {
    throw new Error(`OAuth2 token exchange failed: ${resp.status}`);
  }

  const data = await resp.json();
  cachedToken = data.access_token;
  cachedTokenExpiry = now + 3300; // refresh 5 min early
  return cachedToken;
}

/**
 * Send FCM notification directly via the HTTP v1 API.
 */
async function sendDirect(tokens, title, body, dataPayload) {
  const accessToken = await getAccessToken();
  const url = `https://fcm.googleapis.com/v1/projects/${projectId}/messages:send`;

  const results = { success: 0, failure: 0, failedTokens: [] };

  // FCM v1 API sends one message at a time — fire all in parallel
  const promises = tokens.map(async (token) => {
    try {
      const resp = await fetch(url, {
        method: 'POST',
        headers: {
          'Authorization': `Bearer ${accessToken}`,
          'Content-Type': 'application/json',
        },
        body: JSON.stringify({
          message: {
            token,
            notification: { title, body },
            data: dataPayload,
            android: {
              priority: 'high',
              notification: { channel_id: 'haven_messages' },
            },
          },
        }),
      });

      if (resp.ok) {
        results.success++;
      } else {
        results.failure++;
        const errBody = await resp.json().catch(() => ({}));
        const errCode = errBody?.error?.details?.[0]?.errorCode || '';
        if (errCode === 'UNREGISTERED' || resp.status === 404) {
          results.failedTokens.push(token);
        }
      }
    } catch {
      results.failure++;
    }
  });

  await Promise.all(promises);
  return results;
}

/**
 * Send FCM notification via an external relay.
 */
async function sendViaRelay(tokens, title, body, dataPayload) {
  try {
    const resp = await fetch(relayUrl, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        'x-push-key': relayKey,
      },
      body: JSON.stringify({ tokens, title, body, data: dataPayload }),
    });

    if (!resp.ok) return { success: 0, failure: tokens.length, failedTokens: [] };
    return await resp.json();
  } catch (err) {
    console.error('FCM relay error:', err.message);
    return { success: 0, failure: tokens.length, failedTokens: [] };
  }
}

/**
 * Send push notifications to the given FCM tokens.
 * Automatically uses direct or relay mode based on config.
 *
 * @param {string[]} tokens - FCM device tokens
 * @param {string} title - Notification title
 * @param {string} body - Notification body text
 * @param {Object} data - Data payload (channelCode, etc.)
 * @returns {Promise<{success: number, failure: number, failedTokens: string[]}>}
 */
async function sendFcm(tokens, title, body, data = {}) {
  if (!tokens || tokens.length === 0) return { success: 0, failure: 0, failedTokens: [] };

  // Convert all data values to strings (FCM requirement)
  const dataPayload = Object.fromEntries(
    Object.entries(data).map(([k, v]) => [k, String(v)])
  );

  if (serviceAccount) {
    return sendDirect(tokens, title, body, dataPayload);
  }
  if (relayUrl && relayKey) {
    return sendViaRelay(tokens, title, body, dataPayload);
  }
  return { success: 0, failure: 0, failedTokens: [] };
}

/**
 * Check if FCM is available (either direct or relay mode).
 */
function isFcmEnabled() {
  return !!(serviceAccount || (relayUrl && relayKey));
}

/**
 * Get the push relay key (for the relay endpoint authentication).
 * Returns null if no service account is loaded (relay mode disabled on this server).
 */
function getRelayKey() {
  if (!serviceAccount) return null;
  return process.env.HAVEN_PUSH_KEY || null;
}

module.exports = { initFcm, sendFcm, isFcmEnabled, getRelayKey };
