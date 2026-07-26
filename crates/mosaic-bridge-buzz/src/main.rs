//! mosaic-bridge-buzz — Nostr WebSocket relay bridge for Mosaic ↔ Nostr.
//!
//! Connects to a Nostr relay via WebSocket, subscribes to tracked npubs,
//! and relays events between Mosaic and the Nostr network.
//!
//! Environment variables:
//!   BUZZ_RELAY_URL  — Nostr relay WebSocket URL (default: wss://relay.damus.io)
//!   MIS_URL         — Mosaic Identity Service base URL (default: http://localhost:8081)
//!   TRACKED_NPUBS   — Comma-separated list of npubs to track
//!
//! On startup, probes MIS for all keys and their bindings to discover
//! Nostr-bound identities. Subscribes to kinds 1, 7, and 9734.

use anyhow::{Context, Result};
use futures_util::{SinkExt, StreamExt};
use mosaic_client::MosaicClient;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::env;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{watch, Mutex};
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;
use tracing::{info, warn};

// ─── Configuration ───────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
struct Config {
    relay_url: String,
    tracked_npubs: Vec<String>,
}

impl Config {
    fn from_env() -> Self {
        let relay_url =
            env::var("BUZZ_RELAY_URL").unwrap_or_else(|_| "wss://relay.damus.io".to_string());
        let tracked_npubs = env::var("TRACKED_NPUBS")
            .unwrap_or_default()
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        Self {
            relay_url,
            tracked_npubs,
        }
    }
}

// ─── Nostr types (NIP-01) ───────────────────────────────────────────────────

/// A Nostr event as defined by NIP-01.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct NostrEvent {
    #[serde(default)]
    id: Option<String>,
    pubkey: String,
    created_at: u64,
    kind: u64,
    tags: Vec<Vec<String>>,
    content: String,
    #[serde(default)]
    sig: Option<String>,
}

impl NostrEvent {
    /// Compute the NIP-01 event ID: SHA-256 of the canonical serialization.
    ///
    /// Per NIP-01, the serialization format is:
    ///   `[0, "<pubkey>", <created_at>, <kind>, <tags>, "<content>"]`
    fn compute_id(&self) -> String {
        let tags_json = serde_json::to_string(&self.tags).expect("tags always serializable");
        let content_json =
            serde_json::to_string(&self.content).expect("content always serializable");
        let canonical = format!(
            "[0,\"{}\",{},{},{},{}]",
            self.pubkey, self.created_at, self.kind, tags_json, content_json
        );
        let mut hasher = Sha256::new();
        hasher.update(canonical.as_bytes());
        hex::encode(hasher.finalize())
    }
}

/// NIP-01 relay message types.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
enum RelayMessage {
    Event {
        #[allow(dead_code)]
        subscription_id: String,
        event: NostrEvent,
    },
    Notice {
        message: String,
    },
    Eose {
        subscription_id: String,
    },
    Ok {
        event_id: String,
        ok: bool,
        message: String,
    },
    Other(Vec<serde_json::Value>),
}

// ─── MIS probing ─────────────────────────────────────────────────────────────

/// Probe MIS for Nostr-bound identities, returning their hex public keys.
async fn probe_mis_nostr_identities(mis: &MosaicClient) -> Vec<String> {
    info!("[BUZZ] Probing MIS for Nostr-bound identities...");

    let keys = match mis.list_keys().await {
        Ok(keys) => keys,
        Err(e) => {
            warn!("[BUZZ] Failed to list MIS keys: {e}. Will retry on reconnect.");
            return Vec::new();
        }
    };

    info!("[BUZZ] Found {} keys in MIS", keys.len());
    let mut nostr_hex_pubkeys = Vec::new();

    for key in &keys {
        match mis.get_key_bindings(&key.key_id).await {
            Ok(bindings) => {
                for binding in &bindings.bindings {
                    if binding.protocol == "nostr" {
                        let short = &binding.external_id[..binding.external_id.len().min(12)];
                        info!(
                            "[BUZZ] Bound identity: key={} → nostr:{}...",
                            key.key_id, short
                        );
                        nostr_hex_pubkeys.push(binding.external_id.clone());
                    }
                }
            }
            Err(e) => {
                warn!("[BUZZ] Failed to get bindings for key {}: {e}", key.key_id);
            }
        }
    }

    if nostr_hex_pubkeys.is_empty() {
        info!("[BUZZ] No Nostr-bound identities found via MIS probe");
    } else {
        info!(
            "[BUZZ] Found {} Nostr-bound identity/identities",
            nostr_hex_pubkeys.len()
        );
    }

    nostr_hex_pubkeys
}

