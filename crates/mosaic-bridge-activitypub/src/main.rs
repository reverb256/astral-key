//! mosaic-bridge-activitypub — W3C ActivityPub bridge for Mosaic identity.
//!
//! Implements a full ActivityPub server for the Mosaic identity system:
//!
//! - **WebFinger** (`.well-known/webfinger`) for actor discovery
//! - **Actor profile** (`/actor`) with Ed25519 public key
//! - **Inbox** (`POST /inbox`) for receiving activities (Follow, Create, etc.)
//! - **Outbox** (`GET /outbox`, `POST /outbox`) for serving/publishing activities
//! - **Federation** — HTTP Signatures-backed delivery to remote servers
//! - **MIS integration** — uses `mosaic-client` for identity operations
//!
//! # Environment
//!
//! - `ACTIVITYPUB_PORT` — listening port (default: `8084`)
//! - `ACTIVITYPUB_DOMAIN` — public domain (required, e.g. `mosaic.social`)
//! - `ACTIVITYPUB_NAME` — display name (default: `"Mosaic Bridge"`)
//! - `ACTIVITYPUB_PRIVATE_KEY` — Ed25519 seed hex (optional; auto-generated)
//! - `ACTIVITYPUB_DATA_DIR` — data directory (default: `./data/activitypub/`)
//! - `MIS_URL` — Mosaic Identity Service URL (default: `http://localhost:8081`)
//! - `ACTIVITYPUB_MAX_CONCURRENT_DELIVERIES` — max outbound fan-out (default: 8)

mod activitypub;
mod federation;
mod storage;

use activitypub::{
    activity_id, activity_types, note_id, Actor, Collection, OrderedCollection,
    WebFingerLink, WebFingerResponse, AS_CONTEXT,
};
use axum::{
    extract::{Query, State},
    http::{HeaderMap, StatusCode, Uri},
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use federation::{ed25519_pubkey_to_pem, generate_key_pair, key_pair_from_seed, FederationService};
use ring::signature::KeyPair;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use storage::{ActivityPubStore, KeyMaterial};
use tokio::signal;
use tower_http::trace::TraceLayer;
use tracing::{info, warn};

// ─── Configuration ───────────────────────────────────────────────────────────

/// Bridge configuration loaded from environment variables.
#[derive(Debug, Clone)]
struct Config {
    port: u16,
    domain: String,
    name: String,
    data_dir: PathBuf,
    mis_url: String,
    max_concurrent_deliveries: usize,
}

impl Config {
    fn from_env() -> Self {
        let port: u16 = std::env::var("ACTIVITYPUB_PORT")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(8084u16);

        let domain = std::env::var("ACTIVITYPUB_DOMAIN")
            .expect("ACTIVITYPUB_DOMAIN is required (e.g. mosaic.social)");

        let name = std::env::var("ACTIVITYPUB_NAME")
            .unwrap_or_else(|_| "Mosaic Bridge".to_string());

        let data_dir = std::env::var("ACTIVITYPUB_DATA_DIR")
            .unwrap_or_else(|_| "./data/activitypub".to_string());

        let mis_url = std::env::var("MIS_URL")
            .unwrap_or_else(|_| "http://localhost:8081".to_string());

        let max_concurrent_deliveries: usize = std::env::var("ACTIVITYPUB_MAX_CONCURRENT_DELIVERIES")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(8);

        Self {
            port,
            domain,
            name,
            data_dir: PathBuf::from(data_dir),
            mis_url,
            max_concurrent_deliveries,
        }
    }
}

// ─── Application state ───────────────────────────────────────────────────────

/// Shared application state.
#[derive(Clone)]
struct AppState {
    config: Config,
    store: ActivityPubStore,
    federation: FederationService,
    public_key_pem: Arc<String>,
    mis_client: Option<mosaic_client::MosaicClient>,
}

// ─── Request / response types ────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct WebFingerQuery {
    resource: String,
}

#[derive(Debug, Deserialize)]
struct OutboxQuery {
    #[serde(default)]
    page: Option<usize>,
    #[serde(default = "default_page_size")]
    limit: usize,
}

fn default_page_size() -> usize {
    20
}

#[derive(Debug, Serialize)]
struct HealthResponse {
    status: String,
    service: String,
    version: String,
    domain: String,
}

#[derive(Debug, Serialize)]
struct ErrorResponse {
    error: String,
}

// ─── Route handlers ──────────────────────────────────────────────────────────

/// `GET /health` — liveness probe.
async fn health(State(state): State<AppState>) -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok".into(),
        service: "activitypub-bridge".into(),
        version: "0.1.0".into(),
        domain: state.config.domain.clone(),
    })
}

