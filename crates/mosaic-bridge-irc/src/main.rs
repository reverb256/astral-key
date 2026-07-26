//! mosaic-bridge-irc — Mosaic ↔ IRC bridge daemon.
//!
//! Connects to an IRC server over TLS, joins configured channels,
//! and relays messages bidirectionally between IRC and the Mosaic
//! event bus.
//!
//! # Environment
//!
//! | Variable       | Default                  | Description                        |
//! |----------------|--------------------------|------------------------------------|
//! | `MIS_URL`      | `http://localhost:8081`   | Mosaic Identity Service base URL   |
//! | `IRC_SERVER`   | `irc.libera.chat`        | IRC server hostname                |
//! | `IRC_PORT`     | `6697`                   | IRC server port (TLS)              |
//! | `IRC_NICK`     | `MosaicBridge`           | IRC nickname                       |
//! | `IRC_SASL_USER`| *(none)*                 | SASL PLAIN username (optional)     |
//! | `IRC_SASL_PASS`| *(none)*                 | SASL PLAIN password (optional)     |
//! | `IRC_CHANNELS` | *(none)*                 | Comma-separated channels to join   |

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use base64::Engine as _;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;
use tokio::sync::{broadcast, mpsc, watch, RwLock};
use tracing::{debug, error, info, warn};

// Re-export the broadcast channel sender type for external consumers.
pub use broadcast::Sender as BroadcastSender;

// ─── Configuration ──────────────────────────────────────────────────────────

/// All configuration is loaded from environment variables with sensible defaults.
#[derive(Debug, Clone)]
struct Config {
    mis_url: String,
    irc_server: String,
    irc_port: u16,
    irc_nick: String,
    irc_sasl_user: Option<String>,
    irc_sasl_pass: Option<String>,
    irc_channels: Vec<String>,
}

impl Config {
    fn from_env() -> Self {
        Self {
            mis_url: std::env::var("MIS_URL")
                .unwrap_or_else(|_| "http://localhost:8081".to_string()),
            irc_server: std::env::var("IRC_SERVER")
                .unwrap_or_else(|_| "irc.libera.chat".to_string()),
            irc_port: std::env::var("IRC_PORT")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(6697),
            irc_nick: std::env::var("IRC_NICK").unwrap_or_else(|_| "MosaicBridge".to_string()),
            irc_sasl_user: std::env::var("IRC_SASL_USER")
                .ok()
                .filter(|s| !s.is_empty()),
            irc_sasl_pass: std::env::var("IRC_SASL_PASS")
                .ok()
                .filter(|s| !s.is_empty()),
            irc_channels: std::env::var("IRC_CHANNELS")
                .ok()
                .map(|s| {
                    s.split(',')
                        .map(|c| c.trim().to_string())
                        .filter(|c| !c.is_empty())
                        .collect()
                })
                .unwrap_or_default(),
        }
    }
}

// ─── Shared types ───────────────────────────────────────────────────────────

/// A message from the Mosaic event bus destined for an IRC channel.
#[derive(Debug, Clone)]
pub struct MosaicToIrcMessage {
    /// Mosaic channel code (e.g. "libera-mychannel")
    pub channel_code: String,
    /// Message text body
    pub text: String,
    /// Display name of the author
    pub author: String,
}

/// An event originating from IRC to be forwarded to Mosaic.
#[derive(Debug, Clone)]
pub struct IrcEvent {
    /// Nick of the sender
    pub nick: String,
    /// IRC channel (e.g. "#mychannel")
    pub channel: String,
    /// Message text
    pub text: String,
}

/// A parsed IRC message.
#[derive(Debug)]
struct IrcMessage {
    /// Optional prefix (sender or server)
    prefix: Option<String>,
    /// IRC command (e.g. PRIVMSG, PING, 001)
    command: String,
    /// Raw params before the trailing `:...`
    params: String,
    /// Trailing message after `:...`
    trailing: Option<String>,
}

