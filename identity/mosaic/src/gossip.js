'use strict';

/**
 * Mosaic P2P Gossip (Phase 6)
 *
 * WebSocket-based peer-to-peer gossip for exchanging signed event logs
 * between Mosaic nodes.  Uses Ed25519 challenge-response authentication
 * on connect, then exchanges event batches in a subscribe/replicate
 * pattern (inspired by atproto's subscribeRepos).
 *
 * The `ws` package is a transitive dependency of Socket.IO but is listed
 * as optional in Mosaic's package.json for clarity.
 *
 * Architecture:
 *   GossipServer  — runs alongside the HTTP server, accepts WS connections
 *   GossipClient  — connects to remote peers, handles reconnection + sync
 *
 * Handshake:
 *   1. Client opens WS to peer
 *   2. Server sends { challenge: '<random_nonce>' }
 *   3. Client responds { pubkey: '<ed25519:...>', signature: '<signed_challenge>' }
 *   4. Server verifies signature against pubkey
 *   5. Both exchange { type: 'sync_request', pubkeys: [...], from_seq: {...} }
 *   6. Exchange event batches
 *   7. Switch to streaming mode (new events pushed in real time)
 */

const crypto = require('crypto');
const { verify, sign, fingerprint } = require('./identity');
const { verifyEvent, hashEvent } = require('./events');
const { queryEventsSince, queryLatestSeq, appendToLog } = require('./event-log');

// ─── Constants ──────────────────────────────────────────────

const RECONNECT_BASE_MS = 1000;
const RECONNECT_MAX_MS = 60000;
const BACKOFF_MULTIPLIER = 2;
const CHALLENGE_BYTES = 32;
const SYNC_BATCH_SIZE = 100;

// ─── GossipServer ──────────────────────────────────────────

/**
 * Create a WebSocket gossip server that mounts alongside Socket.IO.
 *
 * @param {object} httpServer - Node.js http.Server instance
 * @param {object} [opts]
 * @param {function} [opts.onPeer] - Called when a new peer authenticates: (pubkey, ws)
 * @param {function} [opts.onEvent] - Called when an event arrives from a peer: (event, pubkey)
 * @returns {{ close: function, peers: function }}
 */
function GossipServer(httpServer, opts) {
  opts = opts || {};
  let wss = null;

  try {
    const WebSocketServer = require('ws').Server;
    wss = new WebSocketServer({ server: httpServer, path: '/mosaic/gossip' });

    wss.on('connection', (ws, req) => {
      let authenticated = false;
      let peerPubkey = null;
      const challenge = crypto.randomBytes(CHALLENGE_BYTES).toString('hex');

      // Step 1: Send challenge
      ws.send(JSON.stringify({ type: 'challenge', challenge }));

      const closeHandler = () => {
        if (peerPubkey && opts.onPeer) {
          opts.onPeer(peerPubkey, null); // signal disconnect
        }
      };

      ws.on('message', (raw) => {
        let msg;
        try {
          msg = JSON.parse(raw.toString());
        } catch {
          ws.send(JSON.stringify({ type: 'error', message: 'invalid JSON' }));
          return;
        }

        if (!authenticated) {
          // Step 2–4: Authenticate
          if (msg.type !== 'auth_response' || !msg.pubkey || !msg.signature) {
            ws.send(JSON.stringify({ type: 'error', message: 'expected auth_response with pubkey + signature' }));
            ws.close();
            return;
          }

          const isValid = verify(challenge, msg.signature, msg.pubkey);
          if (!isValid) {
            ws.send(JSON.stringify({ type: 'error', message: 'signature verification failed' }));
            ws.close();
            return;
          }

          authenticated = true;
          peerPubkey = msg.pubkey;
          ws.send(JSON.stringify({ type: 'auth_ok', pubkey: msg.pubkey }));
          console.log(`[gossip] peer authenticated: ${fingerprint(peerPubkey)}`);

          if (opts.onPeer) opts.onPeer(peerPubkey, ws);
          return;
        }

        // Handle authenticated messages
        handleMessage(ws, peerPubkey, msg, opts);
      });

      ws.on('close', closeHandler);
      ws.on('error', closeHandler);
    });
  } catch (err) {
    console.error('[gossip] WebSocket server setup failed:', err.message);
    console.error('[gossip] Install optional dep: npm install ws');
  }

  return {
    /**
     * Close the gossip server.
     */
    close() {
      if (wss) {
        wss.close();
        wss = null;
      }
    },

    /**
     * Get a list of currently connected peer pubkeys.
     * @returns {string[]}
     */
    peers() {
      if (!wss) return [];
      const connected = [];
      wss.clients.forEach((client) => {
        if (client._peerPubkey) connected.push(client._peerPubkey);
      });
      return connected;
    },
  };
}

