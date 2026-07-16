'use strict';

/**
 * Mosaic mDNS/LAN Discovery (Phase 6)
 *
 * Broadcasts the Mosaic node's presence on the local network using
 * multicast DNS (mDNS / RFC 6762) so other Mosaic instances on the
 * same LAN segment can discover each other automatically.
 *
 * Service type: _mosaic._tcp.local
 * TXT record carries the node's Ed25519 pubkey and protocol version.
 *
 * The `multicast-dns` package is an optional dependency — if it is
 * not installed, start() logs a warning and is a no-op.
 */

const crypto = require('crypto');

let mdns = null;
let browser = null;
let peers = new Map();   // name -> { host, port, pubkey }
const listeners = new Set();

// ─── mDNS helpers ──────────────────────────────────────────

/**
 * Start advertising this Mosaic node on the LAN.
 *
 * @param {object} opts
 * @param {number} opts.port       - HTTP server port (advertised in SRV)
 * @param {string} opts.pubkey     - Node's Ed25519 public key
 * @param {string} [opts.host]     - Hostname to advertise (default: os.hostname())
 * @param {function} [opts.onError] - Error callback
 */
function start(opts) {
  opts = opts || {};
  if (!opts.port) throw new Error('transport-lan: port is required');
  if (!opts.pubkey) throw new Error('transport-lan: pubkey is required');

  let multicastDns;
  try {
    multicastDns = require('multicast-dns');
  } catch {
    console.warn('[transport-lan] multicast-dns not available. Install: npm install multicast-dns');
    return;
  }

  const host = opts.host || require('os').hostname();
  const serviceName = `mosaic-${crypto.randomBytes(4).toString('hex')}`;

  try {
    mdns = multicastDns();

    // Advertise service
    mdns.on('ready', () => {
      try {
        if (typeof mdns.registerService === 'function') {
          mdns.registerService({
            name: serviceName,
            host: host,
            port: opts.port,
            type: 'mosaic',
            protocol: 'tcp',
            txt: {
              pubkey: opts.pubkey,
              proto: 'v1',
              node: host,
            },
          });
        } else {
          // Fallback: multicast-dns v7+ uses a different API.
          // For full mDNS-SD support, use bonjour-service package instead.
          console.log('[transport-lan] registerService not available (use bonjour-service for full mDNS)');
        }
      } catch (e) {
        console.warn(`[transport-lan] mDNS advertisement error: ${e.message}`);
      }
    });

    // Discover peers
    browser = mdns;
    browser.on('service', (service) => {
      // Only respond to _mosaic._tcp services
      if (service.type !== 'mosaic' || service.protocol !== 'tcp') return;
      if (!service.txt || !service.txt.pubkey) return;

      // Skip self
      if (service.txt.pubkey === opts.pubkey) return;

      const name = service.fullname || service.name;
      const existing = peers.get(name);
      const now = Date.now();

      peers.set(name, {
        host: service.host,
        port: service.port,
        pubkey: service.txt.pubkey,
        proto: service.txt.proto || 'v1',
        node: service.txt.node || '',
        firstSeen: existing ? existing.firstSeen : now,
        lastSeen: now,
      });

      // Notify callbacks
      const peerInfo = peers.get(name);
      for (const cb of listeners) {
        try { cb(peerInfo); } catch { /* swallow */ }
      }
    });

    // Query for Mosaic services periodically
    const interval = setInterval(() => {
      if (mdns) {
        mdns.query({ type: 'mosaic', protocol: 'tcp' });
      } else {
        clearInterval(interval);
      }
    }, 30000);

    // Send initial query
    mdns.query({ type: 'mosaic', protocol: 'tcp' });

  } catch (err) {
    console.error('[transport-lan] mDNS setup failed:', err.message);
    if (opts.onError) opts.onError(err);
  }
}

/**
 * Stop advertising and browsing.
 */
function stop() {
  if (mdns) {
    try { mdns.destroy(); } catch { /* ignore */ }
    mdns = null;
  }
  peers.clear();
}

/**
 * Register a callback for peer discovery events.
 *
 * @param {function} callback - Called with { host, port, pubkey, proto, node, firstSeen, lastSeen }
 * @returns {function} Unsubscribe function
 */
function onPeer(callback) {
  if (typeof callback !== 'function') throw new Error('onPeer requires a function');
  listeners.add(callback);
  // Fire for already-discovered peers
  for (const peer of peers.values()) {
    try { callback(peer); } catch { /* swallow */ }
  }
  return () => listeners.delete(callback);
}

/**
 * Get the list of currently discovered LAN peers.
 *
 * @returns {object[]}
 */
function listPeers() {
  return Array.from(peers.values());
}

module.exports = {
  start,
  stop,
  onPeer,
  listPeers,
};