/// Compute a Mosaic channel code from an IRC channel name.
///
/// Example: `#libera-project` → `libera-project`
fn irc_channel_to_code(ch: &str) -> String {
    ch.trim_start_matches('#')
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '-' || *c == '_')
        .collect()
}

// ─── IRC message parser ─────────────────────────────────────────────────────

/// Parse a raw IRC line into a structured `IrcMessage`.
///
/// Handles both tagged (`:sender!user@host COMMAND ...`) and
/// untagged (`PING :...`) message formats.
fn parse_irc_line(raw: &str) -> Option<IrcMessage> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }

    let mut rest = trimmed;

    // Extract optional prefix (starts with ':')
    let prefix = if rest.starts_with(':') {
        // Find the first space after the prefix
        let space = rest[1..].find(' ')?;
        let p = rest[1..=space].trim_end().to_string();
        rest = rest[space + 1..].trim_start();
        Some(p)
    } else {
        None
    };

    // Extract command
    let space = rest.find(' ').unwrap_or(rest.len());
    let command = rest[..space].to_string();
    rest = rest[space..].trim_start();

    // Split params and trailing (delimited by " :")
    let (params, trailing) = if let Some(pos) = rest.find(" :") {
        (
            rest[..pos].trim().to_string(),
            Some(rest[pos + 2..].to_string()),
        )
    } else if let Some(trailing_text) = rest.strip_prefix(':') {
        // Handle "PING :server" style (no space before colon)
        (String::new(), Some(trailing_text.to_string()))
    } else {
        (rest.trim().to_string(), None)
    };

    Some(IrcMessage {
        prefix,
        command,
        params,
        trailing,
    })
}

// ─── TLS connection ─────────────────────────────────────────────────────────

/// Create a TLS connector backed by the system webpki root certificates.
fn build_tls_connector() -> Result<tokio_rustls::TlsConnector> {
    let mut root_certs = tokio_rustls::rustls::RootCertStore::empty();
    // Load Mozilla's root CA bundle via webpki-roots
    root_certs.extend(webpki_roots::TLS_SERVER_ROOTS.iter().map(|ta| ta.clone()));

    let config = tokio_rustls::rustls::ClientConfig::builder()
        .with_root_certificates(root_certs)
        .with_no_client_auth();

    Ok(tokio_rustls::TlsConnector::from(Arc::new(config)))
}

/// Connect to an IRC server via TLS.
async fn connect_tls(server: &str, port: u16) -> Result<tokio_rustls::TlsStream<TcpStream>> {
    let addr = format!("{}:{}", server, port);
    info!("Connecting to {} via TLS...", addr);

    let stream = TcpStream::connect(&addr)
        .await
        .with_context(|| format!("Failed to TCP connect to {}", addr))?;

    // Resolve the DNS name for TLS SNI
    let server_name = tokio_rustls::rustls::pki_types::ServerName::try_from(server.to_owned())
        .map_err(|_| anyhow::anyhow!("Invalid DNS server name: {}", server))?;

    let connector = build_tls_connector()?;
    let tls_stream = connector
        .connect(server_name, stream)
        .await
        .with_context(|| format!("TLS handshake failed for {}", addr))?;

    info!("TLS connection established to {}", addr);
    Ok(tokio_rustls::TlsStream::Client(tls_stream))
}

// ─── IRC protocol helpers ──────────────────────────────────────────────────

/// Send a raw line to the IRC server (appends CRLF).
async fn irc_send(writer: &mut (impl AsyncWriteExt + Unpin), line: &str) -> Result<()> {
    let raw = format!("{}\r\n", line);
    debug!("[IRC →] {}", line);
    writer.write_all(raw.as_bytes()).await?;
    Ok(())
}

/// Register with NICK and USER.
async fn irc_register(writer: &mut (impl AsyncWriteExt + Unpin), nick: &str) -> Result<()> {
    irc_send(writer, "CAP LS 302").await?;
    irc_send(writer, &format!("NICK {}", nick)).await?;
    irc_send(writer, &format!("USER {} 0 * :Mosaic Bridge", nick)).await?;
    Ok(())
}