/// Resolve an npub to hex via MIS.
async fn resolve_npub_to_hex(mis: &MosaicClient, npub: &str) -> Option<String> {
    match mis.resolve_npub(npub).await {
        Ok(resp) => {
            let short = &resp.hex_pubkey[..resp.hex_pubkey.len().min(12)];
            info!("[BUZZ] Resolved npub {npub} → hex {short}...");
            Some(resp.hex_pubkey)
        }
        Err(e) => {
            warn!("[BUZZ] Failed to resolve npub {npub}: {e}");
            None
        }
    }
}

// ─── Relay client ────────────────────────────────────────────────────────────

/// Manages a WebSocket connection to a Nostr relay.
struct NostrRelayClient {
    config: Config,
    subscribed_keys: Arc<Mutex<HashSet<String>>>,
}

impl NostrRelayClient {
    fn new(config: Config) -> Self {
        Self {
            config,
            subscribed_keys: Arc::new(Mutex::new(HashSet::new())),
        }
    }

    /// Build a NIP-01 REQ message subscribing to the given hex pubkeys.
    fn make_subscription(&self, hex_pubkeys: &[String]) -> serde_json::Value {
        let subscription_id = format!("mosaic-bridge-{}", unix_now());
        serde_json::json!([
            "REQ",
            subscription_id,
            {
                "kinds": [1, 7, 9734],
                "authors": hex_pubkeys,
                "limit": 10
            }
        ])
    }

    /// Run the bridge loop with reconnection.
    async fn run(&self, shutdown: &mut watch::Receiver<bool>) {
        loop {
            // Create MIS client
            let mis = MosaicClient::from_env();

            // Gather tracked pubkeys: resolve env vars + probe MIS
            let mut tracked_hex: Vec<String> = Vec::new();

            for npub in &self.config.tracked_npubs {
                if let Some(hex) = resolve_npub_to_hex(&mis, npub).await {
                    if !tracked_hex.contains(&hex) {
                        tracked_hex.push(hex);
                    }
                }
            }

            let mis_identities = probe_mis_nostr_identities(&mis).await;
            for hex in mis_identities {
                if !tracked_hex.contains(&hex) {
                    tracked_hex.push(hex);
                }
            }

            if tracked_hex.is_empty() {
                info!("[BUZZ] No tracked pubkeys configured. Bridge will connect without subscriptions.");
            } else {
                info!("[BUZZ] Tracking {} pubkey(s)", tracked_hex.len());
            }

            match self.connect_and_listen(&tracked_hex, shutdown).await {
                Ok(()) => {
                    info!("[BUZZ] Connection closed cleanly");
                    return; // graceful shutdown
                }
                Err(e) => {
                    warn!("[BUZZ] Connection error: {e:#}");
                    if *shutdown.borrow() {
                        return;
                    }
                    info!("[BUZZ] Reconnecting in 5 seconds...");
                    tokio::time::sleep(Duration::from_secs(5)).await;
                }
            }
        }
    }

    /// Connect to the relay, subscribe, and process messages.
    async fn connect_and_listen(
        &self,
        hex_pubkeys: &[String],
        shutdown: &mut watch::Receiver<bool>,
    ) -> Result<()> {
        let url = url::Url::parse(&self.config.relay_url)
            .with_context(|| format!("Invalid relay URL: {}", self.config.relay_url))?;

        info!("[BUZZ] Connecting to relay: {}", self.config.relay_url);
        let (ws_stream, _) = connect_async(url.as_str())
            .await
            .context("WebSocket connection failed")?;

        info!("[BUZZ] Connected to relay");
        let (mut write, mut read) = ws_stream.split();

        // Subscribe if we have pubkeys
        if !hex_pubkeys.is_empty() {
            let sub = self.make_subscription(hex_pubkeys);
            let sub_msg =
                serde_json::to_string(&sub).context("Failed to serialize subscription")?;
            info!(
                "[BUZZ] Subscribing to {} pubkeys (kinds: 1, 7, 9734)",
                hex_pubkeys.len()
            );
            write
                .send(Message::Text(sub_msg.into()))
                .await
                .context("Failed to send subscription")?;

            let mut subs = self.subscribed_keys.lock().await;
            for hex in hex_pubkeys {
                subs.insert(hex.clone());
            }
        }

        // Process messages
        loop {
            tokio::select! {
                msg = read.next() => {
                    match msg {
                        Some(Ok(Message::Text(text))) => {
                            self.handle_relay_message(&text);
                        }
                        Some(Ok(Message::Ping(data))) => {
                            if let Err(e) = write.send(Message::Pong(data)).await {
                                warn!("[BUZZ] Failed to send pong: {e}");
                                break;
                            }
                        }
                        Some(Ok(Message::Close(_))) => {
                            info!("[BUZZ] Relay closed connection");
                            break;
                        }
                        Some(Err(e)) => {
                            return Err(anyhow::anyhow!("WebSocket error: {e}"));
                        }
                        None => {
                            info!("[BUZZ] WebSocket stream ended");
                            break;
                        }
                        _ => {} // binary, frame, etc.
                    }
                }
                _ = shutdown.changed() => {
                    if *shutdown.borrow() {
                        info!("[BUZZ] Shutdown signal received, closing connection");
                        let _ = write.send(Message::Close(None)).await;
                        return Ok(());
                    }
                }
            }
        }

        Ok(())
    }

