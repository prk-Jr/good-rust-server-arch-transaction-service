//! HTTP Inbound Adapter
//!
//! Axum-based HTTP server that drives the application layer.

mod handlers;
mod server;

pub use server::HttpServer;