/// Perform SASL PLAIN authentication.
async fn irc_sasl_auth(
    writer: &mut (impl AsyncWriteExt + Unpin),
    user: &str,
    pass: &str,
) -> Result<()> {
    info!("Attempting SASL PLAIN authentication...");

    // Request SASL capability
    irc_send(writer, "CAP REQ :sasl").await?;

    // Encode PLAIN auth string: \0username\0password
    let auth_str = format!("\0{}\0{}", user, pass);
    let encoded = base64::engine::general_purpose::STANDARD.encode(auth_str.as_bytes());

    irc_send(writer, "AUTHENTICATE PLAIN").await?;
    // The server will respond with AUTHENTICATE +, then we send the blob
    irc_send(writer, &format!("AUTHENTICATE {}", encoded)).await?;
    // After auth, send CAP END to finish capability negotiation
    irc_send(writer, "CAP END").await?;

    info!("SASL PLAIN credentials sent");
    Ok(())
}

/// Join a single IRC channel.
async fn irc_join(writer: &mut (impl AsyncWriteExt + Unpin), channel: &str) -> Result<()> {
    info!("Joining channel {}", channel);
    irc_send(writer, &format!("JOIN {}", channel)).await?;
    Ok(())
}

/// Send a PRIVMSG to a target (channel or user).
async fn irc_privmsg(
    writer: &mut (impl AsyncWriteExt + Unpin),
    target: &str,
    text: &str,
) -> Result<()> {
    // Split multi-line messages and truncate each line to 400 chars
    for line in text.lines() {
        let truncated = if line.len() > 400 { &line[..400] } else { line };
        irc_send(writer, &format!("PRIVMSG {} :{}", target, truncated)).await?;
    }
    Ok(())
}

/// Respond to a PING with PONG.
async fn irc_pong(writer: &mut (impl AsyncWriteExt + Unpin), server: &str) -> Result<()> {
    irc_send(writer, &format!("PONG :{}", server)).await?;
    Ok(())
}

/// Send QUIT message and close the connection gracefully.
async fn irc_quit(writer: &mut (impl AsyncWriteExt + Unpin)) -> Result<()> {
    info!("Sending QUIT...");
    irc_send(writer, "QUIT :Bridge shutting down").await?;
    Ok(())
}

// ─── MIS probe ──────────────────────────────────────────────────────────────

/// Probe the Mosaic Identity Service on startup.
///
/// Checks MIS health and logs the result. A failed probe is non-fatal —
/// the bridge will start regardless but logs a warning.
async fn probe_mis(mis_url: &str) -> Result<()> {
    let health_url = format!("{}/health", mis_url.trim_end_matches('/'));
    info!("Probing MIS health at {}...", health_url);

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .context("Failed to build reqwest client for MIS probe")?;

    let resp = client
        .get(&health_url)
        .send()
        .await
        .with_context(|| format!("MIS health check failed at {}", health_url))?;

    if resp.status().is_success() {
        let body: serde_json::Value = resp
            .json()
            .await
            .context("MIS returned non-JSON health response")?;
        info!("MIS health check OK: {}", body);
    } else {
        warn!("MIS returned status {} for health check", resp.status());
    }

    Ok(())
}

// ─── IRC connection loop (with auto-reconnect) ─────────────────────────────