    /// Handle an incoming relay text message (NIP-01).
    fn handle_relay_message(&self, text: &str) {
        match serde_json::from_str::<RelayMessage>(text) {
            Ok(RelayMessage::Event { event, .. }) => {
                let computed_id = event.compute_id();
                let id_matches = match &event.id {
                    Some(id) => id == &computed_id,
                    None => true,
                };

                let event_id = event.id.clone().unwrap_or_else(|| computed_id.clone());
                let short_id = &event_id[..event_id.len().min(12)];
                let short_pk = &event.pubkey[..event.pubkey.len().min(12)];
                let content_preview: String = event
                    .content
                    .chars()
                    .take(80)
                    .collect::<String>()
                    .replace('\n', " ");

                info!(
                    "[BUZZ] EVENT id={} pubkey={} kind={} content=\"{}\"",
                    short_id, short_pk, event.kind, content_preview
                );

                if !id_matches {
                    let computed_short = &computed_id[..computed_id.len().min(12)];
                    if let Some(reported_id) = &event.id {
                        info!(
                            "[BUZZ] Event ID mismatch: relay={reported_id}, computed={computed_short}"
                        );
                    }
                    info!("[BUZZ] Using computed event ID: {computed_short}");
                }
            }
            Ok(RelayMessage::Notice { message }) => {
                info!("[NOSTR NOTICE] {message}");
            }
            Ok(RelayMessage::Eose { subscription_id }) => {
                info!("[NOSTR EOSE] End of stored events for sub {subscription_id}");
            }
            Ok(RelayMessage::Ok {
                event_id,
                ok,
                message,
            }) => {
                let short = &event_id[..event_id.len().min(12)];
                if ok {
                    info!("[NOSTR OK] Event {short} accepted");
                } else {
                    warn!("[NOSTR OK] Event {short} rejected: {message}");
                }
            }
            Ok(RelayMessage::Other(vals)) => {
                if let Some(first) = vals.first().and_then(|v| v.as_str()) {
                    if first != "UNKNOWN" {
                        info!("[BUZZ] Unhandled relay message type: {first}");
                    }
                }
            }
            Err(_) => {
                // Non-JSON (keepalive pings etc.)
            }
        }
    }
}

// ─── Utilities ───────────────────────────────────────────────────────────────

fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

// ─── Signal handling ─────────────────────────────────────────────────────────

async fn signal_listener(shutdown_tx: watch::Sender<bool>) {
    let mut term = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        .expect("failed to register SIGTERM handler");
    let mut int = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::interrupt())
        .expect("failed to register SIGINT handler");

    tokio::select! {
        _ = term.recv() => info!("[BUZZ] Received SIGTERM"),
        _ = int.recv() => info!("[BUZZ] Received SIGINT"),
    }

    let _ = shutdown_tx.send(true);
}

// ─── Main ────────────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();

    info!("=== Buzz Transport Plugin (Rust) ===");

    let config = Config::from_env();
    info!("Relay: {}", config.relay_url);
    if config.tracked_npubs.is_empty() {
        info!("Tracked npubs: (none — using MIS probe)");
    } else {
        info!("Tracked npubs: {}", config.tracked_npubs.join(", "));
    }

    let (shutdown_tx, mut shutdown_rx) = watch::channel(false);

    let sig_tx = shutdown_tx.clone();
    tokio::spawn(async move {
        signal_listener(sig_tx).await;
    });

    let client = NostrRelayClient::new(config);
    client.run(&mut shutdown_rx).await;

    info!("[BUZZ] Shutdown complete");
    Ok(())
}