// ─── Message handler ────────────────────────────────────────

function handleMessage(ws, peerPubkey, msg, opts) {
  switch (msg.type) {
    case 'sync_request': {
      // Peer wants events since their last cursor
      const pubkeys = msg.pubkeys || [];
      const fromSeq = msg.from_seq || {};
      const responses = [];

      for (const pubkey of pubkeys) {
        const since = fromSeq[pubkey] || 0;
        const events = queryEventsSince(pubkey, since);
        responses.push(...events.map(e => ({
          type: 'event',
          event: e,
        })));
      }

      // Send in batches
      for (let i = 0; i < responses.length; i += SYNC_BATCH_SIZE) {
        const batch = responses.slice(i, i + SYNC_BATCH_SIZE);
        ws.send(JSON.stringify({ type: 'sync_batch', events: batch }));
      }

      // Signal end of sync
      ws.send(JSON.stringify({ type: 'sync_done' }));
      break;
    }

    case 'event': {
      // Incoming event from peer
      if (!msg.event) return;
      const event = msg.event;
      const { pubkey } = event;

      // Verify before accepting
      const verification = verifyEvent(event, (payload, sig, pk) => verify(payload, sig, pk));
      if (!verification.valid) {
        console.warn(`[gossip] invalid event from ${fingerprint(peerPubkey)}: ${verification.reason}`);
        return;
      }

      // Append to local log
      const seq = appendToLog(event);
      if (seq && opts.onEvent) {
        opts.onEvent(event, peerPubkey);
      }
      break;
    }

    case 'ping':
      ws.send(JSON.stringify({ type: 'pong' }));
      break;

    default:
      break;
  }
}

// ─── GossipClient ───────────────────────────────────────────

/**
 * Connect to a remote Mosaic gossip peer.
 *
 * Handles:
 *   - Ed25519 challenge-response auth
 *   - Exponential backoff reconnection (1s → 2s → 4s → … → 60s max)
 *   - Initial sync request for missing events
 *   - Real-time event streaming after sync
 *
 * @param {string}  peerUrl   - WebSocket URL, e.g. 'ws://192.168.1.50:3000/mosaic/gossip'
 * @param {object}  keypair   - { pubkey, privkey } of the local identity
 * @param {object}  [opts]
 * @param {function} [opts.onEvent] - Called when an event arrives: (event)
 * @param {function} [opts.onStatus] - Called on status changes: ('connected'|'disconnected'|'reconnecting')
 * @returns {{ close: function }}
 */