/// Run one IRC connection lifecycle: connect, register, authenticate,
/// join channels, and process messages until disconnect or shutdown.
async fn irc_connect_once(
    config: &Config,
    shutdown_rx: &mut watch::Receiver<bool>,
    mosaic_to_irc_rx: &mut broadcast::Receiver<MosaicToIrcMessage>,
    irc_to_mosaic_tx: &mpsc::Sender<IrcEvent>,
    channel_map: &Arc<RwLock<HashMap<String, String>>>,
) -> Result<()> {
    // ── TLS connect ────────────────────────────────────────────────────────
    let tls_stream = connect_tls(&config.irc_server, config.irc_port).await?;

    // Split into read/write halves so we can read and write concurrently.
    let (read_half, mut write_half) = tokio::io::split(tls_stream);
    let mut reader = BufReader::new(read_half);
    let mut line_buf = String::new();
    let mut registered = false;

    // ── Register ───────────────────────────────────────────────────────────
    irc_register(&mut write_half, &config.irc_nick).await?;

    // ── Main event loop ────────────────────────────────────────────────────
    loop {
        // Prepare the IRC read future
        line_buf.clear();
        let read_fut = reader.read_line(&mut line_buf);
        tokio::pin!(read_fut);

        // Prepare the shutdown / mosaic message futures
        let mut shutdown_check = shutdown_rx.clone();
        let shutdown_fut = shutdown_check.changed();
        tokio::pin!(shutdown_fut);

        let irc_msg_fut = mosaic_to_irc_rx.recv();
        tokio::pin!(irc_msg_fut);

        let items = tokio::select! {
            biased;

            // Shutdown signal takes highest priority
            _ = shutdown_fut.as_mut() => {
                if *shutdown_rx.borrow() {
                    let _ = irc_quit(&mut write_half).await;
                    return Ok(());
                }
                continue;
            }

            // Incoming IRC data
            result = read_fut.as_mut() => {
                match result {
                    Ok(0) => {
                        info!("IRC connection closed remotely");
                        return Ok(());
                    }
                    Ok(_n) => line_buf.clone(),
                    Err(e) => {
                        error!("IRC read error: {}", e);
                        return Err(e.into());
                    }
                }
            }

            // Mosaic → IRC message relay
            msg = irc_msg_fut.as_mut() => {
                match msg {
                    Ok(m) => {
                        // Look up the IRC channel for this Mosaic channel code
                        let map = channel_map.read().await;
                        let target = map.iter().find_map(|(irc_ch, code)| {
                            if *code == m.channel_code { Some(irc_ch.clone()) } else { None }
                        });
                        drop(map);

                        if let Some(target) = target {
                            let text = if m.author.is_empty() {
                                m.text
                            } else {
                                format!("<{}> {}", m.author, m.text)
                            };
                            if let Err(e) = irc_privmsg(&mut write_half, &target, &text).await {
                                warn!("Failed to relay message to IRC: {}", e);
                            }
                        } else {
                            debug!("No IRC channel mapping for Mosaic channel code '{}'", m.channel_code);
                        }
                    }
                    Err(broadcast::error::RecvError::Closed) => {
                        info!("Mosaic→IRC channel closed, shutting down IRC connection");
                        let _ = irc_quit(&mut write_half).await;
                        return Ok(());
                    }
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        warn!("Mosaic→IRC receiver lagged by {} messages", n);
                    }
                }
                continue;
            }
        };

        // ── Process the IRC line ───────────────────────────────────────────
        let raw_line = items;
        let trimmed = raw_line.trim();
        if trimmed.is_empty() {
            continue;
        }

        debug!("[IRC ←] {}", trimmed);

        // Handle PING specially (no prefix)
        if let Some(server) = trimmed.strip_prefix("PING :") {
            if let Err(e) = irc_pong(&mut write_half, server).await {
                warn!("Failed to send PONG: {}", e);
            }
            continue;
        }

        // Parse structured IRC message
        let Some(msg) = parse_irc_line(trimmed) else {
            debug!("Unparseable IRC line: {}", trimmed);
            continue;
        };

        // Handle numeric 001 (RPL_WELCOME) → registration confirmed
        if msg.command == "001" && !registered {
            registered = true;
            info!("Registered on IRC as {}", config.irc_nick);

            // Build channel map and join channels
            let mut map = channel_map.write().await;
            map.clear();
            for ch in &config.irc_channels {
                let code = irc_channel_to_code(ch);
                map.insert(ch.clone(), code);
                if let Err(e) = irc_join(&mut write_half, ch).await {
                    warn!("Failed to JOIN {}: {}", ch, e);
                }
            }
            drop(map);

            // SASL authentication (must happen during registration)
            if let (Some(user), Some(pass)) = (&config.irc_sasl_user, &config.irc_sasl_pass) {
                if let Err(e) = irc_sasl_auth(&mut write_half, user, pass).await {
                    warn!("SASL authentication failed (non-fatal): {}", e);
                }
            }

            // End capability negotiation
            let _ = irc_send(&mut write_half, "CAP END").await;

            continue;
        }

        // Handle SASL responses (900 = logged in, 903 = SASL success, 904 = failure)
        if msg.command == "900" || msg.command == "903" {
            info!("SASL authentication successful");
            continue;
        }
        if msg.command == "904" || msg.command == "905" || msg.command == "906" {
            warn!("SASL authentication failed (command={})", msg.command);
            continue;
        }

        // Handle CAP acknowledgements
        if msg.command == "CAP" {
            debug!(
                "CAP: {} {}",
                msg.params,
                msg.trailing.as_deref().unwrap_or("")
            );
            continue;
        }

        // Handle PRIVMSG from a channel
        if msg.command == "PRIVMSG" {
            let target = &msg.params; // Channel or user
            let text = msg.trailing.as_deref().unwrap_or("");

            // Only handle channel messages (starting with #)
            if target.starts_with('#') {
                let nick = msg
                    .prefix
                    .as_ref()
                    .and_then(|p| p.split('!').next())
                    .unwrap_or("unknown");

                debug!("[IRC ←] {} in {}: {}", nick, target, text);

                // Forward to the Mosaic event bus
                let event = IrcEvent {
                    nick: nick.to_string(),
                    channel: target.clone(),
                    text: text.to_string(),
                };

                // Non-blocking send; drop if the receiver is gone
                if let Err(e) = irc_to_mosaic_tx.try_send(event) {
                    match &e {
                        mpsc::error::TrySendError::Full(_) => {
                            warn!(
                                "IRC→Mosaic channel full; dropping message from {} in {}",
                                nick, target
                            );
                        }
                        mpsc::error::TrySendError::Closed(_) => {
                            // Channel closed during shutdown — expected
                        }
                    }
                }
            }
            continue;
        }

        // Handle JOIN acknowledgements
        if msg.command == "JOIN" {
            let channel = msg.trailing.as_deref().unwrap_or("");
            info!("Joined channel {}", channel);
            continue;
        }

        // Handle KICK
        if msg.command == "KICK" {
            let parts: Vec<&str> = msg.params.split_whitespace().collect();
            if parts.len() >= 2 && parts[1] == config.irc_nick {
                warn!("Kicked from {} (rejoining)", parts[0]);
                let _ = irc_join(&mut write_half, parts[0]).await;
            }
            continue;
        }

        // Handle other numerics silently
        if msg.command.chars().all(|c| c.is_ascii_digit()) && msg.command.len() == 3 {
            debug!(
                "Numeric {}: {} {}",
                msg.command,
                msg.params,
                msg.trailing.as_deref().unwrap_or("")
            );
            continue;
        }
    }
}

