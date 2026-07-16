//! Astral Key - API routes
//!
//! Route definitions for the HTTP API.

use axum::{
    middleware,
    routing::{delete, get, post},
    Router,
};

use crate::api::middleware::{audit, rate_limit};
use crate::auth::jwt::middleware::jwt_auth_middleware;
use crate::state::AppState;

use super::handlers;

/// Build the API router
pub fn routes(router: Router<AppState>, state: AppState) -> Router {
    // Initialise rate limiter (sensible defaults for homelab single-replica).
    rate_limit::init(100, 200);

    // Create protected routes with JWT middleware
    let protected_routes = Router::new()
        // FIDO2 registration (require authentication)
        .route(
            "/auth/fido2/register/options",
            post(handlers::fido2::register_options),
        )
        .route(
            "/auth/fido2/register/verify",
            post(handlers::fido2::register_verify),
        )
        .route("/auth/fido2/credentials", get(handlers::fido2::credentials))
        .route(
            "/auth/fido2/credentials/:id",
            delete(handlers::fido2::delete_credential),
        )
        // API key management (require authentication)
        .route("/auth/keys", post(handlers::keys::create_key))
        .route("/auth/keys", get(handlers::keys::list_keys))
        .route("/auth/keys/:id", delete(handlers::keys::delete_key))
        .route("/auth/keys/:id/revoke", post(handlers::keys::revoke_key))
        // Session management (require authentication)
        .route("/auth/sessions", get(handlers::session::list_sessions))
        .route(
            "/auth/sessions/:id",
            delete(handlers::session::revoke_session),
        )
        // Identity and contacts (require authentication)
        .route("/identity", post(handlers::identity::create_identity))
        .route("/identity", get(handlers::identity::list_identities))
        .route(
            "/identity/current",
            get(handlers::identity::current_identity),
        )
        .route(
            "/identity/:id/set-current",
            post(handlers::identity::set_current_identity),
        )
        .route("/identity/:id", delete(handlers::identity::delete_identity))
        .route("/contacts", get(handlers::identity::list_contacts))
        .route("/contacts", post(handlers::identity::upsert_contact))
        .route("/contacts/scan", post(handlers::identity::scan_qr))
        .route(
            "/contacts/:pubkey",
            delete(handlers::identity::delete_contact),
        )
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            jwt_auth_middleware,
        ));

    // Create public routes (no authentication)
    let public_routes = Router::new()
        // Token verification (public — validates Bearer tokens for external services)
        .route("/auth/verify", post(handlers::auth::verify_token))
        // Signature verification (public — clients sign locally, server verifies)
        .route(
            "/identity/verify",
            post(handlers::identity::verify_signature),
        )
        // QR code generation (public — for sharing a public key)
        .route("/identity/qr/:pubkey", get(handlers::identity::generate_qr))
        // Web3 authentication (public - for login)
        .route("/auth/web3/nonce", post(handlers::web3::nonce))
        .route("/auth/web3/verify", post(handlers::web3::verify))
        .route("/auth/web3/chains", get(handlers::web3::chains))
        // FIDO2 authentication (public - for login)
        .route(
            "/auth/fido2/authenticate/options",
            post(handlers::fido2::authenticate_options),
        )
        .route(
            "/auth/fido2/authenticate/verify",
            post(handlers::fido2::authenticate_verify),
        )
        // Token refresh (public — uses refresh token, not JWT auth)
        .route(
            "/auth/token/refresh",
            post(handlers::session::refresh_token),
        );

    router
        .nest(
            "/api/v1",
            public_routes
                .merge(protected_routes)
                .layer(middleware::from_fn(rate_limit::rate_limit_middleware))
                .layer(middleware::from_fn(audit::audit_middleware)),
        )
        .with_state(state)
}