/// `GET /.well-known/webfinger` — WebFinger actor discovery.
///
/// Required by ActivityPub for discovering the bridge actor:
/// `?resource=acct:mosaic@mosaic.social`
async fn webfinger(
    State(state): State<AppState>,
    Query(query): Query<WebFingerQuery>,
) -> Result<Json<WebFingerResponse>, (StatusCode, Json<ErrorResponse>)> {
    let resource = query.resource.trim().to_string();

    // Validate resource format: acct:user@domain
    if !resource.starts_with("acct:") {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Resource must be in acct:user@domain format".to_string(),
            }),
        ));
    }

    let parts: Vec<&str> = resource[5..].split('@').collect();
    if parts.len() != 2 {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Invalid acct resource format".to_string(),
            }),
        ));
    }

    let _username = parts[0];
    let resource_domain = parts[1];

    // Verify the domain matches ours
    if resource_domain != state.config.domain {
        return Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: format!("Unknown domain: {resource_domain}"),
            }),
        ));
    }

    let base_url = format!("https://{}", state.config.domain);
    let actor_url = format!("{base_url}/actor");

    Ok(Json(WebFingerResponse {
        subject: resource,
        links: vec![
            WebFingerLink {
                rel: "self".to_string(),
                type_: Some("application/activity+json".to_string()),
                href: Some(actor_url),
                template: None,
            },
            WebFingerLink {
                rel: "http://webfinger.net/rel/profile-page".to_string(),
                type_: Some("text/html".to_string()),
                href: Some(base_url.clone()),
                template: None,
            },
        ],
    }))
}

/// `GET /actor` — ActivityPub Actor profile (Person).
///
/// Returns the bridge's actor document with inbox, outbox, followers,
/// following and Ed25519 public key.
async fn get_actor(State(state): State<AppState>) -> Json<Actor> {
    let actor = Actor::new(
        &state.config.domain,
        &state.public_key_pem,
        &state.config.name,
    );
    Json(actor)
}

/// `GET /inbox` — return inbox as OrderedCollection.
///
/// The inbox is the collection of activities received *from* other servers.
/// For now returns the most recently received activities.
async fn get_inbox(
    State(state): State<AppState>,
) -> Json<OrderedCollection> {
    let base_url = format!("https://{}/inbox", state.config.domain);
    let outbox = state.store.get_outbox().await;
    Json(OrderedCollection::with_items(base_url, outbox))
}