/// Run the IRC connection loop with automatic reconnection.
///
/// On disconnect, waits 10 seconds before attempting to reconnect.
/// Exits when the shutdown signal is received.
async fn irc_connection_loop(
    config: Config,
    mut shutdown_rx: watch::Receiver<bool>,
    mut mosaic_to_irc_rx: broadcast::Receiver<MosaicToIrcMessage>,
    irc_to_mosaic_tx: mpsc::Sender<IrcEvent>,
    channel_map: Arc<RwLock<HashMap<String, String>>>,
) {
    let reconnect_delay = Duration::from_secs(10);

    loop {
        // Check for shutdown signal before attempting connection
        if *shutdown_rx.borrow() {
            info!("Shutdown signal received; exiting connection loop");
            return;
        }

        info!("Attempting IRC connection...");

        let connect_result = irc_connect_once(
            &config,
            &mut shutdown_rx,
            &mut mosaic_to_irc_rx,
            &irc_to_mosaic_tx,
            &channel_map,
        )
        .await;

        match &connect_result {
            Ok(()) => info!("IRC connection closed normally"),
            Err(e) => warn!("IRC connection error: {:?}", e),
        }

        // Check shutdown again — if the connection exited due to shutdown, don't reconnect
        if *shutdown_rx.borrow() {
            info!("Shutdown signal received after connection closed");
            return;
        }

        // Wait for reconnect delay, but check shutdown signal during the wait
        info!("Reconnecting in {} seconds...", reconnect_delay.as_secs());

        let sleep = tokio::time::sleep(reconnect_delay);
        tokio::pin!(sleep);
        let mut shutdown_check = shutdown_rx.clone();
        let shutdown_fut = shutdown_check.changed();
        tokio::pin!(shutdown_fut);

        tokio::select! {
            _ = sleep.as_mut() => {
                // Backoff expired, continue to reconnect
            }
            _ = shutdown_fut.as_mut() => {
                if *shutdown_rx.borrow() {
                    info!("Shutdown during reconnect backoff");
                    return;
                }
            }
        }
    }
}

