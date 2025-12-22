//! HTTP Inbound Adapter
//!
//! Axum-based HTTP server that drives the application layer.

pub mod auth;
mod handlers;
pub mod rate_limit;
mod server;

pub use auth::auth_middleware;
pub use rate_limit::{RateLimiterState, rate_limit_middleware};
pub use server::HttpServer;
