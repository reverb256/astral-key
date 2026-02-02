//! Astral Key - API routes
//!
//! Route definitions for the HTTP API.

use axum::{
    routing::{delete, get, post},
    Router,
};

use crate::state::AppState;

use super::handlers;

/// Build the API router
pub fn routes(router: Router<AppState>, state: AppState) -> Router<AppState> {
    router
        .nest("/api/v1", api_v1_routes())
        .with_state(state)
}

/// API v1 routes
fn api_v1_routes() -> Router<AppState> {
    Router::new()
        // Web3 authentication
        .route("/auth/web3/nonce", post(handlers::web3::nonce))
        .route("/auth/web3/verify", post(handlers::web3::verify))
        .route("/auth/web3/chains", get(handlers::web3::chains))
        // FIDO2 authentication
        .route(
            "/auth/fido2/register/options",
            post(handlers::fido2::register_options),
        )
        .route(
            "/auth/fido2/register/verify",
            post(handlers::fido2::register_verify),
        )
        .route(
            "/auth/fido2/authenticate/options",
            post(handlers::fido2::authenticate_options),
        )
        .route(
            "/auth/fido2/authenticate/verify",
            post(handlers::fido2::authenticate_verify),
        )
        .route("/auth/fido2/credentials", get(handlers::fido2::credentials))
        .route(
            "/auth/fido2/credentials/:id",
            delete(handlers::fido2::delete_credential),
        )
        // Session management
        .route("/sessions/refresh", post(handlers::session::refresh))
        .route("/sessions/current", delete(handlers::session::logout))
        .route("/sessions", get(handlers::session::list))
        // User management
        .route("/users/me", get(handlers::user::me))
        .route("/users/me", post(handlers::user::update))
        .route("/users/me", delete(handlers::user::delete))
        .route("/users/me/security-keys", get(handlers::user::security_keys))
}
