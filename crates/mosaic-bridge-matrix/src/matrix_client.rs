use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use thiserror::Error;
use tracing::debug;

/// Errors from the Matrix Client-Server API.
#[derive(Error, Debug)]
pub enum MatrixError {
    #[error("HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),
    #[error("Matrix API error (HTTP {status}): {message}")]
    Api { status: u16, message: String },
    #[error("Configuration error: {0}")]
    Config(String),
}

/// Response from sending a message event.
#[derive(Debug, Deserialize)]
pub struct SendEventResponse {
    pub event_id: String,
}

/// Matrix Client-Server API client.
///
/// Sends messages and profile requests to a Matrix homeserver using
/// a bot account's access token for authentication.
pub struct MatrixClient {
    homeserver_url: String,
    bot_token: String,
    bot_user_id: String,
    client: Client,
}

impl MatrixClient {
    /// Create a new Matrix client from explicit configuration.
    pub fn new(homeserver_url: String, bot_token: String, bot_user_id: String) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(15))
            .build()
            .expect("Failed to build HTTP client");
        Self {
            homeserver_url: homeserver_url.trim_end_matches('/').to_string(),
            bot_token,
            bot_user_id,
            client,
        }
    }

    /// Create a Matrix client from environment variables.
    ///
    /// Required: `MATRIX_HOMESERVER_URL`, `MATRIX_BOT_TOKEN`
    /// Optional: `MATRIX_BOT_USER` (default: `@mosaic-bridge:matrix.local`)
    pub fn from_env() -> Result<Self, MatrixError> {
        let homeserver_url = std::env::var("MATRIX_HOMESERVER_URL")
            .map_err(|_| MatrixError::Config("MATRIX_HOMESERVER_URL must be set".into()))?;
        let bot_token = std::env::var("MATRIX_BOT_TOKEN")
            .map_err(|_| MatrixError::Config("MATRIX_BOT_TOKEN must be set".into()))?;
        let bot_user_id = std::env::var("MATRIX_BOT_USER")
            .unwrap_or_else(|_| "@mosaic-bridge:matrix.local".to_string());
        Ok(Self::new(homeserver_url, bot_token, bot_user_id))
    }

    /// Build the full URL for a Matrix CS API path.
    fn api_url(&self, path: &str) -> String {
        format!("{}{}", self.homeserver_url, path)
    }

    /// Perform an authenticated GET request against the CS API.
    async fn get(&self, path: &str) -> Result<serde_json::Value, MatrixError> {
        let url = self.api_url(path);
        debug!("Matrix GET {}", url);

        let resp = self
            .client
            .get(&url)
            .query(&[("access_token", &self.bot_token)])
            .send()
            .await?;

        let status = resp.status();
        let body: serde_json::Value = resp.json().await?;

        if status.is_client_error() || status.is_server_error() {
            let msg = body
                .get("error")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown error");
            return Err(MatrixError::Api {
                status: status.as_u16(),
                message: msg.to_string(),
            });
        }

        Ok(body)
    }

    /// Perform an authenticated PUT request against the CS API with a JSON body.
    async fn put(
        &self,
        path: &str,
        body: &impl Serialize,
    ) -> Result<serde_json::Value, MatrixError> {
        let url = self.api_url(path);
        debug!("Matrix PUT {}", url);

        let resp = self
            .client
            .put(&url)
            .query(&[("access_token", &self.bot_token)])
            .json(body)
            .send()
            .await?;

        let status = resp.status();
        let body: serde_json::Value = resp.json().await?;

        if status.is_client_error() || status.is_server_error() {
            let msg = body
                .get("error")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown error");
            return Err(MatrixError::Api {
                status: status.as_u16(),
                message: msg.to_string(),
            });
        }

        Ok(body)
    }

    /// Perform an authenticated POST request against the CS API with a JSON body.
    async fn post(
        &self,
        path: &str,
        body: &impl Serialize,
    ) -> Result<serde_json::Value, MatrixError> {
        let url = self.api_url(path);
        debug!("Matrix POST {}", url);

        let resp = self
            .client
            .post(&url)
            .query(&[("access_token", &self.bot_token)])
            .json(body)
            .send()
            .await?;

        let status = resp.status();
        let body: serde_json::Value = resp.json().await?;

        if status.is_client_error() || status.is_server_error() {
            let msg = body
                .get("error")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown error");
            return Err(MatrixError::Api {
                status: status.as_u16(),
                message: msg.to_string(),
            });
        }

        Ok(body)
    }

    /// Send a text message to a Matrix room.
    ///
    /// * `room_id` — Matrix room ID (e.g. `!abc:matrix.org`)
    /// * `body` — Message text content
    /// * `msg_type` — Message type (default `m.text`)
    ///
    /// Returns the `event_id` of the sent message.
    pub async fn send_message(
        &self,
        room_id: &str,
        body: &str,
        msg_type: &str,
    ) -> Result<SendEventResponse, MatrixError> {
        let txn_id = uuid::Uuid::new_v4().to_string();
        let path = format!(
            "/_matrix/client/v3/rooms/{}/send/m.room.message/{}",
            urlencoding(room_id),
            txn_id
        );

        let payload = serde_json::json!({
            "msgtype": msg_type,
            "body": body,
            "formatted_body": body,
            "format": "org.matrix.custom.html",
        });

        let resp = self.put(&path, &payload).await?;
        let event_response: SendEventResponse =
            serde_json::from_value(resp).map_err(|e| MatrixError::Api {
                status: 200,
                message: format!("Failed to parse send response: {}", e),
            })?;

        Ok(event_response)
    }

    /// Join a room by its alias.
    pub async fn join_room(&self, room_alias: &str) -> Result<serde_json::Value, MatrixError> {
        let path = format!("/_matrix/client/v3/join/{}", urlencoding(room_alias));
        self.post(&path, &serde_json::json!({})).await
    }

    /// Get the full state of a room.
    pub async fn get_room_state(&self, room_id: &str) -> Result<serde_json::Value, MatrixError> {
        let path = format!("/_matrix/client/v3/rooms/{}/state", urlencoding(room_id));
        self.get(&path).await
    }

    /// Get the bridge bot's own profile.
    pub async fn get_profile(&self) -> Result<serde_json::Value, MatrixError> {
        let path = format!(
            "/_matrix/client/v3/profile/{}",
            urlencoding(&self.bot_user_id)
        );
        self.get(&path).await
    }

    /// Access the configured bot user ID.
    pub fn bot_user_id(&self) -> &str {
        &self.bot_user_id
    }

    /// Access the configured homeserver URL.
    pub fn homeserver_url(&self) -> &str {
        &self.homeserver_url
    }
}

/// Simple URL path encoding (no full crate dependency needed).
fn urlencoding(s: &str) -> String {
    url::form_urlencoded::byte_serialize(s.as_bytes()).collect()
}