function GossipClient(peerUrl, keypair, opts) {
  opts = opts || {};
  let ws = null;
  let backoff = RECONNECT_BASE_MS;
  let closed = false;
  let reconnectTimer = null;

  function connect() {
    if (closed) return;

    let WebSocket;
    try {
      WebSocket = require('ws');
    } catch {
      console.error('[gossip-client] ws package not available. Install: npm install ws');
      return;
    }

    try {
      ws = new WebSocket(peerUrl);
    } catch (err) {
      console.error(`[gossip-client] connection failed: ${err.message}`);
      scheduleReconnect();
      return;
    }

    let authenticated = false;
    let syncDone = false;

    ws.on('open', () => {
      console.log(`[gossip-client] connected to ${peerUrl}`);
      if (opts.onStatus) opts.onStatus('connected');
    });

    ws.on('message', (raw) => {
      let msg;
      try {
        msg = JSON.parse(raw.toString());
      } catch {
        return;
      }

      if (!authenticated) {
        if (msg.type === 'challenge') {
          // Sign the challenge with our identity
          const sig = sign(msg.challenge, keypair.privkey);
          ws.send(JSON.stringify({
            type: 'auth_response',
            pubkey: keypair.pubkey,
            signature: sig,
          }));
        } else if (msg.type === 'auth_ok') {
          authenticated = true;
          backoff = RECONNECT_BASE_MS; // reset backoff on successful auth
          // Request sync for all known pubkeys
          sendSyncRequest();
        } else if (msg.type === 'error') {
          console.error(`[gossip-client] server error: ${msg.message}`);
          ws.close();
        }
        return;
      }

      // Authenticated message handling
      if (msg.type === 'sync_batch') {
        for (const item of (msg.events || [])) {
          if (item.type === 'event' && item.event) {
            const event = item.event;
            const verification = verifyEvent(event, (payload, sig, pk) => verify(payload, sig, pk));
            if (verification.valid) {
              appendToLog(event);
              if (opts.onEvent) opts.onEvent(event);
            }
          }
        }
      } else if (msg.type === 'sync_done') {
        syncDone = true;
        console.log(`[gossip-client] sync complete for ${peerUrl}`);
      } else if (msg.type === 'event') {
        // Real-time event from peer
        if (!msg.event) return;
        const event = msg.event;
        const verification = verifyEvent(event, (payload, sig, pk) => verify(payload, sig, pk));
        if (verification.valid) {
          appendToLog(event);
          if (opts.onEvent) opts.onEvent(event);
        }
      } else if (msg.type === 'pong') {
        // heartbeat response
      }
    });

    ws.on('close', () => {
      authenticated = false;
      syncDone = false;
      ws = null;
      if (opts.onStatus) opts.onStatus('disconnected');
      scheduleReconnect();
    });

    ws.on('error', (err) => {
      console.error(`[gossip-client] websocket error: ${err.message}`);
      // close event will fire next and trigger reconnect
    });
  }

  function sendSyncRequest() {
    if (!ws || ws.readyState !== 1) return;
    // Request all known pubkeys from seq 0 (full sync)
    // In a more advanced implementation, we'd track per-peer cursors
    const pubkeys = [keypair.pubkey]; // also request known contacts
    ws.send(JSON.stringify({
      type: 'sync_request',
      pubkeys,
      from_seq: {},
    }));
  }

  function scheduleReconnect() {
    if (closed) return;
    if (reconnectTimer) clearTimeout(reconnectTimer);
    const delay = Math.min(backoff, RECONNECT_MAX_MS);
    console.log(`[gossip-client] reconnecting in ${delay}ms...`);
    if (opts.onStatus) opts.onStatus('reconnecting');
    reconnectTimer = setTimeout(() => {
      backoff = Math.min(backoff * BACKOFF_MULTIPLIER, RECONNECT_MAX_MS);
      connect();
    }, delay);
  }

  // Start connection
  connect();

  return {
    /**
     * Gracefully close the connection and stop reconnecting.
     */
    close() {
      closed = true;
      if (reconnectTimer) {
        clearTimeout(reconnectTimer);
        reconnectTimer = null;
      }
      if (ws) {
        try { ws.close(); } catch { /* ignore */ }
        ws = null;
      }
    },

    /**
     * Send a raw event to the peer in real-time streaming mode.
     * @param {object} event - Signed event
     */
    sendEvent(event) {
      if (ws && ws.readyState === 1 /* OPEN */) {
        ws.send(JSON.stringify({ type: 'event', event }));
      }
    },
  };
}

module.exports = {
  GossipServer,
  GossipClient,
};