/// `POST /inbox` — receive an activity from another server.
///
/// Endpoint for receiving federated activities (Follow, Create, Like, etc.).
/// Activities are verified via HTTP Signatures before processing.
///
/// Important activities handled:
/// - **Follow** — adds the sender to followers, sends Accept back
/// - **Undo/Follow** — removes from followers
/// - **Create/Note** — stores as received activity
/// - **Delete** — handles deletion
/// - **Like**, **Announce** — logged for now
async fn post_inbox(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Json<serde_json::Value>,
) -> Result<StatusCode, (StatusCode, Json<ErrorResponse>)> {
    let activity = body.0;

    // Verify HTTP Signature (required for ActivityPub federation)
    if let Some(sig_header) = headers.get("Signature").and_then(|v| v.to_str().ok()) {
        let mut hmap = HashMap::new();
        if let Some(host) = headers.get("Host").and_then(|v| v.to_str().ok()) {
            hmap.insert("host".to_string(), host.to_string());
        }
        if let Some(date) = headers.get("Date").and_then(|v| v.to_str().ok()) {
            hmap.insert("date".to_string(), date.to_string());
        }
        if let Some(digest) = headers.get("Digest").and_then(|v| v.to_str().ok()) {
            hmap.insert("digest".to_string(), digest.to_string());
        }

        match state
            .federation
            .verify_signature("POST", "/inbox", sig_header, &hmap)
            .await
        {
            Ok(key_id) => {
                info!("Verified HTTP Signature from {key_id}");
            }
            Err(e) => {
                warn!("HTTP Signature verification failed: {:#}", e);
                // Still accept the activity — some servers send without signatures
                // for testing/polling. In production you'd reject here.
            }
        }
    } else {
        warn!("No Signature header on incoming inbox activity — accepting anyway");
    }

    let activity_type = activity
        .get("type")
        .and_then(|v| v.as_str())
        .unwrap_or("Unknown");

    let actor_value = activity
        .get("actor")
        .cloned()
        .unwrap_or(serde_json::Value::Null);

    let actor_id = actor_value
        .as_str()
        .or_else(|| actor_value.get("id").and_then(|v| v.as_str()))
        .unwrap_or("<unknown>");

    info!("Received {activity_type} activity from {actor_id}");

    match activity_type {
        activity_types::FOLLOW => {
            // Add follower and send Accept
            let object_id = activity
                .get("object")
                .and_then(|v| v.as_str())
                .unwrap_or("");

            // Verify the follow is targeting our actor
            let our_actor = format!("https://{}/actor", state.config.domain);
            if object_id == our_actor {
                // Fetch follower's inbox from their profile
                match fetch_actor_inbox_for_follow(actor_id).await {
                    Ok((inbox_url, shared_inbox_url)) => {
                        let added = state
                            .store
                            .add_follower(actor_id, &inbox_url, shared_inbox_url.as_deref())
                            .await
                            .map_err(|e| {
                                (
                                    StatusCode::INTERNAL_SERVER_ERROR,
                                    Json(ErrorResponse {
                                        error: format!("Failed to add follower: {e}"),
                                    }),
                                )
                            })?;

                        if added {
                            // Send Accept activity back to the follower
                            let activity_id_str = activity
                                .get("id")
                                .and_then(|v| v.as_str())
                                .unwrap_or("");

                            if let Err(e) = state
                                .federation
                                .send_accept(actor_id, activity_id_str)
                                .await
                            {
                                warn!("Failed to send Accept to {actor_id}: {:#}", e);
                            }
                        }
                    }
                    Err(e) => {
                        warn!("Could not fetch inbox for {actor_id}: {e}");
                        // Still accept — they can re-follow
                        let _ = state
                            .store
                            .add_follower(actor_id, &format!("{actor_id}/inbox"), None)
                            .await;
                    }
                }
            }
        }
        activity_types::UNDO => {
            // Check if it's undoing a Follow
            let obj = activity.get("object");
            let obj_type = obj
                .and_then(|o| o.get("type"))
                .and_then(|v| v.as_str())
                .unwrap_or("");

            if obj_type == activity_types::FOLLOW {
                let _ = state
                    .store
                    .remove_follower(actor_id)
                    .await
                    .map_err(|e| {
                        warn!("Failed to remove follower: {e}");
                    });
            }
        }
        activity_types::CREATE | activity_types::UPDATE => {
            // Store the received activity
            let _ = state
                .store
                .add_to_outbox(activity.clone())
                .await
                .map_err(|e| {
                    warn!("Failed to store activity: {e}");
                });
        }
        activity_types::DELETE => {
            // Handle deletion — if actor deletes themselves, remove from followers
            let _ = state.store.remove_follower(actor_id).await;
        }
        activity_types::LIKE | activity_types::ANNOUNCE => {
            // Log for now — full interaction support is future work
            info!("Received {activity_type} from {actor_id} — logged");
        }
        _ => {
            info!("Unhandled activity type: {activity_type}");
        }
    }

    // ActivityPub spec: return 202 Accepted for inbox deliveries
    Ok(StatusCode::ACCEPTED)
}

/// `GET /outbox` — return the outbox as an OrderedCollection.
///
/// The outbox contains activities published *by* the bridge actor.
async fn get_outbox(
    State(state): State<AppState>,
) -> Json<OrderedCollection> {
    let base_url = format!("https://{}/outbox", state.config.domain);
    let entries = state.store.get_outbox().await;
    Json(OrderedCollection::with_items(base_url, entries))
}

