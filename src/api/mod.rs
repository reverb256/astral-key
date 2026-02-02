//! Astral Key - API layer
//!
//! HTTP API routes and handlers.

use axum::Router;

use crate::state::AppState;

pub mod handlers;
pub mod middleware;
pub mod routes;

pub use routes::routes;
