//! Astral Key - OIDC client registry
//!
//! Rows of the `oidc_clients` table — the relying parties allowed to drive
//! the authorization-code flow. /oidc/authorize and /oidc/token resolve
//! their client here, so a client added with SQL is usable without a restart.

use sqlx::sqlite::SqlitePool;
use sqlx::Row;

use crate::auth::oidc::OidcClient;
use crate::config::OidcClientConfig;
use crate::error::{AuthError, Result};

/// SELECT column list shared by all queries in this module.
const OIDC_CLIENT_COLUMNS: &str = "client_id, client_secret_hash, redirect_uris, allowed_scopes";

/// Load a registered client by id. `Ok(None)` means "not registered".
pub async fn get(pool: &SqlitePool, client_id: &str) -> Result<Option<OidcClient>> {
    let row = sqlx::query(&format!(
        "SELECT {OIDC_CLIENT_COLUMNS} FROM oidc_clients WHERE client_id = ?1"
    ))
    .bind(client_id)
    .fetch_optional(pool)
    .await?;

    row.map(|r| row_to_client(&r)).transpose()
}

/// Insert a client, or replace its secret/URIs/scopes when it already exists.
pub async fn upsert(pool: &SqlitePool, client: &OidcClient) -> Result<()> {
    let redirect_uris = encode_json_list(&client.redirect_uris, "redirect_uris")?;
    let allowed_scopes = match &client.allowed_scopes {
        Some(scopes) => Some(encode_json_list(scopes, "allowed_scopes")?),
        None => None,
    };

    sqlx::query(
        "INSERT INTO oidc_clients (client_id, client_secret_hash, redirect_uris, allowed_scopes) \
         VALUES (?1, ?2, ?3, ?4) \
         ON CONFLICT(client_id) DO UPDATE SET \
             client_secret_hash = excluded.client_secret_hash, \
             redirect_uris = excluded.redirect_uris, \
             allowed_scopes = excluded.allowed_scopes",
    )
    .bind(&client.client_id)
    .bind(client.client_secret_hash.as_deref())
    .bind(redirect_uris)
    .bind(allowed_scopes)
    .execute(pool)
    .await?;

    Ok(())
}

/// Upsert the legacy env-configured clients (`OIDC_CLIENT_ID` /
/// `OIDC_CLIENT_SECRET`) so installations that predate the registry keep
/// working. Returns how many clients were written.
///
/// An empty configured secret registers the client as PUBLIC (PKCE S256
/// required) — an empty secret is not a secret.
pub async fn upsert_legacy(pool: &SqlitePool, legacy: &[OidcClientConfig]) -> Result<usize> {
    for config in legacy {
        let client = if config.client_secret.is_empty() {
            OidcClient::public(config.client_id.clone(), config.redirect_uris.clone())
        } else {
            OidcClient::confidential(
                config.client_id.clone(),
                &config.client_secret,
                config.redirect_uris.clone(),
                None,
            )
        };

        upsert(pool, &client).await?;
    }

    Ok(legacy.len())
}

/// Build an OidcClient from a sqlx row.
fn row_to_client(row: &sqlx::sqlite::SqliteRow) -> Result<OidcClient> {
    let client_id: String = row.get::<&str, _>("client_id").to_string();
    let client_secret_hash: Option<String> = row
        .get::<Option<&str>, _>("client_secret_hash")
        .map(|s| s.to_string());
    let redirect_uris = decode_json_list(row.get::<&str, _>("redirect_uris"), "redirect_uris")?;
    let allowed_scopes = match row.get::<Option<&str>, _>("allowed_scopes") {
        Some(raw) => Some(decode_json_list(raw, "allowed_scopes")?),
        None => None,
    };

    Ok(OidcClient {
        client_id,
        client_secret_hash,
        redirect_uris,
        allowed_scopes,
    })
}

fn encode_json_list(values: &[String], column: &str) -> Result<String> {
    serde_json::to_string(values)
        .map_err(|e| AuthError::Internal(format!("Failed to store oidc_clients.{column}: {e}")))
}

fn decode_json_list(raw: &str, column: &str) -> Result<Vec<String>> {
    serde_json::from_str(raw).map_err(|e| {
        AuthError::Database(sqlx::Error::Protocol(format!(
            "oidc_clients.{column} is not a JSON string array: {e}"
        )))
    })
}