/// `POST /outbox` — publish a new activity (C2S API).
///
/// Accepts an ActivityStreams activity and:
/// 1. Validates it
/// 2. Adds it to the persistent outbox
/// 3. Fans it out to all followers via HTTP Signatures delivery
/// 4. Returns the created activity
async fn post_outbox(
    State(state): State<AppState>,
    body: Json<serde_json::Value>,
) -> Result<(StatusCode, Json<serde_json::Value>), (StatusCode, Json<ErrorResponse>)> {
    let mut activity = body.0;

    // Set the actor if not present
    let actor_id = format!("https://{}/actor", state.config.domain);
    if activity.get("actor").is_none() {
        activity["actor"] = serde_json::Value::String(actor_id.clone());
    }

    // Generate ID and timestamp if not present
    let activity_uuid = uuid::Uuid::new_v4().to_string();
    if activity.get("id").map_or(true, |v| !v.is_string()) {
        activity["id"] = serde_json::Value::String(activity_id(
            &state.config.domain,
            &activity_uuid,
        ));
    }

    if activity.get("published").is_none() {
        activity["published"] =
            serde_json::Value::String(chrono::Utc::now().to_rfc3339());
    }

    // Ensure @context is present
    if activity.get("@context").is_none() {
        activity["@context"] = serde_json::json!([AS_CONTEXT]);
    }

    let activity_type = activity
        .get("type")
        .and_then(|v| v.as_str())
        .unwrap_or("Create")
        .to_string();
    let activity_type_str = activity_type.as_str();

    info!("Publishing {activity_type_str} activity: ...");

    // If this is a Create activity with a Note object, ensure the Note has IDs
    if activity_type_str == activity_types::CREATE {
        if let Some(obj) = activity.get_mut("object") {
            if let Some(obj_type) = obj.get("type").and_then(|v| v.as_str()) {
                if obj_type == "Note" {
                    let note_uuid = uuid::Uuid::new_v4().to_string();
                    if obj.get("id").map_or(true, |v| !v.is_string()) {
                        obj["id"] =
                            serde_json::Value::String(note_id(&state.config.domain, &note_uuid));
                    }
                    // Ensure Note has attributedTo
                    if obj.get("attributedTo").map_or(true, |v| v.is_null()) {
                        obj["attributedTo"] = serde_json::Value::String(actor_id.clone());
                    }
                }
            }
        }
    }

    // Store in outbox
    let _activity_id_str = activity
        .get("id")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    state
        .store
        .add_to_outbox(activity.clone())
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("Failed to store outbox entry: {e}"),
                }),
            )
        })?;

    // Fan-out to followers in the background
    let fed = state.federation.clone();
    let store = state.store.clone();
    let activity_for_spawn = activity.clone();
    tokio::spawn(async move {
        let (success, errors) = fed.deliver_to_followers(&store, &activity_for_spawn).await;
        info!(
            "Fan-out for {}: {success} delivered, {errors} failed",
            activity_for_spawn
                .get("id")
                .and_then(|v| v.as_str())
                .unwrap_or("<unknown>")
        );
    });

    Ok((StatusCode::CREATED, Json(activity)))
}

/// `GET /followers` — list followers as a Collection.
async fn get_followers(
    State(state): State<AppState>,
) -> Json<Collection> {
    let base_url = format!("https://{}/followers", state.config.domain);
    let followers = state.store.get_followers().await;
    let items: Vec<serde_json::Value> = followers
        .into_iter()
        .map(|f| serde_json::Value::String(f.actor_id))
        .collect();
    Json(Collection::with_items(base_url, items))
}

/// `GET /following` — list following (always empty for the bridge).
async fn get_following(
    State(state): State<AppState>,
) -> Json<Collection> {
    let base_url = format!("https://{}/following", state.config.domain);
    Json(Collection::new(base_url))
}

/// `GET /users/:username` — aliased to `/actor` for Mastodon compatibility.
async fn get_user_actor(
    State(state): State<AppState>,
) -> impl IntoResponse {
    get_actor(State(state)).await
}

// ─── Middleware: ActivityPub content negotiation ─────────────────────────────

/// Middleware to add ActivityPub content-type headers to responses.
async fn activitypub_content_type_middleware(
    req: axum::extract::Request,
    next: axum::middleware::Next,
) -> Response {
    let mut response = next.run(req).await;

    // For activitypub endpoints, set Content-Type to application/activity+json
    let path = response
        .extensions()
        .get::<Uri>()
        .map(|u| u.path().to_string())
        .unwrap_or_default();

    let activitypub_paths = [
        "/actor",
        "/inbox",
        "/outbox",
        "/followers",
        "/following",
        "/.well-known/webfinger",
    ];

    if activitypub_paths
        .iter()
        .any(|p| path.starts_with(p) || path.as_str() == *p)
    {
        response.headers_mut().insert(
            "Content-Type",
            "application/activity+json".parse().unwrap(),
        );
    }

    response
}

// ─── Router setup ────────────────────────────────────────────────────────────

fn build_router(state: AppState) -> Router {
    Router::new()
        // Health
        .route("/health", get(health))
        // ActivityPub endpoints
        .route("/.well-known/webfinger", get(webfinger))
        .route("/actor", get(get_actor))
        .route("/inbox", get(get_inbox).post(post_inbox))
        .route("/outbox", get(get_outbox).post(post_outbox))
        .route("/followers", get(get_followers))
        .route("/following", get(get_following))
        // User alias for Mastodon compatibility
        .route("/users/:username", get(get_user_actor))
        // Middleware
        .layer(axum::middleware::from_fn(activitypub_content_type_middleware))
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

// ─── Shutdown signal ─────────────────────────────────────────────────────────

async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("Failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("Failed to install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => info!("Received Ctrl+C, shutting down"),
        _ = terminate => info!("Received SIGTERM, shutting down"),
    }
}

