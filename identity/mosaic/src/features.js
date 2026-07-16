'use strict';

/**
 * Mosaic Feature Flag System
 *
 * Reads the FEATURES environment variable and provides query functions
 * so every Mosaic subsystem can check whether it should activate.
 *
 * FEATURES format:
 *   all               — enable everything (default)
 *   identity,profiles — enable only the listed features
 *   chat              — Haven-only mode (no Mosaic features)
 *
 * Recognised feature names:
 *   chat, identity, profiles, feeds, connections, moderation
 */

function getEnabledFeatures() {
  const raw = process.env.FEATURES || 'all';
  if (raw === 'all') {
    return ['chat', 'identity', 'profiles', 'feeds', 'connections', 'moderation'];
  }
  return raw.split(',').map(s => s.trim());
}

function isEnabled(name) {
  return getEnabledFeatures().includes(name);
}

function isChatEnabled()      { return isEnabled('chat'); }
function isIdentityEnabled()  { return isEnabled('identity'); }
function isProfilesEnabled()  { return isEnabled('profiles'); }
function isFeedsEnabled()     { return isEnabled('feeds'); }
function isConnectionsEnabled() { return isEnabled('connections'); }
function isModerationEnabled() { return isEnabled('moderation'); }

module.exports = {
  getEnabledFeatures,
  isEnabled,
  isChatEnabled,
  isIdentityEnabled,
  isProfilesEnabled,
  isFeedsEnabled,
  isConnectionsEnabled,
  isModerationEnabled,
};