// ─── Signal handling ────────────────────────────────────────────────────────

/// Wait for SIGTERM or SIGINT and notify via the watch channel.
async fn signal_handler(shutdown_tx: watch::Sender<bool>) {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{signal, SignalKind};

        let mut term = signal(SignalKind::terminate()).expect("Failed to register SIGTERM handler");
        let mut int = signal(SignalKind::interrupt()).expect("Failed to register SIGINT handler");

        tokio::select! {
            _ = term.recv() => {
                info!("Received SIGTERM");
            }
            _ = int.recv() => {
                info!("Received SIGINT");
            }
        }
    }

    #[cfg(not(unix))]
    {
        // Fallback for non-Unix platforms: use ctrl-c
        let _ = tokio::signal::ctrl_c().await;
        info!("Received Ctrl+C");
    }

    let _ = shutdown_tx.send(true);
}

// ─── Main ───────────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "mosaic_bridge_irc=info,tokio_rustls=warn".into()),
        )
        .with_target(true)
        .init();

    // Load configuration
    let config = Config::from_env();
    info!("=== mosaic-bridge-irc ===");
    info!("MIS URL: {}", config.mis_url);
    info!("IRC Server: {}:{}", config.irc_server, config.irc_port);
    info!("IRC Nick: {}", config.irc_nick);
    info!(
        "IRC Channels: {}",
        if config.irc_channels.is_empty() {
            "(none)".to_string()
        } else {
            config.irc_channels.join(", ")
        }
    );
    if config.irc_sasl_user.is_some() {
        info!("SASL authentication: enabled");
    } else {
        info!("SASL authentication: disabled");
    }

    // Probe MIS on startup (non-fatal)
    match probe_mis(&config.mis_url).await {
        Ok(()) => info!("MIS probe completed"),
        Err(e) => warn!("MIS probe failed (bridge will still start): {:?}", e),
    }

    // Create channels for inter-task communication
    let (_mosaic_to_irc_tx, mosaic_to_irc_rx) = broadcast::channel::<MosaicToIrcMessage>(256);
    let (irc_to_mosaic_tx, mut irc_to_mosaic_rx) = mpsc::channel::<IrcEvent>(256);
    let (shutdown_tx, shutdown_rx) = watch::channel(false);

    let channel_map: Arc<RwLock<HashMap<String, String>>> = Arc::new(RwLock::new(HashMap::new()));

    // Spawn the IRC connection loop (handles auto-reconnect)
    let mut irc_handle = tokio::spawn(irc_connection_loop(
        config,
        shutdown_rx,
        mosaic_to_irc_rx,
        irc_to_mosaic_tx,
        channel_map,
    ));

    // Spawn signal handler
    let shutdown_tx_clone = shutdown_tx.clone();
    let mut signal_handle = tokio::spawn(async move {
        signal_handler(shutdown_tx_clone).await;
    });

    // Spawn a task to log IRC→Mosaic events (in a real deployment, these
    // would be forwarded to a Mosaic event bus via HTTP or IPC)
    let event_logger = tokio::spawn(async move {
        while let Some(event) = irc_to_mosaic_rx.recv().await {
            info!(
                "[IRC→Mosaic] <{}> on {}: {}",
                event.nick, event.channel, event.text
            );
        }
        info!("IRC→Mosaic event logger shutting down");
    });

    // Wait for either the IRC loop or signal handler to complete
    tokio::select! {
        _ = &mut irc_handle => {
            info!("IRC connection loop exited");
        }
        _ = &mut signal_handle => {
            info!("Signal handler triggered shutdown");
        }
    }

    // Signal shutdown to all components
    info!("Initiating graceful shutdown...");
    let _ = shutdown_tx.send(true);

    // Give components time to shut down
    tokio::time::sleep(Duration::from_millis(500)).await;

    // Abort remaining tasks
    irc_handle.abort();
    signal_handle.abort();
    event_logger.abort();

    info!("Bridge shut down");
    Ok(())
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_irc_channel_to_code() {
        assert_eq!(irc_channel_to_code("#mychannel"), "mychannel");
        assert_eq!(irc_channel_to_code("#libera-project"), "libera-project");
        assert_eq!(irc_channel_to_code("#my_channel!"), "my_channel");
        assert_eq!(irc_channel_to_code("mychannel"), "mychannel");
        assert_eq!(irc_channel_to_code(""), "");
    }

    #[test]
    fn test_parse_privmsg() {
        let line = ":nick!user@host PRIVMSG #channel :Hello world!";
        let msg = parse_irc_line(line).unwrap();
        assert_eq!(msg.prefix, Some("nick!user@host".to_string()));
        assert_eq!(msg.command, "PRIVMSG");
        assert_eq!(msg.params, "#channel");
        assert_eq!(msg.trailing, Some("Hello world!".to_string()));
    }

    #[test]
    fn test_parse_ping() {
        let line = "PING :irc.libera.chat";
        let msg = parse_irc_line(line).unwrap();
        assert_eq!(msg.prefix, None);
        assert_eq!(msg.command, "PING");
        assert_eq!(msg.params, "");
        assert_eq!(msg.trailing, Some("irc.libera.chat".to_string()));
    }

    #[test]
    fn test_parse_numeric_001() {
        let line = ":server 001 nick :Welcome to the IRC network";
        let msg = parse_irc_line(line).unwrap();
        assert_eq!(msg.prefix, Some("server".to_string()));
        assert_eq!(msg.command, "001");
        assert_eq!(msg.params, "nick");
        assert_eq!(msg.trailing, Some("Welcome to the IRC network".to_string()));
    }

    #[test]
    fn test_parse_join_without_trailing() {
        let line = ":nick!user@host JOIN #channel";
        let msg = parse_irc_line(line).unwrap();
        assert_eq!(msg.prefix, Some("nick!user@host".to_string()));
        assert_eq!(msg.command, "JOIN");
        assert_eq!(msg.params, "#channel");
        assert_eq!(msg.trailing, None);
    }

    #[test]
    fn test_parse_empty_line() {
        assert!(parse_irc_line("").is_none());
        assert!(parse_irc_line("  ").is_none());
    }

    #[test]
    fn test_parse_tagged_cap() {
        let line = ":server CAP nick ACK :sasl=";
        let msg = parse_irc_line(line).unwrap();
        assert_eq!(msg.command, "CAP");
        assert_eq!(msg.params, "nick ACK");
        assert_eq!(msg.trailing, Some("sasl=".to_string()));
    }
}