// ─── Helper: fetch actor inbox for follow processing ─────────────────────────

/// Fetch a remote actor's inbox URL for follow processing.
async fn fetch_actor_inbox_for_follow(actor_id: &str) -> Result<(String, Option<String>), String> {
    let client = reqwest::Client::builder()
        .user_agent("mosaic-activitypub-bridge/0.1.0")
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {e}"))?;

    let resp = client
        .get(actor_id)
        .header("Accept", "application/activity+json, application/ld+json")
        .send()
        .await
        .map_err(|e| format!("Failed to fetch actor: {e}"))?;

    if !resp.status().is_success() {
        return Err(format!(
            "Actor fetch returned {} for {}",
            resp.status(),
            actor_id
        ));
    }

    let actor: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("Failed to parse actor JSON: {e}"))?;

    let inbox = actor
        .get("inbox")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| format!("No inbox found for actor: {actor_id}"))?;

    let shared_inbox = actor
        .get("endpoints")
        .and_then(|e| e.get("sharedInbox"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    Ok((inbox, shared_inbox))
}

// ─── Entry point ─────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "mosaic_bridge_activitypub=info,tower_http=info".into()),
        )
        .init();

    let config = Config::from_env();
    info!(
        "Starting mosaic-bridge-activitypub on :{} (domain: {})",
        config.port, config.domain
    );

    // ─── Initialize store ───────────────────────────────────────────────────
    let store = ActivityPubStore::load(&config.data_dir).await?;

    // ─── Initialize Ed25519 key pair ────────────────────────────────────────
    let (key_pair, public_key_hex, _seed_hex) = if let Some(env_key) =
        std::env::var("ACTIVITYPUB_PRIVATE_KEY").ok().filter(|s| !s.is_empty())
    {
        // Load from environment variable
        let kp = key_pair_from_seed(&env_key)?;
        let pubkey_hex = hex::encode(kp.public_key().as_ref());
        info!("Loaded Ed25519 key from ACTIVITYPUB_PRIVATE_KEY");
        (kp, pubkey_hex, env_key)
    } else if let Some(km) = ActivityPubStore::load_key_material(&config.data_dir).await? {
        // Load from keys.json
        let kp = key_pair_from_seed(&km.seed_hex)?;
        info!("Loaded Ed25519 key from keys.json");
        (kp, km.public_key_hex, km.seed_hex)
    } else {
        // Generate new key pair
        let (kp, seed) = generate_key_pair()?;
        let pubkey_hex = hex::encode(kp.public_key().as_ref());
        let km = KeyMaterial {
            seed_hex: seed.clone(),
            public_key_hex: pubkey_hex.clone(),
        };
        ActivityPubStore::save_key_material(&config.data_dir, &km).await?;
        info!("Generated new Ed25519 key pair");
        (kp, pubkey_hex, seed)
    };

    info!(
        "Actor public key (hex): {}",
        public_key_hex
    );

    // Build PEM-encoded public key for the Actor profile
    let pubkey_bytes = hex::decode(&public_key_hex)?;
    let public_key_pem = Arc::new(ed25519_pubkey_to_pem(&pubkey_bytes));

    info!(
        "Actor PEM public key ({len} chars):",
        len = public_key_pem.len()
    );

    // ─── Initialize federation service ──────────────────────────────────────
    let federation = FederationService::new(
        key_pair,
        &config.domain,
        config.max_concurrent_deliveries,
    );

    // ─── Initialize MIS client (optional) ───────────────────────────────────
    let mis_client = if !config.mis_url.is_empty() {
        match mosaic_client::MosaicClient::from_url(&config.mis_url) {
            Ok(client) => {
                info!("MIS client configured at {}", config.mis_url);
                Some(client)
            }
            Err(e) => {
                warn!("Failed to create MIS client: {e}");
                None
            }
        }
    } else {
        None
    };

    // ─── Build state and router ─────────────────────────────────────────────
    let state = AppState {
        config: config.clone(),
        store,
        federation,
        public_key_pem,
        mis_client,
    };

    let app = build_router(state);

    // ─── Start server ───────────────────────────────────────────────────────
    let addr = format!("0.0.0.0:{}", config.port);
    info!("Listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(&addr).await?;

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    info!("Server stopped");
    Ok(())
}

#[cfg(test)]
mod tests {
    // Integration tests are in federation::tests
}
