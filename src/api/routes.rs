//! Astral Key - API routes
//!
//! Route definitions for the HTTP API.

use axum::{
    routing::{delete, get, post},
    Router,
    middleware,
};

use crate::state::AppState;
use crate::auth::jwt::middleware::jwt_auth_middleware;

use super::handlers;

/// Build the API router
pub fn routes(router: Router<AppState>, state: AppState) -> Router {
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
        // Session management
        .route("/sessions/current", delete(handlers::session::logout))
        .route("/sessions", get(handlers::session::list))
        // User management
        .route("/users/me", get(handlers::user::me))
        .route("/users/me", post(handlers::user::update))
        .route("/users/me", delete(handlers::user::delete))
        .route("/users/me/security-keys", get(handlers::user::security_keys))
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            jwt_auth_middleware,
        ));

    // Create public routes (no authentication)
    let public_routes = Router::new()
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
        // Session refresh (public - uses refresh token)
        .route("/sessions/refresh", post(handlers::session::refresh));

    router
        .nest("/api/v1", public_routes.merge(protected_routes))
        .with_state(state)
}
